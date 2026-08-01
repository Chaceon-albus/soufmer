use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::{Cursor, Read, Write},
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, GetDiskFreeSpaceExW},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};
use zip::ZipArchive;

use crate::{
    domain::{AppError, ErrorCode, InitializationStep, ProgressValue},
    download::{
        DownloadProgress, DownloadRequest, Downloader, ZipExtractionLimits, extract_zip_safely,
    },
    process::{CancellationToken, ProcessRunner, ProcessSpec},
    runtime::{
        AppPaths, RuntimeManifest, atomic_write_json,
        embedded::{
            BOOTSTRAP_ARCHIVE_BYTES, BOOTSTRAP_ARCHIVE_LENGTH, BOOTSTRAP_ARCHIVE_SHA256,
            BOOTSTRAP_ENTRY_MANIFEST_PATH, BOOTSTRAP_VERSION,
        },
    },
};

const MAX_BOOTSTRAP_ENTRIES: usize = 10_000;
const MAX_BOOTSTRAP_BYTES: u64 = 512 * 1024 * 1024;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_ABANDONED: u32 = 0x80;
const WAIT_TIMEOUT: u32 = 0x102;
const WAIT_FAILED: u32 = u32::MAX;
const MUTEX_WAIT_MILLISECONDS: u32 = 100;

