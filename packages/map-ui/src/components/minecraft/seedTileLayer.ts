import L from "leaflet";
import type { MinecraftSeedTileQuery } from "../../contracts";
import { biomeColor, WORLD_LIMIT, type MapPoint } from "./mapMath";
import { SEED_MAX_LEVEL, SEED_MAX_TILES, type SeedTiles, type SeedTile } from "./seedTiles";

type Options = { map: L.Map; maxZoom: number; store: SeedTiles; world: string; y: number | null; changed: () => void };
export const MAX_TERRAIN_TILES = 256;
/** Native squaremap tiles stop at zero; bound the extra PNG fan-out at overview zooms. */
export function minimumMapZoom(width: number, height: number, maxZoom: number): number {
  let zoom = Math.max(-2, maxZoom - SEED_MAX_LEVEL);
  while (zoom < 0 && (Math.ceil(width / (512 * 2 ** zoom)) + 3) * (Math.ceil(height / (512 * 2 ** zoom)) + 3) > MAX_TERRAIN_TILES) ++zoom;
  return zoom;
}
export const nativeTerrainZoom = (zoom: number, maxZoom: number) => Math.max(0, Math.min(maxZoom, Math.round(zoom)));

/** Small backing canvases preserve a fixed sample budget while Leaflet handles smooth transforms. */
export function createSeedTileLayer(options: Options): { layer: L.GridLayer; dispose: () => void } {
  const releases = new Map<HTMLElement, () => void>();
  let neighbors: (() => void)[] = [];
  class SeedLayer extends L.GridLayer {
    override createTile(coords: L.Coords, done: L.DoneCallback): HTMLElement {
      const canvas = document.createElement("canvas"); canvas.width = 64; canvas.height = 64; canvas.className = "minecraft-seed-tile";
      const level = options.maxZoom - coords.z;
      const query: MinecraftSeedTileQuery = { world: options.world, y: options.y, tile_x: coords.x, tile_z: coords.y, level };
      const span = 256 * 2 ** level;
      if (releases.size >= SEED_MAX_TILES || coords.x * span >= WORLD_LIMIT || (coords.x + 1) * span <= -WORLD_LIMIT || coords.y * span >= WORLD_LIMIT || (coords.y + 1) * span <= -WORLD_LIMIT) {
        queueMicrotask(() => done(undefined, canvas)); return canvas;
      }
      const context = canvas.getContext("2d");
      let completed = false;
      const complete = () => { if (!completed) { completed = true; queueMicrotask(() => done(undefined, canvas)); } };
      // Failed requests must release Leaflet's old zoom levels even when retries continue.
      const deadline = window.setTimeout(complete, 12_000);
      const draw = (tile: SeedTile | null) => {
        context?.clearRect(0, 0, 64, 64);
        canvas.dataset.ready = tile ? "true" : "false";
        if (context && tile?.image) context.drawImage(tile.image, 0, 0);
        else if (context && tile) {
          const colors = tile.palette.map(biomeColor);
          for (let y = 0; y < 64; ++y) for (let x = 0; x < 64;) {
            const index = tile.indices[y * 64 + x] ?? 65535, color = colors[index];
            let end = x + 1;
            while (end < 64 && tile.indices[y * 64 + end] === index) ++end;
            if (color) { context.fillStyle = color; context.fillRect(x, y, end - x, 1); }
            x = end;
          }
        }
        if (tile) { window.clearTimeout(deadline); complete(); }
        options.changed();
      };
      const unsubscribe = options.store.subscribe(query, draw, () => {
        const center = options.map.project(options.map.getCenter(), coords.z).divideBy(256);
        return Math.hypot(coords.x + 0.5 - center.x, coords.y + 0.5 - center.y);
      });
      releases.set(canvas, () => { window.clearTimeout(deadline); unsubscribe(); });
      return canvas;
    }
  }
  const layer = new SeedLayer({ pane: "minecraft-seeds", tileSize: 256, minZoom: options.maxZoom - SEED_MAX_LEVEL, minNativeZoom: options.maxZoom - SEED_MAX_LEVEL, maxNativeZoom: options.maxZoom, noWrap: true, keepBuffer: 1, updateWhenIdle: false, updateInterval: 150 });
  layer.on("tileunload", (event: L.TileEvent) => { releases.get(event.tile)?.(); releases.delete(event.tile); });
  const prefetch = () => {
    for (const release of neighbors) release(); neighbors = [];
    const zoom = Math.max(options.maxZoom - SEED_MAX_LEVEL, Math.min(options.maxZoom, Math.round(options.map.getZoom()))), level = options.maxZoom - zoom;
    const bounds = options.map.getBounds(), min = options.map.project(bounds.getNorthWest(), zoom), max = options.map.project(bounds.getSouthEast(), zoom);
    const left = Math.floor(min.x / 256), top = Math.floor(min.y / 256), right = Math.floor((max.x - 1) / 256), bottom = Math.floor((max.y - 1) / 256), span = 256 * 2 ** level;
    for (let y = top - 1; y <= bottom + 1; ++y) for (let x = left - 1; x <= right + 1; ++x) {
      if (neighbors.length >= 64 || x >= left && x <= right && y >= top && y <= bottom || x * span >= WORLD_LIMIT || (x + 1) * span <= -WORLD_LIMIT || y * span >= WORLD_LIMIT || (y + 1) * span <= -WORLD_LIMIT) continue;
      neighbors.push(options.store.subscribe({ world: options.world, y: options.y, level, tile_x: x, tile_z: y }, () => undefined, () => 1_000_000 + Math.hypot(x - (left + right) / 2, y - (top + bottom) / 2)));
    }
  };
  options.map.on("moveend", prefetch); layer.on("add", prefetch);
  return { layer, dispose: () => { options.map.off("moveend", prefetch); for (const release of neighbors) release(); neighbors = []; layer.remove(); for (const release of releases.values()) release(); releases.clear(); } };
}

