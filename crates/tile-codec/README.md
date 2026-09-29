# Biome tile codec

`minecraft-tile-codec` provides public biome tile types, the CYBM v1/v2 encoder and native PNG tiles independently of the website framework. It preserves the production encoder bytes and bounds: 64 by 64 cells, up to 256 biome names, a reserved null symbol, at most 15 seconds of visibility validity and a 40,000-byte CYBM frame cap. Both encoders reject inconsistent world, preset, height and canonical tile geometry.

The encoder compares bit-packed palette symbols with variable-length runs and chooses the shorter payload before HTTP compression. It never includes a world seed. The browser decoder lives in `packages/map-ui`; the website adapter retains its matching original encoder and typed HTTP boundary. The benchmark includes a separate bounded decoder used only for measurements.

A numeric `y` retains the version 1 fixed-height frame. `None` selects the Overworld surface climate projection and encodes version 2 with the reserved signed Y value `-32768`; other dimensions reject this mode. The byte layout is otherwise unchanged. Surface climate does not claim an actual terrain height. PNG carries the same distinction in its embedded metadata.

PNG uses the same biome colors as the browser, transparent null pixels and the smallest indexed bit depth. A 256-biome palette uses RGBA to retain the additional null symbol. The private ancillary `cyBM` chunk contains the complete CYBM frame, allowing native bitmap decoding and exact hover names even when two biomes share a display color. Balanced PNG compression is the default; the total image is capped at 65,536 bytes. Hosts must preserve visibility expiry and private `no-store` response handling and bound concurrent compression work; an image representation does not make these responses public cache assets.

Run `cargo test --package minecraft-tile-codec`. Tests cover exact header/payload bytes, null masks, all 256 palette indices, invalid metadata, empty tiles, PNG pixel/metadata consistency and duplicate display colors.
