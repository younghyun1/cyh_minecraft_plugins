# Source origins

This repository consolidates custom integration source. It does not replace the upstream Paper, squaremap or Pumpkin projects, and it contains no live world or player records.

| Component | Origin | Snapshot notes |
| --- | --- | --- |
| Map-control plugin | [younghyun1/cyhdev](https://github.com/younghyun1/cyhdev), `minecraft/map-control` | Plugin 1.4.0 dimension changes at 7bac701; compiled and tested against Paper 26.3 build 49. |
| Biome sampler | [younghyun1/cyhdev](https://github.com/younghyun1/cyhdev), `tools/minecraft-seed` | Overworld, Nether and End sampling and synthetic reference fixtures at 7bac701; retains pinned Pumpkin dependencies and GPL notices. |
| Biome codec and benchmark | [younghyun1/cyhdev](https://github.com/younghyun1/cyhdev), Minecraft API and benchmark binary | Codec and benchmark changes at a2c5b22. Encoding preserved; website DTO and error dependencies replaced with local public types and error. Benchmark imports the extracted crates. |
| Website source adapters | [younghyun1/cyhdev](https://github.com/younghyun1/cyhdev), Minecraft feature and map UI | Final snapshot at edcb4e8, including gzip-only binary transport and the Binary/PNG selector, with original paths, tests and migration; not a standalone website. |
| Reusable map UI | [younghyun1/cyhdev](https://github.com/younghyun1/cyhdev), map renderer, codecs, cache and browser benchmark | Source hashes correspond to edcb4e8 before package-local transport, asset and import adaptations; independently typechecked, tested and bundled. |
| Starting-equipment plugin | Personal StartingEquipment project | Preserves the current working source based on commit cb401c4, including uncommitted fixes. Plugin metadata was aligned with the existing Gradle version 1.0.2. |
| Playtime parser | Custom `parse_logs` project | Source-only snapshot of the uncommitted Rust project. Input logs, output reports and generated binaries were excluded. The private host-specific musl compiler configuration was replaced by the portable workspace default. |
| Lag probe | Custom Minecraft network diagnostic | Replaced fixed private endpoints with required command-line targets. Added frame length validation and tests. The original hosts file and diagnostic output were excluded. |
| Legacy `mc` backup | Personal `mc` project | Working source based on 334bcbd; retained as an unsupported experiment. |
| Legacy `mc-backup` | [younghyun1/minecraft-backup](https://github.com/younghyun1/minecraft-backup) | Working source based on 03a9496, including uncommitted local changes. |
| Streaming backup component | Personal `db-backup-goog` project | Minecraft tar/zstd component only; unrelated database and Google Drive credentials/integration were excluded. |
| Spawn-chunk datapack | Custom `cyh_spawn_chunks` project | Five source files with fixed origin-centered footprint; no live chunk tickets or world data. |
| OpenRC service | Custom parameterized mcserver service | Service code only; its production conf.d values and historical configurations are excluded. An example uses generic paths and an unprivileged account. |

Consolidation preserved the original directories, histories and working changes without moving or resetting them. The cyhdev application received the dimension and transport feature edits separately; those changes are identified by their source commits above. This repository uses source snapshots so unrelated website history and private deployment data are not copied.

The final cyhdev source revision is `edcb4e8ae95593f28d12f4c44a456d7c3726e7d9`. Its history includes the dimension implementation at `7bac701c38099def05f8406749e3f0a7d88a7994` and tile codecs at `a2c5b2256281eae11b8f5708120642d3e1de868e`. The adapter and reusable map package retain per-file source hashes in their `SOURCE_SNAPSHOT.json` manifests; adapted documentation records both original and consolidated hashes.

The clean [squaremap](https://github.com/jpenilla/squaremap) checkout and [Pumpkin contribution fork](https://github.com/younghyun1/Pumpkin) remain external dependencies and projects. Preserve exact dependency revisions and their licenses instead of copying their complete servers and assets here. The empty `squaremap_rs` Hello World prototype has no implementation to consolidate.
