use std::{
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};

use windows_sys::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
};

use crate::domain::{AppError, ErrorCode};

#[derive(Clone, Debug)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    #[cfg(test)]
    pub(crate) fn from_test_root(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn discover() -> Result<Self, AppError> {
        let mut raw = std::ptr::null_mut();
        let result = unsafe {
            SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut raw)
        };
        if result < 0 || raw.is_null() {
            return Err(AppError::new(
                ErrorCode::LocalDataUnavailable,
                "could not resolve FOLDERID_LocalAppData",
            ));
        }
        let base = unsafe {
            std::ffi::OsString::from_wide(std::slice::from_raw_parts(raw, wide_len(raw)))
        };
        unsafe {
            CoTaskMemFree(raw.cast());
        }
        Ok(Self {
            root: Self::from_local_app_data(PathBuf::from(base)),
        })
    }

    pub fn from_local_app_data(local_app_data: PathBuf) -> PathBuf {
        local_app_data.join("soufmer")
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("state").join("settings.json")
    }
    pub fn runtime_manifest_file(&self) -> PathBuf {
        self.root.join("state").join("runtime-manifest.json")
    }
    pub fn current_bootstrap_file(&self) -> PathBuf {
        self.root.join("state").join("current-bootstrap.json")
    }
    pub fn current_runtime_file(&self) -> PathBuf {
        self.root.join("state").join("current-runtime.json")
    }
    pub fn bootstrap_versions(&self) -> PathBuf {
        self.root.join("bootstrap").join("versions")
    }
    pub fn runtime_versions(&self) -> PathBuf {
        self.root.join("runtime").join("versions")
    }
    pub fn tools(&self) -> PathBuf {
        self.root.join("tools")
    }
    pub fn models(&self) -> PathBuf {
        self.root.join("models")
    }
    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    pub fn uv_cache(&self) -> PathBuf {
        self.root.join("cache").join("uv")
    }
    pub fn jobs(&self) -> PathBuf {
        self.root.join("jobs")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn diagnostics(&self) -> PathBuf {
        self.root.join("diagnostics")
    }
}

fn wide_len(value: *const u16) -> usize {
    let mut length = 0;
    unsafe {
        while *value.add(length) != 0 {
            length += 1;
        }
    }
    length
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use std::path::PathBuf;
    #[test]
    fn root_is_derived_only_from_local_app_data() {
        let root = AppPaths::from_local_app_data(PathBuf::from(r"D:\Users\Test\AppData\Local"));
        assert_eq!(root, PathBuf::from(r"D:\Users\Test\AppData\Local\soufmer"));
        assert!(!root.starts_with(std::env::current_dir().unwrap()));
        assert!(!root.starts_with(std::env::current_exe().unwrap().parent().unwrap()));
    }
}