/** A 64-pixel overlay reuses the seed store's tile requests and exact biome indices. */
export function createBiomeHighlightLayer(options: Omit<Options, "changed">, biomes: readonly string[]): { layer: L.GridLayer; setBiomes: (values: readonly string[]) => void; dispose: () => void } {
  const active = new Map<HTMLCanvasElement, { redraw: () => void; unsubscribe: () => void }>();
  let selected = new Set(biomes);
  class HighlightLayer extends L.GridLayer {
    override createTile(coords: L.Coords): HTMLElement {
      const canvas = document.createElement("canvas"); canvas.width = 64; canvas.height = 64; canvas.className = "minecraft-biome-highlight-tile";
      canvas.dataset.tileX = String(coords.x); canvas.dataset.tileZ = String(coords.y); canvas.dataset.level = String(options.maxZoom - coords.z);
      const level = options.maxZoom - coords.z, span = 256 * 2 ** level;
      if (active.size >= SEED_MAX_TILES || coords.x * span >= WORLD_LIMIT || (coords.x + 1) * span <= -WORLD_LIMIT || coords.y * span >= WORLD_LIMIT || (coords.y + 1) * span <= -WORLD_LIMIT) return canvas;
      const context = canvas.getContext("2d");
      let lastTile: SeedTile | null = null;
      const redraw = () => {
        context?.clearRect(0, 0, 64, 64);
        if (!context || !lastTile) return;
        const matches = lastTile.palette.map(name => selected.has(name));
        context.fillStyle = "rgba(255, 232, 96, 0.42)";
        for (let y = 0; y < 64; ++y) for (let x = 0; x < 64;) {
          const index = lastTile.indices[y * 64 + x] ?? 65535, match = matches[index] === true;
          let end = x + 1;
          while (end < 64 && (matches[lastTile.indices[y * 64 + end] ?? 65535] === true) === match) ++end;
          if (match) context.fillRect(x, y, end - x, 1);
          x = end;
        }
      };
      const query: MinecraftSeedTileQuery = { world: options.world, y: options.y, tile_x: coords.x, tile_z: coords.y, level };
      const unsubscribe = options.store.subscribe(query, tile => { lastTile = tile; canvas.dataset.ready = tile ? "true" : "false"; redraw(); }, () => {
        const center = options.map.project(options.map.getCenter(), coords.z).divideBy(256);
        return Math.hypot(coords.x + 0.5 - center.x, coords.y + 0.5 - center.y);
      });
      active.set(canvas, { redraw, unsubscribe });
      return canvas;
    }
  }
  const layer = new HighlightLayer({ pane: "minecraft-biome-highlights", tileSize: 256, minZoom: options.maxZoom - SEED_MAX_LEVEL, minNativeZoom: options.maxZoom - SEED_MAX_LEVEL, maxNativeZoom: options.maxZoom, noWrap: true, keepBuffer: 1, updateWhenIdle: false, updateInterval: 150 });
  layer.on("tileunload", (event: L.TileEvent) => {
    if (!(event.tile instanceof HTMLCanvasElement)) return;
    active.get(event.tile)?.unsubscribe(); active.delete(event.tile);
  });
  return {
    layer,
    setBiomes(values: readonly string[]) { selected = new Set(values); for (const tile of active.values()) tile.redraw(); },
    dispose() { layer.remove(); for (const tile of active.values()) tile.unsubscribe(); active.clear(); },
  };
}

