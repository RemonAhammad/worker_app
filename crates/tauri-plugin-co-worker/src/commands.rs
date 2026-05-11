//! Tauri command handlers. Each is a thin async wrapper around the shared
//! [`co_worker_client::Client`] or a sandboxed filesystem tool.

use std::path::PathBuf;
use std::sync::Arc;

use co_worker_client::{
    AgentResponse, ChatResponse, Client, DebugContext, HealthResponse, ListModelsResponse, Memory,
    MessageResponse, ModelCatalog, ModelCatalogEntry, Session, SessionWithMessages, ToolResult,
};
use tauri::{AppHandle, Runtime, State};
use uuid::Uuid;

use crate::tools as fs_tools;
use crate::workspace;
use crate::{Error, PluginState, Result};

async fn current_client(state: &State<'_, Arc<PluginState>>) -> Client {
    state.client.lock().await.clone()
}

// ----- configuration -----

#[tauri::command]
pub async fn set_base_url(
    state: State<'_, Arc<PluginState>>,
    base_url: String,
) -> Result<String> {
    let client = Client::new(&base_url).map_err(|_| Error::InvalidUrl(base_url.clone()))?;
    let mut guard = state.client.lock().await;
    *guard = client;
    Ok(guard.base_url().to_string())
}

#[tauri::command]
pub async fn get_base_url(state: State<'_, Arc<PluginState>>) -> Result<String> {
    Ok(state.client.lock().await.base_url().to_string())
}

// ----- backend metadata -----

#[tauri::command]
pub async fn health(state: State<'_, Arc<PluginState>>) -> Result<HealthResponse> {
    Ok(current_client(&state).await.health().await?)
}

#[tauri::command]
pub async fn list_models(
    state: State<'_, Arc<PluginState>>,
) -> Result<ListModelsResponse> {
    Ok(current_client(&state).await.list_models().await?)
}

#[tauri::command]
pub async fn model_catalog(
    state: State<'_, Arc<PluginState>>,
) -> Result<ModelCatalog> {
    Ok(current_client(&state).await.model_catalog().await?)
}

#[tauri::command]
pub async fn load_model(
    state: State<'_, Arc<PluginState>>,
    name: String,
) -> Result<ModelCatalogEntry> {
    Ok(current_client(&state).await.load_model(&name).await?)
}

// ----- sessions -----

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, Arc<PluginState>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Session>> {
    Ok(current_client(&state)
        .await
        .list_sessions(limit.unwrap_or(50), offset.unwrap_or(0))
        .await?)
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, Arc<PluginState>>,
    id: Uuid,
) -> Result<SessionWithMessages> {
    Ok(current_client(&state).await.get_session(id).await?)
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, Arc<PluginState>>,
    title: String,
    system_prompt: Option<String>,
) -> Result<Session> {
    Ok(current_client(&state)
        .await
        .create_session(&title, system_prompt.as_deref())
        .await?)
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, Arc<PluginState>>,
    id: Uuid,
) -> Result<()> {
    Ok(current_client(&state).await.delete_session(id).await?)
}

#[tauri::command]
pub async fn update_session(
    state: State<'_, Arc<PluginState>>,
    id: Uuid,
    title: Option<String>,
    system_prompt: Option<String>,
) -> Result<Session> {
    Ok(current_client(&state)
        .await
        .update_session(id, title.as_deref(), system_prompt.as_deref())
        .await?)
}

#[tauri::command]
pub async fn debug_session(
    state: State<'_, Arc<PluginState>>,
    id: Uuid,
) -> Result<DebugContext> {
    Ok(current_client(&state).await.debug_session(id).await?)
}

// ----- messages -----

#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<PluginState>>,
    session_id: Uuid,
    content: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<MessageResponse> {
    Ok(current_client(&state)
        .await
        .send_message(
            session_id,
            &content,
            max_tokens.unwrap_or(1024),
            temperature.unwrap_or(0.7),
        )
        .await?)
}

#[tauri::command]
pub async fn chat(
    state: State<'_, Arc<PluginState>>,
    content: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    system_prompt: Option<String>,
) -> Result<ChatResponse> {
    Ok(current_client(&state)
        .await
        .chat(
            &content,
            max_tokens.unwrap_or(1024),
            temperature.unwrap_or(0.7),
            system_prompt.as_deref(),
        )
        .await?)
}

// ----- memories -----

#[tauri::command]
pub async fn list_memories(
    state: State<'_, Arc<PluginState>>,
) -> Result<Vec<Memory>> {
    Ok(current_client(&state).await.list_memories().await?)
}

