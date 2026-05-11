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
    "get_workspace",
    "set_workspace",
    "tool_list_dir",
    "tool_read_file",
    "tool_write_file",
    "tool_append_file",
    "tool_delete_path",
    "tool_move_path",
    "tool_create_dir",
    "tool_search",
    "tool_preview_write",
    "get_auto_allow",
    "set_auto_allow",
    "agent_send",
    "agent_continue",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