#[derive(Clone, Debug)]
pub struct InitUpdate {
    pub step: InitializationStep,
    pub current: ProgressValue,
    pub fraction: f64,
    pub bytes: Option<(u64, Option<u64>, u64)>,
    pub detail: &'static str,
}
#[derive(Clone, Debug)]
pub struct ActiveRuntime {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub python: PathBuf,
    pub worker_cwd: PathBuf,
    pub worker_module: String,
    pub checkpoint: PathBuf,
    pub config: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub logs: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Current {
    schema_version: u8,
    id: String,
    #[serde(default)]
    manifest_digest: String,
    activated_at: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryManifest {
    schema_version: u8,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    length: u64,
    sha256: String,
}

pub fn environment_status(paths: &AppPaths) -> Result<crate::domain::EnvironmentStatus, AppError> {
    verify_archive()?;
    let manifest_bytes = embedded_manifest_bytes()?;
    let manifest = RuntimeManifest::parse(
        std::str::from_utf8(&manifest_bytes)
            .map_err(|_| runtime_error(std::io::Error::other("manifest UTF-8")))?,
    )?;
    let bootstrap = paths.bootstrap_versions().join(BOOTSTRAP_VERSION);
    if !bootstrap.exists() || validate_active_bootstrap(&bootstrap).is_err() {
        return Ok(crate::domain::EnvironmentStatus::NotInstalled {
            estimated_download_bytes: Some(manifest.estimates.download_bytes),
            estimated_disk_bytes: Some(manifest.estimates.installed_bytes),
        });
    }
    let current = read_current(&paths.current_runtime_file())?;
    let Some(current) = current else {
        return Ok(crate::domain::EnvironmentStatus::NotInstalled {
            estimated_download_bytes: Some(manifest.estimates.download_bytes),
            estimated_disk_bytes: Some(manifest.estimates.installed_bytes),
        });
    };
    let runtime = paths.runtime_versions().join(&current.id);
    if current.manifest_digest == manifest.digest(&manifest_bytes)
        && validate_runtime(
            paths,
            &runtime,
            &manifest,
            &manifest.digest(&manifest_bytes),
        )
        .is_ok()
    {
        Ok(crate::domain::EnvironmentStatus::Ready {
            runtime_version: manifest.bootstrap_version,
            model_version: manifest.model.revision,
            ffmpeg_version: manifest.ffmpeg.version,
        })
    } else {
        Ok(crate::domain::EnvironmentStatus::RepairRequired {
            reason_code: "RUNTIME_VALIDATION_FAILED".into(),
        })
    }
}
pub fn resolve_active_runtime(paths: &AppPaths) -> Result<ActiveRuntime, AppError> {
    let manifest_bytes = embedded_manifest_bytes()?;
    let manifest = RuntimeManifest::parse(
        std::str::from_utf8(&manifest_bytes)
            .map_err(|_| invalid("embedded runtime manifest is not UTF-8"))?,
    )?;
    let current = read_current(&paths.current_runtime_file())?.ok_or_else(|| {
        AppError::new(
            ErrorCode::EnvironmentNotInitialized,
            "no active runtime metadata",
        )
    })?;
    if current.manifest_digest != manifest.digest(&manifest_bytes) {
        return Err(AppError::new(
            ErrorCode::EnvironmentNotInitialized,
            "active runtime manifest digest mismatch",
        ));
    }
    let root = paths.runtime_versions().join(&current.id);
    validate_runtime(paths, &root, &manifest, &current.manifest_digest)?;
    Ok(ActiveRuntime {
        ffmpeg: root.join("tools/ffmpeg").join(&manifest.ffmpeg.ffmpeg_path),
        ffprobe: root
            .join("tools/ffmpeg")
            .join(&manifest.ffmpeg.ffprobe_path),
        python: root.join("venv/Scripts/python.exe"),
        worker_cwd: root.join("worker"),
        worker_module: "accompaniment_worker".into(),
        checkpoint: model_path(paths, &manifest),
        config: root.join(
            "worker/vendor/msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml",
        ),
        environment: private_environment(paths, &root)?,
        logs: paths.logs(),
    })
}

pub fn initialize(
    paths: &AppPaths,
    cancellation: &CancellationToken,
    emit: &mut impl FnMut(InitUpdate),
) -> Result<(), AppError> {
    let _mutex = RuntimeMutex::acquire(cancellation)?;
    ensure_root(paths)?;
    emit(update(
        InitializationStep::CheckingSystem,
        0.02,
        "validating private runtime state",
    ));
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    let bootstrap = install_bootstrap(paths)?;
    let source = fs::read(bootstrap.join("runtime-manifest.json")).map_err(runtime_error)?;
    let manifest = RuntimeManifest::parse(
        std::str::from_utf8(&source).map_err(|_| invalid("runtime manifest is not UTF-8"))?,
    )?;
    ensure_free_space(paths.root(), manifest.estimates.minimum_free_bytes)?;
    if let Ok(crate::domain::EnvironmentStatus::Ready { .. }) = environment_status(paths) {
        return Ok(());
    }
    emit(update(
        InitializationStep::PreparingTools,
        0.05,
        "validated embedded bootstrap",
    ));
    let runtime_id = format!(
        "runtime-{}-cuda-{}-{}",
        manifest.bootstrap_version,
        &manifest.digest(&source)[..12],
        Uuid::new_v4()
    );
    let runtime = paths.runtime_versions().join(&runtime_id);
    if runtime.exists() {
        return Err(invalid("inactive runtime version already exists"));
    }
    fs::create_dir_all(&runtime).map_err(runtime_error)?;
    let result = install_runtime(paths, &bootstrap, &runtime, &manifest, cancellation, emit);
    if result.is_err() {
        let _ = fs::remove_dir_all(&runtime);
        return result;
    }
    validate_runtime(paths, &runtime, &manifest, &manifest.digest(&source))?;
    write_json_atomic(
        &paths.current_runtime_file(),
        &Current {
            schema_version: 1,
            id: runtime_id,
            manifest_digest: manifest.digest(&source),
            activated_at: now(),
        },
    )?;
    emit(update(
        InitializationStep::Activating,
        1.0,
        "activated validated runtime",
    ));
    Ok(())
}

fn install_runtime(
    paths: &AppPaths,
    bootstrap: &Path,
    runtime: &Path,
    manifest: &RuntimeManifest,
    cancellation: &CancellationToken,
    emit: &mut impl FnMut(InitUpdate),
) -> Result<(), AppError> {
    copy_tree(&bootstrap.join("worker"), &runtime.join("worker"))?;
    let uv = bootstrap.join("bin/uv.exe");
    let environment = private_environment(paths, runtime)?;
    emit(update(
        InitializationStep::InstallingPython,
        0.15,
        "installing managed Python",
    ));
    run(
        &uv,
        &["python", "install", "3.11"],
        runtime,
        &environment,
        cancellation,
        ErrorCode::PythonSyncFailed,
    )?;
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    emit(InitUpdate {
        step: InitializationStep::SyncingEnvironment,
        current: ProgressValue::Indeterminate,
        fraction: 0.60,
        bytes: None,
        detail: "synchronizing locked CUDA environment",
    });
    let args = manifest
        .worker
        .production_sync_arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    run(
        &uv,
        &args,
        &runtime.join("worker"),
        &environment,
        cancellation,
        ErrorCode::PythonSyncFailed,
    )?;
    install_ffmpeg(paths, runtime, manifest, cancellation, emit)?;
    install_model(paths, manifest, cancellation, emit)?;
    emit(update(
        InitializationStep::SelfTesting,
        0.99,
        "running private worker self-test",
    ));
    let python = runtime.join("venv/Scripts/python.exe");
    let model = model_path(paths, manifest);
    let config = runtime
        .join("worker/vendor/msst/configs/KimberleyJensen/config_vocals_mel_band_roformer_kj.yaml");
    let self_test = capture_output(
        &python,
        &[
            "-m",
            "accompaniment_worker",
            "self-test",
            "--checkpoint",
            model.to_str().unwrap_or_default(),
            "--config",
            config.to_str().unwrap_or_default(),
            "--device",
            "cuda:0",
        ],
        &runtime.join("worker"),
        &environment,
        cancellation,
        ErrorCode::InferenceFailed,
    )?;
    let verified = parse_self_test_event(&self_test)?;
    fs::write(runtime.join("self-test.json"), serde_json::to_vec(&serde_json::json!({"manifestDigest": manifest.digest(&fs::read(bootstrap.join("runtime-manifest.json")).map_err(runtime_error)?), "profile":"cuda", "worker": verified})).map_err(|_| invalid("self test serialization"))?).map_err(runtime_error)?;
    fs::write(runtime.join("READY"), b"ready\n").map_err(runtime_error)
}
fn install_ffmpeg(
    paths: &AppPaths,
    runtime: &Path,
    manifest: &RuntimeManifest,
    cancellation: &CancellationToken,
    emit: &mut impl FnMut(InitUpdate),
) -> Result<(), AppError> {
    let archive = paths
        .downloads()
        .join(format!("ffmpeg-{}.zip", manifest.ffmpeg.version));
    let start = Instant::now();
    let downloader = Downloader::new()?;
    let mut progress = |p: DownloadProgress| {
        let fraction = p
            .total_bytes
            .map(|total| p.completed_bytes as f64 / total as f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        emit(InitUpdate {
            step: InitializationStep::PreparingTools,
            current: ProgressValue::Determinate { fraction },
            fraction: 0.60 + fraction * 0.05,
            bytes: Some((
                p.completed_bytes,
                p.total_bytes,
                speed(p.completed_bytes, start),
            )),
            detail: "downloading FFmpeg",
        });
    };
    downloader.download(
        &DownloadRequest {
            url: manifest.ffmpeg.archive_url.clone(),
            destination: archive.clone(),
            expected_sha256: manifest.ffmpeg.archive_sha256.clone(),
            expected_size_bytes: Some(manifest.ffmpeg.archive_size_bytes),
        },
        cancellation,
        &mut progress,
    )?;
    let stage = paths.staging().join(format!("ffmpeg-{}", Uuid::new_v4()));
    extract_zip_safely(&archive, &stage, ZipExtractionLimits::default())?;
    let installed = runtime.join("tools").join("ffmpeg");
    fs::create_dir_all(
        installed
            .parent()
            .ok_or_else(|| invalid("FFmpeg install parent missing"))?,
    )
    .map_err(runtime_error)?;
    fs::rename(stage.join(&manifest.ffmpeg.archive_root), &installed).map_err(runtime_error)?;
    let env = private_environment(paths, runtime)?;
    for binary in [&manifest.ffmpeg.ffmpeg_path, &manifest.ffmpeg.ffprobe_path] {
        run(
            &installed.join(binary),
            &["-version"],
            runtime,
            &env,
            cancellation,
            ErrorCode::FfmpegNotAvailable,
        )?;
    }
    emit(InitUpdate {
        step: InitializationStep::PreparingTools,
        current: ProgressValue::Determinate { fraction: 1.0 },
        fraction: 0.65,
        bytes: Some((
            manifest.ffmpeg.archive_size_bytes,
            Some(manifest.ffmpeg.archive_size_bytes),
            speed(manifest.ffmpeg.archive_size_bytes, start),
        )),
        detail: "installed verified FFmpeg",
    });
    Ok(())
}
fn install_model(
    paths: &AppPaths,
    manifest: &RuntimeManifest,
    cancellation: &CancellationToken,
    emit: &mut impl FnMut(InitUpdate),
) -> Result<(), AppError> {
    let target = model_path(paths, manifest);
    let start = Instant::now();
    let mut progress = |p: DownloadProgress| {
        let fraction = p
            .total_bytes
            .map(|total| p.completed_bytes as f64 / total as f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        emit(InitUpdate {
            step: InitializationStep::DownloadingModel,
            current: ProgressValue::Determinate { fraction },
            fraction: 0.65 + fraction * 0.30,
            bytes: Some((
                p.completed_bytes,
                p.total_bytes,
                speed(p.completed_bytes, start),
            )),
            detail: "downloading model",
        });
    };
    Downloader::new()
        .map_err(model_download_error)?
        .download(
            &DownloadRequest {
                url: manifest.model.download_url.clone(),
                destination: target,
                expected_sha256: manifest.model.sha256.clone(),
                expected_size_bytes: Some(manifest.model.size_bytes),
            },
            cancellation,
            &mut progress,
        )
        .map_err(model_download_error)?;
    emit(InitUpdate {
        step: InitializationStep::DownloadingModel,
        current: ProgressValue::Determinate { fraction: 1.0 },
        fraction: 0.95,
        bytes: Some((
            manifest.model.size_bytes,
            Some(manifest.model.size_bytes),
            speed(manifest.model.size_bytes, start),
        )),
        detail: "downloaded verified model",
    });
    Ok(())
}

fn model_download_error(error: AppError) -> AppError {
    match error.code {
        ErrorCode::EnvironmentDownloadFailed => {
            AppError::new(ErrorCode::ModelDownloadFailed, error.technical_detail)
        }
        ErrorCode::TaskCancelled | ErrorCode::EnvironmentHashMismatch => error,
        _ => error,
    }
}

fn install_bootstrap(paths: &AppPaths) -> Result<PathBuf, AppError> {
    ensure_root(paths)?;
    verify_archive()?;
    let target = paths.bootstrap_versions().join(BOOTSTRAP_VERSION);
    if validate_active_bootstrap(&target).is_ok() {
        return Ok(target);
    }
    let stage = paths
        .staging()
        .join(format!("bootstrap-{}", Uuid::new_v4()));
    extract_embedded(&stage)?;
    validate_bootstrap_content(&stage)?;
    fs::write(stage.join("READY"), b"ready\n").map_err(runtime_error)?;
    fs::create_dir_all(paths.bootstrap_versions()).map_err(runtime_error)?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(runtime_error)?;
    }
    fs::rename(&stage, &target).map_err(runtime_error)?;
    write_json_atomic(
        &paths.current_bootstrap_file(),
        &Current {
            schema_version: 1,
            id: BOOTSTRAP_VERSION.into(),
            manifest_digest: String::new(),
            activated_at: now(),
        },
    )?;
    Ok(target)
}
fn verify_archive() -> Result<(), AppError> {
    verify_archive_descriptor(
        BOOTSTRAP_ARCHIVE_BYTES,
        BOOTSTRAP_ARCHIVE_LENGTH,
        BOOTSTRAP_ARCHIVE_SHA256,
    )
}
fn verify_archive_descriptor(
    bytes: &[u8],
    expected_length: usize,
    expected_sha256: &str,
) -> Result<(), AppError> {
    let actual_sha256 = format!("{:x}", Sha256::digest(bytes));
    if bytes.len() != expected_length || !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        Err(AppError::new(
            ErrorCode::EnvironmentHashMismatch,
            "embedded bootstrap archive descriptor mismatch",
        ))
    } else {
        Ok(())
    }
}
fn embedded_manifest_bytes() -> Result<Vec<u8>, AppError> {
    let mut zip = ZipArchive::new(Cursor::new(BOOTSTRAP_ARCHIVE_BYTES))
        .map_err(|_| invalid("embedded bootstrap ZIP invalid"))?;
    let mut entry = zip
        .by_name("runtime-manifest.json")
        .map_err(|_| invalid("embedded runtime manifest missing"))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(runtime_error)?;
    Ok(bytes)
}
fn extract_embedded(destination: &Path) -> Result<(), AppError> {
    fs::create_dir_all(destination).map_err(runtime_error)?;
    no_reparse(destination)?;
    let mut zip = ZipArchive::new(Cursor::new(BOOTSTRAP_ARCHIVE_BYTES))
        .map_err(|_| invalid("embedded bootstrap ZIP invalid"))?;
    if zip.len() > MAX_BOOTSTRAP_ENTRIES {
        return Err(invalid("embedded bootstrap has too many entries"));
    }
    let manifest_bytes = {
        let mut e = zip
            .by_name(BOOTSTRAP_ENTRY_MANIFEST_PATH)
            .map_err(|_| invalid("bootstrap entry manifest missing"))?;
        let mut b = Vec::new();
        e.read_to_end(&mut b).map_err(runtime_error)?;
        b
    };
    let manifest: EntryManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| invalid("bootstrap entry manifest invalid"))?;
    if manifest.schema_version != 1 {
        return Err(invalid("unsupported bootstrap entry manifest"));
    }
    let expected = manifest
        .entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|_| invalid("bootstrap ZIP read failed"))?;
        if entry.name() == BOOTSTRAP_ENTRY_MANIFEST_PATH {
            continue;
        }
        if entry.is_dir() || entry.is_symlink() {
            return Err(invalid("bootstrap ZIP may contain regular files only"));
        }
        let name = entry.name().to_owned();
        let record = expected
            .get(&name)
            .ok_or_else(|| invalid("bootstrap ZIP entry is unmanifested"))?;
        let rel = safe_zip_path(&name)?;
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(invalid("duplicate bootstrap path"));
        }
        if entry.size() != record.length
            || entry.size() > MAX_BOOTSTRAP_BYTES
            || total.saturating_add(entry.size()) > MAX_BOOTSTRAP_BYTES
        {
            return Err(invalid("bootstrap ZIP size limit or manifest mismatch"));
        }
        total += entry.size();
        let output = destination.join(rel);
        let parent = output
            .parent()
            .ok_or_else(|| invalid("bootstrap output parent missing"))?;
        fs::create_dir_all(parent).map_err(runtime_error)?;
        no_reparse_chain(destination, parent)?;
        let mut file = File::create(&output).map_err(runtime_error)?;
        let copied = std::io::copy(&mut entry, &mut file).map_err(runtime_error)?;
        let actual = hash_file(&output)?;
        verify_extracted_entry_descriptor(record, true, copied, &actual)?;
    }
    if seen.len() != expected.len() {
        return Err(invalid("bootstrap ZIP missing manifest entries"));
    }
    Ok(())
}
fn validate_bootstrap_content(root: &Path) -> Result<(), AppError> {
    let bytes = fs::read(root.join("runtime-manifest.json")).map_err(runtime_error)?;
    RuntimeManifest::parse(std::str::from_utf8(&bytes).map_err(|_| invalid("manifest UTF-8"))?)?;
    Ok(())
}
fn validate_active_bootstrap(root: &Path) -> Result<(), AppError> {
    if !root.join("READY").is_file() {
        return Err(invalid("bootstrap marker missing"));
    }
    validate_bootstrap_content(root)?;
    verify_extracted_entries(root)
}
fn verify_extracted_entries(root: &Path) -> Result<(), AppError> {
    let mut zip = ZipArchive::new(Cursor::new(BOOTSTRAP_ARCHIVE_BYTES))
        .map_err(|_| invalid("embedded bootstrap ZIP invalid"))?;
    let mut entry = zip
        .by_name(BOOTSTRAP_ENTRY_MANIFEST_PATH)
        .map_err(|_| invalid("bootstrap entry manifest missing"))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(runtime_error)?;
    let manifest: EntryManifest =
        serde_json::from_slice(&bytes).map_err(|_| invalid("bootstrap entry manifest invalid"))?;
    for record in manifest.entries {
        let path = root.join(safe_zip_path(&record.path)?);
        let metadata = fs::symlink_metadata(&path).map_err(runtime_error)?;
        let regular_file = metadata.file_type().is_file()
            && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0;
        let actual_hash = if regular_file {
            hash_file(&path)?
        } else {
            String::new()
        };
        verify_extracted_entry_descriptor(&record, regular_file, metadata.len(), &actual_hash)?;
    }
    Ok(())
}
fn verify_extracted_entry_descriptor(
    record: &Entry,
    is_regular_file: bool,
    actual_length: u64,
    actual_sha256: &str,
) -> Result<(), AppError> {
    if is_regular_file
        && actual_length == record.length
        && actual_sha256.eq_ignore_ascii_case(&record.sha256)
    {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::EnvironmentHashMismatch,
            "bootstrap entry does not match embedded manifest",
        ))
    }
}
fn validate_runtime(
    paths: &AppPaths,
    runtime: &Path,
    manifest: &RuntimeManifest,
    manifest_digest: &str,
) -> Result<(), AppError> {
    for path in [
        runtime.join("READY"),
        runtime.join("self-test.json"),
        runtime.join("venv/Scripts/python.exe"),
        runtime.join("worker/pyproject.toml"),
        model_path(paths, manifest),
        runtime
            .join("tools/ffmpeg")
            .join(&manifest.ffmpeg.ffmpeg_path),
        runtime
            .join("tools/ffmpeg")
            .join(&manifest.ffmpeg.ffprobe_path),
    ] {
        if !path.is_file() {
            return Err(invalid("active runtime component is missing"));
        }
    }
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(runtime.join("self-test.json")).map_err(runtime_error)?)
            .map_err(|_| invalid("self-test record invalid"))?;
    if record
        .get("manifestDigest")
        .and_then(serde_json::Value::as_str)
        != Some(manifest_digest)
    {
        return Err(invalid("self-test record does not match manifest"));
    }
    let metadata = fs::metadata(model_path(paths, manifest)).map_err(runtime_error)?;
    if metadata.len() != manifest.model.size_bytes
        || hash_file(&model_path(paths, manifest))? != manifest.model.sha256
    {
        return Err(AppError::new(
            ErrorCode::EnvironmentHashMismatch,
            "active model does not match manifest",
        ));
    }
    Ok(())
}
fn private_environment(
    paths: &AppPaths,
    runtime: &Path,
) -> Result<Vec<(OsString, OsString)>, AppError> {
    let temp = runtime.join("tmp");
    let cache = paths.models().join("cache");
    for directory in [
        &temp,
        &paths.uv_cache(),
        &runtime.join("python"),
        &runtime.join("venv"),
        &cache,
        &paths.logs(),
    ] {
        fs::create_dir_all(directory).map_err(runtime_error)?;
        no_reparse(directory)?;
    }
    Ok(vec![
        ("TEMP".into(), temp.clone().into_os_string()),
        ("TMP".into(), temp.into_os_string()),
        ("UV_CACHE_DIR".into(), paths.uv_cache().into_os_string()),
        (
            "UV_PYTHON_INSTALL_DIR".into(),
            runtime.join("python").into_os_string(),
        ),
        (
            "UV_PROJECT_ENVIRONMENT".into(),
            runtime.join("venv").into_os_string(),
        ),
        (
            "HF_HOME".into(),
            paths.models().join("cache").into_os_string(),
        ),
        (
            "TORCH_HOME".into(),
            paths.models().join("cache/torch").into_os_string(),
        ),
        ("XDG_CACHE_HOME".into(), cache.clone().into_os_string()),
        (
            "MPLCONFIGDIR".into(),
            cache.join("matplotlib").into_os_string(),
        ),
        (
            "NUMBA_CACHE_DIR".into(),
            cache.join("numba").into_os_string(),
        ),
        (
            "SOUFMER_RUNTIME_LOG_DIR".into(),
            paths.logs().into_os_string(),
        ),
    ])
}
fn run(
    exe: &Path,
    args: &[&str],
    cwd: &Path,
    environment: &[(OsString, OsString)],
    cancellation: &CancellationToken,
    code: ErrorCode,
) -> Result<(), AppError> {
    run_output(exe, args, cwd, environment, cancellation, code).map(|_| ())
}
fn run_output(
    exe: &Path,
    args: &[&str],
    cwd: &Path,
    environment: &[(OsString, OsString)],
    cancellation: &CancellationToken,
    code: ErrorCode,
) -> Result<crate::process::ProcessOutput, AppError> {
    let result = capture_output(exe, args, cwd, environment, cancellation, code)?;
    if result.exit_code == Some(0) {
        Ok(result)
    } else {
        Err(AppError::new(code, "runtime process exited unsuccessfully"))
    }
}

