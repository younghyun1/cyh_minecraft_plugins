import type { MinecraftSeedTile } from "../../contracts";

export const SEED_TILE_CONTENT_TYPE = "application/vnd.cyhdev.biome-tile";
export const SEED_TILE_MAX_BYTES = 40_000;
const WORLDS = ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"] as const;
const PRESETS = ["default", "large_biomes", "nether", "end"] as const;

/** CYBM v1 numeric slices and v2 surface sampling share the same bounded header and geometry. */
export function decodeSeedTileBinary(buffer: ArrayBuffer): MinecraftSeedTile {
  if (buffer.byteLength < 33 || buffer.byteLength > SEED_TILE_MAX_BYTES) throw new Error("Invalid biome tile size.");
  const view = new DataView(buffer), bytes = new Uint8Array(buffer), decoder = new TextDecoder("utf-8", { fatal: true }); let offset = 0;
  const take = (size: number) => { if (offset + size > bytes.length) throw new Error("Truncated biome tile."); const at = offset; offset += size; return at; };
  const u8 = () => view.getUint8(take(1)), u16 = () => view.getUint16(take(2), true);
  const text = () => { const size = u8(); if (size === 0 || size > 128) throw new Error("Invalid biome tile string."); return decoder.decode(bytes.subarray(take(size), offset)); };
  if (u8() !== 67 || u8() !== 89 || u8() !== 66 || u8() !== 77) throw new Error("Unsupported biome tile.");
  const version = u8();
  if (version !== 1 && version !== 2) throw new Error("Unsupported biome tile.");
  const codec = u8(), world = WORLDS[u8()], preset = PRESETS[u8()], level = u8();
  if (codec > 1 || !world || !preset || level > 12 || (world === WORLDS[0] ? !["default", "large_biomes"].includes(preset) : preset !== (world === WORLDS[1] ? "nether" : "end"))) throw new Error("Invalid biome tile profile.");
  const encodedY = view.getInt16(take(2), true), y = version === 2 && encodedY === -32768 ? null : encodedY;
  const tile_x = view.getInt32(take(4), true), tile_z = view.getInt32(take(4), true);
  const sampled_at_ms = Number(view.getBigUint64(take(8), true)), ttl = u16(), count = u16();
  const step = 4 * 2 ** level, span = step * 64, min_x = tile_x * span, min_z = tile_z * span;
  if (!Number.isSafeInteger(sampled_at_ms) || !Number.isSafeInteger(sampled_at_ms + ttl) || ttl < 1 || ttl > 15000 || count > 256
    || (y === null ? world !== WORLDS[0] : y < (world === WORLDS[0] ? -64 : 0) || y > (world === WORLDS[0] ? 319 : 255))
    || min_x >= 30_000_000 || min_z >= 30_000_000 || min_x + span <= -30_000_000 || min_z + span <= -30_000_000) throw new Error("Invalid biome tile metadata.");
  const profile_epoch = text(), generator_revision = text(), palette = Array.from({ length: count }, text);
  if (!/^[a-zA-Z0-9_-]{16,128}$/.test(profile_epoch) || palette.some(name => !/^[a-z0-9_.-]+:[a-z0-9/_.-]+$/.test(name))) throw new Error("Invalid biome tile identifiers.");
  const bits = count === 0 ? 0 : 32 - Math.clz32(count), mask = 2 ** bits - 1;
  const indices: (number | null)[] = [];
  const append = (symbol: number, length = 1) => {
    if (symbol > count || length < 1 || indices.length + length > 4096) throw new Error("Invalid biome tile palette index.");
    for (let i = 0; i < length; ++i) indices.push(symbol === 0 ? null : symbol - 1);
  };
  if (codec === 0) {
    if (bytes.length - offset !== Math.ceil(4096 * bits / 8)) throw new Error("Invalid packed biome tile.");
    let packed = 0, available = 0;
    while (indices.length < 4096) {
      while (available < bits) { packed |= u8() << available; available += 8; }
      append(packed & mask); packed >>>= bits; available -= bits;
    }
  } else {
    while (indices.length < 4096) {
      let value = 0, shift = 0, next: number;
      do { next = u8(); if (shift >= 28) throw new Error("Oversized biome tile run."); value |= (next & 127) << shift; shift += 7; } while (next & 128);
      if (value < 0) throw new Error("Invalid biome tile run.");
      append(value & mask, (value >>> bits) + 1);
    }
  }
  if (offset !== bytes.length) throw new Error("Trailing biome tile bytes.");
  return { world, preset, level, y, tile_x, tile_z, min_x, min_z, width: 64, height: 64, step, sampled_at_ms, expires_at_ms: sampled_at_ms + ttl, profile_epoch, generator_revision, palette, indices };
}
