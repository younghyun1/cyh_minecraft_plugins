# Player activity pipeline

`input` discovers files and supplies buffered, length-limited readers. `event` recognizes events through string slices rather than regexes or per-line allocations. `app` reconstructs local timestamps and applies events in chronological order. `state` maintains ordered per-name aggregates; output ordering is deterministic. This separation permits additional activity types without mixing recognition with I/O.

The hot path reuses one string buffer. Gzip decoding is incremental and pure Rust. A synchronous pipeline avoids async scheduling and blocking-pool costs for a single sequential disk scan. No caches or per-session history grow with log volume. Player state has an explicit configurable limit; authentication-only names count against it.

Session accounting is conservative about missing joins/leaves, restarts and later duplicate joins. A same-timestamp join is idempotent; a later join closes prior state as incomplete. `Stopping server` supplies a known end; startup invalidates remaining sessions without counting downtime. Only completed durations enter totals. Open durations stop at the final observed log timestamp and are reported separately. Name-based aggregation preserves readable keys but does not merge UUID aliases.

Errors use one crate-wide thiserror enum and a retryability predicate. Diagnostics use tracing JSON with UTC timestamps, flattened fields and only the current span. Reports use naive server timestamps because source records do not contain offsets. No raw source messages enter diagnostics or reports.

The crate uses edition 2024 and the workspace nightly toolchain. Its `src/mod.rs` contains module declarations, and nested Rust modules use mod.rs. Source files stay below 300 lines. The CLI uses jemalloc background purging and unprefixed allocation interception. There is no TLS stack or database dependency. The original host-specific musl configuration is not part of the portable workspace defaults.
