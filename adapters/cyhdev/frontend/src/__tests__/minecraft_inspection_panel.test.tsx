import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import MapInspection from "../components/minecraft/MapInspection";
import type { MapInspection as Inspection } from "../components/minecraft/mapInspectionState";

const observation: Inspection = {
  point: { x: -1, z: 19 }, status: "ready", message: "", block: null,
  biome: { name: "minecraft:plains", source: "observed", sample: { x: -4, z: 16 }, y: null, surfaceY: 72 },
};

describe("terrain inspection panel", () => {
  afterEach(cleanup);

  it("identifies the sampled origin and surface height without claiming an exact block", () => {
    render(() => <MapInspection inspection={observation} />);
    const panel = screen.getByRole("region", { name: "Terrain inspection" });
    expect(panel.textContent).toContain("X -1 · Z 19");
    expect(panel.textContent).toContain("Observed biome: plains");
    expect(panel.textContent).toContain("Surface Y 72 at sample -4, 16");
    expect(panel.textContent).toContain("4 × 4 block sample · surface biome");
    expect(panel.hasAttribute("aria-live")).toBe(false);
    expect(screen.queryByText(/Search match/)).toBeNull();
  });

  it("distinguishes fixed-height predictions from surface observations", () => {
    render(() => <MapInspection inspection={{ ...observation, biome: { name: "minecraft:forest", source: "predicted", sample: { x: -4, z: 16 }, y: 64, surfaceY: null } }} />);
    expect(screen.getByText(/Predicted biome:/).textContent).toContain("forest");
    expect(screen.getByText(/4 × 4 block sample/).textContent).toContain("biome Y 64");
    expect(screen.queryByText(/Surface Y/)).toBeNull();
  });
  it("labels predicted surface biomes without claiming a terrain height", () => {
    render(() => <MapInspection inspection={{ ...observation, biome: { name: "minecraft:forest", source: "predicted", sample: { x: -4, z: 16 }, y: null, surfaceY: null } }} />);
    expect(screen.getByText(/Predicted biome:/).textContent).toContain("forest");
    expect(screen.getByText(/4 × 4 block sample/).textContent).toContain("surface biome · approximate");
    expect(screen.queryByText(/Surface Y|biome Y/)).toBeNull();
  });

  it("bounds exact search-match heights in the compact readout", () => {
    render(() => <MapInspection inspection={{ ...observation, biome: null, block: { name: "minecraft:diamond_ore", ys: Array.from({ length: 20 }, (_, index) => index - 32) } }} />);
    expect(screen.getByText(/Search match:/).textContent).toBe("Search match: diamond ore · Y -32, -31, -30, -29, -28, -27, -26, -25 (+12 more)");
  });

  it("shows unavailable data explicitly and clears on pointer leave", () => {
    const view = render(() => <MapInspection inspection={{ ...observation, biome: null, status: "unavailable", message: "No generated terrain data" }} />);
    expect(screen.getByText("No generated terrain data")).toBeTruthy();
    view.unmount();
    render(() => <MapInspection inspection={null} />);
    expect(screen.queryByRole("region", { name: "Terrain inspection" })).toBeNull();
  });
});
