//! Wire types for talking to the co_worker_lite backend.
//!
//! These mirror the JSON shapes produced by `co_worker_lite::types`. Kept
//! standalone here rather than importing the backend crate so the CLI does
//! not depend on llama.cpp / sqlx / a 4 GB model just to be built.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub title: String,
    pub model_name: String,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: Role,
    pub content: String,
    pub token_count: i64,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionWithMessages {
    #[serde(flatten)]
    pub session: Session,
    pub messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionRequest<'a> {
    pub title: &'a str,
    pub system_prompt: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct CreateMessageRequest<'a> {
    pub content: &'a str,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub message: Message,
    pub usage: Usage,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub model: String,
    pub loaded: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size_bytes: u64,
    pub loaded: bool,
}

/// Shape the backend uses for error bodies: `{"error":{"code":..,"message":..}}`
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}
