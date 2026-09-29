import { afterEach, describe, expect, it, vi } from "vitest";
import { decodeSeedTileBinary, SEED_TILE_CONTENT_TYPE, SEED_TILE_MAX_BYTES } from "../components/minecraft/seedTileBinary";
import { decodeSeedTile } from "../components/minecraft/seedTiles";
import { createSeedTileReader } from "../services/minecraft_seed_tile";
import type { MinecraftSeedTileQuery } from "../contracts";

const transport = vi.hoisted(() => ({ fetch: vi.fn<typeof fetch>() }));
const readSeedTile = createSeedTileReader(transport.fetch, "/api/minecraft/map/seed-tile.bin");
const query: MinecraftSeedTileQuery = { world: "minecraft:overworld", y: 64, tile_x: -1, tile_z: 2, level: 0 };
const EPOCH = "a".repeat(32), REVISION = "fixture", SAMPLE_TIME = 1_000_000;
type FrameOptions = { version?: number; codec?: number; palette?: string[]; payload?: number[]; world?: number; preset?: number; level?: number; y?: number; tileX?: number; tileZ?: number; timestamp?: bigint; ttl?: number; epoch?: string };

/** Deliberately explicit wire offsets make this independent of the production parser. */
function frame(options: FrameOptions = {}): ArrayBuffer {
  const palette = options.palette ?? ["minecraft:plains"], header = new Uint8Array(31), view = new DataView(header.buffer);
  header.set([67, 89, 66, 77, options.version ?? 1, options.codec ?? 1, options.world ?? 0, options.preset ?? 1, options.level ?? 0]);
  view.setInt16(9, options.y ?? 64, true); view.setInt32(11, options.tileX ?? -1, true); view.setInt32(15, options.tileZ ?? 2, true);
  view.setBigUint64(19, options.timestamp ?? BigInt(SAMPLE_TIME), true); view.setUint16(27, options.ttl ?? 15000, true); view.setUint16(29, palette.length, true);
  const fields: number[] = [...header];
  for (const value of [options.epoch ?? EPOCH, REVISION, ...palette]) { const bytes = new TextEncoder().encode(value); fields.push(bytes.length, ...bytes); }
  return Uint8Array.from([...fields, ...options.payload ?? [255, 63]]).buffer;
}

/** Bit-by-bit fixture encoding exercises crossings between bytes, including 9-bit symbols. */
function packed(symbols: readonly number[], bits: number): number[] {
  const bytes = Array<number>(Math.ceil(symbols.length * bits / 8)).fill(0);
  for (let sample = 0; sample < symbols.length; ++sample) for (let bit = 0; bit < bits; ++bit) if ((symbols[sample] ?? 0) & 1 << bit) {
    const offset = sample * bits + bit, byte = Math.floor(offset / 8); bytes[byte] = (bytes[byte] ?? 0) | 1 << offset % 8;
  }
  return bytes;
}

