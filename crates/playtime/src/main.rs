//! Streaming PaperMC activity reporting command.
include!("mod.rs");

use std::process::ExitCode;
use tracing_subscriber::{EnvFilter, fmt::time::UtcTime};

#[global_allocator]
static ALLOCATOR: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Emit diagnostics as JSON and return a failing status without panicking.
fn main() -> ExitCode {
    let filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::new("warn,parse_logs=info"),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(false)
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .init();
    match app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, retryable = error.is_retryable(), "Parser failed");
            ExitCode::FAILURE
        }
    }
}
