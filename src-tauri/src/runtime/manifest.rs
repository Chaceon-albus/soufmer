use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::domain::{AppError, ErrorCode};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u8,
    pub bootstrap_version: String,
    pub compatible_app_versions: String,
    pub platform: Platform,
    pub python: Python,
    pub uv: Uv,
    pub worker: Worker,
    pub ffmpeg: Ffmpeg,
    pub model: Model,
    pub estimates: Estimates,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Python {
    pub minor_version: String,
    pub managed_only: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Uv {
    pub version: String,
    pub target: String,
    pub archive_url: String,
    pub checksum_url: String,
    pub archive_sha256: String,
    pub archive_size_bytes: u64,
    pub embedded_executable_path: String,
    pub embedded_executable_sha256: String,
    pub embedded_executable_size_bytes: u64,
    pub licenses: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Worker {
    pub project_path: String,
    pub lock_file: String,
    pub python_minor_version: String,
    pub production_sync_arguments: Vec<String>,
    pub cuda_profile: CudaProfile,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CudaProfile {
    pub index_url: String,
    pub torch_version: String,
    pub torchaudio_version: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ffmpeg {
    pub version: String,
    pub provider: String,
    pub provider_page_url: String,
    pub archive_url: String,
    pub checksum_url: String,
    pub archive_sha256: String,
    pub archive_size_bytes: u64,
    pub archive_root: String,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub license_path: String,
    pub license_classification: String,
    pub embedded_license_path: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    pub repository: String,
    pub revision: String,
    pub file_name: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub license_classification: String,
    pub notice_path: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Estimates {
    pub download_bytes: u64,
    pub installed_bytes: u64,
    pub minimum_free_bytes: u64,
}

impl RuntimeManifest {
    pub fn parse(json: &str) -> Result<Self, AppError> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|_| invalid("runtime manifest is not valid JSON"))?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<(), AppError> {
        if self.schema_version != 1
            || self.platform.os != "windows"
            || self.platform.architecture != "x86_64"
        {
            return Err(invalid("unsupported manifest platform or schema"));
        }
        if self.bootstrap_version.is_empty()
            || self.python.minor_version != "3.11"
            || self.worker.python_minor_version != self.python.minor_version
            || !self.python.managed_only
        {
            return Err(invalid("invalid managed Python configuration"));
        }
        for value in [
            &self.uv.archive_url,
            &self.uv.checksum_url,
            &self.ffmpeg.archive_url,
            &self.model.download_url,
            &self.worker.cuda_profile.index_url,
        ] {
            validate_https(value)?;
        }
        for value in [
            &self.uv.archive_sha256,
            &self.uv.embedded_executable_sha256,
            &self.ffmpeg.archive_sha256,
            &self.model.sha256,
        ] {
            validate_sha256(value)?;
        }
        if self.uv.archive_size_bytes == 0
            || self.uv.embedded_executable_size_bytes == 0
            || self.ffmpeg.archive_size_bytes == 0
            || self.model.size_bytes == 0
            || self.estimates.download_bytes == 0
            || self.estimates.installed_bytes == 0
            || self.estimates.minimum_free_bytes == 0
        {
            return Err(invalid("manifest size estimates must be non-zero"));
        }
        for path in [
            &self.uv.embedded_executable_path,
            &self.worker.project_path,
            &self.worker.lock_file,
            &self.ffmpeg.archive_root,
            &self.ffmpeg.ffmpeg_path,
            &self.ffmpeg.ffprobe_path,
            &self.ffmpeg.license_path,
            &self.ffmpeg.embedded_license_path,
            &self.model.file_name,
            &self.model.notice_path,
        ] {
            validate_relative_path(path)?;
        }
        for license in &self.uv.licenses {
            validate_relative_path(license)?;
        }
        if self.worker.project_path != "worker"
            || self.worker.lock_file != "worker/uv.lock"
            || self.uv.embedded_executable_path != "bin/uv.exe"
        {
            return Err(invalid("unexpected trusted bootstrap paths"));
        }
        let required = [
            "sync",
            "--locked",
            "--no-dev",
            "--extra",
            "cuda",
            "--no-editable",
            "--managed-python",
            "--no-python-downloads",
        ];
        if self
            .worker
            .production_sync_arguments
            .iter()
            .map(String::as_str)
            .ne(required)
        {
            return Err(invalid("production sync arguments are not exact"));
        }
        Ok(())
    }
    pub fn digest(&self, source: &[u8]) -> String {
        format!("{:x}", Sha256::digest(source))
    }
}
fn invalid(detail: &str) -> AppError {
    AppError::new(ErrorCode::ManifestInvalid, detail)
}
pub fn validate_sha256(value: &str) -> Result<(), AppError> {
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(invalid("SHA-256 must be 64 hexadecimal characters"))
    }
}
fn validate_https(value: &str) -> Result<(), AppError> {
    if value.starts_with("https://") && !value.contains('?') && !value.contains('#') {
        Ok(())
    } else {
        Err(invalid(
            "runtime artifact URLs must be immutable HTTPS URLs",
        ))
    }
}
fn validate_relative_path(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.contains(':')
        || value.starts_with('/')
        || value.starts_with('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        Err(invalid("manifest contains an unsafe relative path"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_hash_format() {
        assert!(validate_sha256("abcdef").is_err());
    }
    #[test]
    fn parses_real_embedded_manifest() {
        RuntimeManifest::parse(include_str!("../../bootstrap/runtime-manifest.json")).unwrap();
    }
    #[test]
    fn rejects_non_https() {
        assert!(
            RuntimeManifest::parse(
                &include_str!("../../bootstrap/runtime-manifest.json")
                    .replace("https://", "http://")
            )
            .is_err()
        );
    }
}
