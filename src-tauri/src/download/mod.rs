use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    thread,
    time::Duration,
};

use reqwest::{
    StatusCode,
    blocking::Client,
    header::{CONTENT_RANGE, ETAG, LAST_MODIFIED, RANGE},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
};
use zip::ZipArchive;

use crate::{
    domain::{AppError, ErrorCode},
    process::CancellationToken,
};

const MAX_ATTEMPTS: usize = 3;

#[derive(Clone, Debug)]
pub struct DownloadRequest {
    pub url: String,
    pub destination: PathBuf,
    pub expected_sha256: String,
    pub expected_size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct DownloadProgress {
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DownloadResult {
    pub destination: PathBuf,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialMetadata {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
}

pub struct Downloader {
    client: Client,
    #[cfg(test)]
    allow_http_loopback: bool,
}

impl Downloader {
    pub fn new() -> Result<Self, AppError> {
        let client = Client::builder().build().map_err(|_| {
            AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "could not configure HTTPS downloader",
            )
        })?;
        Ok(Self {
            client,
            #[cfg(test)]
            allow_http_loopback: false,
        })
    }

    #[cfg(test)]
    fn new_for_loopback_tests() -> Self {
        Self {
            client: Client::builder().no_proxy().build().unwrap(),
            allow_http_loopback: true,
        }
    }

    pub fn download(
        &self,
        request: &DownloadRequest,
        cancellation: &CancellationToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<DownloadResult, AppError> {
        if !self.is_allowed_url(&request.url) {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "runtime downloads require HTTPS",
            ));
        }
        validate_sha256(&request.expected_sha256)?;
        if let Some(result) = verified_existing_destination(request)? {
            on_progress(DownloadProgress {
                completed_bytes: result.bytes,
                total_bytes: Some(result.bytes),
            });
            return Ok(result);
        }
        let part = part_path(&request.destination);
        let metadata_path = partial_metadata_path(&part);
        if let Some(parent) = request.destination.parent() {
            ensure_download_directory_safe(parent)?;
            fs::create_dir_all(parent).map_err(download_error)?;
            ensure_download_directory_safe(parent)?;
        }
        for attempt in 1..=MAX_ATTEMPTS {
            match self.download_once(request, &part, &metadata_path, cancellation, on_progress) {
                Ok(result) => return Ok(result),
                Err(error)
                    if error.code == ErrorCode::TaskCancelled
                        || error.code == ErrorCode::EnvironmentHashMismatch =>
                {
                    return Err(error);
                }
                Err(error) if attempt == MAX_ATTEMPTS => return Err(error),
                Err(_) => {
                    thread::sleep(Duration::from_millis((100 * (1 << (attempt - 1))).min(500)))
                }
            }
        }
        unreachable!("bounded retry loop returns on final attempt")
    }

    fn is_allowed_url(&self, url: &str) -> bool {
        if url.starts_with("https://") {
            return true;
        }
        #[cfg(test)]
        if self.allow_http_loopback
            && (url.starts_with("http://127.0.0.1:") || url.starts_with("http://[::1]:"))
        {
            return true;
        }
        false
    }

    fn download_once(
        &self,
        request: &DownloadRequest,
        part: &Path,
        metadata_path: &Path,
        cancellation: &CancellationToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<DownloadResult, AppError> {
        if cancellation.is_cancelled() {
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "download was cancelled",
            ));
        }
        ensure_safe_parent(part)?;
        let (existing, metadata) = inspect_partial_state(part, metadata_path)?;
        let metadata = metadata.filter(|metadata| metadata.url == request.url);
        let can_resume = existing > 0 && metadata.is_some();
        let mut http = self.client.get(&request.url);
        if can_resume {
            http = http.header(RANGE, format!("bytes={existing}-"));
        }
        let mut response = http.send().map_err(download_error)?;
        let response_etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let response_last_modified = response
            .headers()
            .get(LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let validators_match = metadata.as_ref().is_none_or(|known| {
            (known.etag.is_none() || known.etag == response_etag)
                && (known.last_modified.is_none() || known.last_modified == response_last_modified)
        });
        let resumed = can_resume
            && response.status() == StatusCode::PARTIAL_CONTENT
            && validators_match
            && content_range_starts_at(
                response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok()),
                existing,
            );
        if !resumed && !response.status().is_success() {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "runtime download request failed",
            ));
        }
        let starting = if resumed {
            existing
        } else {
            remove_partial(part, metadata_path)?;
            0
        };
        ensure_safe_parent(part)?;
        let total_bytes = response.content_length().map(|length| length + starting);
        let response_metadata = PartialMetadata {
            url: request.url.clone(),
            etag: response_etag,
            last_modified: response_last_modified,
        };
        write_metadata(metadata_path, &response_metadata)?;
        let mut hasher = if resumed {
            hash_file(part)?
        } else {
            Sha256::new()
        };
        let mut file = open_partial_file(part, resumed)?;
        let bytes = stream_response(
            &mut response,
            &mut file,
            &mut hasher,
            starting,
            total_bytes,
            cancellation,
            on_progress,
        )?;
        if request
            .expected_size_bytes
            .is_some_and(|expected| expected != bytes)
        {
            remove_partial(part, metadata_path)?;
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "downloaded artifact length did not match the manifest",
            ));
        }
        file.flush().map_err(download_error)?;
        let actual = format!("{:x}", hasher.finalize());
        if actual != request.expected_sha256.to_ascii_lowercase() {
            remove_partial(part, metadata_path)?;
            return Err(AppError::new(
                ErrorCode::EnvironmentHashMismatch,
                "downloaded artifact SHA-256 did not match the manifest",
            ));
        }
        drop(file);
        require_safe_regular_file(part, "partial download")?;
        fs::rename(part, &request.destination).map_err(download_error)?;
        remove_regular_file_if_exists(metadata_path, "partial download metadata")?;
        Ok(DownloadResult {
            destination: request.destination.clone(),
            bytes,
        })
    }
}

