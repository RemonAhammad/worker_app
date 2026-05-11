//! HTTP client for the co_worker_lite backend.
//!
//! All public methods return `anyhow::Result<T>`; on a non-2xx status, the
//! backend's structured error body is parsed and surfaced as the message.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::types::*;

#[derive(Clone)]
pub struct Client {
    inner: reqwest::Client,
    base: String,
}

impl Client {
    pub fn new(base: impl Into<String>) -> Result<Self> {
        let inner = reqwest::Client::builder()
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            // Inference can take a while; do not impose a tight default.
            .timeout(Duration::from_secs(600))
            .build()
            .context("building reqwest client")?;
        let base = base.into().trim_end_matches('/').to_string();
        Ok(Self { inner, base })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        self.get_json("/health").await
    }

    pub async fn list_models(&self) -> Result<ListModelsResponse> {
        self.get_json("/v1/models").await
    }

    pub async fn list_sessions(&self, limit: i64, offset: i64) -> Result<Vec<Session>> {
        let path = format!("/v1/sessions?limit={limit}&offset={offset}");
        self.get_json(&path).await
    }

    pub async fn get_session(&self, id: Uuid) -> Result<SessionWithMessages> {
        self.get_json(&format!("/v1/sessions/{id}")).await
    }

    pub async fn create_session(
        &self,
        title: &str,
        system_prompt: Option<&str>,
    ) -> Result<Session> {
        let body = CreateSessionRequest { title, system_prompt };
        self.post_json("/v1/sessions", &body).await
    }

    pub async fn delete_session(&self, id: Uuid) -> Result<()> {
        let url = format!("{}/v1/sessions/{id}", self.base);
        let resp = self.inner.delete(&url).send().await?;
        check_status(resp).await.map(|_| ())
    }

    pub async fn send_message(
        &self,
        session_id: Uuid,
        content: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<MessageResponse> {
        let body = CreateMessageRequest {
            content,
            max_tokens,
            temperature,
        };
        self.post_json(&format!("/v1/sessions/{session_id}/messages"), &body)
            .await
    }

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
        let resp = req.send().await.context("sending request")?;
        let resp = check_status(resp).await?;
        resp.json::<T>().await.context("parsing response body")
    }
}

async fn check_status(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let url = resp.url().clone();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(err) = serde_json::from_str::<ApiError>(&body) {
        return Err(anyhow!(
            "{} {} ({}): {}",
            status.as_u16(),
            url,
            err.error.code,
            err.error.message
        ));
    }
    Err(anyhow!(
        "{} {}: {}",
        status.as_u16(),
        url,
        if body.is_empty() { status.canonical_reason().unwrap_or("error").to_string() } else { body }
    ))
}

/// Convenience: turn 404 into `Ok(None)` so callers can treat missing sessions
/// non-fatally without inspecting status codes.
#[allow(dead_code)]
pub fn is_not_found(err: &anyhow::Error) -> bool {
    err.to_string()
        .starts_with(StatusCode::NOT_FOUND.as_u16().to_string().as_str())
}
