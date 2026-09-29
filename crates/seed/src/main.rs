//! Private-pipe entry point; stdout is reserved for the response protocol.

use std::{io, process::ExitCode};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Execute one request; diagnostic text never includes input or the private seed.
fn main() -> ExitCode {
    // A dependency panic must not accidentally print seed-bearing internal state.
    std::panic::set_hook(Box::new(|_| {}));
    match minecraft_seed::run(io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
