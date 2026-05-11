//! Tauri plugin build script.
//!
//! Generates per-command permission manifests so consumers can grant a
//! subset (or rely on the bundled `default` permission that allows all).

const COMMANDS: &[&str] = &[
    "set_base_url",
    "get_base_url",
    "health",
    "list_models",
    "model_catalog",
    "load_model",
    "list_sessions",
    "get_session",
    "create_session",
    "delete_session",
    "update_session",
    "debug_session",
    "send_message",
    "chat",
    "list_memories",
    "create_memory",
    "delete_memory",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
