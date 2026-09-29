# Minecraft tools and plugins

Custom Paper plugins, Minecraft map integration and operational tools. The reusable packages build independently; website-specific adapters retain the existing application's authorization and composition boundaries.

| Component | Location | Build |
| --- | --- | --- |
| Paper map control and world profiles | `plugins/map-control` | Java 25; provided Paper 26.3 and squaremap dependencies |
| First-join equipment plugin | `plugins/starting-equipment` | Gradle and Java 21; provided Paper 1.21.11 API |
| Streaming playtime report | `crates/playtime` | `cargo run --package parse_logs -- --help` |
| Status and ping timing probe | `crates/lag-probe` | `cargo run --package mc-lag-probe -- --help` |
| Persistent Pumpkin biome sampler | `crates/seed` | `cargo test --package minecraft-seed` |
| Native PNG and binary biome tile encoders | `crates/tile-codec` | `cargo test --package minecraft-tile-codec` |
| Transport comparison | `tools/tile-benchmark` | `cargo run --package minecraft-tile-benchmark` |
| Solid/Leaflet map package | `packages/map-ui` | See its package README |
| Website integration snapshot | `adapters/cyhdev` | Builds within the original host application |
| Persistent spawn-chunk datapack | `datapacks/spawn-chunks` | Source functions; no automatic installation |
| Historical backup experiments | `archive` | Preserved source; excluded from maintained workspace checks |

Overworld predictions default to Surface climate projection, with an explicit fixed-Y view for underground biomes. Nether and End retain their dimension-specific sampling. Binary tiles with browser-native gzip are the default; native PNG remains selectable. The [tile protocol](docs/architecture/be/minecraft-tile-protocol.md) records their bounds and measured tradeoffs, the [terrain explorer architecture](docs/architecture/be/minecraft-explorer.md) describes the host integration, and the [map design](docs/design/fe/minecraft.md) covers browser behavior.

See [source origins](docs/source-origins.md), [integration boundaries](docs/integration-boundaries.md) and [deployment boundaries](deploy/README.md). The source adapters include the world-query/control protocols, four-GiB squaremap cache, 512-MiB prediction budget, public waypoints with administrator mutations, generated contracts and browser regression fixtures.

Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` for the current Rust workspace. These commands do not contact a Minecraft server. Tools which read logs or probe servers run only when explicitly invoked with their inputs. Building a plugin does not install it or restart any service.

Do not put worlds, logs, player data, credentials, production configuration, generated map tiles, Minecraft/Paper jars or backup archives in this repository. Each component retains its provenance and applicable license; the Pumpkin-based sampler's notices must travel with its distribution.
