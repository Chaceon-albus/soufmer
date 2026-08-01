use crate::{
    domain::{
        AppError, AppSettings, CancelTaskRequest, EnvironmentStatus, StartBatchRequest,
        TaskAcknowledgement,
    },
    runtime::{AppPaths, environment_status, load_settings, save_settings},
    task::{TaskManager, start_batch as start_batch_task, start_initialization},
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_environment_status() -> Result<EnvironmentStatus, AppError> {
    let paths = AppPaths::discover()?;
    environment_status(&paths)
}
#[tauri::command]
pub fn get_app_settings() -> Result<AppSettings, AppError> {
    let paths = AppPaths::discover()?;
    Ok(load_settings(&paths.settings_file()))
}
#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<AppSettings, AppError> {
    let paths = AppPaths::discover()?;
    save_settings(&paths.settings_file(), &settings)
}
#[tauri::command]
pub fn initialize_environment(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
) -> Result<TaskAcknowledgement, AppError> {
    start_initialization(app, Arc::clone(manager.inner()))
}
#[tauri::command]
pub fn start_batch(
    app: AppHandle,
    manager: State<'_, Arc<TaskManager>>,
    request: StartBatchRequest,
) -> Result<TaskAcknowledgement, AppError> {
    if request.input_path.trim().is_empty() || request.output_directory.trim().is_empty() {
        return Err(AppError::new(
            crate::domain::ErrorCode::InvalidRequest,
            "input and output paths are required",
        ));
    }
    let paths = AppPaths::discover()?;
    if !matches!(environment_status(&paths)?, EnvironmentStatus::Ready { .. }) {
        return Err(AppError::new(
            crate::domain::ErrorCode::EnvironmentNotInitialized,
            "batch processing requires a validated private runtime",
        ));
    }
    start_batch_task(app, Arc::clone(manager.inner()), request)
}
#[tauri::command]
pub fn cancel_active_task(
    manager: State<'_, Arc<TaskManager>>,
    request: CancelTaskRequest,
) -> Result<(), AppError> {
    manager.cancel(&request.task_id)
}

#[tauri::command]
pub fn get_diagnostic_report(diagnostic_id: String) -> Result<String, AppError> {
    let paths = AppPaths::discover()?;
    crate::diagnostics::read(&paths, &diagnostic_id)
}

#[tauri::command]
pub fn get_license_notices() -> Vec<crate::diagnostics::LicenseNotice> {
    crate::diagnostics::license_notices()
}
