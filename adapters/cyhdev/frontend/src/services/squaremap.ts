import type { MapPoint } from "../components/minecraft/mapMath";

export type SquaremapWorld = { readonly name: string; readonly displayName: string; readonly type: string };
export type SquaremapPlayerTracker = { readonly enabled: boolean; readonly nameplates: boolean; readonly heads: boolean; readonly health: boolean; readonly armor: boolean };
export type SquaremapSettings = { readonly maxZoom: number; readonly defaultZoom: number; readonly extraZoom: number; readonly spawn: MapPoint; readonly playerTracker: SquaremapPlayerTracker };
export type SquaremapPlayer = MapPoint & { readonly name: string; readonly world: string; readonly uuid: string | null; readonly health: number | null; readonly armor: number | null };
const MAX_JSON_BYTES = 1_048_576;
const WORLD_NAMES = new Map([["minecraft:overworld", "Overworld"], ["minecraft:the_nether", "Nether"], ["minecraft:the_end", "The End"]]);

function record(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid map metadata.");
  return value as Record<string, unknown>;
}
function text(value: unknown, limit: number): string {
  if (typeof value !== "string" || value.length > limit) throw new Error("Invalid map metadata text.");
  return value;
}
function number(value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < min || value > max) throw new Error("Invalid map metadata coordinates.");
  return value;
}
function flag(value: unknown, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") throw new Error("Invalid map player settings.");
  return value;
}
function playerUuid(value: unknown): string | null {
  if (value === undefined) return null;
  const uuid = text(value, 36);
  if (!/^(?:[a-f\d]{32}|[a-f\d]{8}-[a-f\d]{4}-[a-f\d]{4}-[a-f\d]{4}-[a-f\d]{12})$/i.test(uuid)) throw new Error("Invalid map player identifier.");
  return uuid.replaceAll("-", "").toLowerCase();
}
function playerStat(value: unknown): number | null {
  return value === undefined ? null : Math.floor(Math.min(20, number(value, 0, 2048)));
}
export function mapId(value: unknown): string {
  const name = text(value, 128);
  if (!/^[a-zA-Z0-9_-][a-zA-Z0-9_.-]*$/.test(name) || name.includes("..")) throw new Error("Invalid map world path.");
  return name;
}
export function parseWorlds(value: unknown): SquaremapWorld[] {
  const worlds = record(value).worlds;
  if (!Array.isArray(worlds) || worlds.length > 64) throw new Error("Invalid map world list.");
  return worlds.map((entry: unknown) => {
    const world = record(entry);
    const name = mapId(world.name), displayName = text(world.display_name, 160);
    return { name, displayName: WORLD_NAMES.get(displayName) ?? displayName, type: text(world.type, 64) };
  });
}
export function parseSettings(value: unknown): SquaremapSettings {
  const settings = record(value), zoom = record(settings.zoom), spawn = record(settings.spawn);
  const maxZoom = number(zoom.max, 0, 12), extraZoom = number(zoom.extra, 0, 8);
  const defaultZoom = number(zoom.def, 0, maxZoom + extraZoom);
  if (![maxZoom, extraZoom, defaultZoom].every(Number.isInteger)) throw new Error("Invalid map zoom.");
  const tracker = settings.player_tracker === undefined ? {} : record(settings.player_tracker);
  const plates = tracker.nameplates === undefined ? {} : record(tracker.nameplates);
  return { maxZoom, defaultZoom, extraZoom, spawn: { x: number(spawn.x, -30_000_000, 30_000_000), z: number(spawn.z, -30_000_000, 30_000_000) }, playerTracker: {
    enabled: flag(tracker.enabled, true), nameplates: flag(plates.enabled, true), heads: flag(plates.show_heads, true), health: flag(plates.show_health, true), armor: flag(plates.show_armor, true),
  } };
}
export function parsePlayers(value: unknown): SquaremapPlayer[] {
  const players = record(value).players;
  if (!Array.isArray(players) || players.length > 1000) throw new Error("Invalid map player list.");
  return players.flatMap((entry: unknown) => {
    const player = record(entry);
    if (player.x === undefined || player.z === undefined) return [];
    return [{ name: text(player.name, 80), world: mapId(player.world), uuid: playerUuid(player.uuid), health: playerStat(player.health), armor: playerStat(player.armor), x: number(player.x, -30_000_000, 30_000_000), z: number(player.z, -30_000_000, 30_000_000) }];
  });
}

/** Server-generated map JSON is untrusted; bound the stream before parsing it. */
async function readJson(path: string, signal: AbortSignal): Promise<unknown> {
  const response = await fetch(`/minecraft/map/tiles/${path}`, { signal, cache: "no-store", credentials: "omit" });
  if (!response.ok || !response.body) throw new Error("Map data is unavailable. Try again shortly.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let size = 0, content = "";
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      size += next.value.byteLength;
      if (size > MAX_JSON_BYTES) throw new Error("Map metadata exceeds its size limit.");
      content += decoder.decode(next.value, { stream: true });
    }
    content += decoder.decode();
    return JSON.parse(content) as unknown;
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
}

export const squaremap = {
  worlds: async (signal: AbortSignal) => parseWorlds(await readJson("settings.json", signal)),
  settings: async (world: string, signal: AbortSignal) => parseSettings(await readJson(`${mapId(world)}/settings.json`, signal)),
  players: async (signal: AbortSignal) => parsePlayers(await readJson("players.json", signal)),
};
