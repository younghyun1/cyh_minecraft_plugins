import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MinecraftMapData, MinecraftWaypoint } from "../generated";
import Minecraft from "../pages/minecraft";
import { setSuperuser } from "../state/auth";

const transport = vi.hoisted(() => ({ query: vi.fn(), waypoints: vi.fn(), create: vi.fn(), update: vi.fn(), remove: vi.fn(), worlds: vi.fn(), settings: vi.fn(), players: vi.fn() }));
vi.mock("../services/account_api", () => ({ contractApi: { minecraftMapQuery: transport.query, minecraftMapWaypoints: transport.waypoints, createMinecraftMapWaypoint: transport.create, updateMinecraftMapWaypoint: transport.update, deleteMinecraftMapWaypoint: transport.remove } }));
vi.mock("../services/squaremap", () => ({ squaremap: { worlds: transport.worlds, settings: transport.settings, players: transport.players } }));
vi.mock("../components/minecraft/MapCanvas", () => ({ default: (props: { mapId: string; seedWorld: string | null; predictionY: number | null; area: MinecraftMapData | null; onPoint: (point: { x: number; z: number }) => void }) => <button data-testid="terrain" data-world={props.mapId} data-seed-world={props.seedWorld} data-seed-y={props.predictionY ?? "surface"} data-cells={props.area?.cells.length ?? 0} onClick={() => props.onPoint({ x: -1, z: 32 })}>Terrain fixture</button> }));

const worlds = [{ id: "minecraft:overworld", name: "Overworld", map_id: "minecraft_overworld", min_y: -64, max_y: 319 }, { id: "minecraft:the_nether", name: "Nether", map_id: "minecraft_the_nether", min_y: 0, max_y: 255 }, { id: "minecraft:the_end", name: "The End", map_id: "minecraft_the_end", min_y: 0, max_y: 255 }];
function data(kind: MinecraftMapData["kind"], overrides: Partial<MinecraftMapData> = {}): { data: MinecraftMapData } {
  return { data: { kind, world: kind === "catalog" ? null : worlds[0]?.id, worlds: kind === "catalog" ? worlds : [], blocks: ["minecraft:diamond_ore", "minecraft:stone"], sampled_at_ms: 1000, scanned_chunks: 0, missing_chunks: 0, truncated: false, cells: [], structures: [], matches: [], ...overrides } };
}
const waypoint: MinecraftWaypoint = { id: "00000000-0000-4000-8000-000000000001", world: "minecraft:overworld", name: "Spawn house", description: "Public shelter", x: 10, y: 64, z: 20 };
function deferred<T>() { let resolve: ((value: T) => void) | undefined; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve: (value: T) => { if (resolve) resolve(value); } }; }
async function openLayers() {
  fireEvent.click(screen.getByRole("button", { name: "Layers" }));
  await screen.findByLabelText("Predicted biomes");
}
async function observedLayers() {
  await openLayers();
  fireEvent.click(screen.getByLabelText("Predicted biomes"));
  await Promise.resolve();
}

