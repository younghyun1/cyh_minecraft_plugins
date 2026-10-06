# Map UI

Reusable Solid components and browser state for squaremap terrain, continuous biome predictions, terrain inspection, and map overlays. This package includes the renderer and bounded caches; the host application owns page routing, world selection, authentication, public waypoint loading, administrator mutations, and API authorization.

## Verification

Run `npm ci`, then `npm run check` in this directory. The check typechecks the complete source, runs the browser-state and protocol unit tests, and emits an ES module, declarations, and CSS into `dist/`. It needs no sibling website checkout, Minecraft server, or backend credentials. The package is private in npm metadata; committing it to a public source repository does not publish it to npm.

Solid, its signals/web runtimes, and its compiler are pinned to the coordinated `2.0.0-rc.13` graph used by the source application. The Vite plugin is pinned to `3.0.0-next.47`; Leaflet is pinned to `1.9.4`. Do not independently update one Solid prerelease package. Vite 8.3.3, Vitest 5.0.3, and jsdom 30.1.2 are pinned and verified together; the combined toolchain requires Node.js 22.22.2+, 24.15.0+, or 26+ on those supported release lines. `.npmrc` preserves the source application's prerequisite handling, and the lockfile fixes the complete tested graph.

## Public boundary

| Export | Host responsibility |
| --- | --- |
| `MapCanvas`, `MapCanvasProps`, `MapAssets` | Supply loaded settings, current data, callbacks, and explicit terrain/player image URLs. Key the component by selected world; settings and asset functions stay fixed within that instance. |
| `MapInspection`, `createMapInspection`, `createMapQueryGate` | Supply authorized observed-world reads and lifecycle cleanup; render the inspection value where appropriate. |
| `createSeedTiles`, `createSeedTileReader`, `createSeedTilePngReader` | Supply the fetch transport and binary or PNG endpoint, configure dimension/surface-or-Y/visibility, and dispose the store when leaving the map. |
| `createSquaremapClient` | Supply a transport and the base URL for public squaremap JSON. |
| `createSeedTileLayer`, `createBiomeHighlightLayer`, `createTerrainBoundary`, `biomeChoices` | Integrate prediction tiles, transparent selected-biome highlights and the pinned dimension list with an existing Leaflet map when not using `MapCanvas`. |
| `decodeSeedTileBinary`, `decodeSeedTilePng`, `decodeSeedTile`, geometry helpers and generated contracts | Reuse framing, validation, colors, bounds, and typed data without adopting the page controller. |

The transport signature is `ApiTransport = (path: string, init: RequestInit) => Promise<Response>`. Authentication, origin selection, credential policy, and session redirects belong in that function. The package performs no global session mutation and has no hardcoded production origin. Metadata requests preserve the source public squaremap policy (`credentials: "omit"`); an authenticated deployment can override that in its supplied transport.

```tsx
import { MapCanvas, createSeedTileReader, createSquaremapClient, type ApiTransport, type MapAssets, type MapCanvasProps } from "@cyh-minecraft/map-ui";
import "@cyh-minecraft/map-ui/styles.css";

const transport: ApiTransport = (path, init) => fetch(path, init);
const metadata = createSquaremapClient(transport, "/map/tiles");
const readSeedTile = createSeedTileReader(transport, "/api/map/seed-tile.bin");
const assets: MapAssets = {
  terrainTileUrl: (world, refresh) => `/map/tiles/${encodeURIComponent(world)}/{z}/{x}_{y}.png${refresh === undefined ? "" : `?refresh=${refresh}`}`,
  playerFallbackUrl: "/map/images/icon/player.png",
  playerVitalUrl: (kind, value) => `/map/images/${kind}/${value}.png`,
};

function EmbeddedMap(props: Omit<MapCanvasProps, "assets">) {
  return <div class="minecraft-atlas" style={{ height: "70vh" }}>
    <div class="minecraft-atlas-stage"><MapCanvas {...props} assets={assets} /></div>
  </div>;
}
```

The host obtains settings from `metadata`, supplies `readSeedTile` to `createSeedTiles`, and binds the resulting store and state to `MapCanvasProps`. No image provider is assumed: adding `playerHeadUrl` explicitly enables remote player heads. Leaflet's CSS import remains in the renderer; use a bundler that supports CSS imports. Fonts fall back to monospace; no font service is contacted. The optional `.minecraft-page` fullscreen class accepts `--site-header-height`, `--site-footer-height`, and safe-area offsets, all defaulting to zero.

## Limits and contracts

