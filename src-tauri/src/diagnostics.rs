use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    domain::{AppError, ErrorCode},
    runtime::{AppPaths, RuntimeManifest, atomic_write_json},
};

const MAX_REPORT_BYTES: u64 = 512 * 1024;
const MAX_OPTIONAL_STATE_BYTES: u64 = 64 * 1024;

const UV_LICENSE_MIT: &str = include_str!("../bootstrap/licenses/UV_LICENSE_MIT.txt");
const UV_LICENSE_APACHE: &str = include_str!("../bootstrap/licenses/UV_LICENSE_APACHE.txt");
const MSST_NOTICE: &str = include_str!("../bootstrap/licenses/MSST_NOTICE.md");
const MSST_LICENSE: &str = include_str!("../../worker/vendor/MSST_LICENSE");
const MODEL_NOTICE: &str = include_str!("../bootstrap/licenses/MODEL_NOTICE.md");
const MODEL_CARD: &str = include_str!("../bootstrap/licenses/MODEL_CARD.md");
const FFMPEG_GPL: &str = include_str!("../bootstrap/licenses/FFMPEG_GPL-3.0.txt");
const SOURCES: &str = include_str!("../bootstrap/licenses/SOURCES.md");
const RUNTIME_MANIFEST: &str = include_str!("../bootstrap/runtime-manifest.json");

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseNotice {
    pub id: &'static str,
    pub title: &'static str,
    pub text: &'static str,
}

