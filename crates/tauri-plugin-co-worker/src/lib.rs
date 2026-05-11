//! Tauri plugin exposing the co_worker_lite chat backend.
//!
//! Install once in your `tauri::Builder`:
//!
//! ```ignore
//! tauri::Builder::default()
//!     .plugin(tauri_plugin_co_worker::init())
//!     .run(tauri::generate_context!())
//!     .expect("error while running tauri application");
//! ```
//!
//! By default the plugin targets `http://localhost:6969`; the frontend can
//! change that at runtime via the `setBaseUrl` command (or by setting the
//! `CO_WORKER_URL` env var before app launch).
//!
//! The plugin re-exports `co_worker_client` so consumers needing to talk to
//! the backend from native code (background tasks, custom commands) can do
//! so through the same client the commands use.

use std::sync::Arc;

use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

pub use co_worker_client;
pub use error::{Error, Result};

mod commands;
mod error;

/// Default backend URL used when no override is configured.
pub const DEFAULT_BASE_URL: &str = "http://localhost:6969";
const ENV_VAR: &str = "CO_WORKER_URL";

/// Shared state owned by the plugin. Wrapped in `Arc` so Tauri can clone
/// the managed handle without cloning the underlying `Mutex`.
pub struct PluginState {
    pub client: tokio::sync::Mutex<co_worker_client::Client>,
}

impl PluginState {
    fn new(base_url: &str) -> Result<Self> {
        let client = co_worker_client::Client::new(base_url)
            .map_err(|_| Error::InvalidUrl(base_url.to_string()))?;
        Ok(Self {
            client: tokio::sync::Mutex::new(client),
        })
    }
}

/// Build the plugin. Plug into `tauri::Builder` via `.plugin(init())`.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("co-worker")
        .invoke_handler(tauri::generate_handler![
            commands::set_base_url,
            commands::get_base_url,
            commands::health,
            commands::list_models,
            commands::model_catalog,
            commands::load_model,
            commands::list_sessions,
            commands::get_session,
            commands::create_session,
            commands::delete_session,
            commands::update_session,
            commands::debug_session,
            commands::send_message,
            commands::chat,
            commands::list_memories,
            commands::create_memory,
            commands::delete_memory,
        ])
        .setup(|app, _api| {
            let base_url = std::env::var(ENV_VAR).unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
            let state = PluginState::new(&base_url).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            app.manage(Arc::new(state));
            Ok(())
        })
        .build()
}
