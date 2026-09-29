//! Offline wiki snapshot, section search, and private Minecraft chat transport.
mod bridge;
mod cli;
mod corpus;
mod download;
mod error;
mod index;
mod protocol;
mod retrieval;
#[cfg(test)]
mod retrieval_tests;
mod rpc;
mod sessions;

use clap::Parser;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Keep stdout reserved for the machine protocol and search results.
fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_writer(std::io::stderr)
        .init();
    match cli::Cli::parse().run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, retryable = error.retryable(), "Operation failed");
            std::process::ExitCode::FAILURE
        }
    }
}