#[tauri::command]
pub async fn create_memory(
    state: State<'_, Arc<PluginState>>,
    content: String,
) -> Result<Memory> {
    Ok(current_client(&state).await.create_memory(&content).await?)
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, Arc<PluginState>>,
    id: Uuid,
) -> Result<()> {
    Ok(current_client(&state).await.delete_memory(id).await?)
}

// ----- workspace -----

#[tauri::command]
pub async fn get_workspace(state: State<'_, Arc<PluginState>>) -> Result<Option<String>> {
    Ok(state
        .workspace
        .get()
        .await
        .map(|p| p.display().to_string()))
}

#[tauri::command]
pub async fn set_workspace<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<PluginState>>,
    path: Option<String>,
) -> Result<Option<String>> {
    let new_root: Option<PathBuf> = match path {
        Some(s) => {
            let p = PathBuf::from(&s);
            if !p.is_dir() {
                return Err(Error::ToolBadPath(format!(
                    "`{s}` is not a directory"
                )));
            }
            Some(p.canonicalize().map_err(Error::Io)?)
        }
        None => None,
    };
    state.workspace.set(new_root.clone()).await;
    if let Err(e) = workspace::save_persisted(&app, new_root.as_deref()).await {
        // No tracing dep here; the persisted file is best-effort and the
        // in-memory state is authoritative.
        eprintln!("co_worker plugin: could not persist workspace selection: {e}");
    }
    Ok(new_root.map(|p| p.display().to_string()))
}

// ----- filesystem tools -----

async fn workspace_root(state: &State<'_, Arc<PluginState>>) -> Result<PathBuf> {
    state.workspace.get().await.ok_or(Error::NoWorkspace)
}

#[tauri::command]
pub async fn tool_list_dir(
    state: State<'_, Arc<PluginState>>,
    path: String,
) -> Result<fs_tools::ListDirResult> {
    let root = workspace_root(&state).await?;
    fs_tools::list_dir(&root, &path).await
}

#[tauri::command]
pub async fn tool_read_file(
    state: State<'_, Arc<PluginState>>,
    path: String,
    max_bytes: Option<u64>,
) -> Result<fs_tools::ReadFileResult> {
    let root = workspace_root(&state).await?;
    fs_tools::read_file(&root, &path, max_bytes).await
}

#[tauri::command]
pub async fn tool_write_file(
    state: State<'_, Arc<PluginState>>,
    path: String,
    content: String,
) -> Result<fs_tools::WriteFileResult> {
    let root = workspace_root(&state).await?;
    fs_tools::write_file(&root, &path, &content).await
}

#[tauri::command]
pub async fn tool_append_file(
    state: State<'_, Arc<PluginState>>,
    path: String,
    content: String,
) -> Result<fs_tools::WriteFileResult> {
    let root = workspace_root(&state).await?;
    fs_tools::append_file(&root, &path, &content).await
}

#[tauri::command]
pub async fn tool_delete_path(
    state: State<'_, Arc<PluginState>>,
    path: String,
) -> Result<fs_tools::DeleteResult> {
    let root = workspace_root(&state).await?;
    fs_tools::delete_path(&root, &path).await
}

#[tauri::command]
pub async fn tool_move_path(
    state: State<'_, Arc<PluginState>>,
    from: String,
    to: String,
) -> Result<fs_tools::MoveResult> {
    let root = workspace_root(&state).await?;
    fs_tools::move_path(&root, &from, &to).await
}

#[tauri::command]
pub async fn tool_create_dir(
    state: State<'_, Arc<PluginState>>,
    path: String,
) -> Result<fs_tools::CreateDirResult> {
    let root = workspace_root(&state).await?;
    fs_tools::create_dir(&root, &path).await
}

// ----- agent loop -----

/// Return the workspace path as a string the model can use as a hint, or
/// `None` if none is set.
async fn workspace_hint(state: &State<'_, Arc<PluginState>>) -> Option<String> {
    state.workspace.get().await.map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn agent_send(
    state: State<'_, Arc<PluginState>>,
    session_id: Uuid,
    content: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<AgentResponse> {
    let hint = workspace_hint(&state).await;
    Ok(current_client(&state)
        .await
        .agent_send(
            session_id,
            &content,
            max_tokens.unwrap_or(1024),
            temperature.unwrap_or(0.7),
            hint.as_deref(),
        )
        .await?)
}

#[tauri::command]
pub async fn agent_continue(
    state: State<'_, Arc<PluginState>>,
    session_id: Uuid,
    assistant_id: Uuid,
    results: Vec<ToolResult>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<AgentResponse> {
    let hint = workspace_hint(&state).await;
    Ok(current_client(&state)
        .await
        .agent_continue(
            session_id,
            assistant_id,
            &results,
            max_tokens.unwrap_or(1024),
            temperature.unwrap_or(0.7),
            hint.as_deref(),
        )
        .await?)
}