fn verified_existing_destination(
    request: &DownloadRequest,
) -> Result<Option<DownloadResult>, AppError> {
    ensure_safe_parent(&request.destination)?;
    let metadata = match safe_existing_metadata(&request.destination, "download destination")? {
        Some(metadata) => metadata,
        None => return Ok(None),
    };
    if !metadata.is_file() {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "download destination is not a regular file",
        ));
    }
    let correct_length = request
        .expected_size_bytes
        .is_none_or(|expected| expected == metadata.len());
    let correct_hash = format!("{:x}", hash_file(&request.destination)?.finalize())
        .eq_ignore_ascii_case(&request.expected_sha256);
    if correct_length && correct_hash {
        return Ok(Some(DownloadResult {
            destination: request.destination.clone(),
            bytes: metadata.len(),
        }));
    }
    // This is an application-managed cache file; removing the invalid artifact before the
    // verified .part is promoted avoids Windows rename failures over an existing destination.
    // The symlink/reparse check above is deliberately performed before hashing or deleting.
    fs::remove_file(&request.destination).map_err(download_error)?;
    Ok(None)
}

fn safe_existing_metadata(path: &Path, purpose: &str) -> Result<Option<fs::Metadata>, AppError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(download_error(error)),
    };
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            format!("{purpose} must not be a symlink or reparse point"),
        ));
    }
    Ok(Some(metadata))
}

fn require_safe_regular_file(path: &Path, purpose: &str) -> Result<fs::Metadata, AppError> {
    match safe_existing_metadata(path, purpose)? {
        Some(metadata) if metadata.is_file() => Ok(metadata),
        Some(_) => Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            format!("{purpose} is not a regular file"),
        )),
        None => Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            format!("{purpose} is missing"),
        )),
    }
}