The browser admits at most 2,048 active seed targets, with four concurrent seed requests. The seed-data budget starts at 16 MiB and scales by 128 KiB per active target to a 256 MiB ceiling; validated tiles cost at most 116,736 bytes including native bitmap storage. Each of the prediction and highlight layers retains at most 2,048 small canvases, with a 64 MiB two-buffer reserve per layer. Tile data plus both layer reserves total at most 384 MiB, excluding DOM/object/driver overhead. Native PNG tiles reserve 32 KiB each for CPU/GPU bitmap memory within that budget. The cache closes owned bitmaps on eviction, expiry, invalidation, cancellation, and disposal; canceled native decodes retain admission until settlement. Permission-bearing responses expire within 15 seconds. Packed terrain alpha and frontier geometry are bounded independently. The map permits two additional overview zoom levels while dynamically keeping native squaremap overview tiles within a 256-tile viewport budget. Wider viewports and the maximum seed level can reduce those extra levels. Browser zoom can enlarge the CSS viewport; prediction and biome-highlight grids retain their original 256 CSS-pixel tiles and seed level, preserving detail while admitting the larger visible set within the client bounds. Predictions remain beneath actual imagery, and actual terrain retains observed hover provenance.

Chunk grid lines are disabled across all dimensions. `MapCanvasProps` has no grid option.

`MapCanvas` includes Home, which centers the current dimension at X 0, Z 0 without changing zoom, and a kilometre scale that updates on zoom and resize. The scale treats one block as one metre in every dimension and accounts for squaremap's coordinate normalization. Frozen rivers use a distinct icy cyan in both canvas and PNG palettes.

Public waypoint names remain visible beside their markers, including while menus are closed. Waypoint markers and names draw above player banners and below popups. Clicking or tapping a marker opens its coordinates and optional description. Names and details use text nodes rather than HTML; administrator editing remains a host responsibility.

The host defaults Overworld predictions to Surface mode. Pass `null` for Y to select surface climate projection, or a number for an explicit fixed-height slice; Nether and End require numeric Y. The cache and hover data keep these modes distinct. Numeric slices retain CYBM v1; v2 carries the Overworld surface sentinel, including inside PNG metadata. Surface hover never claims a terrain height. The actual/predicted boundary hides during zoom and reappears after the settled canvas redraw; panning at the same zoom keeps it visible.

Binary with native HTTP gzip is the default in the host adapter; the Layers menu can choose PNG and persist that preference. A host that switches transports must call `store.invalidate()` to clear imagery and reject outstanding old replies. The optional PNG reader accepts `image/png` responses of at most 65,536 bytes; the browser decodes pixels, while the validated `cyBM` chunk preserves exact biome hover data. Fetch performs HTTP decompression before either bounded reader. The binary reader accepts `application/vnd.cyhdev.biome-tile` frames of at most 40,000 bytes. The decoded logical types and `ApiContractError` runtime under `src/generated/` are copied unchanged from the backend OpenAPI generator. `src/contracts.ts` is a handwritten export barrel, not a replacement schema. Update generated shapes in their backend owner and refresh this snapshot; do not edit the copied generated declarations by hand. `SOURCE_SNAPSHOT.json` records source paths and hashes before package-local import, transport, asset, and CSS adaptations.

## Provenance and host adapter

The implementation originated in [cyhdev](https://github.com/younghyun1/cyhdev), under `solid-csr-spa-template/src/components/minecraft`, its map styles and services, and their tests. The MIT notice is retained in [LICENSE](LICENSE). This browser package communicates with the sampler over HTTP and contains no Pumpkin source; the server-side sampler has its own licensing and third-party notices.

The complete application-specific snapshot belongs under [`adapters/cyhdev/frontend`](../../adapters/cyhdev/frontend). Its controllers and authenticated waypoint controls have not been turned into library dependencies. Original website sources remain unchanged by this extraction. The package's unit tests validate the extracted modules; original-site browser regressions are recorded separately and do not claim that the new package has been deployed.

## Browser transport benchmark

The opt-in Playwright benchmark is independently runnable in this package. Generate the real sampler corpus with the Rust exporter described in the repository protocol documentation, using `target/browser-tiles` at the repository root, then run `npx playwright install chromium` once and `npm run benchmark:browser`. Alternatively set `SEED_BENCHMARK_CORPUS` to a directory containing `manifest.json` and its generated tiles. The corpus is not committed. The default unit/build check does not run this benchmark. Set `SEED_BENCHMARK_SMOKE=1` for a short integration check with one preset and local delivery; it writes `browser-smoke.json` and preserves the complete measurement receipt.

The local fixture server compares binary with native HTTP gzip, raw PNG, and PNG with native HTTP gzip. It uses the real codecs, bounded cache, and Leaflet renderer on a 1024 by 512 viewport across all four presets, cold loads, pans, cached revisits, zooms, and local or constrained networking. The corpus and checked-in receipt use fixed Y=64; the historical timings do not measure Surface generation. It verifies exact hover availability and zero fixture misses. First and complete frames are requestAnimationFrame paint proxies in headless Chromium, not physical display timings. Decode values sum asynchronous elapsed time, not CPU time. Sampling and server encoding are excluded. The browser receives the full PNG hover metadata; no JavaScript image inflation occurs. The benchmark writes its JSON result into the corpus directory.
