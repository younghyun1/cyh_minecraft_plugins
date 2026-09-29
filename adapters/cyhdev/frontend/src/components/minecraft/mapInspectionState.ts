import { createSignal } from "solid-js";
import type { MinecraftMapData } from "../../generated";
import { type MapPoint, WORLD_LIMIT } from "./mapMath";
import type { SeedSample } from "./seedTiles";

export type MapInspection = {
  point: MapPoint;
  status: "loading" | "ready" | "unavailable";
  biome: { name: string; source: "observed" | "predicted"; sample: MapPoint; y: number | null; surfaceY: number | null; step?: number } | null;
  block: { name: string; ys: readonly number[] } | null;
  message: string;
};
type Job = { valid: () => boolean; run: () => Promise<void> };

/** One queued action per priority; pointer reads never disable explicit controls. */
export function createMapQueryGate(allowed: () => boolean, busy: (value: boolean) => void) {
  let running: "manual" | "hover" | "background" | null = null, manual: Job | null = null, hover: Job | null = null, background: Job | null = null;
  let timer: number | undefined, availableAt = 0;
  const pump = () => {
    window.clearTimeout(timer);
    if (running) return;
    if (manual && (!allowed() || !manual.valid())) { manual = null; busy(false); }
    if (hover && (!allowed() || !hover.valid())) hover = null;
    if (background && (!allowed() || !background.valid())) background = null;
    const job = manual ?? hover ?? background;
    if (!job) return;
    const wait = availableAt - Date.now();
    if (wait > 0) { timer = window.setTimeout(pump, wait); return; }
    running = manual ? "manual" : hover ? "hover" : "background";
    const priority = running;
    if (manual) manual = null; else if (hover) hover = null; else background = null;
    void job.run().finally(() => {
      running = null; availableAt = Date.now() + 1000;
      if (priority === "manual") busy(false);
      pump();
    });
  };
  return {
    manual(job: Job) {
      if (!allowed() || running === "manual" || manual) return false;
      manual = job; busy(true); pump(); return true;
    },
    hover(job: Job) { if (allowed()) { hover = job; pump(); } },
    background(job: Job) { if (allowed()) { background = job; pump(); } },
    clearBackground() { background = null; if (!manual && !hover) window.clearTimeout(timer); },
    clearHover() { hover = null; if (!manual && !background) window.clearTimeout(timer); },
    invalidate() { manual = null; hover = null; background = null; window.clearTimeout(timer); busy(running === "manual"); },
  };
}

type Context = {
  world: string | null; area: MinecraftMapData | null; areaSlice: number | null;
  matches: MinecraftMapData | null; matchedBlock: string; seed: (point: MapPoint) => SeedSample | null;
};
type Options = {
  allowed: () => boolean; context: () => Context;
  read: (world: string, chunkX: number, chunkZ: number) => Promise<MinecraftMapData>;
  schedule: (job: Job) => void; clearQueued: () => void;
};
type Cached = { received: number; data: MinecraftMapData | null };
const TTL = 30_000, CACHE_LIMIT = 64;
const identifier = (value: string) => /^[a-z0-9_.-]+:[a-z0-9/_.-]+$/.test(value) && value.length <= 128;
const chunkKey = (world: string, point: MapPoint) => `${world}:${Math.floor(point.x / 16)},${Math.floor(point.z / 16)}`;

/** Only a complete, correctly addressed chunk can become reusable hover evidence. */
export function validInspectionChunk(data: MinecraftMapData, world: string, x: number, z: number): boolean {
  if (data.kind !== "area" || data.world !== world || ![0, 1].includes(data.scanned_chunks)
    || data.missing_chunks !== 1 - data.scanned_chunks || data.cells.length !== data.scanned_chunks * 16) return false;
  const positions = new Set<string>();
  for (const cell of data.cells) {
    if (!Number.isInteger(cell.x) || !Number.isInteger(cell.z) || !Number.isInteger(cell.y)
      || cell.x < x * 16 || cell.x >= x * 16 + 16 || cell.z < z * 16 || cell.z >= z * 16 + 16
      || cell.x % 4 !== 0 || cell.z % 4 !== 0 || cell.y < -2032 || cell.y > 2031 || !identifier(cell.biome)) return false;
    positions.add(`${cell.x},${cell.z}`);
  }
  return positions.size === data.cells.length;
}

