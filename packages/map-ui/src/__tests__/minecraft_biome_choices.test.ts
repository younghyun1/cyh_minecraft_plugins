import { describe, expect, it } from "vitest";
import { biomeChoices } from "../components/minecraft/biomeChoices";

describe("Biome picker choices", () => {
  it("offers the pinned dimension biomes without cross-dimension entries", () => {
    const overworld = biomeChoices("minecraft:overworld");
    expect(overworld).toContain("minecraft:forest");
    expect(overworld).toContain("minecraft:sulfur_caves");
    expect(overworld).not.toContain("minecraft:warped_forest");
    expect(new Set(overworld).size).toBe(overworld.length);
    expect(biomeChoices("minecraft:the_nether")).toEqual(["minecraft:basalt_deltas", "minecraft:crimson_forest", "minecraft:nether_wastes", "minecraft:soul_sand_valley", "minecraft:warped_forest"]);
    expect(biomeChoices("minecraft:the_end")).toEqual(["minecraft:end_barrens", "minecraft:end_highlands", "minecraft:end_midlands", "minecraft:small_end_islands", "minecraft:the_end"]);
    expect(biomeChoices("custom:world")).toEqual([]);
  });
});
