use serde::{Deserialize, Serialize};

pub const APP_SETTINGS_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u8,
    pub locale: Locale,
    pub last_input_mode: InputMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_output_directory: Option<String>,
    pub processing_mode: ProcessingMode,
    pub recursive: bool,
    pub preserve_directory_structure: bool,
    pub conflict_policy: ConflictPolicy,
    pub output_format: OutputFormat,
    pub generate_both_modes: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: APP_SETTINGS_SCHEMA_VERSION,
            locale: Locale::ZhCn,
            last_input_mode: InputMode::File,
            last_output_directory: None,
            processing_mode: ProcessingMode::Compatibility44100,
            recursive: true,
            preserve_directory_structure: true,
            conflict_policy: ConflictPolicy::Skip,
            output_format: OutputFormat::Flac,
            generate_both_modes: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InputMode {
    File,
    Folder,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ProcessingMode {
    #[serde(rename = "compatibility44100")]
    Compatibility44100,
    #[serde(rename = "sourceSampleRate")]
    SourceSampleRate,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    Skip,
    Overwrite,
    AutoNumber,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum OutputFormat {
    #[serde(rename = "flac")]
    Flac,
    #[serde(rename = "wavFloat32")]
    WavFloat32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartBatchRequest {
    pub input_mode: InputMode,
    pub input_path: String,
    pub output_directory: String,
    pub processing_mode: ProcessingMode,
    pub recursive: bool,
    pub preserve_directory_structure: bool,
    pub conflict_policy: ConflictPolicy,
    pub output_format: OutputFormat,
    pub generate_both_modes: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    pub task_id: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAcknowledgement {
    pub task_id: String,
    pub accepted_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EnvironmentStatus {
    NotInstalled {
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_download_bytes: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_disk_bytes: Option<u64>,
    },
    Installing {
        runtime_version: String,
    },
    Ready {
        runtime_version: String,
        model_version: String,
        ffmpeg_version: String,
    },
    RepairRequired {
        reason_code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_download_bytes: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_disk_bytes: Option<u64>,
    },
    Unsupported {
        reason_code: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgressValue {
    Determinate { fraction: f64 },
    Indeterminate,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InitializationStep {
    CheckingSystem,
    PreparingTools,
    InstallingPython,
    SyncingEnvironment,
    DownloadingModel,
    SelfTesting,
    Activating,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationProgress {
    pub runtime_version: String,
    pub step_index: u8,
    pub step_count: u8,
    pub step_id: InitializationStep,
    pub overall: ProgressValue,
    pub current: ProgressValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_completed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_per_second: Option<u64>,
    pub detail: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchStage {
    Probing,
    PreparingInput,
    Separating,
    BuildingCompatibilityOutput,
    BuildingSourceRateOutput,
    ValidatingOutput,
    CleaningUp,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub item_index: u32,
    pub item_count: u32,
    pub current_input_path: String,
    pub current_display_name: String,
    pub stage: BatchStage,
    pub overall: ProgressValue,
    pub current: ProgressValue,
    pub completed_duration_seconds: f64,
    pub total_duration_seconds: f64,
    pub elapsed_seconds: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemResult {
    pub item_index: u32,
    pub input_path: String,
    pub outputs: Vec<String>,
    pub duration_seconds: f64,
    pub warnings: Vec<String>,
    pub error_code: Option<super::ErrorCode>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub task_id: String,
    pub output_directory: String,
    pub succeeded: u32,
    pub failed: u32,
    pub skipped: u32,
    pub cancelled: bool,
    pub items: Vec<BatchItemResult>,
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, EnvironmentStatus, ProgressValue};
    use crate::domain::{AppError, ErrorCode};
    #[test]
    fn frontend_protocol_values_have_the_documented_shape() {
        assert_eq!(
            serde_json::to_value(ProgressValue::Determinate { fraction: 0.5 }).unwrap(),
            serde_json::json!({"kind":"determinate","fraction":0.5})
        );
        assert_eq!(
            serde_json::to_value(EnvironmentStatus::NotInstalled {
                estimated_download_bytes: None,
                estimated_disk_bytes: None
            })
            .unwrap(),
            serde_json::json!({"type":"notInstalled"})
        );
        assert_eq!(
            serde_json::to_value(EnvironmentStatus::RepairRequired {
                reason_code: "RUNTIME_VALIDATION_FAILED".into(),
                estimated_download_bytes: Some(3_500_000_000),
                estimated_disk_bytes: Some(7_000_000_000),
            })
            .unwrap(),
            serde_json::json!({
                "type":"repairRequired",
                "reasonCode":"RUNTIME_VALIDATION_FAILED",
                "estimatedDownloadBytes":3_500_000_000_u64,
                "estimatedDiskBytes":7_000_000_000_u64
            })
        );
        let error = AppError::new(
            ErrorCode::EnvironmentNotInitialized,
            "technical details stay in logs",
        );
        let serialized = serde_json::to_value(error).unwrap();
        assert_eq!(serialized["code"], "ENV_NOT_INITIALIZED");
        assert_eq!(serialized["messageKey"], "error.environmentNotInitialized");
        assert!(serialized.get("message").is_none());
        assert!(serialized.get("itemPath").is_none());
        assert!(
            serde_json::to_value(AppSettings::default())
                .unwrap()
                .get("lastOutputDirectory")
                .is_none()
        );
    }
}
