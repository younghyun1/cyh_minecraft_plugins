# Cyhdev map control

This Paper plugin exposes squaremap's persistent player visibility and bounded actual-world queries through separate Unix sockets. The website and Minecraft must run as the same OS user. It depends on squaremap and uses its public `PlayerManager` and mapped-world APIs. It does not dispatch console commands, listen on TCP, or maintain a second visibility database. It targets Paper 26.3 and Java 25; Folia is not supported.

The socket is `plugins/CyhdevMapControl/control.sock` relative to Minecraft's working directory. Its parent directory is mode `0700`; the socket is `0600`. The plugin processes one connection at a time with a three-second deadline, a 64-byte request limit, and at most 1000 status rows. Bukkit and squaremap calls run on the main server thread. Expired queued requests are cancelled; as with any external mutation, a lost acknowledgement can still follow a completed change. Do not retry automatically.

## Build and install

Compile against the installed Paper API, mapped Paper server jar, squaremap jar, and Paper's Gson dependency without bundling them. The prediction adapter uses version-specific server and squaremap internals to inspect the effective generator and pending chunk holders without loading chunks. The following commands run in Bash from this directory; set `MC_SERVER` to the Minecraft directory. Explicit Paper and Gson entries prevent accidentally compiling against older cached server libraries. Verify `java`, `javac`, and `jar` are installed first; the compiler must support Java 25.

```bash
MC_SERVER="$HOME/mcserver"
MAP_CLASSPATH="$MC_SERVER/libraries/io/papermc/paper/paper-api/26.3.build.49-alpha/paper-api-26.3.build.49-alpha.jar"
MAP_CLASSPATH="$MAP_CLASSPATH:$MC_SERVER/versions/26.3/paper-26.3.jar"
MAP_CLASSPATH="$MAP_CLASSPATH:$MC_SERVER/plugins/squaremap-paper-mc26.3-1.4.1-SNAPSHOT+ac71dd2.jar"
MAP_CLASSPATH="$MAP_CLASSPATH:$MC_SERVER/libraries/com/google/code/gson/gson/2.14.0/gson-2.14.0.jar"
MAP_CLASSPATH="$MAP_CLASSPATH:$MC_SERVER/libraries/com/mojang/datafixerupper/10.0.21/datafixerupper-10.0.21.jar"
while IFS= read -r dependency; do MAP_CLASSPATH="$MAP_CLASSPATH:$dependency"; done < <(rg --files "$MC_SERVER/libraries" -g '*.jar')
mkdir -p target/classes
javac --release 25 -Xlint:all,-classfile -Werror -cp "$MAP_CLASSPATH" -d target/classes src/com/cyhdev/minecraft/*.java
jar --create --file target/cyhdev-map-control.jar --manifest MANIFEST.MF -C target/classes . -C . plugin.yml
```

`-classfile` suppresses missing optional annotation warnings in external dependencies; source warnings remain errors. Compilation and packaging are local verification. Installation is a separate deployment action: copy the jar to the server's `plugins/` directory, configure the website, and restart Minecraft once. Set `MINECRAFT_MAP_CONTROL_SOCKET` in the website environment to the absolute `control.sock` path and `MINECRAFT_WORLD_SOCKET` to the absolute `world.sock` path, then deploy the website normally. Subsequent visibility toggles and world queries need no restart. Removing each variable disables its corresponding website capability; removing the plugin jar takes effect on the next restart. Existing squaremap visibility preferences remain intact.

## Protocol and verification

Each connection accepts one ASCII line: `STATUS`, `HIDE <canonical UUID>`, or `SHOW <canonical UUID>`, terminated by a newline. A successful mutation returns `OK\n`. Status returns `OK\n` followed by zero or more `<UUID> <0|1>\n` rows; `1` means squaremap's explicit hidden state. The server closes the connection after the response. Invalid requests or offline targets return `ERROR\n`; deadlines may instead close the connection without a response. Visibility allowed does not override spectator, invisibility, equipment, or world tracker settings.

