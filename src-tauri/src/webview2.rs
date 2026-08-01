use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegOpenKeyExW,
        RegQueryValueExW,
    },
    UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
};

const MACHINE_CLIENT_KEY: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const USER_CLIENT_KEY: &str =
    "Software\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const PRIVATE_DATA_ERROR_TITLE: &str = "Soufmer - 应用数据不可用 / Application data unavailable";
const PRIVATE_DATA_ERROR_MESSAGE: &str = "Soufmer 无法访问其应用数据文件夹。请确认当前 Windows 用户可以写入本地应用数据，然后重新打开 Soufmer。\n\nSoufmer cannot access its application data folder. Check that this Windows account can write to Local AppData, then reopen Soufmer.\n\nDiagnostic code: LOCAL_DATA_UNAVAILABLE";

pub fn ensure_runtime_or_show_recovery() -> bool {
    if has_runtime() {
        return true;
    }
    show_error_message(
        "Soufmer - WebView2 prerequisite",
        "未检测到 Microsoft Edge WebView2 Runtime。请安装或更新 Evergreen WebView2 Runtime 后重新打开 Soufmer。\n\nMicrosoft Edge WebView2 Runtime was not found. Install or update the Evergreen WebView2 Runtime, then reopen Soufmer.",
    );
    false
}

pub fn show_private_data_recovery() {
    show_error_message(PRIVATE_DATA_ERROR_TITLE, PRIVATE_DATA_ERROR_MESSAGE);
}

fn show_error_message(title: &str, message: &str) {
    let title = wide(title);
    let message = wide(message);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn has_runtime() -> bool {
    [
        (HKEY_LOCAL_MACHINE, MACHINE_CLIENT_KEY),
        (HKEY_CURRENT_USER, USER_CLIENT_KEY),
    ]
    .into_iter()
    .filter_map(|(root, path)| registry_pv(root, path))
    .any(|version| valid_version(&version))
}

fn registry_pv(root: HKEY, path: &str) -> Option<String> {
    let path = wide(path);
    let name = wide("pv");
    let mut key = std::ptr::null_mut();
    if unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ, &mut key) } != ERROR_SUCCESS {
        return None;
    }
    let mut kind = 0_u32;
    let mut size = 0_u32;
    let first = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if first != ERROR_SUCCESS || kind != REG_SZ || !(2..=1024).contains(&size) {
        unsafe {
            RegCloseKey(key);
        }
        return None;
    }
    let mut value = vec![0_u16; (size as usize).div_ceil(2)];
    let second = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            value.as_mut_ptr().cast(),
            &mut size,
        )
    };
    unsafe {
        RegCloseKey(key);
    }
    (second == ERROR_SUCCESS && kind == REG_SZ).then(|| {
        String::from_utf16_lossy(&value)
            .trim_end_matches('\0')
            .to_owned()
    })
}

fn valid_version(value: &str) -> bool {
    let mut fields = value.split('.');
    let parsed = (0..4)
        .map(|_| fields.next().and_then(|field| field.parse::<u32>().ok()))
        .collect::<Option<Vec<_>>>();
    matches!(parsed, Some(parts) if fields.next().is_none() && parts.iter().any(|part| *part != 0))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::valid_version;
    #[test]
    fn only_usable_four_field_evergreen_versions_are_accepted() {
        assert!(valid_version("122.0.2365.92"));
        assert!(!valid_version("0.0.0.0"));
        assert!(!valid_version("122.0"));
        assert!(!valid_version("122.0.abc.92"));
    }
}