fn capture_output(
    exe: &Path,
    args: &[&str],
    cwd: &Path,
    environment: &[(OsString, OsString)],
    cancellation: &CancellationToken,
    code: ErrorCode,
) -> Result<crate::process::ProcessOutput, AppError> {
    let mut spec = ProcessSpec::new(exe);
    spec.arguments = args.iter().map(OsString::from).collect();
    spec.current_dir = Some(cwd.to_path_buf());
    spec.environment = environment.to_vec();
    if let Some((_, directory)) = environment
        .iter()
        .find(|(name, _)| name == "SOUFMER_RUNTIME_LOG_DIR")
    {
        spec.stderr_log =
            Some(PathBuf::from(directory).join(format!("runtime-{}.stderr.log", Uuid::new_v4())));
    }
    ProcessRunner::run(spec, cancellation.clone(), Arc::new(|_| {})).map_err(|e| {
        if e.code == ErrorCode::TaskCancelled {
            e
        } else {
            AppError::new(code, e.technical_detail)
        }
    })
}
fn parse_self_test_event(
    output: &crate::process::ProcessOutput,
) -> Result<serde_json::Value, AppError> {
    if output.stdout_lines.len() != 1 {
        return Err(AppError::new(
            ErrorCode::InferenceFailed,
            "worker self-test must emit exactly one JSON Lines event",
        ));
    }
    let event: serde_json::Value = serde_json::from_str(&output.stdout_lines[0]).map_err(|_| {
        AppError::new(
            ErrorCode::InferenceFailed,
            "worker self-test emitted invalid JSON",
        )
    })?;
    let success = event
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && event.get("type").and_then(serde_json::Value::as_str) == Some("selfTest")
        && event
            .pointer("/payload/status")
            .and_then(serde_json::Value::as_str)
            == Some("READY")
        && event
            .pointer("/payload/inferenceAvailable")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        && event
            .pointer("/payload/device")
            .and_then(serde_json::Value::as_str)
            == Some("cuda:0");
    if success {
        if output.exit_code == Some(0) {
            return Ok(event);
        }
        return Err(AppError::new(
            ErrorCode::InferenceFailed,
            "worker self-test reported readiness but exited unsuccessfully",
        ));
    }

    if event
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && event.get("type").and_then(serde_json::Value::as_str) == Some("error")
    {
        let code = match event
            .pointer("/payload/code")
            .and_then(serde_json::Value::as_str)
        {
            Some("CUDA_NOT_AVAILABLE") => ErrorCode::CudaNotAvailable,
            Some("CUDA_OUT_OF_MEMORY") => ErrorCode::CudaOutOfMemory,
            _ => ErrorCode::InferenceFailed,
        };
        return Err(AppError::new(
            code,
            "worker self-test reported an error event",
        ));
    }

    Err(AppError::new(
        ErrorCode::InferenceFailed,
        "worker self-test did not emit a valid success or error event",
    ))
}
fn ensure_root(paths: &AppPaths) -> Result<(), AppError> {
    for path in [
        paths.root(),
        &paths.staging(),
        &paths.downloads(),
        &paths.runtime_versions(),
        &paths.bootstrap_versions(),
        &paths.tools(),
        &paths.models(),
        &paths.uv_cache(),
    ] {
        fs::create_dir_all(path).map_err(runtime_error)?;
        no_reparse(path)?;
    }
    Ok(())
}

