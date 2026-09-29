export type MapPoint = { readonly x: number; readonly z: number };
export const WORLD_LIMIT = 30_000_000;
const DIMENSION_BIOME_COLORS: Readonly<Record<string, string>> = {
  "minecraft:nether_wastes": "#914646", "minecraft:soul_sand_valley": "#655044", "minecraft:crimson_forest": "#b1324c", "minecraft:warped_forest": "#338b80", "minecraft:basalt_deltas": "#666676",
  "minecraft:the_end": "#c8c28c", "minecraft:end_highlands": "#b6ae72", "minecraft:end_midlands": "#d1c895", "minecraft:small_end_islands": "#a293bc", "minecraft:end_barrens": "#716885",
};
export const supportsSeedWorld = (world: string) => ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"].includes(world);

export function coordinate(value: string): number | null {
  if (!/^-?\d+$/.test(value.trim())) return null;
  const result = Number(value);
  return Number.isSafeInteger(result) && Math.abs(result) <= WORLD_LIMIT ? result : null;
}

/** Floor division preserves chunk boundaries west and north of the origin. */
export function scanOrigin(point: MapPoint, width: number) {
  const margin = Math.floor(width / 2);
  return { chunk_x: Math.max(-1_875_000, Math.min(1_875_000 - width, Math.floor(point.x / 16) - margin)), chunk_z: Math.max(-1_875_000, Math.min(1_875_000 - width, Math.floor(point.z / 16) - margin)), width, height: width };
}

/** A drag selects complete chunks, capped before any world query is constructed. */
export function selectionRegion(start: MapPoint, end: MapPoint) {
  const chunk = (value: number) => Math.max(-1_875_000, Math.min(1_874_999, Math.floor(value / 16)));
  const x = chunk(start.x), z = chunk(start.z);
  const endX = Math.max(x - 7, Math.min(x + 7, chunk(end.x)));
  const endZ = Math.max(z - 7, Math.min(z + 7, chunk(end.z)));
  return { chunk_x: Math.min(x, endX), chunk_z: Math.min(z, endZ), width: Math.abs(endX - x) + 1, height: Math.abs(endZ - z) + 1 };
}

export function distance(a: MapPoint, b: MapPoint): number {
  return Math.hypot(b.x - a.x, b.z - a.z);
}

export function biomeColor(biome: string): string {
  const dimensionColor = DIMENSION_BIOME_COLORS[biome];
  if (dimensionColor) return dimensionColor;
  if (biome.includes("ocean") || biome.endsWith(":river")) return "#537fba";
  if (/snow|frozen|ice|grove/.test(biome)) return "#d6e4e1";
  if (/desert|beach/.test(biome)) return "#d4c28a";
  if (/badlands/.test(biome)) return "#b77852";
  if (/swamp/.test(biome)) return "#647c59";
  if (/jungle/.test(biome)) return "#3e7848";
  if (/forest|taiga/.test(biome)) return "#62945b";
  if (/plains|meadow/.test(biome)) return "#9aaf70";
  if (/savanna/.test(biome)) return "#b6ad67";
  if (/peak|stony|mountain/.test(biome)) return "#989f95";
  let hash = 0;
  for (const character of biome) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 62% 55%)`;
}

export function displayName(value: string): string {
  return value.replace(/^minecraft:/, "").replaceAll("_", " ");
}

export function elevationColor(y: number, min: number, max: number): string {
  const fraction = Math.max(0, Math.min(1, (y - min) / Math.max(1, max - min)));
  return `hsl(${130 - fraction * 110} 65% ${30 + fraction * 45}%)`;
}