describe("Minecraft binary biome tile decoder", () => {
  it("decodes surface v2 separately from numeric v1 and v2 slices", () => {
    const surface = decodeSeedTileBinary(frame({ version: 2, y: -32768 }));
    expect(surface).toMatchObject({ world: "minecraft:overworld", y: null });
    expect(decodeSeedTile(surface, { ...query, y: null }, SAMPLE_TIME)).not.toBeNull();
    expect(decodeSeedTile(surface, query, SAMPLE_TIME)).toBeNull();
    for (const version of [1, 2]) expect(decodeSeedTileBinary(frame({ version, y: -16 })).y).toBe(-16);
    for (const options of [{ version: 1, y: -32768 }, { version: 2, y: -32767 }, { version: 2, y: -32768, world: 1, preset: 2 }, { version: 2, y: -32768, world: 2, preset: 3 }, { version: 3, y: 64 }]) expect(() => decodeSeedTileBinary(frame(options))).toThrow();
  });

  it("decodes the Rust compact_runs_keep_exact_metadata_and_nulls frame", () => {
    // The Rust encoder test pins these two 2,048-cell runs to FE 1F FF 1F.
    const bytes = frame({ payload: [254, 31, 255, 31] }), tile = decodeSeedTileBinary(bytes);
    expect(tile).toMatchObject({ ...query, preset: "large_biomes", min_x: -256, min_z: 512, width: 64, height: 64, step: 4, profile_epoch: EPOCH, generator_revision: REVISION, sampled_at_ms: SAMPLE_TIME, expires_at_ms: SAMPLE_TIME + 15000 });
    expect(tile.indices).toEqual([...Array<null>(2048).fill(null), ...Array<number>(2048).fill(0)]);
    expect(bytes.byteLength).toBeLessThan(100);
    expect(decodeSeedTile(tile, query, SAMPLE_TIME)).not.toBeNull();
    expect(decodeSeedTile(tile, { ...query, tile_x: 0 }, SAMPLE_TIME)).toBeNull();
  });

  it("decodes packed biome values and hidden cells across byte boundaries", () => {
    const symbols = Array.from({ length: 4096 }, (_, index) => index % 3);
    const tile = decodeSeedTileBinary(frame({ codec: 0, palette: ["minecraft:plains", "minecraft:forest"], payload: packed(symbols, 2) }));
    expect(tile.indices).toEqual(symbols.map(value => value === 0 ? null : value - 1));
  });

  it("preserves the 256th palette entry beside null using nine bits", () => {
    const symbols = Array.from({ length: 4096 }, (_, index) => index % 257);
    const palette = Array.from({ length: 256 }, (_, index) => `minecraft:fixture_${index}`), payload = packed(symbols, 9);
    expect(payload).toHaveLength(4608);
    const tile = decodeSeedTileBinary(frame({ codec: 0, palette, payload }));
    expect(tile.indices).toEqual(symbols.map(value => value === 0 ? null : value - 1));
    expect(tile.indices[256]).toBe(255);
  });

  it.each([0, 1])("decodes an entirely hidden tile with codec %s", codec => {
    expect(decodeSeedTileBinary(frame({ codec, palette: [], payload: codec === 0 ? [] : [255, 31] })).indices).toEqual(Array<null>(4096).fill(null));
  });

  it.each([[1, 2, "minecraft:the_nether", "nether"], [2, 3, "minecraft:the_end", "end"]] as const)("decodes dimension %s with its matching profile", (world, preset, name, profile) => {
    expect(decodeSeedTileBinary(frame({ world, preset, level: 5, y: 255, tileX: -244, tileZ: 244 }))).toMatchObject({ world: name, preset: profile, step: 128, min_x: -1_998_848, min_z: 1_998_848 });
  });

  it("rejects truncated frames at every boundary and trailing bytes", () => {
    const bytes = frame();
    for (let length = 0; length < bytes.byteLength; ++length) expect(() => decodeSeedTileBinary(bytes.slice(0, length))).toThrow();
    const extra = new Uint8Array(bytes.byteLength + 1); extra.set(new Uint8Array(bytes));
    expect(() => decodeSeedTileBinary(extra.buffer)).toThrow("Trailing");
    expect(() => decodeSeedTileBinary(new ArrayBuffer(SEED_TILE_MAX_BYTES + 1))).toThrow("size");
    expect(() => decodeSeedTileBinary(frame({ codec: 0, payload: packed(Array<number>(4095).fill(1), 1).slice(0, -1) }))).toThrow("packed");
  });

  it.each([{ codec: 2 }, { world: 3 }, { preset: 4 }, { world: 0, preset: 2 }, { world: 1, preset: 1 }, { world: 2, preset: 2 }, { level: 13 }])("rejects unsupported or mismatched profile %j", options => {
    expect(() => decodeSeedTileBinary(frame(options))).toThrow("profile");
  });

  it.each([{ ttl: 0 }, { ttl: 15001 }, { timestamp: BigInt(Number.MAX_SAFE_INTEGER) }, { timestamp: 1n << 63n }, { y: -65 }, { y: 320 }, { world: 1, preset: 2, y: -1 }, { world: 2, preset: 3, y: 256 }, { tileX: 2_147_483_647 }, { tileZ: -2_147_483_648 }])("rejects invalid metadata case %#", options => {
    expect(() => decodeSeedTileBinary(frame(options))).toThrow("metadata");
  });

  it("rejects malformed strings, invalid UTF-8, registry identifiers, and oversized palettes", () => {
    for (const epoch of ["", "a".repeat(129), "a".repeat(15), "a".repeat(31) + "!"]) expect(() => decodeSeedTileBinary(frame({ epoch }))).toThrow();
    const bytes = frame(); new Uint8Array(bytes)[32] = 255;
    expect(() => decodeSeedTileBinary(bytes)).toThrow();
    expect(() => decodeSeedTileBinary(frame({ palette: ["../private"] }))).toThrow("identifiers");
    expect(() => decodeSeedTileBinary(frame({ palette: Array<string>(257).fill("minecraft:plains") }))).toThrow("metadata");
    for (const index of [0, 4]) { const invalid = frame(); new Uint8Array(invalid)[index] = 0; expect(() => decodeSeedTileBinary(invalid)).toThrow("Unsupported"); }
  });

  it("rejects impossible symbols, run lengths, overlong varints, and truncated run payloads", () => {
    expect(() => decodeSeedTileBinary(frame({ codec: 0, palette: ["minecraft:plains", "minecraft:forest"], payload: packed(Array<number>(4096).fill(3), 2) }))).toThrow("palette index");
    for (const payload of [[0], [128], [129, 64], [255, 255, 255, 255, 0]]) expect(() => decodeSeedTileBinary(frame({ payload }))).toThrow();
    expect(() => decodeSeedTileBinary(frame({ palette: ["minecraft:plains", "minecraft:forest"], payload: [3] }))).toThrow("palette index");
  });
});

