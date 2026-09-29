//! One byte reservation shared by every process-local seed cache.

use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

pub(super) const MAX_BYTES: usize = 512 * 1024 * 1024;
// Combined bounded hash indexes, worker density buffers, sampler and response scratch.
const RESERVED_BYTES: usize = 128 * 1024 * 1024;

pub(super) fn shared() -> Arc<Semaphore> {
    static BUDGET: OnceLock<Arc<Semaphore>> = OnceLock::new();
    Arc::clone(BUDGET.get_or_init(|| Arc::new(Semaphore::new(MAX_BYTES - RESERVED_BYTES))))
}