The ignored Rust tests `inspect_live_map_control` and `inspect_empty_plugin_rejections` exercise the real plugin. The latter requires an isolated Paper server with no players; it checks unsupported commands, invalid and offline targets, oversized requests, incomplete-frame timeout, and recovery. An SSH Unix-socket forward can connect the local Rust tests to a remote development instance. Normal tests cover strict acknowledgements, response bounds, and status parsing without a running JVM.

## World data protocol

`plugins/CyhdevMapControl/world.sock` uses the same private directory and mode `0600`. It has its own worker, processes one query at a time, and closes connections after one response. The request must be one UTF-8 JSON line, including its newline within 4096 bytes. Responses are one JSON line capped at 2 MiB. Every query, including incomplete request framing, has a 15-second deadline. Invalid requests return `{"error":"invalid_request"}`; failed queries return `{"error":"unavailable"}`. Expiry may return `{"error":"timeout"}` or close the connection. The website must treat disconnection as failure, rather than an empty scan.

Accepted requests are:

```json
{"kind":"catalog"}
{"kind":"area","world":"minecraft:overworld","chunk_x":0,"chunk_z":0,"width":8,"height":8,"y":null}
{"kind":"blocks","world":"minecraft:overworld","chunk_x":0,"chunk_z":0,"width":4,"height":4,"block":"minecraft:diamond_ore","min_y":-64,"max_y":64}
```

`area` accepts a rectangle of 1–8 chunks on each axis. Its optional `y` selects a biome slice; omitted or null means surface biomes. `blocks` accepts 1–4 chunks on each axis and an inclusive height range of at most 512 layers. Heights must be within both `-2032..2031` and the selected world's actual build limits. The complete rectangle must fall within `-30000000..30000000` block coordinates. Namespaced world and material identifiers are at most 128 characters; material matching is exact. Unknown fields, duplicate keys, noninteger coordinates, and noncanonical identifiers are rejected. Only currently enabled squaremap worlds can be queried. These operations never return a seed.

Successful catalog, area, and block responses include these fields; arrays unused by the requested operation are empty:

```json
{
  "kind":"catalog",
  "world":null,
  "sampled_at_ms":1790553600000,
  "scanned_chunks":0,
  "missing_chunks":0,
  "truncated":true,
  "worlds":[{"id":"minecraft:overworld","name":"world","map_id":"minecraft_overworld","min_y":-64,"max_y":319}],
  "blocks":["minecraft:stone"],
  "cells":[],
  "structures":[],
  "matches":[]
}
```

The catalog returns up to 16 `{id,name,map_id,min_y,max_y}` worlds and 4096 namespaced block identifiers. `id` is Paper's namespaced world identifier. `map_id` uses squaremap's exact tile-directory transformation, replacing `:` with `_`, for example `minecraft_overworld`. Both catalog heights are inclusive. Catalog response `world` is null.

An area response has at most 1024 `{x,z,y,biome}` cells and 256 `{kind,min_x,min_y,min_z,max_x,max_y,max_z}` structures, with inclusive structure bounds. Cells start at each 4×4 block grid origin, sample that origin's column, and always report its highest non-air block as `y`, including when a separate biome slice is requested. Elevation includes water, leaves, buildings, and the Nether roof. It represents current surface height, rather than a terrain-only or cave-height model. Biomes and block matches come from immutable snapshots of actual chunks, including placed and mined blocks. One non-generating asynchronous chunk load is pending at a time; snapshot capture runs on the server thread at most once per tick. Snapshot processing and saved-file reads run on the socket worker. Ungenerated chunks increment `missing_chunks` and contribute no cells or matches. A block response contains at most 512 exact `{x,y,z}` matches; finding another match stops the search and sets `truncated`. A query is a sequence of chunk snapshots, not an atomic world snapshot. `sampled_at_ms` is its start time.

