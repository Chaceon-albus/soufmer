pub mod audio;
mod commands;
pub mod diagnostics;
pub mod domain;
pub mod download;
pub mod jobs;
pub mod process;
pub mod progress;
pub mod runtime;
mod task;
mod webview2;

use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use task::TaskManager;

pub fn run() {
    if !webview2::ensure_runtime_or_show_recovery() {
        return;
    }
    let webview_data =
        match runtime::AppPaths::discover().and_then(|paths| paths.ensure_webview_data()) {
            Ok(webview_data) => webview_data,
            Err(_) => {
                webview2::show_private_data_recovery();
                return;
            }
        };
    configure_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(TaskManager::default()))
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("Soufmer")
                .inner_size(820.0, 740.0)
                .min_inner_size(760.0, 720.0)
                .resizable(true)
                .data_directory(webview_data)
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_environment_status,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::initialize_environment,
            commands::start_batch,
            commands::cancel_active_task,
            commands::get_diagnostic_report,
            commands::get_license_notices,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Soufmer application");
}

#[derive(Clone)]
struct PrivateLogWriter(Arc<Mutex<std::fs::File>>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for PrivateLogWriter {
    type Writer = PrivateLogGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        PrivateLogGuard(self.0.lock().expect("private log lock poisoned"))
    }
}

struct PrivateLogGuard<'a>(std::sync::MutexGuard<'a, std::fs::File>);

impl Write for PrivateLogGuard<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

fn configure_logging() {
    let paths = match runtime::AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!(
                "Soufmer log directory unavailable: {}",
                error.technical_detail
            );
            configure_console_logging();
            return;
        }
    };
    if let Err(error) = std::fs::create_dir_all(paths.logs()) {
        eprintln!("Soufmer log directory unavailable: {error}");
        configure_console_logging();
        return;
    }
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.logs().join("soufmer.log"))
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!("Soufmer log file unavailable: {error}");
            configure_console_logging();
            return;
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_writer(PrivateLogWriter(Arc::new(Mutex::new(file))))
        .init();
}

fn configure_console_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();
}
