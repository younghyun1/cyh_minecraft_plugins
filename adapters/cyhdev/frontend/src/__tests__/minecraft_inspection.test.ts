import { createRoot, flush } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MinecraftMapData } from "../generated";
import type { SeedSample } from "../components/minecraft/seedTiles";
import { createMapInspection, createMapQueryGate, validInspectionChunk } from "../components/minecraft/mapInspectionState";

const WORLD = "minecraft:overworld";
function chunk(x = 0, z = 0, changes: Partial<MinecraftMapData> = {}): MinecraftMapData {
  return { kind: "area", world: WORLD, sampled_at_ms: 1, scanned_chunks: 1, missing_chunks: 0, truncated: false, structures: [], matches: [], worlds: [], blocks: [],
    cells: Array.from({ length: 16 }, (_, i) => ({ x: x * 16 + i % 4 * 4, z: z * 16 + Math.floor(i / 4) * 4, y: 72, biome: "minecraft:forest" })), ...changes };
}
function preview(): SeedSample { return { name: "minecraft:plains", sample: { x: 0, z: 0 }, y: 64, step: 4, expires: Date.now() + 15000 }; }
function deferred<T>() {
  let resolve: (value: T) => void = () => { throw new Error("Promise not initialized"); };
  const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve };
}
const disposals: (() => void)[] = [];
function setup(useGate = true) {
  const context = { world: WORLD, area: null as MinecraftMapData | null, areaSlice: null as number | null, matches: null as MinecraftMapData | null, matchedBlock: "", prediction: null as SeedSample | null, seed: (): SeedSample | null => context.prediction };
  let allowed = true;
  const busy = vi.fn();
  const gate = createMapQueryGate(() => allowed, busy);
  const read = vi.fn(async (world: string, x: number, z: number) => chunk(x, z, { world }));
  const controller = createRoot(dispose => {
    disposals.push(dispose);
    return createMapInspection({ allowed: () => allowed, context: () => context, read,
      schedule: job => { if (useGate) gate.hover(job); else if (job.valid()) void job.run(); }, clearQueued: () => gate.clearHover() });
  });
  disposals.push(() => { controller.invalidate(); gate.invalidate(); });
  return { context, read, controller, gate, busy, hide: () => { allowed = false; controller.invalidate(); gate.invalidate(); } };
}
async function tick(ms = 0) { await vi.advanceTimersByTimeAsync(ms); flush(); }