Structures come from the queried chunks' own last-saved structure starts in dimension-specific region files. Bounding boxes combine saved structure pieces. Structures that intersect the query but start outside it are absent; saved metadata can remain after players demolish a structure. Newly generated, unsaved structures are unavailable until the normal server save. Reading saved starts avoids `Chunk.getStructures()`, whose reference traversal may synchronously load or generate neighboring chunks. Structure reads never force a save or follow structure references. Gzip, zlib, and raw region entries are supported; external chunks, LZ4, concurrent header changes, missing saved data, malformed NBT, and result limits set `truncated`, while terrain cells remain usable. Encoded chunk reads are capped at 1 MiB, decoded data at 8 MiB, NBT nesting at 64 levels, and NBT work at 100,000 tags. Result truncation and missing-chunk counts must remain visible to callers.

## Private prediction context

The Rust prediction service can request the effective generator profile and conservative chunk coverage over the same private `world.sock`. This operation has the same framing, deadline, world, height, and 1–8 chunk rectangle limits as `area`, but requires an explicit non-null `y`:

```json
{"kind":"prediction_context","world":"minecraft:overworld","chunk_x":1000000,"chunk_z":1000000,"width":1,"height":1,"y":64}
```

Its separate response shape is shown below with synthetic test values. `seed` is a signed 64-bit JSON integer. This response must stay inside the same-user backend/worker path; never forward it to HTTP clients or log it.

```json
{"kind":"prediction_context","world":"minecraft:overworld","world_id":"00000000-0000-0000-0000-000000000001","sampled_at_ms":1790553600000,"seed":1,"preset":"large_biomes","profile_revision":"0000000000000000000000000000000000000000000000000000000000000000","coverage":[{"chunk_x":1000000,"chunk_z":1000000,"state":"ungenerated"}]}
```

The legacy `prediction_context` request supports only Minecraft 26.3's normal overworld dimension with vanilla or large-biomes noise settings, the vanilla overworld multi-noise biome source, and standard build heights. Continuous `seed_profile` requests additionally support the vanilla Nether and End as described below. Built-in `vanilla` and `paper` packs are supported. Additional directory datapacks may contain only functions and function tags, such as spawn-chunk maintenance; their complete trees are inspected on the socket worker before and after coverage reads. Generation resources, other resource kinds, ZIP packs, symlinks, filters, overlays, feature flags, and ambiguous metadata remain unsupported. Inspection permits at most eight enabled packs, 256 entries and ten path components per custom pack, 64 KiB per resource, and 1 MiB per pack. Metadata is limited to 8 KiB and a single `pack` object containing a string description and integer format fields. Inspection observes the request deadline and rejects changing files or directories.

A Bukkit custom generator/biome provider, another noise preset, unsupported datapacks, custom dimensions, incompatible internals, and unavailable visibility information return `{"error":"unsupported_prediction"}`. Preset detection reads effective `NoiseGeneratorSettings`, not `server.properties`, so existing large-biomes worlds remain identified correctly after property changes. The seed comes from that world's live generator context. `world_id` is its persistent UUID. The 64-character `profile_revision` is a keyed digest covering dimension key, world identity, effective seed/preset, server build, enabled pack IDs and verified contents, the resource-manager identity, and generator object identities. Its random key changes on each plugin start, invalidating old cache entries without exposing a seed-derived public hash. No seed is added to the public world catalog.

Custom packs must also have contained only those supported resources when the current world registries loaded. Editing a previously loaded generation pack into a function-only pack does not restore vanilla registries without a server restart or appropriate reload. File inspection cannot reconstruct those earlier contents; deployments enabling compatibility should verify the pack fingerprint before and after the restart.

