# Minecraft biome sampler

This Rust library samples Java 26.3 Overworld, Nether and End biomes using Pumpkin revision `4426d1113a211e6018a2db416e33b6b8a7802614`. It initializes only biome noise state. It does not start a Minecraft server, read world files, or generate chunks. Default and large-biomes Overworlds use their respective noise routers; matching the configured dimension and preset is required.

An embedding application calls `Predictor::new()` once and awaits `predict(request)` on its clonable handle. One dedicated CPU worker retains the seeded router and borrowing sampler across requests, rebuilding when seed, dimension or preset changes. Its private single-thread Rayon pool prevents per-core density-buffer pools from multiplying retained memory. Eight queued jobs are permitted; excess admission returns `Error::Busy`. Dropping an awaiting future cancels queued work and active sampling checks cancellation between rows. All handles dropping closes the worker. The standalone executable supports private pipe integrations and offline fixtures.

Build from the workspace root with `cargo build --locked --package minecraft-seed`. This uses the development profile. The package requires Pumpkin's Rust 1.96 minimum and uses the repository's nightly toolchain. The executable is `target/debug/minecraft-seed` unless a Cargo target override changes its location.

The original cyhdev Alpine builders needed `RUST_MIN_STACK=33554432` to compile Pumpkin's generated tables without overflowing rustc's default stack. This is a compiler setting for affected builders, not a sampler runtime requirement. This package does not supply a Docker build or deploy services.

Send one JSON object followed by a newline to stdin, then close stdin. The seed travels only through this private pipe, never command arguments. Read one JSON response followed by a newline from stdout. A nonzero exit indicates failure; the process intentionally emits no diagnostic input or seed. The supervising backend must bound subprocess concurrency, cap stdout at 2 MiB, enforce a deadline, and kill and reap cancelled or timed-out processes.

```json
{"seed":1,"large_biomes":false,"y":64,"min_x":176,"min_z":148,"width":1,"height":1,"step":4}
```

```json
{"generator_revision":"pumpkin-4426d1113a211e6018a2db416e33b6b8a7802614-java26.3","large_biomes":false,"cells":[{"x":176,"z":148,"biome":"minecraft:stony_shore"}]}
```

Requests are at most 4096 bytes including their newline. `seed` is a signed 64-bit integer. Optional `dimension` is `overworld` (the default), `nether`, or `end`; the Rust API exports `Dimension::{Overworld, Nether, End}`. Large biomes are rejected outside the Overworld. Width and height are positive and their product is at most 4096. Step is positive, all sample X/Z coordinates are within ±30,000,000, and Y is within -64 through 319 in the Overworld or 0 through 255 elsewhere. Coordinates and step are in blocks. Sampling uses the containing raw quart biome, rounding negative coordinates toward negative infinity. The result is a fixed-Y slice, not the surface biome or a Voronoi-blended rendering. Cells are ordered by Z row, then X column. The response shape is unchanged for existing pipe consumers.

The in-process path uses the same bounds without serializing a pipe request. A 64 by 64 tile fixes the largest density buffer at 16 KiB. Pumpkin's pool retains at most 1,024 such buffers on the owning CPU thread. This library owns no terrain tile cache; the embedding application must bound retained results and active requests. The cyhdev adapter reserves sampler and index headroom within its shared 512 MiB prediction budget, accounting retained palettes and indices separately from bounded active requests.

The same vanilla biome decision tree serves both presets; large-biomes behavior comes from its distinct noise router. Aligned grids use Pumpkin's volume sampler, while arbitrary block grids sample their individually rounded quart coordinates. Each tree lookup starts without a previous leaf so a coordinate remains stable across overlapping viewports and request order. Vanilla retains the previous nearest leaf when two candidates have equal distance; this preview's deterministic choice can differ from saved biomes at those boundaries. Predictions are approximate and cannot represent player edits, previously generated terrain from older game versions, custom datapacks, or custom generators.

The Nether has five biomes and uses its own decision tree with legacy-seeded temperature and vegetation noise, independent of Y. The pinned router's ordinary noise nodes omit that legacy initialization; zero-shift nodes select Pumpkin's correct existing legacy initializer. The End also has five biomes. Its center is `the_end`; outer chunks select highlands, midlands, barrens or small islands from erosion. The End adapter reuses Pumpkin's legacy random and simplex primitive but preserves Paper's separate float operations and widened squared-distance arithmetic. Pumpkin's fused operations change far-island radii, and its existing biome supplier overflows a 32-bit squared distance near the world edge. A fixed 64-slot chunk cache reuses each native tile's 16 identical quart lookups per chunk without growing retained state.

The checked-in synthetic fixture `tests/fixtures/paper26_3_dimensions.tsv` records 1,024 coordinates from retained Paper 26.3 build 28 noise classes at seeds 1, -1 and both signed 64-bit extremes. It covers negative and unaligned coordinates, the center boundary and ±30,000,000. Nether biome decisions match, with climate values differing by at most one 1/10,000 quantization unit due to intermediate precision; End erosion matches exactly. `tests/PaperDimensionFixture.java` reproduces the fixture using Paper's classes and bootstrapped registries without starting a server or generating a world. Put the matching Paper API, server, DataFixerUpper and Gson jars first on the classpath when retained libraries include older versions. Compile the test with Java 25 and run it from an ignored directory, passing the output TSV path. The ordinary Rust test suite checks this fixture, all five biomes in each dimension, dimension switches, cancellation and world-edge arithmetic.

Independent saved Paper 26.3-28-dev `93c9b2a` fixtures using synthetic seed 1 contain 75,264 raw quart samples per preset, across 49 chunks and all 96 Overworld quart heights. The default fixture has 97 differences, all equal-distance beach/stony-shore choices. The large-biomes fixture has 96 differences, all equal-distance beach/plains choices and includes negative Z coordinates. Combined, 150,335 of 150,528 samples match. This validates the sampled areas and preset selection, not every biome, seed or coordinate. Scalar and volume climate values agree across 1024 additional sample points per preset; overlapping-grid and scalar prediction tests verify stable output.

Run focused checks from this repository's root with `cargo test --locked --package minecraft-seed`, `cargo clippy --locked --package minecraft-seed --all-targets -- -D warnings`, and `cargo fmt --package minecraft-seed -- --check`. The ordinary tests use only checked-in synthetic fixtures and need no Java installation, server or world. An ignored test accepts an independently extracted synthetic seed-1 fixture with TSV columns `quart_x`, `quart_y`, `quart_z`, and namespaced `biome`, with one header row:

```sh
MINECRAFT_SEED_FIXTURE=/path/to/default.tsv cargo test --locked --package minecraft-seed compares_external_vanilla_fixture -- --ignored --nocapture
MINECRAFT_SEED_FIXTURE=/path/to/large.tsv MINECRAFT_SEED_LARGE_BIOMES=1 cargo test --locked --package minecraft-seed compares_external_vanilla_fixture -- --ignored --nocapture
```

The fixture comparison reports every mismatch count and rejects any mismatch whose saved and predicted biome distances differ. Add `MINECRAFT_SEED_REQUIRE_EXACT=1` to require exact saved-palette equality; that stricter check fails for both fixtures above. Retain this distinction when replacing the pinned generator.

Keep real seeds and world fixtures outside Git. This package is GPL-3.0-only; see [LICENSE](LICENSE) and [third-party notices](THIRD_PARTY_NOTICES.md).
