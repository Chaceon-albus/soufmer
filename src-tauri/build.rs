use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

const ENTRY_MANIFEST_PATH: &str = "_bootstrap-entry-manifest.json";
const REQUIRED_LICENSES: &[&str] = &[
    "UV_LICENSE_MIT.txt",
    "UV_LICENSE_APACHE.txt",
    "FFMPEG_GPL-3.0.txt",
    "SOURCES.md",
    "MODEL_NOTICE.md",
    "MODEL_CARD.md",
    "MSST_NOTICE.md",
];
const WORKER_PACKAGE_FILES: &[&str] = &[
    "__init__.py",
    "__main__.py",
    "cli.py",
    "config.py",
    "errors.py",
    "inference.py",
    "model_config.py",
    "model_metadata.py",
    "protocol.py",
    "request.py",
    "resources/__init__.py",
    "resources/kimberley-melbandroformer.yaml",
    "vendor/__init__.py",
    "vendor_integrity.py",
];

#[derive(Debug)]
struct BootstrapEntry {
    archive_path: String,
    source_path: PathBuf,
    bytes: Vec<u8>,
}

fn main() {
    if let Err(error) = build_bootstrap() {
        panic!("deterministic bootstrap build failed: {error}");
    }

    tauri_build::build();
}

fn build_bootstrap() -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=SOUFMER_BOOTSTRAP_TEST_NONCE");

    let crate_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|error| error.to_string())?);
    let repository_root = crate_root
        .parent()
        .ok_or_else(|| "src-tauri must have a repository parent".to_owned())?
        .to_path_buf();
    let bootstrap_root = crate_root.join("bootstrap");
    let worker_root = repository_root.join("worker");

    for root in [
        bootstrap_root.join("bin"),
        bootstrap_root.join("licenses"),
        bootstrap_root.join("runtime-manifest.json"),
        worker_root.join("pyproject.toml"),
        worker_root.join("uv.lock"),
        worker_root.join(".python-version"),
        worker_root.join("src/accompaniment_worker"),
        worker_root.join("vendor"),
    ] {
        println!("cargo:rerun-if-changed={}", root.display());
    }

    validate_required_files(&bootstrap_root, &worker_root)?;
    let runtime_manifest = read_json(&bootstrap_root.join("runtime-manifest.json"))?;
    reject_placeholders(&runtime_manifest, "runtime-manifest.json")?;
    let source_manifest = read_json(&worker_root.join("vendor/source-manifest.json"))?;
    reject_placeholders(&source_manifest, "worker/vendor/source-manifest.json")?;
    let model_manifest = read_json(&worker_root.join("vendor/model-manifest.json"))?;
    reject_placeholders(&model_manifest, "worker/vendor/model-manifest.json")?;
    validate_lockfile(&worker_root.join("uv.lock"))?;
    validate_vendor_tree(&worker_root.join("vendor"), &source_manifest)?;
    validate_worker_source_tree(&worker_root.join("src/accompaniment_worker"))?;
    validate_manifest_cross_references(
        &runtime_manifest,
        &model_manifest,
        &bootstrap_root.join("bin/uv.exe"),
    )?;

    let mut entries = Vec::new();
    add_file(
        &mut entries,
        &bootstrap_root.join("bin/uv.exe"),
        "bin/uv.exe",
    )?;
    add_file(
        &mut entries,
        &bootstrap_root.join("runtime-manifest.json"),
        "runtime-manifest.json",
    )?;
    add_tree(
        &mut entries,
        &bootstrap_root.join("licenses"),
        "licenses",
        false,
    )?;
    for name in ["pyproject.toml", "uv.lock", ".python-version"] {
        add_file(
            &mut entries,
            &worker_root.join(name),
            &format!("worker/{name}"),
        )?;
    }
    add_tree(
        &mut entries,
        &worker_root.join("src/accompaniment_worker"),
        "worker/src/accompaniment_worker",
        true,
    )?;
    add_tree(
        &mut entries,
        &worker_root.join("vendor"),
        "worker/vendor",
        true,
    )?;

    entries.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));
    ensure_unique_paths(&entries)?;
    for entry in &entries {
        println!("cargo:rerun-if-changed={}", entry.source_path.display());
    }

    let entry_manifest = entry_manifest_bytes(&entries)?;
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|error| error.to_string())?);
    let archive_path = out_dir.join("soufmer-bootstrap.zip");
    write_archive(&archive_path, &entries, &entry_manifest)?;
    let archive = fs::read(&archive_path).map_err(io_error)?;
    let archive_hash = sha256_hex(&archive);
    let bootstrap_version = required_string(&runtime_manifest, &["bootstrapVersion"])?;
    let generated = format!(
        "pub const BOOTSTRAP_VERSION: &str = {bootstrap_version:?};\n\
         pub const BOOTSTRAP_ARCHIVE_SHA256: &str = {archive_hash:?};\n\
         pub const BOOTSTRAP_ARCHIVE_LENGTH: usize = {};\n\
         pub const BOOTSTRAP_ENTRY_MANIFEST_PATH: &str = {ENTRY_MANIFEST_PATH:?};\n\
         pub static BOOTSTRAP_ARCHIVE_BYTES: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/soufmer-bootstrap.zip\"));\n",
        archive.len()
    );
    fs::write(out_dir.join("embedded_bootstrap.rs"), generated).map_err(io_error)?;
    Ok(())
}

