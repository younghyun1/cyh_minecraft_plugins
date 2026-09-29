import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { createSeedTiles, seedTileKey, type SeedTileResponse } from "../src/components/minecraft/seedTiles";
import { createSeedTileLayer } from "../src/components/minecraft/seedTileLayer";
import { decodeSeedTileBinary } from "../src/components/minecraft/seedTileBinary";
import { decodeSeedTilePng } from "../src/components/minecraft/seedTilePng";
import type { MinecraftSeedTileQuery } from "../src/contracts";

export type SeedBenchmarkOptions = { endpoint: string; preset: string; world: string; wire: "binary" | "png" | "png-gzip" };
type StageResult = { stage: string; firstFrameMs: number; completeFrameMs: number; requests: number; wireBytes: number; decodeMs: number; drawMs: number; hoverMs: number; hoverHits: number; stalls: number; maxFrameMs: number };
declare global { interface Window { runSeedBenchmark: (options: SeedBenchmarkOptions) => Promise<StageResult[]> } }

window.runSeedBenchmark = async options => {
  Date.now = () => 1_800_000_000_001;
  const host = document.getElementById("map"); if (!host) throw new Error("Missing benchmark map");
  const map = L.map(host, { crs: L.CRS.Simple, attributionControl: false, zoomControl: false, zoomAnimation: false, fadeAnimation: false, minZoom: -2, maxZoom: 3 });
  map.setView([-32, 32], 3); map.createPane("minecraft-seeds");
  const ready = new Set<string>(), subscriptions = new Map<string, number>(), results: StageResult[] = [];
  let requests = 0, wireBytes = 0, decodeMs = 0, drawMs = 0;
  const read = async (query: MinecraftSeedTileQuery, signal: AbortSignal): Promise<SeedTileResponse> => {
    ++requests;
    const response = await fetch(options.endpoint, { method: "POST", headers: { "Content-Type": "application/json", "X-Benchmark-Preset": options.preset, "X-Benchmark-Wire": options.wire }, body: JSON.stringify(query), signal });
    if (!response.ok) throw new Error(`Fixture ${response.status}`);
    wireBytes += Number(response.headers.get("content-length"));
    const bytes = await response.arrayBuffer(), start = performance.now();
    if (options.wire === "binary" && new TextDecoder().decode(bytes.slice(0, 4)) !== "CYBM") throw new Error("Native HTTP decoding did not return CYBM");
    const tile = options.wire === "binary" ? decodeSeedTileBinary(bytes) : await decodeSeedTilePng(bytes, signal);
    decodeMs += performance.now() - start; return tile;
  };
  // Warm CORS permission outside the measured interval; every actual tile remains no-store.
  const warm = await read({ world: options.world, y: 64, level: 0, tile_x: 0, tile_z: 0 }, new AbortController().signal); warm.image?.close();
  requests = 0; wireBytes = 0; decodeMs = 0;
  const store = createSeedTiles({ read, changed: () => undefined, updated: () => undefined });
  store.configure(options.world, 64, true);
  const measuredStore: typeof store = { ...store, subscribe(query, listener, priority) {
    const key = seedTileKey(query); subscriptions.set(key, (subscriptions.get(key) ?? 0) + 1);
    const release = store.subscribe(query, tile => { const start = performance.now(); listener(tile); drawMs += performance.now() - start; if (tile) ready.add(key); else ready.delete(key); }, priority);
    return () => { release(); const count = (subscriptions.get(key) ?? 1) - 1; if (count === 0) { subscriptions.delete(key); ready.delete(key); } else subscriptions.set(key, count); };
  } };
  const layer = createSeedTileLayer({ map, maxZoom: 3, store: measuredStore, world: options.world, y: 64, changed: () => undefined });
  const visibleKeys = () => {
    const zoom = Math.min(3, Math.round(map.getZoom())), level = 3 - zoom, bounds = map.getPixelBounds(), keys: string[] = [];
    for (let z = Math.floor(bounds.min.y / 256); z <= Math.floor((bounds.max.y - 1) / 256); ++z) for (let x = Math.floor(bounds.min.x / 256); x <= Math.floor((bounds.max.x - 1) / 256); ++x) keys.push(seedTileKey({ world: options.world, y: 64, tile_x: x, tile_z: z, level }));
    return keys;
  };
  const stage = async (name: string, action: () => void) => {
    const initial = { requests, wireBytes, decodeMs, drawMs }, start = performance.now(); let first = 0, last = start, stalls = 0, maxFrame = 0;
    action();
    await new Promise<void>((resolve, reject) => {
      const frame = (timestamp: number) => {
        // A callback queued mid-frame may receive the older frame timestamp; wait for the next one.
        if (timestamp < start) { requestAnimationFrame(frame); return; }
        const now = performance.now();
        const delta = now - last; last = now; maxFrame = Math.max(maxFrame, delta); if (delta > 32) ++stalls;
        const keys = visibleKeys(), count = keys.filter(key => ready.has(key)).length;
        if (count > 0 && first === 0) first = now - start;
        if (count === keys.length && count > 0) resolve();
        else if (now - start > 10_000) reject(new Error(`Timed out ${name}: ${count}/${keys.length}`));
        else requestAnimationFrame(frame);
      }; requestAnimationFrame(frame);
    });
    const complete = performance.now() - start, hoverStart = performance.now(); let hits = 0;
    const bounds = map.getBounds();
    for (let z = 0; z < 16; ++z) for (let x = 0; x < 32; ++x) {
      const point = { x: Math.floor((bounds.getWest() + (x + 0.5) / 32 * (bounds.getEast() - bounds.getWest())) * 8), z: Math.floor(-(bounds.getNorth() + (z + 0.5) / 16 * (bounds.getSouth() - bounds.getNorth())) * 8) };
      if (store.sample(options.world, point, 64)) ++hits;
    }
    results.push({ stage: name, firstFrameMs: first, completeFrameMs: complete, requests: requests - initial.requests, wireBytes: wireBytes - initial.wireBytes, decodeMs: decodeMs - initial.decodeMs, drawMs: drawMs - initial.drawMs, hoverMs: performance.now() - hoverStart, hoverHits: hits, stalls, maxFrameMs: maxFrame });
    // Let the ordinary neighboring-ring prefetch finish before the next pan.
    const deadline = performance.now() + 4000;
    while (store.stats().running > 0 && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 5));
  };
  try {
    await stage("cold", () => layer.layer.addTo(map));
    await stage("pan", () => map.panTo([-32, 96], { animate: false }));
    await stage("revisit", () => map.panTo([-32, 32], { animate: false }));
    await stage("zoom-out", () => map.setZoom(2, { animate: false }));
    return results;
  } finally { layer.dispose(); store.dispose(); map.remove(); }
};