describe("bounded binary biome tile transport", () => {
  afterEach(() => { transport.fetch.mockReset(); });
  const response = (body: ReadableStream<Uint8Array> | ArrayBuffer | null, contentType = SEED_TILE_CONTENT_TYPE, status = 200) => new Response(body, { status, headers: { "Content-Type": contentType } });

  it("streams tiny chunks through the bounded buffer and sends the query, type, and cancellation signal", async () => {
    const bytes = new Uint8Array(frame()), signal = new AbortController().signal; let offset = 0;
    const stream = new ReadableStream<Uint8Array>({ pull(controller) { if (offset === bytes.length) controller.close(); else { controller.enqueue(bytes.subarray(offset, ++offset)); } } });
    transport.fetch.mockResolvedValue(response(stream, `${SEED_TILE_CONTENT_TYPE}; charset=binary`));
    expect((await readSeedTile(query, signal)).indices).toEqual(Array<number>(4096).fill(0));
    expect(transport.fetch).toHaveBeenCalledExactlyOnceWith("/api/minecraft/map/seed-tile.bin", expect.objectContaining({ method: "POST", body: JSON.stringify(query), signal, headers: { "Content-Type": "application/json", Accept: SEED_TILE_CONTENT_TYPE } }));
    expect(stream.locked).toBe(false);
  });

  it("cancels oversized responses before retaining them and releases the reader", async () => {
    const cancel = vi.fn(), stream = new ReadableStream<Uint8Array>({ start(controller) { controller.enqueue(new Uint8Array(SEED_TILE_MAX_BYTES)); controller.enqueue(new Uint8Array(1)); }, cancel });
    transport.fetch.mockResolvedValue(response(stream));
    await expect(readSeedTile(query, new AbortController().signal)).rejects.toThrow("too large");
    expect(cancel).toHaveBeenCalledOnce(); expect(stream.locked).toBe(false);
  });

  it.each([[503, SEED_TILE_CONTENT_TYPE], [200, "application/json"]] as const)("cancels rejected HTTP status/content type %s %s", async (status, type) => {
    const cancel = vi.fn(), stream = new ReadableStream<Uint8Array>({ cancel });
    transport.fetch.mockResolvedValue(response(stream, type, status));
    const request = readSeedTile(query, new AbortController().signal);
    if (status === 503) await expect(request).rejects.toMatchObject({ status: 503 }); else await expect(request).rejects.toThrow("response");
    expect(cancel).toHaveBeenCalledOnce(); expect(stream.locked).toBe(false);
  });

  it("rejects missing bodies and propagates stream failures without retaining a reader lock", async () => {
    transport.fetch.mockResolvedValueOnce(response(null));
    await expect(readSeedTile(query, new AbortController().signal)).rejects.toThrow("response");
    const stream = new ReadableStream<Uint8Array>({ start(controller) { controller.error(new Error("Disconnected")); } });
    transport.fetch.mockResolvedValueOnce(response(stream));
    await expect(readSeedTile(query, new AbortController().signal)).rejects.toThrow("Disconnected"); expect(stream.locked).toBe(false);
  });
});