fn validate_required_files(bootstrap_root: &Path, worker_root: &Path) -> Result<(), String> {
    for path in [
        bootstrap_root.join("bin/uv.exe"),
        bootstrap_root.join("runtime-manifest.json"),
        worker_root.join("pyproject.toml"),
        worker_root.join("uv.lock"),
        worker_root.join(".python-version"),
        worker_root.join("vendor/MSST_LICENSE"),
    ] {
        require_regular_file(&path)?;
    }
    for name in REQUIRED_LICENSES {
        require_regular_file(&bootstrap_root.join("licenses").join(name))?;
    }
    Ok(())
}

fn validate_lockfile(lock_path: &Path) -> Result<(), String> {
    let lock = fs::read_to_string(lock_path).map_err(io_error)?;
    let lower = lock.to_ascii_lowercase();
    for forbidden in [
        "git+",
        "git =",
        "vcs =",
        "branch =",
        "source = { git",
        "source = { path",
        "file://",
        "msst",
        "music-source-separation-training",
    ] {
        if lower.contains(forbidden) {
            return Err(format!(
                "uv.lock contains forbidden mutable or local source: {forbidden}"
            ));
        }
    }
    if lower.contains(":\\") || lower.contains("/users/") || lower.contains("/home/") {
        return Err("uv.lock contains an absolute developer-machine path".to_owned());
    }
    Ok(())
}

fn validate_vendor_tree(
    vendor_root: &Path,
    source_manifest: &serde_json::Value,
) -> Result<(), String> {
    let mut allowed = BTreeSet::new();
    for path in [
        "MSST_LICENSE",
        "UPSTREAM.md",
        "source-manifest.json",
        "model-manifest.json",
    ] {
        allowed.insert(path.to_owned());
    }
    for record in required_array(source_manifest, &["files"])? {
        let path = required_string(record, &["path"])?;
        let expected = required_string(record, &["vendoredSha256"])?;
        let file = vendor_root.join(&path);
        require_regular_file(&file)?;
        verify_hash(&file, &expected)?;
        allowed.insert(path);
    }
    for record in required_array(source_manifest, &["patches"])? {
        let path = required_string(record, &["path"])?;
        let expected = required_string(record, &["sha256"])?;
        let file = vendor_root.join(&path);
        require_regular_file(&file)?;
        verify_hash(&file, &expected)?;
        allowed.insert(path);
    }

    let mut actual = Vec::new();
    collect_worker_files(vendor_root, vendor_root, &mut actual)?;
    for file in actual {
        let relative = archive_relative(vendor_root, &file)?;
        if !allowed.contains(&relative) {
            return Err(format!("unrecorded vendored MSST file: {relative}"));
        }
    }
    Ok(())
}

fn validate_worker_source_tree(source_root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect_worker_files(source_root, source_root, &mut files)?;
    for file in files {
        let relative = archive_relative(source_root, &file)?;
        validate_worker_source_relative(&relative)?;
    }
    Ok(())
}

fn validate_worker_source_relative(relative: &str) -> Result<(), String> {
    if WORKER_PACKAGE_FILES.contains(&relative) {
        Ok(())
    } else {
        Err(format!(
            "unallowlisted worker package file: {relative}; update the fixed bootstrap allowlist"
        ))
    }
}

