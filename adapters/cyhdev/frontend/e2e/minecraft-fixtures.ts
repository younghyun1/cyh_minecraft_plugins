import type { Page } from "@playwright/test";
import type { MinecraftMapData, MinecraftMapQuery, MinecraftSeedTile, MinecraftSeedTileQuery, MinecraftWaypoint } from "../src/generated";
import { encodeSeedTileFixture, encodeSeedTilePngFixture } from "./seed-tile-fixture";

export const mapWorlds = [{ id: "minecraft:overworld", name: "Overworld", map_id: "minecraft_overworld", min_y: -64, max_y: 319 }, { id: "minecraft:the_nether", name: "Nether", map_id: "minecraft_the_nether", min_y: 0, max_y: 255 }, { id: "minecraft:the_end", name: "The End", map_id: "minecraft_the_end", min_y: 0, max_y: 255 }];
export const initialWaypoint: MinecraftWaypoint = { id: "00000000-0000-4000-8000-000000000001", world: "minecraft:overworld", name: "Oakridge base", description: "Storage, beds, and the northern trail", x: 146, y: 72, z: 116 };
export function emptyMapData(kind: MinecraftMapData["kind"]): MinecraftMapData {
  return { kind, world: kind === "catalog" ? null : "minecraft:overworld", sampled_at_ms: Date.now(), scanned_chunks: 0, missing_chunks: 0, truncated: false, worlds: [], blocks: [], cells: [], structures: [], matches: [] };
}

export function seedMapData(query: MinecraftSeedTileQuery, now = Date.now()): MinecraftSeedTile {
  const step = 4 * 2 ** query.level, min_x = query.tile_x * 256 * 2 ** query.level, min_z = query.tile_z * 256 * 2 ** query.level;
  const preset = query.world === "minecraft:the_nether" ? "nether" : query.world === "minecraft:the_end" ? "end" : "large_biomes";
  const palette = preset === "nether" ? ["minecraft:nether_wastes", "minecraft:warped_forest"] : preset === "end" ? ["minecraft:the_end", "minecraft:end_highlands", "minecraft:end_midlands", "minecraft:small_end_islands", "minecraft:end_barrens"] : ["minecraft:plains", "minecraft:forest"];
  return { ...query, min_x, min_z, step, width: 64, height: 64, preset, profile_epoch: "a".repeat(64), sampled_at_ms: now, expires_at_ms: now + 15000, generator_revision: "Pumpkin 26.3 / fixture-revision", palette, indices: Array.from({ length: 4096 }, (_, i) => preset === "end" ? Math.abs(query.tile_x + query.tile_z + i % 64) % 5 : min_x + i % 64 * step > 144 ? 1 : 0) };
}

