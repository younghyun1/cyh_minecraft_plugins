import type { MinecraftSeedTile, MinecraftSeedTileQuery } from "../../contracts";
import { ApiContractError } from "../../contracts";
import type { MapPoint } from "./mapMath";

export const SEED_TILE_SIZE = 64, SEED_MAX_LEVEL = 12, SEED_MAX_TILES = 256;
const CACHE_BYTES = 16 * 1024 * 1024, CACHE_ENTRIES = 1024, MAX_REQUESTS = 4, EMPTY = 65535;
export type SeedSample = { name: string; sample: MapPoint; y: number; step: number; expires: number };
export type SeedTileResponse = MinecraftSeedTile & { readonly image?: ImageBitmap };
export type SeedTile = Omit<SeedTileResponse, "indices"> & { readonly indices: Uint16Array; readonly cost: number };
type Listener = { draw: (tile: SeedTile | null) => void; priority: () => number };
type Target = { query: MinecraftSeedTileQuery; listeners: Map<symbol, Listener>; retryAt: number; attempts: number; drawn: SeedTile | null };
type Options = { read: (query: MinecraftSeedTileQuery, signal: AbortSignal) => Promise<SeedTileResponse>; changed: (state: { loading: boolean; notice: string; preset: MinecraftSeedTile["preset"] | null }) => void; updated: () => void };
export const seedTileKey = (query: MinecraftSeedTileQuery) => `${query.world}:${query.y}:${query.level}:${query.tile_x}:${query.tile_z}`;
export const seedTileOrigin = (query: Pick<MinecraftSeedTileQuery, "level" | "tile_x" | "tile_z">) => ({ x: query.tile_x * (256 * 2 ** query.level), z: query.tile_z * (256 * 2 ** query.level) });

/** Network shapes are checked before palette offsets reach a canvas or hover lookup. */
export function decodeSeedTile(data: SeedTileResponse, query: MinecraftSeedTileQuery, now = Date.now()): SeedTile | null {
  const origin = seedTileOrigin(query);
  if (data.world !== query.world || data.tile_x !== query.tile_x || data.tile_z !== query.tile_z || data.level !== query.level || data.y !== query.y
    || data.min_x !== origin.x || data.min_z !== origin.z || data.step !== 4 * 2 ** query.level || data.width !== 64 || data.height !== 64
    || !Number.isInteger(query.level) || query.level < 0 || query.level > SEED_MAX_LEVEL || !Number.isInteger(query.tile_x) || !Number.isInteger(query.tile_z)
    || !Number.isFinite(data.expires_at_ms) || data.expires_at_ms <= now || data.expires_at_ms > now + 15_000
    || !Number.isFinite(data.sampled_at_ms) || data.sampled_at_ms > now + 1000 || data.sampled_at_ms < now - 15_000 || data.sampled_at_ms >= data.expires_at_ms || data.expires_at_ms > data.sampled_at_ms + 15_000
    || !/^[a-zA-Z0-9_-]{16,128}$/.test(data.profile_epoch) || !["default", "large_biomes", "nether", "end"].includes(data.preset)
    || data.palette.length > 256 || data.palette.some(name => !/^[a-z0-9_.-]+:[a-z0-9/_.-]+$/.test(name) || name.length > 128) || data.indices.length !== 4096) return null;
  const indices = new Uint16Array(4096);
  for (let i = 0; i < indices.length; ++i) {
    const value = data.indices[i];
    if (value !== null && (!Number.isInteger(value) || value === undefined || value < 0 || value >= data.palette.length)) return null;
    indices[i] = value ?? EMPTY;
  }
  // Reserve two RGBA buffers for native image and GPU storage within the same cache budget.
  const cost = indices.byteLength + data.palette.reduce((bytes, name) => bytes + name.length * 2 + 32, 0) + 2048 + (data.image ? 64 * 64 * 8 : 0);
  return { ...data, indices, cost };
}

