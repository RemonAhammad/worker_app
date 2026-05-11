//! Shared async HTTP client for the `co_worker_lite` backend.
//!
//! Re-used by the CLI binary and by the Tauri plugin. All operations return
//! `Result<T, ClientError>` so callers can map onto their own error types
//! (axum, Tauri command errors, anyhow, etc.).

use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub use chrono::{DateTime, Utc};

// ---------------------------------------------------------------------------
// Wire types — must match `co_worker_lite::types`.
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWithMessages {
    pub id: Uuid,
    pub title: String,
    pub model_name: String,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageResponse {
    pub message: Message,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatResponse {
    pub session_id: Uuid,
    pub message: Message,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub model: String,
    pub loaded: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size_bytes: u64,
    pub loaded: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCatalog {
    pub current: String,
    pub entries: Vec<ModelCatalogEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCatalogEntry {
    pub name: String,
    pub kind: ModelKind,
    pub repo: String,
    pub filename: String,
    pub context_length: u32,
    pub size_bytes: Option<u64>,
    pub min_ram_gib: Option<u32>,
    pub description: Option<String>,
    pub present: bool,
    pub loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Preset,
    Local,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Memory {
    pub id: Uuid,
    pub content: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugTurn {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugContext {
    pub session_id: Uuid,
    pub context_length: u32,
    pub turns: Vec<DebugTurn>,
    pub prompt_tokens_estimate: u32,
    pub memories_injected: usize,
}

// ---------------------------------------------------------------------------
// Errors.
// ---------------------------------------------------------------------------

/// Body shape used by the backend for all non-2xx responses.
#[derive(Debug, Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("{status} {url}: {code}: {message}")]
    Api {
        status: u16,
        url: String,
        code: String,
        message: String,
    },
    #[error("{status} {url}: {body}")]
    UnknownStatus {
        status: u16,
        url: String,
        body: String,
    },
}

impl ClientError {
    /// True when the backend responded with `404 Not Found`.
    pub fn is_not_found(&self) -> bool {
        matches!(self, ClientError::Api { status: 404, .. })
    }
}

pub type Result<T> = std::result::Result<T, ClientError>;

// ---------------------------------------------------------------------------
// Client.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Client {
    inner: reqwest::Client,
    base: String,
}

impl Client {
    /// Build a client targeting `base_url` (e.g. `http://localhost:6969`).
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let inner = reqwest::Client::builder()
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            // Inference can take a while.
            .timeout(Duration::from_secs(600))
            .build()?;
        let base = base_url.into().trim_end_matches('/').to_string();
        Ok(Self { inner, base })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    // ----- health & metadata -----

    pub async fn health(&self) -> Result<HealthResponse> {
        self.get_json("/health").await
    }

    pub async fn list_models(&self) -> Result<ListModelsResponse> {
        self.get_json("/v1/models").await
    }

    /// Rich model catalog: every preset + every local GGUF, tagged with
    /// `present`/`loaded`. The UI uses this to render the switcher dropdown.
    pub async fn model_catalog(&self) -> Result<ModelCatalog> {
        self.get_json("/v1/models/catalog").await
    }

    /// Hot-swap the active model on the backend. Downloads the GGUF first
    /// if it's not on disk. Takes 10-60s depending on file presence and
    /// model size.
    pub async fn load_model(&self, name: &str) -> Result<ModelCatalogEntry> {
        self.post_json(
            "/v1/models/load",
            &serde_json::json!({ "name": name }),
        )
        .await
    }

    // ----- sessions -----

    pub async fn list_sessions(&self, limit: i64, offset: i64) -> Result<Vec<Session>> {
        self.get_json(&format!("/v1/sessions?limit={limit}&offset={offset}"))
            .await
    }

    pub async fn get_session(&self, id: Uuid) -> Result<SessionWithMessages> {
        self.get_json(&format!("/v1/sessions/{id}")).await
    }

    pub async fn create_session(
        &self,
        title: &str,
        system_prompt: Option<&str>,
    ) -> Result<Session> {
        self.post_json(
            "/v1/sessions",
            &serde_json::json!({ "title": title, "system_prompt": system_prompt }),
        )
        .await
    }

    pub async fn delete_session(&self, id: Uuid) -> Result<()> {
        let url = format!("{}/v1/sessions/{id}", self.base);
        let resp = self.inner.delete(&url).send().await?;
        check_status(resp).await.map(|_| ())
    }

    /// Patch a session's title and/or system prompt. Pass `None` to leave
    /// a field untouched; pass `Some("")` for `system_prompt` to clear it.
    pub async fn update_session(
        &self,
        id: Uuid,
        title: Option<&str>,
        system_prompt: Option<&str>,
    ) -> Result<Session> {
        let url = format!("{}/v1/sessions/{id}", self.base);
        let body = serde_json::json!({
            "title": title,
            "system_prompt": system_prompt,
        });
        let req = self.inner.patch(&url).json(&body);
        self.send_for_json(req).await
    }

    pub async fn debug_session(&self, id: Uuid) -> Result<DebugContext> {
        self.get_json(&format!("/v1/sessions/{id}/debug")).await
    }

    // ----- messages -----

    pub async fn send_message(
        &self,
        session_id: Uuid,
        content: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<MessageResponse> {
        self.post_json(
            &format!("/v1/sessions/{session_id}/messages"),
            &serde_json::json!({
                "content": content,
                "max_tokens": max_tokens,
                "temperature": temperature,
            }),
        )
        .await
    }

    /// Sticky-session chat: no session id needed; the server reuses the
    /// most-recently-updated session (or creates one).
    pub async fn chat(
        &self,
        content: &str,
        max_tokens: u32,
        temperature: f32,
        system_prompt: Option<&str>,
    ) -> Result<ChatResponse> {
        self.post_json(
            "/v1/chat",
            &serde_json::json!({
                "content": content,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "system_prompt": system_prompt,
            }),
        )
        .await
    }

    // ----- memories -----

    pub async fn list_memories(&self) -> Result<Vec<Memory>> {
        self.get_json("/v1/memories").await
    }

    pub async fn create_memory(&self, content: &str) -> Result<Memory> {
        self.post_json(
            "/v1/memories",
            &serde_json::json!({ "content": content }),
        )
        .await
    }

    pub async fn delete_memory(&self, id: Uuid) -> Result<()> {
        let url = format!("{}/v1/memories/{id}", self.base);
        let resp = self.inner.delete(&url).send().await?;
        check_status(resp).await.map(|_| ())
    }

    // ----- internals -----

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let req = self.inner.request(Method::GET, format!("{}{path}", self.base));
        self.send_for_json(req).await
    }

    async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let req = self
            .inner
            .request(Method::POST, format!("{}{path}", self.base))
            .json(body);
        self.send_for_json(req).await
    }

    async fn send_for_json<T: DeserializeOwned>(&self, req: RequestBuilder) -> Result<T> {
        let resp = req.send().await?;
        let resp = check_status(resp).await?;
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

async fn check_status(resp: Response) -> Result<Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let url = resp.url().to_string();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
        return Err(ClientError::Api {
            status: status.as_u16(),
            url,
            code: err.error.code,
            message: err.error.message,
        });
    }
    let body = if body.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("error")
            .to_string()
    } else {
        body
    };
    Err(ClientError::UnknownStatus {
        status: status.as_u16(),
        url,
        body,
    })
}

// Silence a clippy lint about unused StatusCode import on some configs.
#[doc(hidden)]
const _: Option<StatusCode> = None;
