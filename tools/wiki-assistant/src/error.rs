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
    #[error("Codex rate limit reached")]
    RateLimit,
    #[error("Codex authentication or access failed")]
    Access,
    #[error("Codex service is unavailable")]
    Unavailable,
}

impl Error {
    /// Classify only documented status fields; never relay arbitrary upstream error text.
    pub fn provider(error: &serde_json::Value) -> Self {
        let info = &error["codexErrorInfo"];
        let http = info.as_object().and_then(|map| {
            map.values()
                .find_map(|value| value["httpStatusCode"].as_u64())
        });
        match (
            info.as_str(),
            http.or_else(|| info["httpStatusCode"].as_u64()),
        ) {
            (Some("usageLimitExceeded"), _) | (_, Some(429)) => Self::RateLimit,
            (Some("unauthorized"), _) | (_, Some(401 | 403)) => Self::Access,
            (
                Some(
                    "httpConnectionFailed"
                    | "responseStreamConnectionFailed"
                    | "responseStreamDisconnected",
                ),
                _,
            )
            | (_, Some(500..=599)) => Self::Unavailable,
            _ => Self::Codex,
        }
    }

    /// Stable machine codes let the adapter display fixed, safe failure messages.
    pub fn chat_code(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::RateLimit => "rate_limit",
            Self::Access => "access",
            Self::Unavailable => "service_unavailable",
            Self::Io(_) => "connection",
            Self::Codex | Self::Json(_) => "response",
            Self::Http(_) | Self::Index(_) | Self::Invalid(_) => "local_data",
        }
    }
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
            Self::RateLimit => "Codex rate limit reached",
            Self::Access => "Codex authentication or model access failed",
            Self::Unavailable => "Codex service is unavailable",
        }
    }

    /// Only transient transport failures are eligible for a later request.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.is_timeout() || error.is_connect(),
            Self::Timeout | Self::RateLimit | Self::Unavailable => true,
            Self::Io(_)
            | Self::Json(_)
            | Self::Index(_)
            | Self::Invalid(_)
            | Self::Codex
            | Self::Access => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn provider_categories_never_relay_messages() {
        use super::Error;
        use serde_json::json;
        for (info, expected) in [
            (json!("usageLimitExceeded"), "rate_limit"),
            (
                json!({"httpConnectionFailed":{"httpStatusCode":503}}),
                "service_unavailable",
            ),
            (
                json!({"httpConnectionFailed":{"httpStatusCode":401}}),
                "access",
            ),
            (json!({"httpStatusCode":429}), "rate_limit"),
            (json!("PRIVATE UNKNOWN CATEGORY"), "response"),
        ] {
            let error = Error::provider(&json!({"message":"PRIVATE", "codexErrorInfo":info}));
            assert_eq!(error.chat_code(), expected);
            assert!(!error.to_string().contains("PRIVATE"));
        }
    }

    #[test]
    fn startup_diagnostics_hide_unstructured_error_details() {
        let error = super::Error::Invalid("synthetic secret content".into());
        assert!(!error.startup_diagnostic().contains("secret"));
        let error =
            super::Error::Invalid("dedicated Codex home must not contain custom skills".into());
        assert!(error.startup_diagnostic().contains("custom skills"));
    }
}
