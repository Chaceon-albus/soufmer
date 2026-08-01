use crate::domain::{APP_SETTINGS_SCHEMA_VERSION, AppError, AppSettings, ErrorCode};
use std::{fs, io, os::windows::ffi::OsStrExt, path::Path};
use uuid::Uuid;
use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

pub fn load_settings(path: &Path) -> AppSettings {
    let Ok(contents) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    match serde_json::from_str::<AppSettings>(&contents) {
        Ok(settings) if settings.schema_version == APP_SETTINGS_SCHEMA_VERSION => settings,
        _ => AppSettings::default(),
    }
}
pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<AppSettings, AppError> {
    if settings.schema_version != APP_SETTINGS_SCHEMA_VERSION {
        return Err(AppError::new(
            ErrorCode::SettingsInvalid,
            "unsupported settings schema",
        ));
    }
    atomic_write_json(path, settings).map_err(|_| {
        AppError::new(
            ErrorCode::LocalDataUnavailable,
            "could not save application settings",
        )
    })?;
    Ok(settings.clone())
}
pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "settings path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state"),
        Uuid::new_v4()
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(&temporary, bytes)?;
    if path.exists() {
        let destination = wide_path(path);
        let replacement = wide_path(&temporary);
        if unsafe {
            ReplaceFileW(
                destination.as_ptr(),
                replacement.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    } else {
        fs::rename(temporary, path)
    }
}
fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{load_settings, save_settings};
    use crate::domain::AppSettings;
    use std::fs;
    use uuid::Uuid;
    #[test]
    fn second_save_replaces_existing_settings_atomically() {
        let root = std::env::temp_dir().join(format!("soufmer-settings-test-{}", Uuid::new_v4()));
        let path = root.join("settings.json");
        let first = AppSettings {
            recursive: false,
            ..AppSettings::default()
        };
        save_settings(&path, &first).unwrap();
        let second = AppSettings {
            recursive: true,
            ..AppSettings::default()
        };
        save_settings(&path, &second).unwrap();
        assert!(load_settings(&path).recursive);
        fs::remove_dir_all(root).unwrap();
    }
}
