//! Tauri command handlers. Each is a thin async wrapper around the shared
//! [`co_worker_client::Client`].

use std::sync::Arc;

use co_worker_client::{
    ChatResponse, Client, DebugContext, HealthResponse, ListModelsResponse, Memory,
    MessageResponse, Session, SessionWithMessages,
};
use tauri::State;
use uuid::Uuid;

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