/** Procedural pixel terrain keeps screenshot fixtures deterministic and network-free. */
function terrainTile(): string {
  const trees = Array.from({ length: 120 }, (_, index) => {
    const x = (index * 43 + 19) % 512, y = (index * 67 + 33) % 512;
    if (x > 195 && x < 285) return "";
    return `<rect x="${x}" y="${y}" width="11" height="12" fill="#365a2c"/><rect x="${x + 2}" y="${y - 2}" width="8" height="9" fill="#567c36"/>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512"><defs><pattern id="grass" width="16" height="16" patternUnits="userSpaceOnUse"><rect width="16" height="16" fill="#7e9455"/><rect width="7" height="4" fill="#879c60"/><rect x="10" y="8" width="5" height="7" fill="#758b4d"/></pattern><pattern id="water" width="12" height="12" patternUnits="userSpaceOnUse"><rect width="12" height="12" fill="#527c95"/><rect x="2" y="4" width="7" height="2" fill="#648da0"/></pattern></defs><rect width="512" height="512" fill="url(#grass)"/><path d="M205 0H260V84H244V164H275V241H250V345H286V512H230V360H205V245H230V174H206V91H190Z" fill="#c3b687"/><path d="M215 0H250V90H234V174H265V231H240V355H276V512H240V350H215V235H240V164H216V81H200Z" fill="url(#water)"/>${trees}<path d="M0 117H174V126H0Z M154 126H164V210H154Z" fill="#b9a27b"/><rect x="140" y="106" width="25" height="24" fill="#514736"/><rect x="142" y="103" width="21" height="22" fill="#aa8460"/><rect x="148" y="103" width="4" height="22" fill="#bc9572"/><rect x="100" y="80" width="20" height="12" fill="#786c46"/><path d="M102 81H118M102 85H118M102 89H118" stroke="#bda85e" stroke-width="2"/></svg>`;
}

export async function installMinecraftMapMocks(page: Page) {
  const queries: MinecraftMapQuery[] = [];
  const predictions: MinecraftSeedTileQuery[] = [];
  const writes: { method: string; body: unknown }[] = [];
  let waypoints = [initialWaypoint];
  await page.route("**/minecraft/map/tiles/settings.json", route => route.fulfill({ json: { worlds: [{ name: "minecraft_overworld", display_name: "Overworld", type: "normal" }, { name: "minecraft_the_nether", display_name: "The Nether", type: "nether" }, { name: "minecraft_the_end", display_name: "The End", type: "the_end" }] } }));
  await page.route("**/minecraft/map/tiles/*/settings.json", route => route.fulfill({ json: { zoom: { max: 3, def: 4, extra: 2 }, spawn: { x: 128, z: 128 } } }));
  await page.route("**/minecraft/map/tiles/players.json", route => route.fulfill({ json: { players: [{ name: "Alex", world: "minecraft_overworld", x: 120, z: 104 }] } }));
  await page.route("**/minecraft/map/tiles/**/*.png*", route => route.fulfill({ contentType: "image/svg+xml", body: terrainTile() }));
  await page.route("**/api/minecraft/map/seed-tile.{bin,png}", async route => {
    const query = route.request().postDataJSON() as MinecraftSeedTileQuery;
    predictions.push(query);
    const tile = seedMapData(query, await page.evaluate(() => Date.now())), png = route.request().url().endsWith(".png");
    return route.fulfill({ contentType: png ? "image/png" : "application/vnd.cyhdev.biome-tile", body: png ? encodeSeedTilePngFixture(tile) : encodeSeedTileFixture(tile) });
  });
  await page.route("**/api/minecraft/map/query", async route => {
    const body = route.request().postDataJSON() as MinecraftMapQuery;
    queries.push(body);
    if (body.kind === "catalog") {
      await route.fulfill({ json: { data: { ...emptyMapData("catalog"), worlds: mapWorlds, blocks: ["minecraft:diamond_ore", "minecraft:stone", "minecraft:oak_log"] } } }); return;
    }
    if (body.kind === "blocks") {
      await route.fulfill({ json: { data: { ...emptyMapData("blocks"), world: body.world, scanned_chunks: 12, missing_chunks: 4, truncated: true, matches: [{ x: 132, y: -20, z: 126 }, { x: 132, y: -19, z: 127 }] } } }); return;
    }
    const chunkCount = body.width * body.height, missing = chunkCount === 64 ? 4 : 0, scanned = chunkCount - missing;
    const columns = body.width * 4;
    const cells = Array.from({ length: chunkCount * 16 }, (_, index) => {
      const x = body.chunk_x * 16 + index % columns * 4, z = body.chunk_z * 16 + Math.floor(index / columns) * 4;
      return { x, z, y: Math.round(68 + Math.sin(x / 25) * 10 + Math.cos(z / 30) * 7), biome: x > 144 ? "minecraft:forest" : z > 134 ? "minecraft:river" : "minecraft:plains" };
    }).filter(cell => (Math.floor(cell.z / 16) - body.chunk_z) * body.width + Math.floor(cell.x / 16) - body.chunk_x < scanned);
    await route.fulfill({ json: { data: { ...emptyMapData("area"), world: body.world, scanned_chunks: scanned, missing_chunks: missing, cells, structures: chunkCount === 64 ? [{ kind: "minecraft:village_plains", min_x: 108, min_y: 64, min_z: 90, max_x: 162, max_y: 84, max_z: 133 }] : [] } } });
  });
  await page.route("**/api/minecraft/map/waypoints?*", route => route.fulfill({ json: { data: new URL(route.request().url()).searchParams.get("world") === "minecraft:overworld" ? waypoints : [] } }));
  await page.route("**/api/admin/minecraft/map/waypoints{,/*}", async route => {
    const method = route.request().method(); const body: unknown = method === "DELETE" ? null : route.request().postDataJSON();
    writes.push({ method, body });
    if (method === "DELETE") { waypoints = []; await route.fulfill({ json: { data: { acknowledged: true } } }); return; }
    const id = method === "POST" ? "00000000-0000-4000-8000-000000000002" : new URL(route.request().url()).pathname.split("/").at(-1) ?? initialWaypoint.id;
    const waypoint = { ...(body as Omit<MinecraftWaypoint, "id">), id };
    waypoints = method === "POST" ? [...waypoints, waypoint] : waypoints.map(item => item.id === id ? waypoint : item);
    await route.fulfill({ json: { data: waypoint } });
  });
  return { queries, predictions, writes };
}