fn validate_manifest_cross_references(
    runtime_manifest: &serde_json::Value,
    model_manifest: &serde_json::Value,
    uv_path: &Path,
) -> Result<(), String> {
    let actual_uv = fs::read(uv_path).map_err(io_error)?;
    if required_string(runtime_manifest, &["uv", "embeddedExecutablePath"])? != "bin/uv.exe" {
        return Err("runtime manifest has an unexpected embedded uv path".to_owned());
    }
    if required_string(runtime_manifest, &["uv", "embeddedExecutableSha256"])?
        != sha256_hex(&actual_uv)
    {
        return Err("embedded uv hash does not match runtime manifest".to_owned());
    }
    if required_u64(runtime_manifest, &["uv", "embeddedExecutableSizeBytes"])?
        != actual_uv.len() as u64
    {
        return Err("embedded uv size does not match runtime manifest".to_owned());
    }
    for key in [
        "repository",
        "revision",
        "fileName",
        "downloadUrl",
        "sha256",
    ] {
        if required_string(runtime_manifest, &["model", key])?
            != required_string(model_manifest, &["model", key])?
        {
            return Err(format!("runtime model metadata mismatch for {key}"));
        }
    }
    if required_u64(runtime_manifest, &["model", "sizeBytes"])?
        != required_u64(model_manifest, &["model", "sizeBytes"])?
    {
        return Err("runtime model size does not match worker model manifest".to_owned());
    }
    Ok(())
}

fn add_file(
    entries: &mut Vec<BootstrapEntry>,
    source_path: &Path,
    archive_path: &str,
) -> Result<(), String> {
    require_regular_file(source_path)?;
    validate_archive_path(archive_path)?;
    entries.push(BootstrapEntry {
        archive_path: archive_path.to_owned(),
        source_path: source_path.to_path_buf(),
        bytes: fs::read(source_path).map_err(io_error)?,
    });
    Ok(())
}

fn add_tree(
    entries: &mut Vec<BootstrapEntry>,
    root: &Path,
    archive_prefix: &str,
    skip_generated_worker_artifacts: bool,
) -> Result<(), String> {
    let mut files = Vec::new();
    if skip_generated_worker_artifacts {
        collect_worker_files(root, root, &mut files)?;
    } else {
        collect_regular_files(root, root, &mut files)?;
    }
    for file in files {
        let relative = archive_relative(root, &file)?;
        add_file(entries, &file, &format!("{archive_prefix}/{relative}"))?;
    }
    Ok(())
}

fn collect_regular_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    collect_files(root, current, files, false)
}

fn collect_worker_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    collect_files(root, current, files, true)
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
    skip_generated_worker_artifacts: bool,
) -> Result<(), String> {
    require_directory(root)?;
    let root_canonical = root.canonicalize().map_err(io_error)?;
    collect_regular_files_inner(
        &root_canonical,
        root,
        current,
        files,
        skip_generated_worker_artifacts,
    )
}

fn collect_regular_files_inner(
    root: &Path,
    source_root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
    skip_generated_worker_artifacts: bool,
) -> Result<(), String> {
    reject_link_or_reparse(current)?;
    let mut children = fs::read_dir(current)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        reject_link_or_reparse(&path)?;
        let relative = path
            .strip_prefix(source_root)
            .map_err(|error| error.to_string())?;
        if skip_generated_worker_artifacts && is_generated_worker_artifact(relative) {
            continue;
        }
        let canonical = path.canonicalize().map_err(io_error)?;
        if !canonical.starts_with(root) {
            return Err(format!(
                "bootstrap path escapes its root: {}",
                path.display()
            ));
        }
        let metadata = fs::metadata(&path).map_err(io_error)?;
        if metadata.is_dir() {
            collect_regular_files_inner(
                root,
                source_root,
                &path,
                files,
                skip_generated_worker_artifacts,
            )?;
        } else if metadata.is_file() {
            files.push(path);
        } else {
            return Err(format!(
                "bootstrap input is not a regular file: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn is_generated_worker_artifact(relative: &Path) -> bool {
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return false;
        };
        let name = name.to_string_lossy().to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "__pycache__" | ".pytest_cache" | ".venv" | "build" | "dist"
        ) {
            return true;
        }
        if components.peek().is_none() && name.ends_with(".pyc") {
            return true;
        }
    }
    false
}

fn write_archive(
    archive_path: &Path,
    entries: &[BootstrapEntry],
    entry_manifest: &[u8],
) -> Result<(), String> {
    let file = File::create(archive_path).map_err(io_error)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(DateTime::default())
        .unix_permissions(0o100644);
    let mut all = BTreeMap::new();
    all.insert(ENTRY_MANIFEST_PATH.to_owned(), entry_manifest);
    for entry in entries {
        all.insert(entry.archive_path.clone(), &entry.bytes);
    }
    for (path, bytes) in all {
        writer
            .start_file(path, options)
            .map_err(|error| error.to_string())?;
        writer.write_all(bytes).map_err(io_error)?;
    }
    writer.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn entry_manifest_bytes(entries: &[BootstrapEntry]) -> Result<Vec<u8>, String> {
    let records = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "path": entry.archive_path,
                "length": entry.bytes.len(),
                "sha256": sha256_hex(&entry.bytes),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&serde_json::json!({ "schemaVersion": 1, "entries": records }))
        .map_err(|error| error.to_string())
}

