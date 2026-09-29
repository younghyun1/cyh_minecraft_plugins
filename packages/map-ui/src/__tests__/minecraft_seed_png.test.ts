import { afterEach, describe, expect, it, vi } from "vitest";
import { decodeSeedTilePng, seedPngMetadata, SEED_PNG_MAX_BYTES } from "../components/minecraft/seedTilePng";

const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];
function chunk(type: string, body: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(body.length + 12), view = new DataView(bytes.buffer);
  view.setUint32(0, body.length); bytes.set(new TextEncoder().encode(type), 4); bytes.set(body, 8);
  let crc = 0xffffffff;
  for (const byte of bytes.subarray(4, -4)) { crc ^= byte; for (let bit = 0; bit < 8; ++bit) crc = crc & 1 ? 0xedb88320 ^ crc >>> 1 : crc >>> 1; }
  view.setUint32(bytes.length - 4, (crc ^ 0xffffffff) >>> 0); return bytes;
}
function header(depth = 8, color = 3, width = 64) {
  const bytes = new Uint8Array(13), view = new DataView(bytes.buffer); view.setUint32(0, width); view.setUint32(4, 64); bytes.set([depth, color], 8); return chunk("IHDR", bytes);
}
function png(...chunks: Uint8Array[]): ArrayBuffer { return Uint8Array.from([...SIGNATURE, ...chunks.flatMap(bytes => [...bytes])]).buffer; }
const metadata = chunk("cyBM", Uint8Array.from([1, 2, 3])), pixels = chunk("IDAT", Uint8Array.from([0])), end = chunk("IEND", new Uint8Array());
function validMetadata(surface = false) {
  const fields = new Uint8Array(31), view = new DataView(fields.buffer);
  fields.set([67, 89, 66, 77, surface ? 2 : 1, 1, 0, 1, 0]); view.setInt16(9, surface ? -32768 : 64, true); view.setBigUint64(19, 1_000_000n, true); view.setUint16(27, 15000, true); view.setUint16(29, 1, true);
  const strings = ["a".repeat(32), "fixture", "minecraft:plains"].flatMap(value => [value.length, ...new TextEncoder().encode(value)]);
  return chunk("cyBM", Uint8Array.from([...fields, ...strings, 255, 63]));
}

describe("PNG seed tile metadata and native decoding", () => {
  afterEach(() => vi.unstubAllGlobals());
  it.each([[1, 3], [2, 3], [4, 3], [8, 3], [8, 6]])("extracts metadata without inflating depth %s color %s pixels", (depth, color) => {
    expect([...new Uint8Array(seedPngMetadata(png(header(depth, color), metadata, pixels, end)))]).toEqual([1, 2, 3]);
  });
  it("rejects bad signatures, checksums, trailing bytes, duplicate metadata, and missing chunks", () => {
    const complete = png(header(), metadata, pixels, end), corrupt = complete.slice(0), changed = new Uint8Array(corrupt); changed[45] = (changed[45] ?? 0) ^ 1;
    for (const bytes of [new ArrayBuffer(10), corrupt, complete.slice(0, -1), png(header(), metadata, metadata, pixels, end), png(header(), pixels, metadata, end), png(header(), metadata, end), png(header(), pixels, end), png(header(), metadata, pixels, end, end)]) expect(() => seedPngMetadata(bytes)).toThrow();
    expect(() => seedPngMetadata(new ArrayBuffer(SEED_PNG_MAX_BYTES + 1))).toThrow();
  });
  it("rejects invalid dimensions, depth, header order, and oversized metadata", () => {
    for (const first of [header(8, 3, 63), header(16, 6), header(1, 6), pixels]) expect(() => seedPngMetadata(png(first, metadata, pixels, end))).toThrow();
    expect(() => seedPngMetadata(png(header(), chunk("cyBM", new Uint8Array(40_001)), pixels, end))).toThrow();
  });
  it("keeps exact hover metadata alongside the browser-decoded bitmap", async () => {
    const image = { width: 64, height: 64, close: vi.fn() }, decode = vi.fn(async () => image); vi.stubGlobal("createImageBitmap", decode);
    const tile = await decodeSeedTilePng(png(header(), validMetadata(), pixels, end));
    expect(tile.image).toBe(image); expect(tile.indices).toEqual(Array<number>(4096).fill(0)); expect(tile.palette).toEqual(["minecraft:plains"]);
    expect(decode).toHaveBeenCalledOnce(); expect(image.close).not.toHaveBeenCalled();
  });
  it("closes a bitmap with inconsistent decoded dimensions", async () => {
    const image = { width: 32, height: 64, close: vi.fn() }; vi.stubGlobal("createImageBitmap", vi.fn(async () => image));
    await expect(decodeSeedTilePng(png(header(), validMetadata(), pixels, end))).rejects.toThrow("dimensions"); expect(image.close).toHaveBeenCalledOnce();
  });
  it("preserves surface mode in PNG metadata without inventing a fixed height", async () => {
    const image = { width: 64, height: 64, close: vi.fn() }; vi.stubGlobal("createImageBitmap", vi.fn(async () => image));
    const tile = await decodeSeedTilePng(png(header(), validMetadata(true), pixels, end));
    expect(tile.y).toBeNull(); expect(tile.world).toBe("minecraft:overworld"); expect(tile.image).toBe(image);
  });
  it("avoids canceled decodes and closes native work canceled while decoding", async () => {
    let finish!: (image: ImageBitmap) => void;
    const decode = vi.fn(() => new Promise<ImageBitmap>(resolve => { finish = resolve; })); vi.stubGlobal("createImageBitmap", decode);
    const aborted = new AbortController(); aborted.abort();
    await expect(decodeSeedTilePng(png(header(), validMetadata(), pixels, end), aborted.signal)).rejects.toMatchObject({ name: "AbortError" }); expect(decode).not.toHaveBeenCalled();
    const active = new AbortController(), result = decodeSeedTilePng(png(header(), validMetadata(), pixels, end), active.signal), close = vi.fn(); active.abort();
    finish({ width: 64, height: 64, close } as unknown as ImageBitmap);
    await expect(result).rejects.toMatchObject({ name: "AbortError" }); expect(close).toHaveBeenCalledOnce();
  });
});