/** Read cached evidence immediately; only settled pointers request missing chunks. */
export function createMapInspection(options: Options) {
  const [inspection, setInspection] = createSignal<MapInspection | null>(null);
  const cache = new Map<string, Cached>();
  let target: MapPoint | null = null, timer: number | undefined, expiryTimer: number | undefined, epoch = 0;
  let ignoredArea: MinecraftMapData | null = null, ignoredMatches: MinecraftMapData | null = null;
  const fresh = (key: string) => {
    const value = cache.get(key);
    if (value && Date.now() - value.received < TTL) return value;
    cache.delete(key); return undefined;
  };
  const remember = (key: string, data: MinecraftMapData | null) => {
    cache.delete(key);
    if (cache.size >= CACHE_LIMIT) { const first = cache.keys().next(); if (!first.done) cache.delete(first.value); }
    cache.set(key, { received: Date.now(), data });
  };
  const refresh = () => {
    window.clearTimeout(timer); window.clearTimeout(expiryTimer);
    options.clearQueued();
    if (!target || !options.allowed()) { setInspection(null); return; }
    const point = target, context = options.context(), world = context.world;
    const result: MapInspection = { point, status: "loading", biome: null, block: null, message: "Loading biome…" };
    const area = context.area === ignoredArea ? null : context.area;
    const matches = context.matches === ignoredMatches ? null : context.matches;
    if (matches?.world === world && context.matchedBlock) {
      const ys = [...new Set(matches.matches.filter(cell => cell.x === point.x && cell.z === point.z).map(cell => cell.y))].sort((a, b) => a - b);
      if (ys.length) result.block = { name: context.matchedBlock, ys };
    }
    const key = world ? chunkKey(world, point) : "", saved = fresh(key);
    let fromCache = false;
    for (const [data, y] of [[area, context.areaSlice], [saved?.data ?? null, null]] as const) {
      const cell = data?.world === world ? data.cells.find(cell => Math.floor(point.x / 4) * 4 === cell.x && Math.floor(point.z / 4) * 4 === cell.z) : undefined;
      if (cell) { result.biome = { name: cell.biome, source: "observed", sample: { x: cell.x, z: cell.z }, y, surfaceY: cell.y }; fromCache = data === saved?.data; break; }
    }
    if (!result.biome) {
      const cell = context.seed(point);
      const observedChunk = [area, matches, saved?.data].some(data => data?.world === world && [...data.cells, ...data.matches].some(sample => Math.floor(sample.x / 16) === Math.floor(point.x / 16) && Math.floor(sample.z / 16) === Math.floor(point.z / 16)));
      if (cell && cell.expires > Date.now() && !observedChunk) {
        result.biome = { name: cell.name, source: "predicted", sample: cell.sample, y: cell.y, surfaceY: null, step: cell.step };
        expiryTimer = window.setTimeout(refresh, cell.expires - Date.now());
      }
    }
    if (saved && (fromCache || !result.biome)) expiryTimer = window.setTimeout(() => {
      if (target === point) setInspection({ ...result, status: "unavailable", biome: null, message: "Biome sample expired. Move or tap to refresh." });
    }, Math.max(0, TTL - (Date.now() - saved.received)));
    if (result.biome) { result.status = "ready"; result.message = ""; setInspection(result); return; }
    const x = Math.floor(point.x / 16), z = Math.floor(point.z / 16);
    if (!world || saved || x * 16 + 15 > WORLD_LIMIT || z * 16 + 15 > WORLD_LIMIT) {
      result.status = "unavailable"; result.message = saved?.data?.missing_chunks === 1 ? "No generated terrain data" : "Biome data is unavailable here."; setInspection(result); return;
    }
    setInspection(result);
    const revision = epoch;
    timer = window.setTimeout(() => options.schedule({
      valid: () => revision === epoch && target !== null && options.allowed() && options.context().world === world && chunkKey(world, target) === key,
      run: async () => {
        let data: MinecraftMapData | null = null;
        try {
          const response = await options.read(world, x, z);
          if (validInspectionChunk(response, world, x, z)) data = { ...response, structures: [], matches: [], worlds: [], blocks: [] };
        } catch { /* A bounded negative cache prevents repeated failures under a stationary pointer. */ }
        if (revision !== epoch) return;
        remember(key, data); refresh();
      },
    }), 500);
  };
  return {
    inspection, refresh,
    inspect(point: MapPoint | null) {
      if (point && (!Number.isFinite(point.x) || !Number.isFinite(point.z) || Math.abs(point.x) > WORLD_LIMIT || Math.abs(point.z) > WORLD_LIMIT)) point = null;
      const next = point ? { x: Math.floor(point.x), z: Math.floor(point.z) } : null;
      if (next && target && next.x === target.x && next.z === target.z && inspection()?.status !== "unavailable") return;
      target = next; options.clearQueued(); refresh();
    },
    invalidate() {
      ++epoch; target = null; cache.clear(); window.clearTimeout(timer); window.clearTimeout(expiryTimer); options.clearQueued(); setInspection(null);
      const context = options.context(); ignoredArea = context.area; ignoredMatches = context.matches;
    },
  };
}
