import { decodeSeedTileBinary, SEED_TILE_MAX_BYTES } from "./seedTileBinary";
import type { SeedTileResponse } from "./seedTiles";

export const SEED_PNG_MAX_BYTES = 65_536;
const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];
const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, initial) => {
  let value = initial;
  for (let bit = 0; bit < 8; ++bit) value = value & 1 ? 0xedb88320 ^ value >>> 1 : value >>> 1;
  return value >>> 0;
});

/** Only the small CYBM ancillary chunk is decoded in JavaScript; the browser decodes pixels. */
export function seedPngMetadata(buffer: ArrayBuffer): ArrayBuffer {
  const bytes = new Uint8Array(buffer), view = new DataView(buffer);
  if (bytes.length > SEED_PNG_MAX_BYTES || !SIGNATURE.every((value, index) => bytes[index] === value)) throw new Error("Invalid biome PNG.");
  let offset = 8, header = false, image = false, metadata: ArrayBuffer | null = null;
  while (offset + 12 <= bytes.length) {
    const length = view.getUint32(offset), end = offset + 12 + length;
    if (end > bytes.length) throw new Error("Truncated biome PNG.");
    const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    let crc = 0xffffffff;
    for (let i = offset + 4; i < end - 4; ++i) crc = (CRC_TABLE[(crc ^ (bytes[i] ?? 0)) & 255] ?? 0) ^ crc >>> 8;
    if ((crc ^ 0xffffffff) >>> 0 !== view.getUint32(end - 4)) throw new Error("Invalid biome PNG checksum.");
    if (!header && type !== "IHDR") throw new Error("Missing biome PNG header.");
    if (type === "IHDR") {
      const depth = bytes[offset + 16], color = bytes[offset + 17];
      if (header || length !== 13 || view.getUint32(offset + 8) !== 64 || view.getUint32(offset + 12) !== 64 || !(color === 3 && [1, 2, 4, 8].includes(depth ?? -1) || color === 6 && depth === 8) || bytes[offset + 18] !== 0 || bytes[offset + 19] !== 0 || bytes[offset + 20] !== 0) throw new Error("Unsupported biome PNG header.");
      header = true;
    } else if (type === "cyBM") {
      if (metadata || image || length > SEED_TILE_MAX_BYTES) throw new Error("Invalid biome PNG metadata.");
      metadata = buffer.slice(offset + 8, end - 4);
    } else if (type === "IDAT") image = true;
    else if (type === "IEND") {
      if (length !== 0 || end !== bytes.length || !image || !metadata) throw new Error("Incomplete biome PNG.");
      return metadata;
    }
    offset = end;
  }
  throw new Error("Incomplete biome PNG.");
}

export async function decodeSeedTilePng(buffer: ArrayBuffer, signal?: AbortSignal): Promise<SeedTileResponse> {
  signal?.throwIfAborted();
  const metadata = decodeSeedTileBinary(seedPngMetadata(buffer));
  const image = await createImageBitmap(new Blob([buffer], { type: "image/png" }));
  if (signal?.aborted) { image.close(); signal.throwIfAborted(); }
  if (image.width !== 64 || image.height !== 64) { image.close(); throw new Error("Invalid biome PNG dimensions."); }
  return { ...metadata, image };
}