type AlphaTile = { coords: L.Coords; alpha: Uint8Array; edges: Uint32Array };
const ALPHA_SIDE = 256, MAX_TILE_EDGES = 4096, MAX_DRAW_EDGES = 8192;
const DIRECTIONS = [[-1, 0], [1, 0], [0, -1], [0, 1]] as const;
export const terrainKey = (x: number, y: number, z: number) => `${z}:${x}:${y}`;
export const packedAlphaAt = (alpha: Uint8Array, index: number) => ((alpha[Math.floor(index / 8)] ?? 0) & 1 << index % 8) === 0 ? 0 : 255;
export function decodeAlphaEdge(value: number, side = ALPHA_SIDE) {
  const index = Math.floor(value / 4), direction = DIRECTIONS[value % 4] ?? DIRECTIONS[0];
  return { x: index % side, y: Math.floor(index / side), dx: direction[0], dy: direction[1] };
}

/** Packed geometry is bounded before allocation; unknown neighbors never create tile seams. */
export function alphaBoundary(alpha: Uint8Array, neighbor: (x: number, y: number) => number | null, allowed: (x: number, y: number) => boolean, side = ALPHA_SIDE, packed = false): Uint32Array {
  if (alpha.every(value => packed ? value === 0 : value < 128)) return new Uint32Array();
  const opaque = alpha.every(value => packed ? value === 255 : value >= 128), edges = new Uint32Array(MAX_TILE_EDGES);
  const at = (index: number) => packed ? packedAlphaAt(alpha, index) : alpha[index] ?? 0;
  let count = 0;
  outer: for (let y = 0; y < side; ++y) for (let x = 0; x < side; ++x) {
    if (at(y * side + x) < 128 || opaque && x > 0 && y > 0 && x < side - 1 && y < side - 1) continue;
    for (let direction = 0; direction < DIRECTIONS.length; ++direction) {
      const [dx, dy] = DIRECTIONS[direction] ?? DIRECTIONS[0], nx = x + dx, ny = y + dy;
      const value = nx >= 0 && ny >= 0 && nx < side && ny < side ? at(ny * side + nx) : neighbor(nx, ny);
      if (value !== null && value !== undefined && value < 128 && allowed(nx, ny)) {
        edges[count++] = (y * side + x) * 4 + direction;
        if (count === MAX_TILE_EDGES) break outer;
      }
    }
  }
  return edges.slice(0, count);
}

