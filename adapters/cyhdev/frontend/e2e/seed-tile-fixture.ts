import type { MinecraftSeedTile } from "../src/generated";
import { deflateSync } from "node:zlib";
import { biomeColor } from "../src/components/minecraft/mapMath";

/** Fixture writer intentionally uses literal runs; production may choose packed values. */
export function encodeSeedTileFixture(tile: MinecraftSeedTile): Buffer {
  const output: number[] = [67, 89, 66, 77, 1, 1];
  output.push(["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"].indexOf(tile.world));
  output.push(["default", "large_biomes", "nether", "end"].indexOf(tile.preset), tile.level);
  const number = (value: number, size: number) => { const bytes = Buffer.alloc(size); if (size === 2) bytes.writeUInt16LE(value & 65535); else if (size === 4) bytes.writeInt32LE(value); else bytes.writeBigUInt64LE(BigInt(value)); output.push(...bytes); };
  const text = (value: string) => { const bytes = Buffer.from(value); output.push(bytes.length, ...bytes); };
  number(tile.y, 2); number(tile.tile_x, 4); number(tile.tile_z, 4); number(tile.sampled_at_ms, 8); number(tile.expires_at_ms - tile.sampled_at_ms, 2); number(tile.palette.length, 2);
  text(tile.profile_epoch); text(tile.generator_revision); for (const name of tile.palette) text(name);
  const bits = tile.palette.length === 0 ? 0 : 32 - Math.clz32(tile.palette.length);
  for (let offset = 0; offset < tile.indices.length;) {
    const index = tile.indices[offset]; let end = offset + 1;
    while (end < tile.indices.length && tile.indices[end] === index) ++end;
    let token = ((end - offset - 1) << bits) | (index === null || index === undefined ? 0 : index + 1);
    while (token >= 128) { output.push(token & 127 | 128); token >>>= 7; }
    output.push(token); offset = end;
  }
  return Buffer.from(output);
}

/** RGBA fixtures cover the same native PNG path without duplicating the server's indexed encoder. */
export function encodeSeedTilePngFixture(tile: MinecraftSeedTile): Buffer {
  const chunk = (type: string, body: Buffer) => {
    const bytes = Buffer.alloc(body.length + 12); bytes.writeUInt32BE(body.length); bytes.write(type, 4); body.copy(bytes, 8);
    let crc = 0xffffffff;
    for (const byte of bytes.subarray(4, -4)) { crc ^= byte; for (let bit = 0; bit < 8; ++bit) crc = crc & 1 ? 0xedb88320 ^ crc >>> 1 : crc >>> 1; }
    bytes.writeUInt32BE((crc ^ 0xffffffff) >>> 0, bytes.length - 4); return bytes;
  };
  const header = Buffer.alloc(13); header.writeUInt32BE(64); header.writeUInt32BE(64, 4); header[8] = 8; header[9] = 6;
  const colors = tile.palette.map(name => { const color = biomeColor(name); if (!/^#[a-f\d]{6}$/i.test(color)) throw new Error(`Fixture needs a hex biome color: ${name}`); return Number.parseInt(color.slice(1), 16); });
  const pixels = Buffer.alloc(64 * (64 * 4 + 1));
  for (let index = 0; index < 4096; ++index) {
    const value = tile.indices[index], color = value === null || value === undefined ? undefined : colors[value];
    if (color === undefined) continue;
    const offset = Math.floor(index / 64) * 257 + 1 + index % 64 * 4;
    pixels[offset] = color >>> 16; pixels[offset + 1] = color >>> 8 & 255; pixels[offset + 2] = color & 255; pixels[offset + 3] = 255;
  }
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("cyBM", encodeSeedTileFixture(tile)), chunk("IDAT", deflateSync(pixels)), chunk("IEND", Buffer.alloc(0))]);
}
