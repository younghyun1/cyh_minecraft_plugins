/** Biomes in the pinned Pumpkin 26.3 climate tree; End biomes use its End source. */
const OVERWORLD = `badlands bamboo_jungle beach birch_forest cherry_grove cold_ocean dappled_forest dark_forest deep_cold_ocean deep_dark deep_frozen_ocean deep_lukewarm_ocean deep_ocean desert dripstone_caves eroded_badlands flower_forest forest frozen_ocean frozen_peaks frozen_river grove ice_spikes jagged_peaks jungle lukewarm_ocean lush_caves mangrove_swamp meadow mushroom_fields ocean old_growth_birch_forest old_growth_pine_taiga old_growth_spruce_taiga pale_garden plains river savanna savanna_plateau snowy_beach snowy_plains snowy_slopes snowy_taiga sparse_jungle stony_peaks stony_shore sulfur_caves sunflower_plains swamp taiga warm_ocean windswept_forest windswept_gravelly_hills windswept_hills windswept_savanna wooded_badlands`.split(" ").map(name => `minecraft:${name}`);
const NETHER = `basalt_deltas crimson_forest nether_wastes soul_sand_valley warped_forest`.split(" ").map(name => `minecraft:${name}`);
const END = `end_barrens end_highlands end_midlands small_end_islands the_end`.split(" ").map(name => `minecraft:${name}`);

export function biomeChoices(world: string | undefined): readonly string[] {
  switch (world) {
    case "minecraft:overworld": return OVERWORLD;
    case "minecraft:the_nether": return NETHER;
    case "minecraft:the_end": return END;
    default: return [];
  }
}