describe("Minecraft explorer", () => {
  beforeEach(() => {
    setSuperuser(false);
    for (const mock of Object.values(transport)) mock.mockReset();
    transport.worlds.mockResolvedValue([{ name: "minecraft_overworld", displayName: "Overworld", type: "normal" }, { name: "minecraft_the_nether", displayName: "Nether", type: "nether" }, { name: "minecraft_the_end", displayName: "The End", type: "the_end" }]);
    transport.settings.mockResolvedValue({ maxZoom: 3, defaultZoom: 3, extraZoom: 2, spawn: { x: 0, z: 0 } });
    transport.players.mockResolvedValue([]);
    transport.query.mockImplementation(({ body }: { body: { kind: MinecraftMapData["kind"] } }) => Promise.resolve(data(body.kind)));
    transport.waypoints.mockResolvedValue({ data: [waypoint] });
    transport.create.mockResolvedValue({ data: waypoint });
    transport.update.mockResolvedValue({ data: { ...waypoint, name: "New name" } });
    transport.remove.mockResolvedValue({ data: { acknowledged: true } });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("keeps dimension selection without the atlas header or legacy iframe", async () => {
    const view = render(() => <Minecraft />);
    await screen.findByTestId("terrain");
    expect(view.container.querySelector("iframe")).toBeNull();
    expect(screen.getByLabelText("Dimension")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Original map" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "World atlas" })).toBeNull();
    expect(screen.queryByText(/Predict a fixed Y slice/)).toBeNull();
    expect(screen.queryByLabelText("Predicted biomes")).toBeNull();
    expect(screen.getByRole("button", { name: "Layers" }).getAttribute("aria-expanded")).toBe("false");
  });

  it("shows terrain without waiting for world analysis", async () => {
    const catalog = deferred<ReturnType<typeof data>>(); transport.query.mockReturnValue(catalog.promise);
    render(() => <Minecraft />);
    await screen.findByTestId("terrain");
    await openLayers();
    expect((screen.getByRole("button", { name: "Surveying…" }) as HTMLButtonElement).disabled).toBe(true);
    catalog.resolve(data("catalog"));
    await screen.findByRole("button", { name: "Survey selected area" });
  });

  it("defers an initial hidden-tab catalog until the page becomes visible", async () => {
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    render(() => <Minecraft />); await screen.findByTestId("terrain");
    expect(transport.query).not.toHaveBeenCalled();
    visibility.mockReturnValue("visible"); fireEvent(document, new Event("visibilitychange"));
    await waitFor(() => expect(transport.query).toHaveBeenCalledExactlyOnceWith({ body: { kind: "catalog" } }, expect.objectContaining({ signal: expect.any(AbortSignal) })));
  });

  it("scans an explicit bounded area once and reports partial coverage", async () => {
    const scan = deferred<ReturnType<typeof data>>();
    transport.query.mockImplementation(({ body }: { body: { kind: MinecraftMapData["kind"] } }) => body.kind === "catalog" ? Promise.resolve(data("catalog")) : scan.promise);
    render(() => <Minecraft />); fireEvent.click(await screen.findByTestId("terrain"));
    await observedLayers();
    await waitFor(() => expect(screen.getByText(/X -1/)).toBeTruthy());
    fireEvent.input(screen.getByLabelText("Biome sampling height"), { target: { value: "-16" } });
    await Promise.resolve();
    const button = screen.getByRole("button", { name: "Survey selected area" });
    fireEvent.click(button); fireEvent.click(button);
    await waitFor(() => expect(transport.query).toHaveBeenCalledTimes(2), { timeout: 2000 });
    expect(transport.query.mock.calls[1]?.[0]).toEqual({ body: { kind: "area", world: "minecraft:overworld", chunk_x: -5, chunk_z: -2, width: 8, height: 8, y: -16 } });
    scan.resolve(data("area", { scanned_chunks: 12, missing_chunks: 52, truncated: true, cells: [{ x: 0, z: 0, y: 64, biome: "minecraft:plains" }] }));
    await screen.findByText(/12 chunks read · 52 unavailable/);
    expect(screen.getByText(/Partial results/)).toBeTruthy();
    expect(screen.getByText(/Biome sample: Y -16/)).toBeTruthy();
    expect(screen.getByTestId("terrain").getAttribute("data-cells")).toBe("1");
  });

  it("discards a scan that finishes after switching dimensions", async () => {
    const scan = deferred<ReturnType<typeof data>>();
    transport.query.mockImplementation(({ body }: { body: { kind: MinecraftMapData["kind"] } }) => body.kind === "catalog" ? Promise.resolve(data("catalog")) : scan.promise);
    render(() => <Minecraft />); await screen.findByTestId("terrain");
    await observedLayers();
    fireEvent.click(screen.getByRole("button", { name: "Survey selected area" }));
    await waitFor(() => expect(transport.query).toHaveBeenCalledTimes(2), { timeout: 2000 });
    fireEvent.change(screen.getByLabelText("Dimension"), { target: { value: "minecraft_the_nether" } });
    await waitFor(() => expect(screen.getByTestId("terrain").getAttribute("data-world")).toBe("minecraft_the_nether"));
    expect((transport.query.mock.calls[1]?.[1] as { signal: AbortSignal }).signal.aborted).toBe(false);
    scan.resolve(data("area", { cells: [{ x: 0, z: 0, y: 64, biome: "minecraft:plains" }] }));
    await openLayers();
    await screen.findByRole("button", { name: "Survey selected area" });
    expect(screen.getByTestId("terrain").getAttribute("data-cells")).toBe("0");
    expect(screen.queryByText(/Sampled/)).toBeNull();
  });

  it("retries unavailable analysis without resetting the selected world or point", async () => {
    transport.query.mockRejectedValueOnce(new Error("Unavailable"));
    render(() => <Minecraft />); await screen.findByTestId("terrain");
    fireEvent.change(screen.getByLabelText("Dimension"), { target: { value: "minecraft_the_nether" } });
    await waitFor(() => expect(screen.getByTestId("terrain").getAttribute("data-world")).toBe("minecraft_the_nether"));
    fireEvent.click(screen.getByTestId("terrain"));
    await waitFor(() => expect(screen.getByText(/X -1/)).toBeTruthy());
    await openLayers();
    fireEvent.click(screen.getByRole("button", { name: "Retry analysis" }));
    await screen.findByRole("button", { name: "Survey selected area" });
    expect(screen.getByTestId("terrain").getAttribute("data-world")).toBe("minecraft_the_nether");
    expect(screen.getByText(/X -1/)).toBeTruthy();
    expect(transport.worlds).toHaveBeenCalledTimes(1);
    expect(transport.settings).toHaveBeenCalledTimes(2);
  });

  it("searches blocks within the chosen vertical range", async () => {
    render(() => <Minecraft />); await screen.findByTestId("terrain");
    fireEvent.click(screen.getByRole("button", { name: "Blocks" }));
    const minimum = await screen.findByLabelText("Minimum Y");
    fireEvent.input(minimum, { target: { value: "-32" } });
    fireEvent.input(screen.getByLabelText("Maximum Y"), { target: { value: "0" } });
    await Promise.resolve();
    fireEvent.click(screen.getByRole("button", { name: "Search selected area" }));
    await waitFor(() => expect(transport.query).toHaveBeenCalledTimes(2));
    expect(transport.query.mock.calls[1]?.[0]).toEqual({ body: { kind: "blocks", world: "minecraft:overworld", block: "minecraft:diamond_ore", min_y: -32, max_y: 0, chunk_x: -2, chunk_z: -2, width: 4, height: 4 } });
  });

  it("defaults to surface predictions and reveals the retained underground Y on demand", async () => {
    render(() => <Minecraft />); await screen.findByTestId("terrain"); await openLayers();
    expect((screen.getByLabelText("Predicted biomes") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("terrain").getAttribute("data-seed-y")).toBe("surface");
    expect(screen.queryByLabelText("Prediction Y")).toBeNull();
    fireEvent.change(screen.getByLabelText("Biome view"), { target: { value: "underground" } });
    await screen.findByLabelText("Prediction Y");
    expect(screen.getByTestId("terrain").getAttribute("data-seed-y")).toBe("64");
    expect(screen.queryByText(/128.*128/)).toBeNull();
    fireEvent.click(screen.getByLabelText("Predicted biomes"));
    await waitFor(() => expect(screen.queryByLabelText("Prediction Y")).toBeNull());
  });

  it("enables all canonical dimensions and clamps a retained height to the next world", async () => {
    render(() => <Minecraft />); await screen.findByTestId("terrain"); await openLayers();
    fireEvent.change(screen.getByLabelText("Biome view"), { target: { value: "underground" } });
    fireEvent.input(await screen.findByLabelText("Prediction Y"), { target: { value: "-16" } });
    await waitFor(() => expect(screen.getByTestId("terrain").getAttribute("data-seed-y")).toBe("-16"));
    for (const name of ["the_nether", "the_end"]) {
      fireEvent.change(screen.getByLabelText("Dimension"), { target: { value: `minecraft_${name}` } });
      await waitFor(() => expect(screen.getByTestId("terrain").getAttribute("data-seed-world")).toBe(`minecraft:${name}`));
      expect(screen.getByTestId("terrain").getAttribute("data-seed-y")).toBe("0");
      await openLayers();
      expect(screen.queryByLabelText("Biome view")).toBeNull();
      expect(screen.getByLabelText("Prediction Y")).toBeTruthy();
      expect((screen.getByLabelText("Predicted biomes") as HTMLInputElement).checked).toBe(true);
    }
  });

  it("makes waypoints public while keeping edit controls administrator-only", async () => {
    render(() => <Minecraft />); await screen.findByTestId("terrain");
    fireEvent.click(screen.getByRole("button", { name: "Places" }));
    await screen.findByText("Spawn house");
    expect(screen.queryByRole("button", { name: "Add waypoint" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Edit waypoint/ })).toBeNull();
    expect(transport.create).not.toHaveBeenCalled();
  });

  it("allows administrators to edit and delete a public waypoint", async () => {
    setSuperuser(true); render(() => <Minecraft />); await screen.findByTestId("terrain");
    fireEvent.click(screen.getByRole("button", { name: "Places" }));
    fireEvent.click(await screen.findByRole("button", { name: "Edit waypoint Spawn house" }));
    fireEvent.input(await screen.findByLabelText("Name"), { target: { value: "New name" } });
    await Promise.resolve();
    fireEvent.click(screen.getByRole("button", { name: "Save waypoint" }));
    await waitFor(() => expect(transport.update).toHaveBeenCalledWith({ path: { waypoint_id: waypoint.id }, body: { world: waypoint.world, name: "New name", description: waypoint.description, x: 10, y: 64, z: 20 } }));
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(await screen.findByRole("button", { name: "Delete waypoint New name" }));
    expect(transport.remove).not.toHaveBeenCalled(); confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Delete waypoint New name" }));
    await waitFor(() => expect(transport.remove).toHaveBeenCalledWith({ path: { waypoint_id: waypoint.id } }));
  });
});
