//! Error type used by Tauri commands.
//!
//! `serde::Serialize` so it crosses the FFI bridge cleanly and the
//! TypeScript side receives a stable `{ kind, message }` shape.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] co_worker_client::ClientError),

    #[error("invalid url: {0}")]
    InvalidUrl(String),

    #[error("workspace is not configured")]
    NoWorkspace,

    #[error("invalid path: {0}")]
    ToolBadPath(String),

    #[error("path `{0}` is outside the workspace root `{1}`")]
    ToolOutsideWorkspace(String, String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid arguments: {0}")]
    BadArgs(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let (kind, message) = match self {
            Error::Client(e) => match e {
                co_worker_client::ClientError::Api { code, message, .. } => {
                    (code.clone(), message.clone())
                }
                co_worker_client::ClientError::Http(err) => {
                    ("http".into(), err.to_string())
                }
                co_worker_client::ClientError::Decode(err) => {
                    ("decode".into(), err.to_string())
                }
                co_worker_client::ClientError::UnknownStatus { body, status, .. } => {
                    (format!("status_{status}"), body.clone())
                }
            },
            Error::InvalidUrl(u) => ("invalid_url".into(), u.clone()),
            Error::NoWorkspace => ("no_workspace".into(), self.to_string()),
            Error::ToolBadPath(_) => ("tool_bad_path".into(), self.to_string()),
            Error::ToolOutsideWorkspace(_, _) => {
                ("tool_outside_workspace".into(), self.to_string())
            }
            Error::Io(_) => ("io".into(), self.to_string()),
            Error::Json(_) => ("json".into(), self.to_string()),
            Error::BadArgs(_) => ("bad_args".into(), self.to_string()),
        };
        let mut s = serializer.serialize_struct("Error", 2)?;
        s.serialize_field("kind", &kind)?;
        s.serialize_field("message", &message)?;
        s.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
