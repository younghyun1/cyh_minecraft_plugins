//! Failure sources shared by the command and streaming reader.
use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O failure: {0}")]
    Io(#[from] io::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "cannot determine date or rotation number for {0}; use dated Paper filenames or --latest-date for latest.log"
    )]
    Filename(PathBuf),
    #[error("no .log or .log.gz files found")]
    NoLogs,
    #[error("line exceeds the 1 MiB limit")]
    LineLimit,
    #[error("player limit reached ({0}); raise --max-players for trusted input")]
    PlayerLimit(usize),
    #[error("date overflow while processing midnight rollover")]
    DateOverflow,
}

impl Error {
    /// Only transient I/O errors warrant retrying the same input.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Io(error) => matches!(
                error.kind(),
                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ),
            Self::Json(_)
            | Self::Filename(_)
            | Self::NoLogs
            | Self::LineLimit
            | Self::PlayerLimit(_)
            | Self::DateOverflow => false,
        }
    }
}