fn ensure_unique_paths(entries: &[BootstrapEntry]) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for entry in entries {
        validate_archive_path(&entry.archive_path)?;
        if !paths.insert(&entry.archive_path) {
            return Err(format!(
                "duplicate normalized archive path: {}",
                entry.archive_path
            ));
        }
    }
    Ok(())
}

fn validate_archive_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\\') || path.contains(':') || path.starts_with('/') {
        return Err(format!("invalid archive path: {path}"));
    }
    // `Path::components` intentionally normalizes some lexical aliases (notably repeated
    // separators and `.`). Archive paths are serialized identifiers, so accept only their
    // single canonical slash spelling rather than relying on the platform path parser.
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(format!("archive path is not normalized: {path}"));
    }
    Ok(())
}

fn archive_relative(root: &Path, file: &Path) -> Result<String, String> {
    let relative = file.strip_prefix(root).map_err(|error| error.to_string())?;
    let path = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_string_lossy().into_owned(),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("/");
    validate_archive_path(&path)?;
    Ok(path)
}

fn require_regular_file(path: &Path) -> Result<(), String> {
    reject_link_or_reparse(path)?;
    if !fs::metadata(path).map_err(io_error)?.is_file() {
        return Err(format!(
            "required regular file is missing: {}",
            path.display()
        ));
    }
    Ok(())
}

fn require_directory(path: &Path) -> Result<(), String> {
    reject_link_or_reparse(path)?;
    if !fs::metadata(path).map_err(io_error)?.is_dir() {
        return Err(format!("required directory is missing: {}", path.display()));
    }
    Ok(())
}

fn reject_link_or_reparse(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(format!(
            "symlink or reparse-point bootstrap input: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    require_regular_file(path)?;
    serde_json::from_slice(&fs::read(path).map_err(io_error)?).map_err(|error| error.to_string())
}

fn required_string(value: &serde_json::Value, path: &[&str]) -> Result<String, String> {
    value
        .pointer(&format!("/{}", path.join("/")))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing string manifest field: {}", path.join(".")))
}

fn required_u64(value: &serde_json::Value, path: &[&str]) -> Result<u64, String> {
    value
        .pointer(&format!("/{}", path.join("/")))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("missing integer manifest field: {}", path.join(".")))
}

fn required_array<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a Vec<serde_json::Value>, String> {
    value
        .pointer(&format!("/{}", path.join("/")))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("missing array manifest field: {}", path.join(".")))
}

fn reject_placeholders(value: &serde_json::Value, name: &str) -> Result<(), String> {
    let encoded = value.to_string().to_ascii_lowercase();
    for placeholder in [
        "placeholder",
        "replace-me",
        "todo",
        "example.invalid",
        "/main/",
    ] {
        if encoded.contains(placeholder) {
            return Err(format!("{name} contains placeholder value: {placeholder}"));
        }
    }
    Ok(())
}

fn verify_hash(path: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_hex(&fs::read(path).map_err(io_error)?);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!("hash mismatch for {}", path.display()));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_paths_reject_escapes_and_windows_forms() {
        assert!(validate_archive_path("worker/src/main.py").is_ok());
        for invalid in [
            "../worker/main.py",
            "C:/worker/main.py",
            "worker\\main.py",
            "worker//main.py",
            "worker/./main.py",
            "./worker/main.py",
            "worker/main.py/",
        ] {
            assert!(validate_archive_path(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn entry_manifest_is_stable_for_identical_entries() {
        let entries = vec![BootstrapEntry {
            archive_path: "worker/a.txt".to_owned(),
            source_path: PathBuf::from("a.txt"),
            bytes: b"stable".to_vec(),
        }];
        assert_eq!(
            entry_manifest_bytes(&entries).unwrap(),
            entry_manifest_bytes(&entries).unwrap()
        );
    }

    #[test]
    fn generated_worker_artifacts_are_skipped_but_unknown_files_are_rejected() {
        assert!(is_generated_worker_artifact(Path::new(
            "resources/__pycache__/config.cpython-311.pyc"
        )));
        assert!(is_generated_worker_artifact(Path::new("build/output.py")));
        assert!(is_generated_worker_artifact(Path::new("worker.pyc")));
        assert!(!is_generated_worker_artifact(Path::new(
            "resources/config.yaml"
        )));
        assert!(validate_worker_source_relative("unexpected.py").is_err());
        assert!(
            validate_worker_source_relative("resources/kimberley-melbandroformer.yaml").is_ok()
        );
    }
}
