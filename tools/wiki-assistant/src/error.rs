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
    /// Operator diagnostics must never include questions, provider responses, or credential contents.
    pub fn startup_diagnostic(&self) -> &str {
        match self {
            Self::Invalid(message)
                if message.starts_with("dedicated Codex home must not contain ") =>
            {
                message
            }
            Self::Invalid(_) => "Invalid wiki snapshot or isolated Codex configuration",
            Self::Io(_) => "Unable to read wiki data or start Codex; check paths and permissions",
            Self::Http(_) => "Network request failed",
            Self::Json(_) => "Invalid JSON in wiki data or Codex protocol",
            Self::Index(_) => "Wiki index could not be loaded",
            Self::Codex => {
                "Codex initialization failed; check dedicated login and CLI compatibility"
            }
            Self::Timeout => "Codex initialization deadline exceeded",
        }
    }

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

#[cfg(test)]
mod tests {
    #[test]
    fn startup_diagnostics_hide_unstructured_error_details() {
        let error = super::Error::Invalid("synthetic secret content".into());
        assert!(!error.startup_diagnostic().contains("secret"));
        let error =
            super::Error::Invalid("dedicated Codex home must not contain custom skills".into());
        assert!(error.startup_diagnostic().contains("custom skills"));
    }
}