fn inspect_partial_state(
    part: &Path,
    metadata_path: &Path,
) -> Result<(u64, Option<PartialMetadata>), AppError> {
    let existing = match safe_existing_metadata(part, "partial download")? {
        Some(metadata) if metadata.is_file() => metadata.len(),
        Some(_) => {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "partial download is not a regular file",
            ));
        }
        None => 0,
    };
    Ok((existing, read_metadata(metadata_path)?))
}

fn ensure_download_directory_safe(path: &Path) -> Result<(), AppError> {
    let mut ancestors = path.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let Some(metadata) = safe_existing_metadata(ancestor, "download directory")? else {
            continue;
        };
        if !metadata.is_dir() {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "download path ancestor is not a directory",
            ));
        }
    }
    Ok(())
}

fn ensure_safe_parent(path: &Path) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "download path has no parent directory",
        )
    })?;
    ensure_download_directory_safe(parent)
}

fn stream_response(
    response: &mut impl Read,
    file: &mut File,
    hasher: &mut Sha256,
    starting: u64,
    total: Option<u64>,
    cancellation: &CancellationToken,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<u64, AppError> {
    let mut completed = starting;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(AppError::new(
                ErrorCode::TaskCancelled,
                "download was cancelled",
            ));
        }
        let read = response.read(&mut buffer).map_err(download_error)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(download_error)?;
        hasher.update(&buffer[..read]);
        completed += read as u64;
        on_progress(DownloadProgress {
            completed_bytes: completed,
            total_bytes: total,
        });
    }
    Ok(completed)
}

fn part_path(destination: &Path) -> PathBuf {
    match destination.extension().and_then(|value| value.to_str()) {
        Some(extension) => destination.with_extension(format!("{extension}.part")),
        None => destination.with_extension("part"),
    }
}
fn partial_metadata_path(part: &Path) -> PathBuf {
    part.with_extension("part.json")
}
fn read_metadata(path: &Path) -> Result<Option<PartialMetadata>, AppError> {
    ensure_safe_parent(path)?;
    let Some(_) = safe_existing_metadata(path, "partial download metadata")? else {
        return Ok(None);
    };
    require_safe_regular_file(path, "partial download metadata")?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(download_error)?;
    reject_open_file_reparse_point(&file, "partial download metadata")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(download_error)?;
    Ok(serde_json::from_str(&contents).ok())
}
fn write_metadata(path: &Path, metadata: &PartialMetadata) -> Result<(), AppError> {
    ensure_safe_parent(path)?;
    let bytes = serde_json::to_vec(metadata).map_err(|_| {
        AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "could not serialize partial download metadata",
        )
    })?;
    let exists = safe_existing_metadata(path, "partial download metadata")?.is_some();
    if exists {
        require_safe_regular_file(path, "partial download metadata")?;
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .truncate(exists)
        .create_new(!exists)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let mut file = options.open(path).map_err(download_error)?;
    reject_open_file_reparse_point(&file, "partial download metadata")?;
    file.write_all(&bytes).map_err(download_error)
}
fn remove_partial(part: &Path, metadata: &Path) -> Result<(), AppError> {
    ensure_safe_parent(part)?;
    ensure_safe_parent(metadata)?;
    let part_exists = validate_regular_file_if_exists(part, "partial download")?;
    let metadata_exists = validate_regular_file_if_exists(metadata, "partial download metadata")?;
    if part_exists {
        fs::remove_file(part).map_err(download_error)?;
    }
    if metadata_exists {
        fs::remove_file(metadata).map_err(download_error)?;
    }
    Ok(())
}
fn remove_regular_file_if_exists(path: &Path, purpose: &str) -> Result<(), AppError> {
    ensure_safe_parent(path)?;
    if validate_regular_file_if_exists(path, purpose)? {
        fs::remove_file(path).map_err(download_error)?;
    }
    Ok(())
}
fn validate_regular_file_if_exists(path: &Path, purpose: &str) -> Result<bool, AppError> {
    match safe_existing_metadata(path, purpose)? {
        Some(metadata) if metadata.is_file() => Ok(true),
        Some(_) => Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            format!("{purpose} is not a regular file"),
        )),
        None => Ok(false),
    }
}
fn open_partial_file(path: &Path, resumed: bool) -> Result<File, AppError> {
    ensure_safe_parent(path)?;
    if resumed {
        require_safe_regular_file(path, "partial download")?;
    } else if safe_existing_metadata(path, "partial download")?.is_some() {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "partial download unexpectedly exists",
        ));
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .append(resumed)
        .create_new(!resumed)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path).map_err(download_error)?;
    reject_open_file_reparse_point(&file, "partial download")?;
    Ok(file)
}
fn content_range_starts_at(value: Option<&str>, expected: u64) -> bool {
    value.is_some_and(|value| value.starts_with(&format!("bytes {expected}-")))
}
fn hash_file(path: &Path) -> Result<Sha256, AppError> {
    ensure_safe_parent(path)?;
    require_safe_regular_file(path, "download file")?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(download_error)?;
    reject_open_file_reparse_point(&file, "download file")?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut HashWriter(&mut hasher)).map_err(download_error)?;
    Ok(hasher)
}
fn reject_open_file_reparse_point(file: &File, purpose: &str) -> Result<(), AppError> {
    let metadata = file.metadata().map_err(download_error)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            format!("{purpose} must not be a reparse point"),
        ));
    }
    Ok(())
}
struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn validate_sha256(value: &str) -> Result<(), AppError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "expected SHA-256 format is invalid",
        ))
    }
}
fn download_error(_: impl std::fmt::Display) -> AppError {
    AppError::new(
        ErrorCode::EnvironmentDownloadFailed,
        "runtime download failed",
    )
}

