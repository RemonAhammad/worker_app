//! Desktop app entrypoint.
//!
//! All chat-backend interaction is delegated to `tauri-plugin-co-worker`, so
//! this file is intentionally tiny — just the Tauri builder + plugin
//! registration. Add app-specific commands here if needed.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_co_worker::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
