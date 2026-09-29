import { describe, expect, it } from "vitest";
import { mapScale } from "../components/minecraft/mapControls";

describe("Minecraft distance scale", () => {
  it.each([
    [3, 3, 500, 500, "500 m"],
    [3, 4, 200, 400, "200 m"],
    [3, -2, 10000, 312.5, "10 km"],
    [12, 0, 2000000, 488.28125, "2000 km"],
    [12, 20, 1, 256, "1 m"],
  ] as const)("corrects squaremap CRS units at native maximum %s and zoom %s", (maxZoom, zoom, metres, pixels, label) => {
    const scale = mapScale(maxZoom, zoom, 1440);
    expect(scale?.metres).toBeCloseTo(metres); expect(scale?.pixels).toBeCloseTo(pixels); expect(scale?.label).toBe(label);
  });

  it("fits a long ruler within the viewport and labels quarter distances", () => {
    expect(mapScale(3, 3, 400)).toMatchObject({ metres: 200, pixels: 200, ticks: ["0", "50", "100", "150", "200"] });
    expect(mapScale(3, 3, 320)).toMatchObject({ metres: 200, pixels: 200 });
    expect(mapScale(3, -2, 1440)?.ticks).toEqual(["0", "2.5", "5", "7.5", "10"]);
    for (const maxZoom of [0, 3, 12]) for (let zoom = -2; zoom <= maxZoom + 8; ++zoom) for (const width of [320, 390, 1440, 3840]) {
      const scale = mapScale(maxZoom, zoom, width);
      if (!scale) throw new Error("Expected a visible scale");
      expect(scale.pixels).toBeLessThanOrEqual(Math.min(500, width * 0.65) + 1e-9);
      expect(scale.pixels * 2 ** (maxZoom - zoom)).toBeCloseTo(scale.metres);
      expect(Number.parseFloat(scale.label) * (scale.label.endsWith("km") ? 1000 : 1)).toBeCloseTo(scale.metres);
      expect(scale.label).toMatch(/^\d+(\.\d+)? (?:m|km)$/);
      expect([1, 2, 4, 5, 8]).toContain(Math.round(scale.metres / 10 ** Math.floor(Math.log10(scale.metres))));
    }
  });

  it("withholds the scale for hidden maps or invalid projections", () => {
    for (const width of [0, -10, NaN]) expect(mapScale(3, 3, width)).toBeNull();
    for (const zoom of [NaN, Infinity, -Infinity]) expect(mapScale(3, zoom, 1440)).toBeNull();
  });
});
