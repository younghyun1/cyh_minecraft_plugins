import { describe, expect, it } from "vitest";
import { biomeColor, coordinate, displayName, distance, scanOrigin, selectionRegion, supportsSeedWorld, WORLD_LIMIT } from "../components/minecraft/mapMath";
import { mapId, parsePlayers, parseSettings, parseWorlds } from "../services/squaremap";

describe("Minecraft metadata boundaries", () => {
  it("supports canonical dimensions and assigns distinct Nether and End biome colors", () => {
    for (const world of ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"]) expect(supportsSeedWorld(world)).toBe(true);
    expect(supportsSeedWorld("custom:nether")).toBe(false);
    const biomes = ["nether_wastes", "soul_sand_valley", "crimson_forest", "warped_forest", "basalt_deltas", "the_end", "end_highlands", "end_midlands", "small_end_islands", "end_barrens"].map(name => `minecraft:${name}`);
    expect(new Set(biomes.map(biomeColor)).size).toBe(biomes.length);
    expect(displayName("minecraft:small_end_islands")).toBe("small end islands");
  });
  it("rejects paths and oversized world lists from plugin metadata", () => {
    for (const value of ["../private", "a/b", "https://evil.test", "a\\b", "..", "a?script=1"]) expect(() => mapId(value)).toThrow();
    expect(() => parseWorlds({ worlds: Array.from({ length: 65 }, () => ({})) })).toThrow();
    expect(mapId("minecraft_the_nether")).toBe("minecraft_the_nether");
  });
  it("bounds zoom and treats display names as plain text", () => {
    expect(parseWorlds({ worlds: [{ name: "world", display_name: "<script>hello</script>", type: "normal" }] })[0]?.displayName).toBe("<script>hello</script>");
    expect(() => parseSettings({ zoom: { max: 30, def: 1, extra: 0 }, spawn: { x: 0, z: 0 } })).toThrow();
    expect(() => parseSettings({ zoom: { max: 3, def: 1, extra: 0 }, spawn: { x: Infinity, z: 0 } })).toThrow();
  });
  it("omits players without published coordinates and bounds player data", () => {
    expect(parsePlayers({ players: [{ name: "Hidden", world: "world" }, { name: "Alex", world: "world", x: -1, z: 2 }] })).toEqual([{ name: "Alex", world: "world", uuid: null, health: null, armor: null, x: -1, z: 2 }]);
    expect(() => parsePlayers({ players: [{ name: "Alex", world: "world", x: 99_000_000, z: 2 }] })).toThrow();
  });
  it("reads the live player schema while restricting head paths and vital images", () => {
    expect(parsePlayers({ players: [{ name: "EmeraldRange", world: "minecraft_overworld", x: -115, y: 66, z: -121, uuid: "61dd44ba-6b44-4b1b-bbcb-5a838082b3cd", health: 20, armor: 10 }] })[0]).toEqual({ name: "EmeraldRange", world: "minecraft_overworld", x: -115, z: -121, uuid: "61dd44ba6b444b1bbbcb5a838082b3cd", health: 20, armor: 10 });
    expect(parsePlayers({ players: [{ name: "Alex", world: "world", x: 0, z: 0, health: 40, armor: 8.5 }] })[0]).toMatchObject({ health: 20, armor: 8 });
    for (const uuid of ["../../private", "x".repeat(32), "https://example.test/face"]) expect(() => parsePlayers({ players: [{ name: "Alex", world: "world", x: 0, z: 0, uuid }] })).toThrow();
    expect(() => parsePlayers({ players: [{ name: "Alex", world: "world", x: 0, z: 0, health: Infinity }] })).toThrow();
  });
  it("respects published player visibility settings and preserves custom dimension names", () => {
    expect(parseSettings({ zoom: { max: 3, def: 3, extra: 2 }, spawn: { x: 0, z: 0 }, player_tracker: { enabled: false, nameplates: { enabled: false, show_heads: false, show_health: false, show_armor: false } } }).playerTracker).toEqual({ enabled: false, nameplates: false, heads: false, health: false, armor: false });
    const worlds = parseWorlds({ worlds: ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end", "Oakridge"].map((display_name, index) => ({ name: `world${index}`, display_name, type: "normal" })) });
    expect(worlds.map(world => world.displayName)).toEqual(["Overworld", "Nether", "The End", "Oakridge"]);
  });
  it("uses floor division at negative coordinates and bounds edge scans", () => {
    expect(scanOrigin({ x: -1, z: -16 }, 8)).toEqual({ chunk_x: -5, chunk_z: -5, width: 8, height: 8 });
    expect(scanOrigin({ x: 30_000_000, z: -30_000_000 }, 4)).toEqual({ chunk_x: 1_874_996, chunk_z: -1_875_000, width: 4, height: 4 });
    expect(coordinate("1e4")).toBeNull(); expect(coordinate("NaN")).toBeNull(); expect(coordinate("30000001")).toBeNull(); expect(coordinate("-12")).toBe(-12);
    expect(distance({ x: 0, z: 0 }, { x: 3, z: 4 })).toBe(5);
  });
  it("selects complete chunks across exact boundaries in either direction", () => {
    expect(selectionRegion({ x: 0, z: 0 }, { x: 15, z: 15 })).toEqual({ chunk_x: 0, chunk_z: 0, width: 1, height: 1 });
    expect(selectionRegion({ x: 0, z: 0 }, { x: 16, z: 16 })).toEqual({ chunk_x: 0, chunk_z: 0, width: 2, height: 2 });
    const region = { chunk_x: 1, chunk_z: 0, width: 2, height: 4 };
    expect(selectionRegion({ x: 47, z: 63 }, { x: 16, z: 0 })).toEqual(region);
    expect(selectionRegion({ x: 16, z: 0 }, { x: 47, z: 63 })).toEqual(region);
  });
  it("uses negative chunk boundaries and caps selection at eight chunks from the drag origin", () => {
    expect(selectionRegion({ x: -1, z: -16 }, { x: -17, z: -33 })).toEqual({ chunk_x: -2, chunk_z: -3, width: 2, height: 3 });
    expect(selectionRegion({ x: -1, z: -1 }, { x: 0, z: 0 })).toEqual({ chunk_x: -1, chunk_z: -1, width: 2, height: 2 });
    expect(selectionRegion({ x: 32, z: 48 }, { x: 100_000, z: -100_000 })).toEqual({ chunk_x: 2, chunk_z: -4, width: 8, height: 8 });
    expect(selectionRegion({ x: WORLD_LIMIT, z: -WORLD_LIMIT }, { x: -WORLD_LIMIT, z: WORLD_LIMIT })).toEqual({ chunk_x: 1_874_992, chunk_z: -1_875_000, width: 8, height: 8 });
  });
});
