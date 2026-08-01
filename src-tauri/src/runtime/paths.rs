use std::{
    fs,
    io::ErrorKind,
    os::windows::ffi::OsStringExt,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};

use windows_sys::Win32::{
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
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
    pub fn runtime_version(&self, runtime_id: &str) -> Result<PathBuf, AppError> {
        const MAX_RUNTIME_ID_LENGTH: usize = 128;
        const UUID_LENGTH: usize = 36;

        if runtime_id.len() > MAX_RUNTIME_ID_LENGTH {
            return Err(invalid_runtime_id());
        }
        let descriptor_end = runtime_id
            .len()
            .checked_sub(UUID_LENGTH + 1)
            .ok_or_else(invalid_runtime_id)?;
        if runtime_id.as_bytes().get(descriptor_end) != Some(&b'-') {
            return Err(invalid_runtime_id());
        }
        let descriptor = runtime_id[..descriptor_end]
            .strip_prefix("runtime-")
            .ok_or_else(invalid_runtime_id)?;
        let (version, digest) = descriptor
            .rsplit_once("-cuda-")
            .ok_or_else(invalid_runtime_id)?;
        if version.is_empty()
            || version.len() > 64
            || !version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
            || digest.len() != 12
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || digest.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(invalid_runtime_id());
        }
        let uuid_text = &runtime_id[descriptor_end + 1..];
        let uuid = uuid::Uuid::parse_str(uuid_text).map_err(|_| invalid_runtime_id())?;
        if uuid.get_version_num() != 4 || uuid.hyphenated().to_string() != uuid_text {
            return Err(invalid_runtime_id());
        }
        Ok(self.runtime_versions().join(runtime_id))
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
    pub fn webview_data(&self) -> PathBuf {
        self.root.join("webview").join("main")
    }

    pub fn ensure_webview_data(&self) -> Result<PathBuf, AppError> {
        let webview_root = self.root.join("webview");
        let data_directory = self.webview_data();
        for directory in [&self.root, &webview_root, &data_directory] {
            match fs::create_dir(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(AppError::new(
                        ErrorCode::LocalDataUnavailable,
                        format!(
                            "could not create private WebView data directory {}: {error}",
                            directory.display()
                        ),
                    ));
                }
            }
            let metadata = fs::symlink_metadata(directory).map_err(|error| {
                AppError::new(
                    ErrorCode::LocalDataUnavailable,
                    format!(
                        "could not inspect private WebView data directory {}: {error}",
                        directory.display()
                    ),
                )
            })?;
            if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(AppError::new(
                    ErrorCode::LocalDataUnavailable,
                    format!(
                        "private WebView data path is not a normal directory: {}",
                        directory.display()
                    ),
                ));
            }
        }
        Ok(data_directory)
    }
}

fn invalid_runtime_id() -> AppError {
    AppError::new(
        ErrorCode::ManifestInvalid,
        "active runtime metadata contains an invalid runtime ID",
    )
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
    use std::{fs, path::PathBuf};
    #[test]
    fn root_is_derived_only_from_local_app_data() {
        let root = AppPaths::from_local_app_data(PathBuf::from(r"D:\Users\Test\AppData\Local"));
        assert_eq!(root, PathBuf::from(r"D:\Users\Test\AppData\Local\soufmer"));
        assert!(!root.starts_with(std::env::current_dir().unwrap()));
        assert!(!root.starts_with(std::env::current_exe().unwrap().parent().unwrap()));
    }

    #[test]
    fn runtime_version_accepts_only_generated_bounded_ids() {
        let paths = AppPaths::from_test_root(PathBuf::from(r"D:\AppData\Local\soufmer"));
        let valid = "runtime-2026.08.01.1-cuda-012345abcdef-00000000-0000-4000-8000-000000000000";
        assert_eq!(
            paths.runtime_version(valid).unwrap(),
            paths.runtime_versions().join(valid)
        );

        for invalid in [
            "",
            "runtime-test",
            "runtime-2026.08.01.1-cuda-012345abcdef-not-a-uuid",
            "runtime-2026.08.01.1-cuda-012345abcdef-00000000-0000-0000-0000-000000000000",
            "runtime-2026.08.01.1-cuda-ABCDEF123456-00000000-0000-4000-8000-000000000000",
            r"C:\outside\runtime-2026.08.01.1-cuda-012345abcdef-00000000-0000-4000-8000-000000000000",
            r"..\runtime-2026.08.01.1-cuda-012345abcdef-00000000-0000-4000-8000-000000000000",
            "parent/runtime-2026.08.01.1-cuda-012345abcdef-00000000-0000-4000-8000-000000000000",
        ] {
            let error = paths.runtime_version(invalid).unwrap_err();
            assert_eq!(
                error.code,
                crate::domain::ErrorCode::ManifestInvalid,
                "{invalid}"
            );
        }
        let oversized = format!(
            "runtime-{}-cuda-012345abcdef-00000000-0000-4000-8000-000000000000",
            "v".repeat(129)
        );
        assert!(paths.runtime_version(&oversized).is_err());
    }

    #[test]
    fn webview_data_is_created_below_the_private_root() {
        let temp = std::env::temp_dir().join(format!("soufmer-paths-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&temp).unwrap();
        let paths = AppPaths::from_test_root(temp.join("soufmer"));

        let data_directory = paths.ensure_webview_data().unwrap();

        assert_eq!(data_directory, paths.root().join("webview").join("main"));
        assert!(data_directory.is_dir());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn webview_data_rejects_a_non_directory_root() {
        let temp = std::env::temp_dir().join(format!("soufmer-paths-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&temp).unwrap();
        let root = temp.join("soufmer");
        fs::write(&root, b"not a directory").unwrap();
        let paths = AppPaths::from_test_root(root);

        let error = paths.ensure_webview_data().unwrap_err();

        assert_eq!(error.code, crate::domain::ErrorCode::LocalDataUnavailable);
        fs::remove_dir_all(temp).unwrap();
    }
}
