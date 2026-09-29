# PaperMC playtime parser

A synchronous, streaming Rust CLI for player activity in PaperMC logs. Reads plain logs and gzip archives directly, sorts dated rotations numerically, deduplicates repeated input paths, and emits a JSON report. Supports standard Paper file/console formats and `System chat:` join/leave messages.

```sh
cargo run --package parse_logs -- /path/to/server/logs --pretty
cargo run --package parse_logs -- logs/2026-09-27-1.log.gz logs/latest.log --latest-date 2026-09-28
# After building:
target/debug/parse_logs /path/to/server/logs > playtime.json
```

Successful stdout contains one JSON report. Fatal diagnostics are structured JSON on stdout with a nonzero exit status; always check that status before consuming a report. `RUST_LOG` controls diagnostic filtering. Input logs are never modified. Omitting input paths reads `../logs` relative to the current directory.

Each `players` entry is keyed by player name and contains:

- `uuid`: authentication UUID if present; otherwise null.
- `playtime_seconds`, `completed_sessions`: observed join-to-leave sessions, including sessions closed by a clean `Stopping server` message.
- `open_session_since`, `open_session_observed_seconds`: an unclosed session and its duration through the last timestamp in the input, excluded from confirmed totals.
- `incomplete_sessions`: joins superseded by another join, a restart, or a gap of more than one calendar day between files; their durations are excluded.
- `unmatched_leaves`, `duplicate_joins`: missing joins and repeated joins at the same timestamp. Login details and `lost connection` lines are not counted twice.
- `first_join`, `last_seen`: observed player event timestamps.

## Timestamp and identity limitations

Times are **server-local wall clock**, without an invented UTC offset. Set `TZ` to the server timezone when its timezone differs from the parser host. Archive dates come from `YYYY-MM-DD-N.log[.gz]`. The first date of `latest.log` defaults to its local modification date; supply `--latest-date YYYY-MM-DD` for copied files or a latest.log spanning multiple days. A backwards jump exceeding twelve hours means midnight; smaller out-of-order timestamps are skipped and counted. DST and manual clock corrections cannot be reconstructed exactly from time-only logs.

Pass logs from one server in a single invocation. Cross-file sessions are preserved, but missing files or crash messages can make continuity unknowable. The totals are log-derived estimates, not a claim of complete historical playtime. Rotated copies with different paths are not deduplicated by content. Player names remain separate across renames; UUIDs are metadata, not aggregation keys. Custom nicknames, Bedrock names, altered plugin messages, and logs with embedded ANSI codes are not supported.

Only newline-terminated records are processed, so a live file's partially written last line is ignored. Each file is read up to its size when opened. Malformed UTF-8, corrupt gzip, and lines over 1 MiB fail explicitly. Unrecognized records are ignored; no chat text, IP addresses, or coordinates appear in reports. Memory is bounded by `--max-players` (default 100000), file discovery, and a 1 MiB line buffer; session history is not retained. This is a batch report, not a live tailing service.

## Build

Requires nightly Rust, a C compiler and make. The workspace toolchain follows rolling nightly and its root Cargo.lock pins dependency resolution. Builds use the local Linux or macOS target and jemalloc. This repository does not impose the original deployment host's CPU baseline or temporary compiler path. A separate static musl build needs an installed musl compiler and a matching target configuration.

```sh
cargo fmt --all --check
cargo clippy --package parse_logs --all-targets -- -D warnings
cargo test --package parse_logs
cargo build --package parse_logs
```

No HTTP, database, async runtime or container deployment is needed for this CLI. Tests generate synthetic log fixtures in temporary directories and do not read server logs.
