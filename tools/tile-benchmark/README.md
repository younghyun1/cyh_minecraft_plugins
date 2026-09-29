# Biome transport benchmark

Development-only comparison of JSON, CYBM binary and native PNG with equivalent metadata and exact biome hover indices. Each representation is measured with identity, gzip and Zstandard transport. PNG calls the production codec, including actual biome colors and the complete identity metadata chunk; additional Fast and High compression variants measure the tradeoff against the default Balanced setting. The deterministic corpus uses synthetic seed 1, fixed Y=64, all four supported presets, three levels, two locations and full/masked/empty visibility, for 72 tiles. The checked-in transport receipts predate Surface mode and remain historical fixed-slice measurements, not a benchmark of the new default projection.

```sh
cargo run --locked --package minecraft-tile-benchmark -- target/biome-transport.json
```

The report checks decoded palette/index equality before recording sizes and repeated encode/decode timings. Sampling, network transfer, HTTP framing and browser rendering are excluded. The executable rejects optimized builds so comparisons remain labeled as development measurements. It starts no Minecraft server and reads no live world or socket.

An optional second argument exports matched JSON, CYBM and PNG browser fixtures plus a manifest. This includes the 72 diagnostic tiles and a fixed grid for each preset at levels 0 and 1, tile X from -3 through 5 and tile Z from -2 through 2, for 432 total tiles. These coordinates cover a 1024 by 512 pixel viewport, a one-tile pan and zoom with a prefetch ring. Locations are fixed independently of codec results. The permission timestamps are synthetic; browser tests freeze only the wall clock while leaving performance measurements active.

```sh
cargo run --locked --package minecraft-tile-benchmark -- target/biome-transport.json target/browser-tiles
```
