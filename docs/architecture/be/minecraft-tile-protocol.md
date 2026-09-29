# Biome tile transport

The map uses compact CYBM with gzip by default, with native PNG available as a selectable alternative. Browser measurements found comparable visible loading with the existing request/cache policy, while CYBM uses substantially fewer bytes. Tile compression belongs to HTTP negotiation: browsers decompress gzip before the bounded tile reader sees the response. Clients declining gzip receive identity encoding. The map does not require a JavaScript decompressor or Zstandard support.

`POST /api/minecraft/map/seed-tile.bin` accepts the same bounded JSON address as `/seed-tile`: `world`, `tile_x`, `tile_z`, `level`, and `y`. Success is `application/vnd.cyhdev.biome-tile`, without a JSON envelope. Errors retain the established HTTP status and JSON error contract. Sensitive-response headers prevent shared caching; the browser manages the short permission lifetime. This route negotiates gzip independently of the website's other compression choices, and the outer HTTP layer cannot recompress it as Zstandard. JavaScript reads decompressed CYBM bytes and must not apply a second decompression pass.

`POST /api/minecraft/map/seed-tile.png` accepts the same request and returns a native `image/png` tile. Its pixels use the map's biome colors and transparent hidden cells; its private `cyBM` ancillary chunk carries the complete CYBM frame for exact hover identities and permission metadata. Different biomes may share a displayed color, so hover never infers identity from RGB. The metadata precedes `IDAT`, uses ordinary PNG CRC protection, and is unsafe to retain when another application edits the pixels.

PNG responses are capped at 65,536 bytes and always decode to 64 by 64 pixels. Up to 255 biome entries use an indexed PNG with a reserved transparent entry and the smallest supported bit depth. The valid 256-biome case uses RGBA because the palette plus transparency needs 257 symbols. Server encoding uses the balanced compression setting, runs outside Tokio workers, and has four admission slots whose permits remain held until the blocking work finishes even if the HTTP caller cancels. Encoding does not create an additional retained cache outside the shared 512 MiB seed budget.

The PNG browser adapter validates framing, checksums, dimensions and bounded identity metadata before native image decoding. It retains the resulting `ImageBitmap` alongside hover indices within the viewport-dependent tile-data budget, starting at 16 MiB and capped at 256 MiB, reserving two RGBA buffers per bitmap for CPU/GPU storage. Eviction, expiry, profile replacement and disposal close retained bitmaps; stale or canceled responses close newly decoded bitmaps. The renderer draws each native bitmap directly into its existing small tile canvas. Permission lifetime and request/profile binding remain identical across transports; neither PNG format nor image caching extends visibility authority.

## CYBM versions 1 and 2

The logical request and response use `y: null` for the Overworld surface climate projection and an integer Y for a height slice. Surface requests are rejected for other dimensions, whose established sampling remains unchanged. Surface and slice cache entries are distinct. The encoder retains version 1 for numeric slices so existing clients and captured fixtures remain compatible; surface frames use version 2 with the reserved signed Y value `-32768`. Version 1 must reject that sentinel. Version 2 accepts it only for the Overworld. The byte layout and all size limits are unchanged. PNG embeds the same versioned metadata, so image pixels and hover provenance agree.

The decoder accepts at most 40,000 decompressed bytes. All multibyte integers are little-endian. A tile always contains 4,096 row-major samples. Its step is `4 << level`, its span is `64 * step`, and its origin is the signed tile address multiplied by that span. Derived values are checked against the requested address and world bounds.

| Offset | Field | Encoding |
| --- | --- | --- |
| 0 | Magic | Four ASCII bytes `CYBM` |
| 4 | Version | `u8`, 1 or 2 |
| 5 | Pixel codec | `u8`: 0 for bit packing, 1 for runs |
| 6 | Dimension | `u8`: Overworld 0, Nether 1, End 2 |
| 7 | Preset | `u8`: default 0, large biomes 1, Nether 2, End 3 |
| 8 | Level | `u8`, zero through twelve |
| 9 | Y | `i16`; version 2 reserves `-32768` for Overworld surface climate |
| 11 | Tile X | `i32` |
| 15 | Tile Z | `i32` |
| 19 | Profile sample time | `u64` Unix milliseconds, within JavaScript's safe integer range |
| 27 | Permission lifetime | `u16` milliseconds, one through 15,000 |
| 29 | Palette count | `u16`, zero through 256 |
| 31 | Strings | Epoch, generator revision, then palette names; each is nonempty UTF-8 prefixed by its `u8` byte length, capped at 128 bytes |
| Variable | Pixel payload | Bit-packed symbols or variable-length runs |

Symbol zero means hidden/no sample; symbol `n + 1` selects palette entry `n`. The symbol width is `ceil(log2(palette_count + 1))`, including zero bits when the palette is empty. This supports all 256 palette entries without losing a null marker. Packed symbols are concatenated least-significant-bit first into bytes. Runs use unsigned base-128 varints whose value is `((length - 1) << bits) | symbol`. Runs may cross row boundaries. The encoder selects whichever uncompressed payload is shorter; HTTP compression is independent.