pub fn license_notices() -> Vec<LicenseNotice> {
    vec![
        LicenseNotice {
            id: "uv-mit",
            title: "uv — MIT License",
            text: UV_LICENSE_MIT,
        },
        LicenseNotice {
            id: "uv-apache-2.0",
            title: "uv — Apache License 2.0",
            text: UV_LICENSE_APACHE,
        },
        LicenseNotice {
            id: "msst-notice",
            title: "Music-Source-Separation-Training notice",
            text: MSST_NOTICE,
        },
        LicenseNotice {
            id: "msst-mit",
            title: "Music-Source-Separation-Training — MIT License",
            text: MSST_LICENSE,
        },
        LicenseNotice {
            id: "model-notice",
            title: "KimberleyJSN MelBandRoformer notice",
            text: MODEL_NOTICE,
        },
        LicenseNotice {
            id: "model-card",
            title: "KimberleyJSN MelBandRoformer model card",
            text: MODEL_CARD,
        },
        LicenseNotice {
            id: "ffmpeg-gpl-3.0",
            title: "FFmpeg build — GNU GPL 3.0",
            text: FFMPEG_GPL,
        },
        LicenseNotice {
            id: "bootstrap-sources",
            title: "Bootstrap artifact sources",
            text: SOURCES,
        },
    ]
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticReport<'a> {
    schema_version: u8,
    created_at: String,
    code: ErrorCode,
    stage: crate::domain::ErrorStage,
    diagnostic_id: &'a str,
    item_path: &'a Option<String>,
    technical_detail: &'a str,
    context: DiagnosticContext,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticContext {
    app_version: &'static str,
    platform: PlatformIdentity,
    app_root: String,
    log_path: String,
    configured_runtime: Option<ConfiguredRuntimeIdentity>,
    active_runtime: Option<ActiveRuntimeIdentity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformIdentity {
    operating_system: &'static str,
    architecture: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredRuntimeIdentity {
    bootstrap_version: String,
    compatible_app_versions: String,
    python_minor_version: String,
    uv_version: String,
    ffmpeg_version: String,
    ffmpeg_license: String,
    model_revision: String,
    model_file_name: String,
    msst_commit: Option<String>,
    cuda_index_url: String,
    torch_version: String,
    torchaudio_version: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentRuntimeRecord {
    id: String,
    manifest_digest: String,
    activated_at: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelfTestRecord {
    manifest_digest: String,
    profile: String,
    worker: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveRuntimeIdentity {
    id: String,
    manifest_digest: String,
    activated_at: String,
    self_test: Option<SelfTestIdentity>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelfTestIdentity {
    manifest_digest: String,
    profile: String,
    worker: serde_json::Value,
}

pub fn persist(paths: &AppPaths, error: &AppError) -> Result<(), AppError> {
    validate_id(&error.diagnostic_id)?;
    fs::create_dir_all(paths.diagnostics()).map_err(storage_error)?;
    let report = DiagnosticReport {
        schema_version: 2,
        created_at: timestamp(),
        code: error.code,
        stage: error.stage,
        diagnostic_id: &error.diagnostic_id,
        item_path: &error.item_path,
        technical_detail: &error.technical_detail,
        context: diagnostic_context(paths),
    };
    atomic_write_json(&report_path(paths, &error.diagnostic_id), &report).map_err(storage_error)
}

pub fn read(paths: &AppPaths, diagnostic_id: &str) -> Result<String, AppError> {
    validate_id(diagnostic_id)?;
    let path = report_path(paths, diagnostic_id);
    let metadata = fs::metadata(&path).map_err(storage_error)?;
    if metadata.len() > MAX_REPORT_BYTES {
        return Err(AppError::new(
            ErrorCode::LocalDataUnavailable,
            "diagnostic report exceeded the retrieval limit",
        ));
    }
    fs::read_to_string(path).map_err(storage_error)
}

fn diagnostic_context(paths: &AppPaths) -> DiagnosticContext {
    DiagnosticContext {
        app_version: env!("CARGO_PKG_VERSION"),
        platform: PlatformIdentity {
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        app_root: paths.root().display().to_string(),
        log_path: paths.logs().join("soufmer.log").display().to_string(),
        configured_runtime: configured_runtime_identity(),
        active_runtime: active_runtime_identity(paths),
    }
}

fn configured_runtime_identity() -> Option<ConfiguredRuntimeIdentity> {
    let manifest = RuntimeManifest::parse(RUNTIME_MANIFEST).ok()?;
    let msst_commit = MSST_NOTICE
        .lines()
        .find_map(|line| line.trim().strip_prefix('`'))
        .and_then(|line| line.strip_suffix("`."))
        .map(str::to_owned);
    Some(ConfiguredRuntimeIdentity {
        bootstrap_version: manifest.bootstrap_version,
        compatible_app_versions: manifest.compatible_app_versions,
        python_minor_version: manifest.python.minor_version,
        uv_version: manifest.uv.version,
        ffmpeg_version: manifest.ffmpeg.version,
        ffmpeg_license: manifest.ffmpeg.license_classification,
        model_revision: manifest.model.revision,
        model_file_name: manifest.model.file_name,
        msst_commit,
        cuda_index_url: manifest.worker.cuda_profile.index_url,
        torch_version: manifest.worker.cuda_profile.torch_version,
        torchaudio_version: manifest.worker.cuda_profile.torchaudio_version,
    })
}

fn active_runtime_identity(paths: &AppPaths) -> Option<ActiveRuntimeIdentity> {
    let current: CurrentRuntimeRecord = read_optional_json(&paths.current_runtime_file())?;
    let self_test_path = paths
        .runtime_versions()
        .join(&current.id)
        .join("self-test.json");
    let self_test =
        read_optional_json::<SelfTestRecord>(&self_test_path).map(|record| SelfTestIdentity {
            manifest_digest: record.manifest_digest,
            profile: record.profile,
            worker: record.worker,
        });
    Some(ActiveRuntimeIdentity {
        id: current.id,
        manifest_digest: current.manifest_digest,
        activated_at: current.activated_at,
        self_test,
    })
}

fn read_optional_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_OPTIONAL_STATE_BYTES {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn report_path(paths: &AppPaths, diagnostic_id: &str) -> std::path::PathBuf {
    paths.diagnostics().join(format!("{diagnostic_id}.json"))
}

fn validate_id(id: &str) -> Result<(), AppError> {
    Uuid::parse_str(id).map(|_| ()).map_err(|_| {
        AppError::new(
            ErrorCode::InvalidRequest,
            "diagnostic ID must be a backend UUID",
        )
    })
}

fn storage_error(_: impl std::fmt::Display) -> AppError {
    AppError::new(
        ErrorCode::LocalDataUnavailable,
        "could not persist or read diagnostic report",
    )
}

fn timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
mod tests {
    use super::{MAX_REPORT_BYTES, license_notices, persist, read};
    use crate::{
        domain::{AppError, ErrorCode},
        runtime::AppPaths,
    };
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn license_notices_embed_required_exact_texts() {
        let notices = license_notices();
        assert_eq!(notices.len(), 8);
        for id in [
            "uv-mit",
            "uv-apache-2.0",
            "msst-notice",
            "msst-mit",
            "model-notice",
            "model-card",
            "ffmpeg-gpl-3.0",
            "bootstrap-sources",
        ] {
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.id == id && !notice.text.is_empty())
            );
        }
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("MIT License"))
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("Apache License"))
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("Music-Source-Separation-Training"))
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("KimberleyJSN"))
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("GNU GENERAL PUBLIC LICENSE"))
        );
    }

    #[test]
    fn reports_include_runtime_context_and_keep_uuid_bounds() {
        let root = std::env::temp_dir().join(format!("soufmer-diagnostic-{}", Uuid::new_v4()));
        let paths = AppPaths::from_test_root(root.clone());
        fs::create_dir_all(root.join("state")).unwrap();
        fs::create_dir_all(root.join("runtime/versions/runtime-test")).unwrap();
        fs::write(
            paths.current_runtime_file(),
            r#"{"id":"runtime-test","manifestDigest":"digest","activatedAt":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        fs::write(
            root.join("runtime/versions/runtime-test/self-test.json"),
            r#"{"manifestDigest":"digest","profile":"cuda","worker":{"device":"cuda:0"}}"#,
        )
        .unwrap();
        let error = AppError::new(ErrorCode::InferenceFailed, "controlled technical detail");
        persist(&paths, &error).unwrap();
        let report = read(&paths, &error.diagnostic_id).unwrap();
        assert!(report.contains("controlled technical detail"));
        assert!(report.contains("appVersion"));
        assert!(report.contains("ffmpegVersion"));
        assert!(report.contains("cudaProfile") || report.contains("cudaIndexUrl"));
        assert!(report.contains("runtime-test"));
        assert!(read(&paths, "../../not-a-uuid").is_err());
        let oversized_id = Uuid::new_v4().to_string();
        fs::write(
            paths.diagnostics().join(format!("{oversized_id}.json")),
            vec![b'x'; (MAX_REPORT_BYTES + 1) as usize],
        )
        .unwrap();
        assert!(read(&paths, &oversized_id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
