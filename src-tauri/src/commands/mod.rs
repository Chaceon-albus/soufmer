use crate::{
    domain::{
        AppError, AppSettings, CancelTaskRequest, EnvironmentStatus, PathKindInfo,
        StartBatchRequest, TaskAcknowledgement,
    },
    runtime::{AppPaths, environment_status, load_settings, save_settings},
    task::{TaskManager, start_batch as start_batch_task, start_initialization},
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn inspect_path(path: String) -> Result<PathKindInfo, AppError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(PathKindInfo {
            exists: false,
            is_dir: false,
            is_file: false,
            parent_dir: None,
        });
    }
    let p = std::path::Path::new(trimmed);
    if !p.exists() {
        return Ok(PathKindInfo {
            exists: false,
            is_dir: false,
            is_file: false,
            parent_dir: None,
        });
    }
    let metadata = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) => {
            return Err(AppError::new(
                crate::domain::ErrorCode::InputUnsupported,
                e.to_string(),
            ));
        }
    };
    let parent_dir = if metadata.is_file() {
        p.parent().map(|parent| parent.to_string_lossy().to_string())
    } else {
        Some(trimmed.to_string())
    };
    Ok(PathKindInfo {
        exists: true,
        is_dir: metadata.is_dir(),
        is_file: metadata.is_file(),
        parent_dir,
    })
}

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

#[tauri::command]
pub fn set_window_content_height(
    window: tauri::WebviewWindow,
    height: f64,
) -> Result<(), AppError> {
    let min_logical_height = 300.0;
    let max_logical_height = 900.0;

    let clamped_height = height.clamp(min_logical_height, max_logical_height);

    let current_inner = match window.inner_size() {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let target_inner_physical_height = (clamped_height * scale_factor).round() as u32;

    if (current_inner.height as i32 - target_inner_physical_height as i32).abs() > 2 {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 760.0,
            height: clamped_height,
        }));
    }

    let _ = window.show();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_path_reports_correct_metadata() {
        let temp_dir = std::env::temp_dir();
        let info = inspect_path(temp_dir.to_string_lossy().to_string()).unwrap();
        assert!(info.exists);
        assert!(info.is_dir);
        assert!(!info.is_file);

        let non_existent = inspect_path("C:\\non_existent_folder_xyz_123".into()).unwrap();
        assert!(!non_existent.exists);
    }
}