Decoders reject unknown versions/codecs, inconsistent dimension/preset pairs, invalid coordinates/heights, unsafe timestamps, excessive lifetimes, malformed UTF-8 or identifiers, missing/out-of-range palette values, truncated or oversized runs, and trailing bytes. Exactly 4,096 symbols must be reconstructed before rendering. The tile store independently checks freshness, request binding and profile replacement order. Palette construction occurs after permission masking, so neither strings nor indices reveal hidden biomes.

The compatibility JSON DTO remains the generated logical browser type. The binary adapter uses the host `apiFetch` policy and a fixed input buffer; it does not duplicate session or origin handling. Rust and TypeScript are covered by explicit wire fixtures and malformed-frame tests.

## Transport measurements

These recorded transport and browser results use the earlier numeric Y=64 corpus. They are fixed-slice measurements, not measurements of surface projection or its generation cost.

Reproduce the comparison with `cargo run --locked --package minecraft-tile-benchmark -- target/minecraft-tile-benchmark.json`. The 72 synthetic tiles cover four presets, three zoom levels, negative/origin regions, and full/masked/empty coverage. Every codec roundtrip is checked for exact palette and index preservation. The following development-build measurements exclude generation, HTTP framing, network and browser rendering.

| Transport | Total bytes | Median encode / decode, microseconds |
| --- | ---: | ---: |
| Palette JSON with gzip | 70,804 | 612 / 526 |
| CYBM with gzip | 53,031 | 128 / 63 |
| CYBM with Zstandard | 53,373 | 84 / 42 |
| Native PNG with hover data, balanced | 124,424 | 476 / 97 |
| Native PNG with hover data and HTTP gzip | 108,551 | 555 / 132 |

Balanced PNG reduces bytes by 9.9% versus the fast encoder for roughly 184 additional microseconds of median encoding time. High compression saves only another 0.2% while increasing median time to 596 microseconds, so balanced is the chosen image setting. Native PNG duplicates the identity grid because the browser exposes decoded RGBA rather than indexed PNG symbols and several biomes share display colors. The earlier 73,005-byte PNG prototype recovered palette indices by decoding `IDAT` in Rust and used synthetic colors; it was not equivalent to the browser-native image path. Its result must not be used to claim the current PNG endpoint is smaller or faster.

## Browser comparison

The opt-in Playwright harness uses the production Leaflet renderer, tile cache and decoders against 432 real sampler-generated fixtures. Seventy-two Chromium runs cover four presets, three repetitions, native PNG, gzip-wrapped PNG, and gzip-wrapped CYBM, both locally and with 1 Mbps download throughput and 80 ms latency. The viewport is 1024 by 512 pixels; each run loads a cold view, pans, revisits cached tiles, and zooms from sample level zero to one. All fixture requests succeeded and each stage returned 512 valid hover samples.

| Constrained-network stage | Compressed CYBM complete viewport | Native PNG complete viewport |
| --- | ---: | ---: |
| Initial view | 310.1 ms | 302.1 ms |
| Pan | 142.7 ms | 143.9 ms |
| Cached revisit | 9.5 ms, zero requests | 9.8 ms, zero requests |
| Zoom out | 499.9 ms | 500.0 ms |

Values are medians over twelve runs per transport and stage. The paint proxy is the first new animation frame after visible tile callbacks draw; it does not measure display photons. Local timings vary with frame phase and do not establish a universal speed multiplier. Median drawing took at most 1.3 ms and 512 hover reads at most 0.1 ms. PNG had no frame interval over 32 ms; CYBM had one 32.6 ms interval. HTTP-gzip PNG had no consistent visible-latency advantage, so the production PNG route retains ordinary image compression behavior. Across the broader 72-tile diagnostic corpus, native PNG transfers 2.35 times as many bytes as gzip CYBM. Slower links, coarse terrain and different browsers/hardware may change the tradeoff.

Export the matching fixtures from this repository with `cargo run --locked --package minecraft-tile-benchmark -- target/minecraft-tile-benchmark.json target/browser-tiles`. Then run `npm ci`, `npx playwright install chromium` and `npm run benchmark:browser` from `packages/map-ui`. The [standalone browser package](../../../packages/map-ui/README.md) defaults to that fixture directory and accepts `SEED_BENCHMARK_CORPUS` for another location. Its fixture server uses loopback port 3107. `SEED_BENCHMARK_SMOKE=1` runs a short local integration check without replacing the complete receipt. These measurements exclude server generation/encoding, use a bounded local fixture server, and freeze only the fixture permission clock while keeping performance clocks and animation frames real. Ordinary unit/build checks exclude this opt-in benchmark.

Sanitized [codec measurements](../../benchmarks/native-png-balanced-benchmark.json) and [browser measurements](../../benchmarks/browser-benchmark.json) retain the detailed receipts.