describe("Minecraft pointer inspection", () => {
  beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(1_000_000); });
  afterEach(() => { while (disposals.length) disposals.pop()?.(); vi.restoreAllMocks(); vi.useRealTimers(); });

  it("debounces one chunk, floors negative quart coordinates, and reuses its 16 samples", async () => {
    const { controller, read, busy } = setup();
    controller.inspect({ x: -8, z: -8 }); await tick(499); expect(read).not.toHaveBeenCalled();
    controller.inspect({ x: -1, z: -1 }); await tick(499); expect(read).not.toHaveBeenCalled();
    await tick(1);
    expect(read).toHaveBeenCalledExactlyOnceWith(WORLD, -1, -1);
    expect(controller.inspection()?.biome).toEqual({ name: "minecraft:forest", source: "observed", sample: { x: -4, z: -4 }, y: null, surfaceY: 72 });
    controller.inspect({ x: -2, z: -3 }); await tick(1500);
    expect(read).toHaveBeenCalledTimes(1); expect(busy).not.toHaveBeenCalledWith(true);
  });

  it("prefers an explicit observed slice and reports only exact-column block matches", async () => {
    const { controller, context, read } = setup();
    context.area = chunk(); context.areaSlice = -16; context.prediction = preview();
    context.matchedBlock = "minecraft:diamond_ore";
    context.matches = chunk(0, 0, { kind: "blocks", matches: [{ x: 2, z: 3, y: 12 }, { x: 2, z: 3, y: -4 }, { x: 2, z: 3, y: 12 }, { x: 0, z: 0, y: 40 }] });
    controller.inspect({ x: 2, z: 3 }); await tick(1000);
    expect(controller.inspection()?.biome).toMatchObject({ source: "observed", y: -16, surfaceY: 72, sample: { x: 0, z: 0 } });
    expect(controller.inspection()?.block).toEqual({ name: "minecraft:diamond_ore", ys: [-4, 12] });
    expect(read).not.toHaveBeenCalled();
  });

  it("uses only unexpired prediction cells permitted by the coverage mask", async () => {
    const { controller, context, read } = setup(); context.prediction = preview();
    controller.inspect({ x: 1, z: 1 }); await tick(500);
    expect(controller.inspection()?.biome).toEqual({ name: "minecraft:plains", source: "predicted", y: 64, surfaceY: null, step: 4, sample: { x: 0, z: 0 } });
    expect(read).not.toHaveBeenCalled();
    context.prediction = null;
    controller.refresh(); await tick(500); expect(read).toHaveBeenCalledTimes(1);
    controller.invalidate(); context.prediction = { ...preview(), expires: Date.now() - 1 };
    controller.inspect({ x: 1, z: 1 }); await tick(1000); expect(read).toHaveBeenCalledTimes(2);
  });

  it("expires cached evidence without polling and refreshes on a same-point tap", async () => {
    const { controller, read } = setup(); controller.inspect({ x: 1, z: 1 }); await tick(500);
    await tick(30_000);
    expect(controller.inspection()?.status).toBe("unavailable"); expect(read).toHaveBeenCalledTimes(1);
    controller.inspect({ x: 1, z: 1 }); await tick(500); expect(read).toHaveBeenCalledTimes(2);
  });

  it("caches missing chunks and malformed replies without stationary retries", async () => {
    const { controller, read } = setup();
    read.mockResolvedValueOnce(chunk(0, 0, { cells: [], scanned_chunks: 0, missing_chunks: 1 }));
    controller.inspect({ x: 1, z: 1 }); await tick(500);
    expect(controller.inspection()?.message).toBe("No generated terrain data");
    controller.inspect({ x: 2, z: 2 }); await tick(2000); expect(read).toHaveBeenCalledTimes(1);
    read.mockResolvedValueOnce(chunk(1, 0, { world: "minecraft:the_nether" }));
    controller.inspect({ x: 16, z: 0 }); await tick(500);
    expect(controller.inspection()?.message).toBe("Biome data is unavailable here.");
    await tick(31_000); expect(read).toHaveBeenCalledTimes(2);
  });

  it("retains at most 64 chunk entries, including unavailable results", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1_000_000);
    const { controller, read } = setup(false);
    read.mockImplementation(async (world, x, z) => chunk(x, z, { world, cells: [], scanned_chunks: 0, missing_chunks: 1 }));
    for (let x = 0; x < 65; x++) { controller.inspect({ x: x * 16, z: 0 }); await tick(500); }
    expect(read).toHaveBeenCalledTimes(65);
    controller.inspect({ x: 63 * 16, z: 0 }); await tick(500); expect(read).toHaveBeenCalledTimes(65);
    controller.inspect({ x: 0, z: 0 }); await tick(500); expect(read).toHaveBeenCalledTimes(66);
  });

  it("prioritizes one manual action after an in-flight hover and completion cooldown", async () => {
    const { controller, read, gate, busy } = setup(); const pending = deferred<MinecraftMapData>();
    const order: string[] = []; read.mockImplementationOnce(() => { order.push("hover"); return pending.promise; });
    controller.inspect({ x: 1, z: 1 }); await tick(500);
    expect(gate.manual({ valid: () => true, run: async () => { order.push("manual"); } })).toBe(true);
    expect(gate.manual({ valid: () => true, run: async () => { order.push("duplicate"); } })).toBe(false);
    expect(busy).toHaveBeenLastCalledWith(true);
    controller.inspect({ x: 17, z: 1 }); await tick(500); expect(read).toHaveBeenCalledTimes(1);
    pending.resolve(chunk()); await tick(); await tick(999); expect(order).toEqual(["hover"]);
    await tick(1); expect(order).toEqual(["hover", "manual"]); expect(read).toHaveBeenCalledTimes(1);
    await tick(1000); expect(read).toHaveBeenCalledTimes(2);
  });

  it("runs manual then hover before the latest background job without losing the cooldown timer", async () => {
    const { gate, busy } = setup(); const pending = deferred<void>(); const order: string[] = [];
    const job = (name: string) => ({ valid: () => true, run: async () => { order.push(name); } });
    gate.hover({ valid: () => true, run: async () => { order.push("running hover"); await pending.promise; } });
    gate.background(job("obsolete background"));
    expect(gate.manual(job("manual"))).toBe(true);
    gate.hover(job("queued hover"));
    pending.resolve(); await tick();
    gate.clearBackground(); gate.background(job("replacement background"));
    await tick(999); expect(order).toEqual(["running hover"]);
    await tick(1); expect(order).toEqual(["running hover", "manual"]);
    expect(busy).toHaveBeenLastCalledWith(false);
    gate.clearBackground(); gate.background(job("latest background"));
    await tick(1000); expect(order).toEqual(["running hover", "manual", "queued hover"]);
    // Clearing an unrelated slot must not cancel the remaining background cooldown.
    gate.clearHover(); await tick(999); expect(order).toHaveLength(3);
    await tick(1); expect(order).toEqual(["running hover", "manual", "queued hover", "latest background"]);
  });

  it("discards obsolete world replies and stops hidden or cleared pointer reads", async () => {
    const { controller, context, read, hide } = setup(); const pending = deferred<MinecraftMapData>(); read.mockReturnValueOnce(pending.promise);
    controller.inspect({ x: 1, z: 1 }); await tick(500);
    controller.invalidate(); context.world = "minecraft:the_nether";
    controller.inspect({ x: 17, z: 1 }); pending.resolve(chunk()); await tick();
    expect(controller.inspection()?.biome).toBeNull();
    await tick(1000); expect(read).toHaveBeenLastCalledWith("minecraft:the_nether", 1, 0);
    controller.inspect({ x: 32, z: 0 }); controller.inspect(null); await tick(2000);
    expect(controller.inspection()).toBeNull(); expect(read).toHaveBeenCalledTimes(2);
    controller.inspect({ x: 48, z: 0 }); hide(); await tick(2000); expect(read).toHaveBeenCalledTimes(2);
  });

  it("rejects incomplete, duplicate, out-of-chunk and misaddressed samples", () => {
    const good = chunk(-1, -1); expect(validInspectionChunk(good, WORLD, -1, -1)).toBe(true);
    expect(validInspectionChunk({ ...good, cells: good.cells.slice(1) }, WORLD, -1, -1)).toBe(false);
    expect(validInspectionChunk({ ...good, cells: good.cells.map(() => good.cells[0]!) }, WORLD, -1, -1)).toBe(false);
    expect(validInspectionChunk(chunk(), WORLD, -1, -1)).toBe(false);
    expect(validInspectionChunk({ ...good, kind: "blocks" }, WORLD, -1, -1)).toBe(false);
    expect(validInspectionChunk({ ...good, cells: [], scanned_chunks: 0, missing_chunks: 0 }, WORLD, -1, -1)).toBe(false);
  });

  it("does not substitute an adjacent chunk beyond the queryable world border", async () => {
    const { controller, read } = setup(); controller.inspect({ x: 30_000_000, z: 0 }); await tick(2000);
    expect(controller.inspection()?.point.x).toBe(30_000_000);
    expect(controller.inspection()?.status).toBe("unavailable"); expect(read).not.toHaveBeenCalled();
  });
});