#[derive(Clone, Copy, Debug)]
pub struct ZipExtractionLimits {
    pub max_entries: usize,
    pub max_total_uncompressed_bytes: u64,
    pub max_entry_uncompressed_bytes: u64,
}
impl Default for ZipExtractionLimits {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_total_uncompressed_bytes: 8 * 1024 * 1024 * 1024,
            max_entry_uncompressed_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

pub fn extract_zip_safely(
    archive_path: &Path,
    destination: &Path,
    limits: ZipExtractionLimits,
) -> Result<(), AppError> {
    if destination.exists() {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "archive destination must not already exist",
        ));
    }
    fs::create_dir_all(destination).map_err(download_error)?;
    ensure_not_reparse_point(destination)?;
    let file = File::open(archive_path).map_err(download_error)?;
    let mut archive = ZipArchive::new(file).map_err(|_| {
        AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "downloaded archive is not a valid ZIP",
        )
    })?;
    if archive.len() > limits.max_entries {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP entry count exceeds extraction limit",
        ));
    }
    let mut normalized_paths = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| {
            AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "could not read ZIP entry",
            )
        })?;
        if entry.is_symlink() {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP symbolic links are not allowed",
            ));
        }
        let relative = validated_zip_path(entry.name(), entry.is_dir())?;
        let normalized = relative
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if !normalized_paths.insert(normalized) {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP contains duplicate normalized paths",
            ));
        }
        let size = entry.size();
        if size > limits.max_entry_uncompressed_bytes
            || total.saturating_add(size) > limits.max_total_uncompressed_bytes
        {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP extraction size limit exceeded",
            ));
        }
        total += size;
        let output = destination.join(&relative);
        ensure_under_root(destination, &output)?;
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(download_error)?;
            ensure_not_reparse_point(&output)?;
            continue;
        }
        let parent = output.parent().ok_or_else(|| {
            AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP output has no parent",
            )
        })?;
        fs::create_dir_all(parent).map_err(download_error)?;
        ensure_path_has_no_reparse_points(destination, parent)?;
        let mut output_file = File::create(&output).map_err(download_error)?;
        let copied = io::copy(&mut entry, &mut output_file).map_err(download_error)?;
        if copied != size || copied > limits.max_entry_uncompressed_bytes {
            return Err(AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP entry length did not match its declared size",
            ));
        }
    }
    Ok(())
}

