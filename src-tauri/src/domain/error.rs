use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ErrorCode {
    #[serde(rename = "ENV_NOT_INITIALIZED")]
    EnvironmentNotInitialized,
    #[serde(rename = "ENV_DOWNLOAD_FAILED")]
    EnvironmentDownloadFailed,
    #[serde(rename = "ENV_HASH_MISMATCH")]
    EnvironmentHashMismatch,
    #[serde(rename = "PYTHON_SYNC_FAILED")]
    PythonSyncFailed,
    #[serde(rename = "MODEL_DOWNLOAD_FAILED")]
    ModelDownloadFailed,
    #[serde(rename = "FFMPEG_NOT_AVAILABLE")]
    FfmpegNotAvailable,
    #[serde(rename = "INPUT_UNSUPPORTED")]
    InputUnsupported,
    #[serde(rename = "OUTPUT_NOT_WRITABLE")]
    OutputNotWritable,
    #[serde(rename = "CUDA_NOT_AVAILABLE")]
    CudaNotAvailable,
    #[serde(rename = "CUDA_OUT_OF_MEMORY")]
    CudaOutOfMemory,
    #[serde(rename = "INFERENCE_FAILED")]
    InferenceFailed,
    #[serde(rename = "POSTPROCESS_FAILED")]
    PostprocessFailed,
    #[serde(rename = "TASK_CANCELLED")]
    TaskCancelled,
    #[serde(rename = "TASK_ALREADY_ACTIVE")]
    TaskAlreadyActive,
    #[serde(rename = "INVALID_REQUEST")]
    InvalidRequest,
    #[serde(rename = "SETTINGS_INVALID")]
    SettingsInvalid,
    #[serde(rename = "MANIFEST_INVALID")]
    ManifestInvalid,
    #[serde(rename = "LOCAL_DATA_UNAVAILABLE")]
    LocalDataUnavailable,
}

impl ErrorCode {
    const fn message_key(self) -> &'static str {
        match self {
            Self::EnvironmentNotInitialized => "error.environmentNotInitialized",
            Self::EnvironmentDownloadFailed => "error.environmentDownloadFailed",
            Self::EnvironmentHashMismatch => "error.environmentHashMismatch",
            Self::PythonSyncFailed => "error.pythonSyncFailed",
            Self::ModelDownloadFailed => "error.modelDownloadFailed",
            Self::FfmpegNotAvailable => "error.ffmpegNotAvailable",
            Self::InputUnsupported => "error.inputUnsupported",
            Self::OutputNotWritable => "error.outputNotWritable",
            Self::CudaNotAvailable => "error.cudaNotAvailable",
            Self::CudaOutOfMemory => "error.cudaOutOfMemory",
            Self::InferenceFailed => "error.inferenceFailed",
            Self::PostprocessFailed => "error.postprocessFailed",
            Self::TaskCancelled => "error.taskCancelled",
            Self::TaskAlreadyActive => "error.taskAlreadyActive",
            Self::InvalidRequest => "error.invalidRequest",
            Self::SettingsInvalid => "error.settingsInvalid",
            Self::ManifestInvalid => "error.manifestInvalid",
            Self::LocalDataUnavailable => "error.localDataUnavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorStage {
    Validation,
    Runtime,
    Download,
    Process,
    Settings,
    Unknown,
}

#[derive(Clone, Debug, Error, Serialize)]
#[error("{code:?}: {message_key}")]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub stage: ErrorStage,
    pub message_key: String,
    pub recoverable: bool,
    pub diagnostic_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_path: Option<String>,
    #[serde(skip)]
    pub technical_detail: String,
}

impl AppError {
    pub fn new(code: ErrorCode, technical_detail: impl Into<String>) -> Self {
        let stage = match code {
            ErrorCode::EnvironmentDownloadFailed
            | ErrorCode::EnvironmentHashMismatch
            | ErrorCode::ModelDownloadFailed => ErrorStage::Download,
            ErrorCode::InferenceFailed
            | ErrorCode::PostprocessFailed
            | ErrorCode::TaskCancelled => ErrorStage::Process,
            ErrorCode::SettingsInvalid => ErrorStage::Settings,
            ErrorCode::InvalidRequest
            | ErrorCode::InputUnsupported
            | ErrorCode::OutputNotWritable => ErrorStage::Validation,
            _ => ErrorStage::Runtime,
        };
        Self {
            code,
            stage,
            message_key: code.message_key().into(),
            recoverable: !matches!(
                code,
                ErrorCode::InvalidRequest | ErrorCode::InputUnsupported
            ),
            diagnostic_id: uuid::Uuid::new_v4().to_string(),
            item_path: None,
            technical_detail: technical_detail.into(),
        }
    }
}