/** Biomes are immutable within a profile; permission-bearing responses expire independently. */
export function createSeedTiles(options: Options) {
  const cache = new Map<string, SeedTile>(), targets = new Map<string, Target>(), running = new Map<string, AbortController>();
  let bytes = 0, disposed = false, enabled = false, context = "", generation = 0, profile = "", profileTime = 0, notice = "";
  let expiryTimer: number | undefined;
  let preset: MinecraftSeedTile["preset"] | null = null;
  const report = () => options.changed({ loading: running.size > 0, notice, preset });
  const drop = (key: string) => { const old = cache.get(key); if (old) { bytes -= old.cost; old.image?.close(); cache.delete(key); } };
  const notify = (key: string, tile: SeedTile | null) => {
    const target = targets.get(key);
    if (target && target.drawn !== tile) { target.drawn = tile; for (const listener of target.listeners.values()) listener.draw(tile); }
  };
  const clear = () => {
    ++generation;
    window.clearTimeout(expiryTimer); expiryTimer = undefined;
    for (const request of running.values()) request.abort();
    // Aborted native decodes retain their slots until settlement; rapid switches cannot exceed four.
    for (const tile of cache.values()) tile.image?.close(); cache.clear(); bytes = 0; profile = ""; profileTime = 0; preset = null; notice = "";
    for (const [key, target] of targets) { target.retryAt = 0; target.attempts = 0; notify(key, null); }
    options.updated(); report();
  };
  const remember = (key: string, tile: SeedTile) => {
    drop(key);
    while (cache.size >= CACHE_ENTRIES || bytes + tile.cost > CACHE_BYTES) {
      const oldest = cache.keys().next(); if (oldest.done) break;
      notify(oldest.value, null); drop(oldest.value);
    }
    cache.set(key, tile); bytes += tile.cost;
    scheduleExpiry();
  };
  const scheduleExpiry = () => {
    window.clearTimeout(expiryTimer);
    const deadline = Math.min(...[...cache.values()].map(tile => tile.expires_at_ms));
    expiryTimer = Number.isFinite(deadline) ? window.setTimeout(pump, Math.max(0, deadline - Date.now())) : undefined;
  };
  const pump = () => {
    if (disposed || !enabled) return;
    const now = Date.now();
    for (const [key, tile] of cache) if (tile.expires_at_ms <= now) { drop(key); notify(key, null); options.updated(); }
    scheduleExpiry();
    const queue = [...targets.entries()].filter(([key, target]) => `${target.query.world}:${target.query.y}` === context && target.listeners.size > 0 && !running.has(key) && target.retryAt <= now && (cache.get(key)?.expires_at_ms ?? 0) - now <= 3000)
      .sort((a, b) => Math.min(...[...a[1].listeners.values()].map(listener => listener.priority())) - Math.min(...[...b[1].listeners.values()].map(listener => listener.priority()))).slice(0, 128);
    for (const [key, target] of queue) {
      if (running.size >= MAX_REQUESTS) break;
      const request = new AbortController(), revision = generation;
      const deadline = window.setTimeout(() => request.abort(), 10_000);
      running.set(key, request); report();
      void options.read(target.query, request.signal).then(response => {
        if (disposed || revision !== generation || request.signal.aborted || !targets.has(key)) { response.image?.close(); return; }
        const tile = decodeSeedTile(response, target.query);
        if (!tile) { response.image?.close(); throw new Error("Invalid or expired seed tile."); }
        if (profile && tile.profile_epoch !== profile) {
          if (tile.sampled_at_ms <= profileTime) { tile.image?.close(); throw new Error("Seed profile changed."); }
          // Invalidate all older-profile tiles and requests before publishing the replacement.
          clear();
        }
        profile = tile.profile_epoch; profileTime = Math.max(profileTime, tile.sampled_at_ms); preset = tile.preset;
        remember(key, tile); target.attempts = 0; target.retryAt = 0; notice = "";
        notify(key, tile); options.updated();
      }).catch((cause: unknown) => {
        if (disposed || revision !== generation || !targets.has(key)) return;
        const permanent = cause instanceof ApiContractError && cause.status >= 400 && cause.status < 500 && ![408, 429].includes(cause.status);
        target.retryAt = permanent ? Infinity : Date.now() + Math.min(30_000, 2000 * 2 ** Math.min(target.attempts++, 4));
        notice = "Seed map is temporarily unavailable.";
      }).finally(() => {
        window.clearTimeout(deadline);
        if (running.get(key) === request) running.delete(key);
        if (!disposed) { report(); queueMicrotask(pump); }
      });
    }
  };
  const timer = window.setInterval(pump, 250);
  return {
    configure(world: string | null, y: number, visible: boolean) {
      const next = `${world}:${y}`;
      if (context !== next || enabled !== visible) { context = next; enabled = visible && world !== null; clear(); }
      pump();
    },
    subscribe(query: MinecraftSeedTileQuery, listener: (tile: SeedTile | null) => void, priority: () => number) {
      const key = seedTileKey(query), token = Symbol();
      let target = targets.get(key);
      if (!target) {
        if (targets.size >= SEED_MAX_TILES) { listener(null); return () => undefined; }
        target = { query, listeners: new Map(), retryAt: 0, attempts: 0, drawn: null }; targets.set(key, target);
      }
      target.listeners.set(token, { draw: listener, priority });
      const tile = cache.get(key);
      if (tile && tile.expires_at_ms > Date.now()) { cache.delete(key); cache.set(key, tile); target.drawn = tile; listener(tile); } else listener(null);
      pump();
      return () => {
        const current = targets.get(key); current?.listeners.delete(token);
        if (current?.listeners.size === 0) { targets.delete(key); running.get(key)?.abort(); }
      };
    },
    sample(world: string, point: MapPoint, y: number): SeedSample | null {
      if (!enabled) return null;
      for (let level = 0; level <= SEED_MAX_LEVEL; ++level) {
        const span = 256 * 2 ** level;
        const tile = cache.get(seedTileKey({ world, y, level, tile_x: Math.floor(point.x / span), tile_z: Math.floor(point.z / span) }));
        if (!tile || tile.expires_at_ms <= Date.now()) continue;
        const x = Math.floor((point.x - tile.min_x) / tile.step), z = Math.floor((point.z - tile.min_z) / tile.step);
        const index = tile.indices[z * 64 + x], name = index === undefined || index === EMPTY ? undefined : tile.palette[index];
        if (!name) return null;
        return { name, sample: { x: tile.min_x + x * tile.step, z: tile.min_z + z * tile.step }, y, step: tile.step, expires: tile.expires_at_ms };
      }
      return null;
    },
    refresh() { for (const target of targets.values()) { target.retryAt = 0; target.attempts = 0; } pump(); },
    invalidate() { clear(); pump(); },
    dispose() { disposed = true; window.clearInterval(timer); clear(); targets.clear(); },
    stats: () => ({ bytes, entries: cache.size, active: targets.size, running: running.size }),
  };
}

export type SeedTiles = ReturnType<typeof createSeedTiles>;