fn validated_zip_path(name: &str, is_directory: bool) -> Result<PathBuf, AppError> {
    if name.is_empty() || name.contains('\\') || name.contains(':') || name.starts_with('/') {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP contains an absolute or noncanonical path",
        ));
    }
    let trimmed = if is_directory {
        name.strip_suffix('/').ok_or_else(|| {
            AppError::new(
                ErrorCode::EnvironmentDownloadFailed,
                "ZIP directory entry is missing its canonical trailing slash",
            )
        })?
    } else {
        name
    };
    if trimmed.is_empty()
        || trimmed
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP contains an unsafe or noncanonical path",
        ));
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP contains an unsafe path",
        ));
    }
    Ok(path.to_path_buf())
}
fn ensure_under_root(root: &Path, candidate: &Path) -> Result<(), AppError> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP extraction path escaped its destination",
        ))
    }
}
fn ensure_path_has_no_reparse_points(root: &Path, target: &Path) -> Result<(), AppError> {
    let mut current = root.to_path_buf();
    ensure_not_reparse_point(&current)?;
    let relative = target.strip_prefix(root).map_err(|_| {
        AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "ZIP target escaped extraction root",
        )
    })?;
    for component in relative.components() {
        current.push(component);
        ensure_not_reparse_point(&current)?;
    }
    Ok(())
}
fn ensure_not_reparse_point(path: &Path) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(path).map_err(download_error)?;
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(AppError::new(
            ErrorCode::EnvironmentDownloadFailed,
            "reparse points are not allowed during extraction",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadRequest, Downloader, PartialMetadata, ZipExtractionLimits, extract_zip_safely,
        inspect_partial_state, partial_metadata_path, remove_partial, stream_response,
        validated_zip_path, verified_existing_destination, write_metadata,
    };
    use crate::domain::ErrorCode;
    use crate::process::CancellationToken;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Cursor, Read, Write},
        net::TcpListener,
        path::PathBuf,
        thread,
        time::Duration,
    };
    use uuid::Uuid;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn test_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!("soufmer-download-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn serve_once(response: Vec<u8>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            stream.write_all(&response).unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}/artifact"), handle)
    }
    #[test]
    fn streamed_bytes_are_hashed_and_reported() {
        let root = test_directory();
        let output = root.join("artifact.part");
        let mut file = fs::File::create(&output).unwrap();
        let mut hasher = Sha256::new();
        let mut progress = |_| {};
        let copied = stream_response(
            &mut Cursor::new(b"fixture-bytes"),
            &mut file,
            &mut hasher,
            0,
            Some(13),
            &CancellationToken::new(),
            &mut progress,
        )
        .unwrap();
        assert_eq!(copied, 13);
        assert_eq!(
            format!("{:x}", hasher.finalize()),
            "c16a40a4584e5bccc84b45172fcdfa922f59ff1edebf3adba7b8266ea04eb39a"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn zip_extraction_rejects_parent_traversal() {
        let root = test_directory();
        let archive_path = root.join("unsafe.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("../escape.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"no").unwrap();
        zip.finish().unwrap();
        assert!(
            extract_zip_safely(
                &archive_path,
                &root.join("output"),
                ZipExtractionLimits::default()
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zip_paths_reject_lexical_aliases_but_allow_canonical_directory_entries() {
        assert!(validated_zip_path("worker/src/main.py", false).is_ok());
        assert!(validated_zip_path("worker/src/", true).is_ok());
        for (path, directory) in [
            ("worker//main.py", false),
            ("worker/./main.py", false),
            ("./worker/main.py", false),
            ("worker/../main.py", false),
            ("worker\\main.py", false),
            ("worker/src/", false),
            ("worker//", true),
        ] {
            assert!(validated_zip_path(path, directory).is_err(), "{path}");
        }
    }

    #[test]
    fn zip_extraction_rejects_casefolded_duplicates_and_limits() {
        let root = test_directory();
        let duplicate_archive = root.join("duplicate.zip");
        let file = fs::File::create(&duplicate_archive).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("worker/file.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"one").unwrap();
        zip.start_file("WORKER/file.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"two").unwrap();
        zip.finish().unwrap();
        assert!(
            extract_zip_safely(
                &duplicate_archive,
                &root.join("duplicate-output"),
                ZipExtractionLimits::default()
            )
            .is_err()
        );

        let limit_archive = root.join("limit.zip");
        let file = fs::File::create(&limit_archive).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("worker/file.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"too-large").unwrap();
        zip.finish().unwrap();
        assert!(
            extract_zip_safely(
                &limit_archive,
                &root.join("entry-limit-output"),
                ZipExtractionLimits {
                    max_entries: 1,
                    max_total_uncompressed_bytes: 8,
                    max_entry_uncompressed_bytes: 8,
                }
            )
            .is_err()
        );
        assert!(
            extract_zip_safely(
                &limit_archive,
                &root.join("count-limit-output"),
                ZipExtractionLimits {
                    max_entries: 0,
                    max_total_uncompressed_bytes: 64,
                    max_entry_uncompressed_bytes: 64,
                }
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_existing_artifact_is_reused_and_invalid_one_is_removed() {
        let root = test_directory();
        let destination = root.join("artifact.bin");
        fs::write(&destination, b"fixture-bytes").unwrap();
        let request = DownloadRequest {
            url: "https://example.invalid/artifact".into(),
            destination: destination.clone(),
            expected_sha256: "c16a40a4584e5bccc84b45172fcdfa922f59ff1edebf3adba7b8266ea04eb39a"
                .into(),
            expected_size_bytes: Some(13),
        };
        assert_eq!(
            verified_existing_destination(&request)
                .unwrap()
                .unwrap()
                .bytes,
            13
        );
        let invalid = DownloadRequest {
            expected_size_bytes: Some(12),
            ..request
        };
        assert!(verified_existing_destination(&invalid).unwrap().is_none());
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cached_destination_reparse_point_is_rejected_before_hash_or_removal() {
        use std::os::windows::fs::symlink_file;

        let root = test_directory();
        let target = root.join("target.bin");
        let destination = root.join("artifact.bin");
        fs::write(&target, b"fixture-bytes").unwrap();
        symlink_file(&target, &destination).unwrap();
        let request = DownloadRequest {
            url: "https://example.invalid/artifact".into(),
            destination: destination.clone(),
            expected_sha256: "c16a40a4584e5bccc84b45172fcdfa922f59ff1edebf3adba7b8266ea04eb39a"
                .into(),
            expected_size_bytes: Some(13),
        };
        assert!(verified_existing_destination(&request).is_err());
        assert!(destination.exists());
        assert!(target.exists());
        fs::remove_file(destination).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ordinary_partial_files_remain_resumable() {
        let root = test_directory();
        let part = root.join("artifact.bin.part");
        let metadata_path = partial_metadata_path(&part);
        fs::write(&part, b"partial-bytes").unwrap();
        write_metadata(
            &metadata_path,
            &PartialMetadata {
                url: "https://example.invalid/artifact".into(),
                etag: Some("fixture-etag".into()),
                last_modified: None,
            },
        )
        .unwrap();

        let (length, metadata) = inspect_partial_state(&part, &metadata_path).unwrap();
        let metadata = metadata.unwrap();
        assert_eq!(length, 13);
        assert_eq!(metadata.url, "https://example.invalid/artifact");
        assert_eq!(metadata.etag.as_deref(), Some("fixture-etag"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_file_reparse_point_is_rejected_without_touching_target() {
        use std::os::windows::fs::symlink_file;

        let root = test_directory();
        let target = root.join("partial-target.bin");
        let part = root.join("artifact.bin.part");
        let metadata_path = partial_metadata_path(&part);
        fs::write(&target, b"target-must-survive").unwrap();
        symlink_file(&target, &part).unwrap();
        write_metadata(
            &metadata_path,
            &PartialMetadata {
                url: "https://example.invalid/artifact".into(),
                etag: None,
                last_modified: None,
            },
        )
        .unwrap();

        assert!(inspect_partial_state(&part, &metadata_path).is_err());
        assert!(remove_partial(&part, &metadata_path).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"target-must-survive");
        assert!(part.exists());
        assert!(metadata_path.exists());
        fs::remove_file(part).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_metadata_reparse_point_is_rejected_without_touching_target() {
        use std::os::windows::fs::symlink_file;

        let root = test_directory();
        let part = root.join("artifact.bin.part");
        let metadata_path = partial_metadata_path(&part);
        let target = root.join("metadata-target.json");
        let target_bytes =
            br#"{"url":"https://example.invalid/artifact","etag":null,"lastModified":null}"#;
        fs::write(&part, b"partial-bytes").unwrap();
        fs::write(&target, target_bytes).unwrap();
        symlink_file(&target, &metadata_path).unwrap();

        assert!(inspect_partial_state(&part, &metadata_path).is_err());
        assert!(remove_partial(&part, &metadata_path).is_err());
        assert_eq!(fs::read(&target).unwrap(), target_bytes);
        assert!(part.exists());
        assert!(metadata_path.exists());
        fs::remove_file(metadata_path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn downloader_resumes_range_and_promotes_verified_destination() {
        let body = b"-bytes";
        let response = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes 7-12/13\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let mut response = response;
        response.extend_from_slice(body);
        let (url, server) = serve_once(response);
        let root = test_directory();
        let destination = root.join("artifact.bin");
        let part = super::part_path(&destination);
        let metadata_path = partial_metadata_path(&part);
        fs::write(&part, b"fixture").unwrap();
        write_metadata(
            &metadata_path,
            &PartialMetadata {
                url: url.clone(),
                etag: Some("\"fixture\"".into()),
                last_modified: None,
            },
        )
        .unwrap();
        let request = DownloadRequest {
            url,
            destination: destination.clone(),
            expected_sha256: "c16a40a4584e5bccc84b45172fcdfa922f59ff1edebf3adba7b8266ea04eb39a"
                .into(),
            expected_size_bytes: Some(13),
        };
        let mut progress = Vec::new();

        let result = Downloader::new_for_loopback_tests()
            .download(&request, &CancellationToken::new(), &mut |update| {
                progress.push(update.completed_bytes)
            })
            .unwrap();
        let received = server.join().unwrap().to_ascii_lowercase();

        assert!(received.contains("range: bytes=7-"), "{received}");
        assert_eq!(result.bytes, 13);
        assert_eq!(fs::read(&destination).unwrap(), b"fixture-bytes");
        assert_eq!(progress.last(), Some(&13));
        assert!(!part.exists());
        assert!(!metadata_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn downloader_hash_mismatch_cleans_partial_state_without_publishing() {
        let body = b"fixture-bytes";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let mut response = response;
        response.extend_from_slice(body);
        let (url, server) = serve_once(response);
        let root = test_directory();
        let destination = root.join("artifact.bin");
        let part = super::part_path(&destination);
        let metadata_path = partial_metadata_path(&part);
        let request = DownloadRequest {
            url,
            destination: destination.clone(),
            expected_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
                .into(),
            expected_size_bytes: Some(13),
        };

        let error = Downloader::new_for_loopback_tests()
            .download(&request, &CancellationToken::new(), &mut |_| {})
            .unwrap_err();
        let received = server.join().unwrap();

        assert!(received.starts_with("GET /artifact HTTP/1.1\r\n"));
        assert_eq!(error.code, ErrorCode::EnvironmentHashMismatch);
        assert!(!destination.exists());
        assert!(!part.exists());
        assert!(!metadata_path.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
