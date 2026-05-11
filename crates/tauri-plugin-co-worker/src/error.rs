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
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let (kind, message) = match self {
            Error::Client(e) => match e {
                co_worker_client::ClientError::Api {
                    code, message, ..
                } => (code.clone(), message.clone()),
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
        };
        let mut s = serializer.serialize_struct("Error", 2)?;
        s.serialize_field("kind", &kind)?;
        s.serialize_field("message", &message)?;
        s.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