The separate `{"kind":"seed_profile","world":"minecraft:overworld"}` request supplies the same authoritative seed, preset, world identity, and opaque revision without querying chunk holders, region files, or terrain. It accepts exactly those two fields. The private response includes `kind`, `world`, `world_id`, `sampled_at_ms`, `seed`, `preset`, `profile_revision`, `world_border`, and `visibility`. `world_border` is an inclusive integer-column rectangle `{min_x,min_z,max_x,max_z}`. Each entry in `visibility` is either `{kind:"rectangle",min_x,min_z,max_x,max_z}` with inclusive bounds or `{kind:"circle",center_x,center_z,radius}` with inclusive squared-distance membership. Shapes form a union; an empty list permits all columns. The union is always intersected with the world border. A sample covering multiple columns must have its entire footprint permitted before being returned. These fields describe visibility, not whether terrain has been generated.

Plugin 1.4.0 adds dimension-specific continuous profiles. The world must have its standard Bukkit environment and build heights, its effective noise settings must match the dimension, and its biome source must match the vanilla source. Large biomes applies only to the Overworld; it does not change Nether or End profiles.

| World | Preset | Inclusive build heights | Required biome source |
| --- | --- | --- | --- |
| `minecraft:overworld` | `default` or `large_biomes` | -64 to 319 | Vanilla Overworld multi-noise preset |
| `minecraft:the_nether` | `nether` | 0 to 255 | Vanilla Nether multi-noise preset |
| `minecraft:the_end` | `end` | 0 to 255 | Exact built-in End source with all five registered End biomes |

The End is not a single biome: `the_end`, `end_highlands`, `end_midlands`, `small_end_islands`, and `end_barrens` are distinct. The profile exposes their generator selection only; terrain remains a prediction until squaremap renders it. Existing `prediction_context` consumers remain Overworld-only, and the wire fields are unchanged.

The geometry adapter is pinned to the installed squaremap 1.4.1 snapshot. It accepts at most 64 exact rectangle, circle, or world-border shape implementations; world-border shapes are converted to rectangles using squaremap's center truncation and exclusive upper edges. Polygon and custom shapes, moving borders, nonfinite or out-of-range coordinates, and circles exceeding radius 46,340 are rejected. That circle bound avoids squaremap's signed-int squared-radius overflow. Minecraft's own floating border is converted using `ceil(min)` and `ceil(max)-1`; accepted coordinates are within ±30,000,000. Policy geometry participates in the opaque revision, and two main-thread profile captures bracket bounded worker-thread pack inspection. The final capture supplies the response timestamp. Consumers may reuse a profile briefly for tile generation, must expire it promptly, and must never expose its seed through HTTP or logs. The earlier `prediction_context` operation retains its chunk-coverage behavior for compatibility.

Coverage always includes exactly one row per requested chunk, in Z-major order. A loaded full chunk is `generated`; any pending or partial live holder is `unknown`. An allocated saved entry whose chunk is unloaded is also `unknown`, because a region header cannot prove generation completed. `ungenerated` requires no live or pending holder in two main-thread observations surrounding a stable absent region entry check. Reads inspect at most two 8 KiB headers per chunk, reject malformed or changing files, and never load chunks, decode NBT, force saves, or schedule generation. Missing region directories, unreadable files, and uncertain storage remain `unknown`. A chunk with even one column outside squaremap's current visibility limits or the world border is `excluded`; visibility is checked before and after file reads. The adapter supports at most 64 configured visibility shapes and fails closed beyond that bound.

Coverage describes observed state, not a reservation against subsequent player exploration. The backend must obtain another context after generating its prediction tile, require the same world/profile/seed/preset, and publish cells only for chunks still marked `ungenerated`. Public tiles must expire promptly and keep predicted biome slices visually distinct from actual surface samples. This operation does not predict terrain heights, structures, or blocks.

The Pumpkin-based sampler is an approximate biome preview, linked directly into the web backend for continuous tiles. Against independent seed `1` Paper fixtures containing 75,264 raw quart samples per Overworld preset, its deterministic per-point lookup differed on 97 normal-preset samples and 96 large-biomes samples. Those differences were equal-distance climate-tree ties: beach versus stony shore in the normal fixture and beach versus plains in the large-biomes fixture. Vanilla's previous-leaf search history can select another tied biome; the preview deliberately keeps a coordinate's answer consistent across viewport requests. These finite fixture checks do not establish exact parity for every seed, dimension, and coordinate.

