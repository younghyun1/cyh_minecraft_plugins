# Transport measurements

These receipts use synthetic seed 1 and contain no live world data or private paths. The map defaults to binary tiles; PNG remains selectable. The [protocol documentation](../architecture/be/minecraft-tile-protocol.md) describes the measured tradeoff and reproduction commands.

- `native-png-balanced-benchmark.json` contains the development-build codec comparison over 72 tiles, preserving exact biome indices across JSON, CYBM and PNG representations. It excludes terrain sampling, networking and browser rendering.
- `browser-benchmark.json` contains 72 Chromium runs across four presets, three repetitions, three transports and two network settings. The corpus field uses a portable synthetic identifier. The measurements exclude backend sampling and encoding; they compare the production browser renderer, decoder and bounded cache against exported fixtures.

The Rust exporter and the browser harness are independently runnable from this repository. Export to `target/browser-tiles`, then follow the [map package benchmark instructions](../../packages/map-ui/README.md#browser-transport-benchmark). The package also retains an aggregated copy of the complete browser receipt; its smoke mode writes a separate result. Timings are observations on the recorded platform, not a universal performance guarantee.
