pub mod embedded;
mod initializer;
mod manifest;
mod paths;
mod settings;

pub use initializer::{
    ActiveRuntime, InitEvent, InitEventSink, InitUpdate, environment_status, initialize,
    resolve_active_runtime,
};
pub use manifest::RuntimeManifest;
pub use paths::AppPaths;
pub use settings::{atomic_write_json, load_settings, save_settings};