fn ensure_free_space(root: &Path, required_bytes: u64) -> Result<(), AppError> {
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut free = 0_u64;
    let wide = root
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
    if result == 0 {
        return Err(AppError::new(
            ErrorCode::LocalDataUnavailable,
            "could not determine available private runtime disk space",
        ));
    }
    if !has_required_free_space(available, required_bytes) {
        return Err(AppError::new(
            ErrorCode::LocalDataUnavailable,
            "insufficient free disk space for the private runtime estimate",
        ));
    }
    Ok(())
}

fn has_required_free_space(available_bytes: u64, required_bytes: u64) -> bool {
    available_bytes >= required_bytes
}
fn copy_tree(source: &Path, target: &Path) -> Result<(), AppError> {
    fs::create_dir_all(target).map_err(runtime_error)?;
    for e in fs::read_dir(source).map_err(runtime_error)? {
        let e = e.map_err(runtime_error)?;
        let from = e.path();
        let to = target.join(e.file_name());
        no_reparse(&from)?;
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(runtime_error)?;
        }
    }
    Ok(())
}
fn model_path(paths: &AppPaths, manifest: &RuntimeManifest) -> PathBuf {
    paths
        .models()
        .join("kimberley-melbandroformer")
        .join(&manifest.model.revision)
        .join(&manifest.model.file_name)
}
fn safe_zip_path(name: &str) -> Result<PathBuf, AppError> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains(':')
        || name.starts_with('/')
        || name.ends_with('/')
        || name
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(invalid("unsafe bootstrap archive path"));
    }
    let path = Path::new(name);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        Err(invalid("unsafe bootstrap archive path"))
    } else {
        Ok(path.to_path_buf())
    }
}
fn no_reparse(path: &Path) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(path).map_err(runtime_error)?;
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        Err(invalid("runtime paths cannot use reparse points"))
    } else {
        Ok(())
    }
}
fn no_reparse_chain(root: &Path, target: &Path) -> Result<(), AppError> {
    let mut current = root.to_path_buf();
    for c in target
        .strip_prefix(root)
        .map_err(|_| invalid("runtime path escaped root"))?
        .components()
    {
        current.push(c);
        no_reparse(&current)?;
    }
    Ok(())
}
fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut f = File::open(path).map_err(runtime_error)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut HashWriter(&mut h)).map_err(runtime_error)?;
    Ok(format!("{:x}", h.finalize()))
}
struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.update(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn read_current(path: &Path) -> Result<Option<Current>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    serde_json::from_slice(&fs::read(path).map_err(runtime_error)?)
        .map(Some)
        .map_err(|_| invalid("runtime state metadata is invalid"))
}
fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), AppError> {
    atomic_write_json(path, value).map_err(runtime_error)
}
fn update(step: InitializationStep, fraction: f64, detail: &'static str) -> InitUpdate {
    InitUpdate {
        step,
        current: ProgressValue::Determinate { fraction: 1.0 },
        fraction,
        bytes: None,
        detail,
    }
}
fn speed(bytes: u64, start: Instant) -> u64 {
    (bytes as f64 / start.elapsed().as_secs_f64().max(0.001)) as u64
}
fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
fn runtime_error(_: std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::LocalDataUnavailable,
        "private runtime storage operation failed",
    )
}
fn invalid(s: &str) -> AppError {
    AppError::new(ErrorCode::ManifestInvalid, s)
}
fn cancelled() -> AppError {
    AppError::new(ErrorCode::TaskCancelled, "runtime initialization cancelled")
}
struct RuntimeMutex(windows_sys::Win32::Foundation::HANDLE);
impl RuntimeMutex {
    fn acquire(cancellation: &CancellationToken) -> Result<Self, AppError> {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let name: Vec<u16> = "Local\\SoufmerRuntimeInitialization"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(runtime_error(std::io::Error::last_os_error()));
        }
        loop {
            if cancellation.is_cancelled() {
                unsafe { CloseHandle(handle) };
                return Err(cancelled());
            }
            match unsafe { WaitForSingleObject(handle, MUTEX_WAIT_MILLISECONDS) } {
                WAIT_OBJECT_0 | WAIT_ABANDONED => return Ok(Self(handle)),
                WAIT_TIMEOUT => continue,
                WAIT_FAILED => {
                    let error = runtime_error(std::io::Error::last_os_error());
                    unsafe { CloseHandle(handle) };
                    return Err(error);
                }
                _ => {
                    unsafe { CloseHandle(handle) };
                    return Err(runtime_error(std::io::Error::other(
                        "unexpected runtime mutex wait result",
                    )));
                }
            }
        }
    }
}
impl Drop for RuntimeMutex {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Entry, parse_self_test_event, safe_zip_path, verify_archive_descriptor,
        verify_extracted_entry_descriptor,
    };
    use crate::domain::ErrorCode;
    use crate::process::ProcessOutput;
    use sha2::{Digest, Sha256};

    fn output(exit_code: Option<i32>, lines: &[&str]) -> ProcessOutput {
        ProcessOutput {
            exit_code,
            stdout_line_count: lines.len(),
            stdout_lines: lines.iter().map(|line| (*line).into()).collect(),
            stderr: String::new(),
            stderr_truncated: false,
        }
    }
    #[test]
    fn self_test_requires_one_ready_cuda_event() {
        let ready = r#"{"schemaVersion":1,"type":"selfTest","payload":{"status":"READY","inferenceAvailable":true,"device":"cuda:0"}}"#;
        assert!(parse_self_test_event(&output(Some(0), &[ready])).is_ok());
        assert!(parse_self_test_event(&output(Some(0), &[])).is_err());
        assert!(parse_self_test_event(&output(Some(0), &[ready, ready])).is_err());
        assert!(parse_self_test_event(&output(Some(0), &[r#"{"schemaVersion":1,"type":"selfTest","payload":{"status":"FAILED","inferenceAvailable":true,"device":"cuda:0"}}"#])).is_err());
    }

    #[test]
    fn self_test_requires_zero_exit_for_ready_event() {
        let ready = r#"{"schemaVersion":1,"type":"selfTest","payload":{"status":"READY","inferenceAvailable":true,"device":"cuda:0"}}"#;
        let error = parse_self_test_event(&output(Some(12), &[ready])).unwrap_err();
        assert_eq!(error.code, ErrorCode::InferenceFailed);
    }

    #[test]
    fn self_test_maps_authoritative_worker_error_events() {
        let unavailable = r#"{"schemaVersion":1,"type":"error","taskId":"self-test","payload":{"code":"CUDA_NOT_AVAILABLE"}}"#;
        let out_of_memory = r#"{"schemaVersion":1,"type":"error","taskId":"self-test","payload":{"code":"CUDA_OUT_OF_MEMORY"}}"#;
        let unknown = r#"{"schemaVersion":1,"type":"error","taskId":"self-test","payload":{"code":"INVALID_REQUEST"}}"#;
        assert_eq!(
            parse_self_test_event(&output(Some(12), &[unavailable]))
                .unwrap_err()
                .code,
            ErrorCode::CudaNotAvailable
        );
        assert_eq!(
            parse_self_test_event(&output(Some(13), &[out_of_memory]))
                .unwrap_err()
                .code,
            ErrorCode::CudaOutOfMemory
        );
        assert_eq!(
            parse_self_test_event(&output(Some(2), &[unknown]))
                .unwrap_err()
                .code,
            ErrorCode::InferenceFailed
        );
    }

    #[test]
    fn bootstrap_paths_require_canonical_forward_slash_components() {
        for valid in [
            "worker/pyproject.toml",
            "bin/uv.exe",
            "licenses/FFmpeg-COPYING.txt",
        ] {
            assert!(safe_zip_path(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "/absolute.txt",
            "\\\\server\\share",
            "C:/drive.txt",
            "worker\\file.py",
            "worker//file.py",
            "worker/./file.py",
            "worker/../file.py",
            "worker/file.txt:stream",
            "worker/",
        ] {
            assert!(safe_zip_path(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn embedded_archive_descriptor_rejects_corruption() {
        let bytes = b"small deterministic archive fixture";
        let expected_sha256 = format!("{:x}", Sha256::digest(bytes));
        assert!(verify_archive_descriptor(bytes, bytes.len(), &expected_sha256).is_ok());

        let mut corrupted = bytes.to_vec();
        corrupted[0] ^= 0xff;
        for result in [
            verify_archive_descriptor(&corrupted, bytes.len(), &expected_sha256),
            verify_archive_descriptor(bytes, bytes.len() + 1, &expected_sha256),
            verify_archive_descriptor(bytes, bytes.len(), &"0".repeat(64)),
        ] {
            assert_eq!(result.unwrap_err().code, ErrorCode::EnvironmentHashMismatch);
        }
    }

    #[test]
    fn extracted_entry_descriptor_rejects_length_and_content_corruption() {
        let expected = b"verified extracted entry";
        let record = Entry {
            path: "worker/file.py".into(),
            length: expected.len() as u64,
            sha256: format!("{:x}", Sha256::digest(expected)),
        };
        assert!(verify_extracted_entry_descriptor(
            &record,
            true,
            expected.len() as u64,
            &record.sha256,
        )
        .is_ok());

        let corrupted_hash = format!("{:x}", Sha256::digest(b"corrupted extracted entry"));
        for result in [
            verify_extracted_entry_descriptor(
                &record,
                true,
                expected.len() as u64 + 1,
                &record.sha256,
            ),
            verify_extracted_entry_descriptor(
                &record,
                true,
                expected.len() as u64,
                &corrupted_hash,
            ),
            verify_extracted_entry_descriptor(
                &record,
                false,
                expected.len() as u64,
                &record.sha256,
            ),
        ] {
            assert_eq!(result.unwrap_err().code, ErrorCode::EnvironmentHashMismatch);
        }
    }

    #[test]
    fn disk_requirement_requires_the_full_manifest_estimate() {
        assert!(super::has_required_free_space(12, 12));
        assert!(!super::has_required_free_space(11, 12));
    }
}