/** PNG alpha is rendering evidence only; the seed store separately enforces server permissions. */
export function createTerrainBoundary(map: L.Map, tiles: L.TileLayer, maxZoom: number, predicted: (point: MapPoint) => boolean, changed: () => void) {
  const alpha = new Map<string, AlphaTile>();
  const renderer = L.canvas({ pane: "minecraft-frontier", padding: 0.1 }), lines = L.layerGroup().addTo(map);
  const pane = map.getPane("minecraft-frontier");
  let enabled = false, zooming = false, frame: number | undefined, revealFrame: number | undefined;
  const samplePoint = (coords: L.Coords, x: number, y: number): MapPoint => {
    const step = 2 ** (maxZoom - coords.z + 1);
    return { x: Math.floor((coords.x * ALPHA_SIDE + x + 0.5) * step), z: Math.floor((coords.y * ALPHA_SIDE + y + 0.5) * step) };
  };
  const rebuild = (coords: L.Coords) => {
    for (const [dx, dy] of [[0, 0], ...DIRECTIONS]) {
      const tile = alpha.get(terrainKey(coords.x + (dx ?? 0), coords.y + (dy ?? 0), coords.z));
      if (!tile) continue;
      tile.edges = alphaBoundary(tile.alpha, (x, y) => {
        const other = alpha.get(terrainKey(tile.coords.x + Math.floor(x / ALPHA_SIDE), tile.coords.y + Math.floor(y / ALPHA_SIDE), tile.coords.z));
        return other ? packedAlphaAt(other.alpha, ((y + ALPHA_SIDE) % ALPHA_SIDE) * ALPHA_SIDE + (x + ALPHA_SIDE) % ALPHA_SIDE) : null;
      }, () => true, ALPHA_SIDE, true);
    }
  };
  const draw = () => {
    frame = undefined;
    if (zooming) return;
    lines.clearLayers();
    // Leaflet redraws the replacement canvas on its next frame; reveal after its redraw.
    const reveal = () => {
      if (pane?.classList.contains("minecraft-frontier-zooming") && revealFrame === undefined) revealFrame = requestAnimationFrame(() => {
        revealFrame = undefined; if (!zooming) pane.classList.remove("minecraft-frontier-zooming");
      });
    };
    if (!enabled) { reveal(); return; }
    const zoom = nativeTerrainZoom(map.getZoom(), maxZoom);
    let remaining = MAX_DRAW_EDGES;
    for (const tile of alpha.values()) {
      if (tile.coords.z !== zoom || remaining <= 0) continue;
      const scale = 2 ** maxZoom, step = 2 ** (maxZoom - tile.coords.z + 1);
      const position = (x: number, y: number): L.LatLngTuple => [-(tile.coords.y * ALPHA_SIDE + y) * step / scale, (tile.coords.x * ALPHA_SIDE + x) * step / scale];
      const segments: L.LatLngTuple[][] = [];
      for (const packed of tile.edges) {
        const edge = decodeAlphaEdge(packed);
        if (!predicted(samplePoint(tile.coords, edge.x + edge.dx, edge.y + edge.dy))) continue;
        const x = edge.x + (edge.dx > 0 ? 1 : 0), y = edge.y + (edge.dy > 0 ? 1 : 0);
        segments.push([position(x, y), position(x + (edge.dx === 0 ? 1 : 0), y + (edge.dy === 0 ? 1 : 0))]);
        if (--remaining <= 0) break;
      }
      if (segments.length) L.polyline(segments, { renderer, pane: "minecraft-frontier", color: "#11150f", weight: 1, opacity: 1, interactive: false, lineCap: "square" }).addTo(lines);
    }
    reveal();
  };
  const refresh = () => { if (frame === undefined) frame = requestAnimationFrame(draw); };
  const zoomStart = () => {
    zooming = true; pane?.classList.add("minecraft-frontier-zooming");
    if (revealFrame !== undefined) { cancelAnimationFrame(revealFrame); revealFrame = undefined; }
  };
  const zoomEnd = () => { zooming = false; refresh(); };
  const loaded = (event: L.TileEvent) => {
    const key = terrainKey(event.coords.x, event.coords.y, event.coords.z);
    const canvas = document.createElement("canvas"); canvas.width = ALPHA_SIDE; canvas.height = ALPHA_SIDE;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context || !(event.tile instanceof HTMLImageElement)) return;
    try {
      context.drawImage(event.tile, 0, 0, ALPHA_SIDE, ALPHA_SIDE);
      const pixels = context.getImageData(0, 0, ALPHA_SIDE, ALPHA_SIDE).data, mask = new Uint8Array(ALPHA_SIDE * ALPHA_SIDE / 8);
      for (let i = 0; i < ALPHA_SIDE * ALPHA_SIDE; ++i) if ((pixels[i * 4 + 3] ?? 0) >= 128) mask[Math.floor(i / 8)] = (mask[Math.floor(i / 8)] ?? 0) | 1 << i % 8;
      if (alpha.size >= MAX_TERRAIN_TILES && !alpha.has(key)) {
        const oldest = alpha.keys().next();
        if (!oldest.done) { const removed = alpha.get(oldest.value); alpha.delete(oldest.value); if (removed) rebuild(removed.coords); }
      }
      alpha.set(key, { coords: event.coords, alpha: mask, edges: new Uint32Array() }); rebuild(event.coords); refresh(); changed();
    } catch { /* Unreadable imagery cannot establish an observed/predicted boundary. */ }
  };
  const unloaded = (event: L.TileEvent) => { alpha.delete(terrainKey(event.coords.x, event.coords.y, event.coords.z)); rebuild(event.coords); refresh(); };
  tiles.on("tileload", loaded).on("tileunload", unloaded); map.on("zoomstart", zoomStart).on("zoomend", zoomEnd);
  return {
    refresh,
    setEnabled(value: boolean) { enabled = value; refresh(); },
    rendered(point: MapPoint) {
      const zoom = nativeTerrainZoom(map.getZoom(), maxZoom), span = 512 * 2 ** (maxZoom - zoom);
      const x = Math.floor(point.x / span), y = Math.floor(point.z / span), tile = alpha.get(terrainKey(x, y, zoom));
      if (!tile) return false;
      const px = Math.floor((point.x - x * span) / span * ALPHA_SIDE), py = Math.floor((point.z - y * span) / span * ALPHA_SIDE);
      return packedAlphaAt(tile.alpha, py * ALPHA_SIDE + px) >= 128;
    },
    dispose() { if (frame !== undefined) cancelAnimationFrame(frame); if (revealFrame !== undefined) cancelAnimationFrame(revealFrame); tiles.off("tileload", loaded).off("tileunload", unloaded); map.off("zoomstart", zoomStart).off("zoomend", zoomEnd); pane?.classList.remove("minecraft-frontier-zooming"); lines.remove(); renderer.remove(); alpha.clear(); },
  };
}
