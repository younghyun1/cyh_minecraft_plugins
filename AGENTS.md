# Minecraft workspace

Read the component README and [integration boundaries](docs/integration-boundaries.md) before changing behavior. Preserve the original host adapter's authorization and visibility rules when extracting reusable packages. Confirm Git status, remotes and upstream state before edits; preserve existing local changes.

## Ownership

- `crates/seed` owns the persistent Pumpkin biome sampler and independent synthetic parity fixtures. The pinned Pumpkin revision, dimension semantics and GPL notices travel together.
- `crates/tile-codec` owns the public tile types and encoder. `packages/map-ui` owns the matching browser reader, Leaflet rendering, inspection and bounded client cache. Coordinate protocol versions and malformed-input tests across both producers and consumers.
- `tools/tile-benchmark` owns development measurements over synthetic terrain. Include equivalent metadata and hover fidelity when comparing transports; distinguish generation, decoding, transfer and rendering costs.
- `plugins/map-control` owns private Paper/squaremap sockets, effective generator/visibility profiles and bounded actual-world reads. Keep Bukkit calls on the server thread, private seed material off HTTP/logs, and unknown generation/visibility states closed.
- `plugins/starting-equipment` owns the separate first-join equipment plugin. Its declared Paper/Java compatibility differs from map-control; do not assume the two share APIs.
- `plugins/wiki-ask` owns private `/ask` replies and Bukkit admission. `tools/wiki-assistant` owns wiki snapshots, bounded local retrieval, per-player conversation state, and persistent Codex IPC. Forward only final short answers, preserve sender UUID/username and prior-message context, and keep corpus acquisition outside the game request path. No shell tools or server-state access are exposed to players.
- `crates/playtime` and `crates/lag-probe` are independent operational CLIs. Tests use synthetic inputs and must not read real logs or contact a server.
- `adapters/cyhdev` is a preserved host integration snapshot, not an independently buildable website. Authentication, account authority, database ownership and composition remain explicit host dependencies.
- `archive` holds historical backup code and is excluded from maintained workspace checks. `datapacks` and `deploy` contain source/examples, not automatic installation steps.

## Bounds and compatibility

Predictions have a 512 MiB server budget across retained data and headroom; the host squaremap file cache has its separate 4 GiB budget. Preserve bounded request admission, worker queues, profile freshness and in-flight cache reservations. Changing limits requires a documented memory calculation and tests. The browser must expire permission-bearing data, retain bounded pan/zoom caches and prefer rendered terrain over predictions.

Public markers are readable by everyone; mutations remain administrator-only under database-current authority. Seeds, world/player data, map tiles, server logs, credentials and production configuration never enter this repository. Preserve same-user Unix sockets and their restrictive permissions. Do not bypass visibility checks to make a dimension or transport appear functional.

## Verification

Run from the root with the checked-in nightly toolchain and lockfile:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix packages/map-ui ci
npm --prefix packages/map-ui run check
```

Use development builds only. The benchmark command is `cargo run --locked --package minecraft-tile-benchmark -- target/biome-transport.json`. Its ignored timing/reference tests require explicit invocation; default tests need no Minecraft service. Run applicable checks after behavior changes and record actual results.

Java checks require locally supplied compatible Paper/squaremap jars and the compiler versions documented in each plugin README. Do not download or launch a Minecraft server as part of ordinary CI, and do not imply Rust/UI tests verify plugin runtime compatibility.

Load the installed Rust, frontend and database conventions for the relevant code. Keep Rust modules below 300 lines, handle failures explicitly, and keep standalone tooling in Rust workspace binaries. Generated contract sources must retain their generation/provenance record. Update durable docs and source-origin records when ownership or integration changes.

No build, test or source synchronization authorizes installing a plugin, changing production configuration, invoking a live control socket, or restarting, stopping, reloading or signaling a service. Deployment actions require their own explicit scope. Preserve original source directories when consolidating additional components.