The local tests need a JVM and Gson but no game server. After the compilation commands above, run:

```bash
mkdir -p target/test-classes
javac --release 25 -Xlint:all,-classfile -Werror -cp "$MAP_CLASSPATH:target/classes" -d target/test-classes src/test/java/com/cyhdev/minecraft/*.java
java -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.WorldProtocolTest
java -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.SavedStructuresTest
java -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.PredictionContextTest
java -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.PredictionPacksTest
java -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.PredictionVisibilityTest
```

These tests cover strict request parsing, frame and numeric bounds, private seed encoding, response framing, inclusive block ranges, negative region coordinates, supported compression, structure-piece bounds, conservative coverage combination, malformed region/NBT data, symlink rejection, decompression limits, function-only pack compatibility, rejected generation resources and metadata, content revisions, pack inspection bounds, and exact visibility-policy parity with squaremap column predicates. Compilation checks compatibility with the real Paper and squaremap APIs. No live world is needed for these local checks. When using a dependency directory retaining multiple Minecraft versions, put the current Gson and DataFixerUpper jars before older cached copies; Paper build 49 uses DataFixerUpper 10.0.21.

The opt-in integration runner checks socket permissions, both socket protocols, mapped-world IDs, actual biome/elevation snapshots, biome slices, the 512-match block cap, private profile identity, stable opaque revisions, conservative chunk coverage, missing generated terrain, invalid frames, the 15-second incomplete-frame timeout, and recovery. It requires a disposable Paper instance with no players, synthetic seed `1`, a fresh normal or large-biomes overworld, saved spawn chunks, and the plugin installed. Use a new temporary directory containing only copies of the server/dependency jars and plugins. Configure `server-ip=127.0.0.1`, `server-port=0`, `white-list=true`, `enforce-whitelist=true`, `max-players=0`, `pause-when-empty-seconds=0`, and disabled management/RCON/query services. Disable squaremap's internal webserver too. Do not copy an existing world's data or connect the runner to production sockets.

For a fresh Minecraft 26.3 world with seed `1`, the spawn was `(160,160)`, or chunk `(10,10)`. Confirm the fixture's spawn from squaremap's generated settings and pass that generated chunk. The final argument is Paper's dimension-specific region directory, which Minecraft 26.3 places under `world/dimensions/minecraft/overworld/region`:

```bash
MC_TEST_DIR="/tmp/your-isolated-paper-instance"
java -Dcyhdev.minecraft.isolated=true -cp "$MAP_CLASSPATH:target/classes:target/test-classes" com.cyhdev.minecraft.WorldDataBridgeIntegrationTest "$MC_TEST_DIR/plugins/CyhdevMapControl" 10 10 "$MC_TEST_DIR/world/dimensions/minecraft/overworld/region"
```

Check that `r.31250.31250.mca` is absent in the fixture's overworld region directory both before and after the runner; its distant query must not generate terrain. Stop the test server through its own console afterward, then verify both socket paths were removed and the loopback listener closed. Keep the integration fixture and any generated jars outside the repository.

For a separate fresh fixture using `level-type=minecraft:large_biomes` and seed `1`, the tested spawn chunk was `(57,-78)`. Pass `-Dcyhdev.minecraft.fixturePreset=large_biomes` and those coordinates to the same runner. Both presets passed 719 integration checks against Paper 26.3 build 28. A separate isolated squaremap reload using a rectangle from `(0,0)` through `(23,31)` verified that chunks `(1,0)` and `(1,1)` are excluded because they extend past the visibility boundary, while `(0,0)` and `(0,1)` remain eligible. That fixture's original visibility configuration was restored before shutdown.
