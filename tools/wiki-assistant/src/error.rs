//! Errors retain causes without logging questions, credentials, or model output.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP failure: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON failure: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Index failure: {0}")]
    Index(#[from] tantivy::TantivyError),
    #[error("Invalid data or configuration: {0}")]
    Invalid(String),
    #[error("Codex protocol failed")]
    Codex,
    #[error("Codex response deadline exceeded")]
    Timeout,
}

impl Error {
    /// Only transient transport failures are eligible for a later request.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.is_timeout() || error.is_connect(),
            Self::Timeout => true,
            Self::Io(_) | Self::Json(_) | Self::Index(_) | Self::Invalid(_) | Self::Codex => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
