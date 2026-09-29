import { describe, expect, it } from "vitest";
import { mapScale } from "../components/minecraft/mapControls";

describe("Minecraft kilometre scale", () => {
  it.each([
    [3, 3, 100, 100, "0.1 km"],
    [3, 4, 50, 100, "0.05 km"],
    [3, -2, 2000, 62.5, "2 km"],
    [12, 0, 200000, 48.828125, "200 km"],
    [12, 20, 0.2, 51.2, "0.0002 km"],
  ] as const)("corrects squaremap CRS units at native maximum %s and zoom %s", (maxZoom, zoom, metres, pixels, label) => {
    const scale = mapScale(maxZoom, zoom, 1440);
    expect(scale?.metres).toBeCloseTo(metres); expect(scale?.pixels).toBeCloseTo(pixels); expect(scale?.label).toBe(label);
  });

  it("fits resize changes using no more than a quarter of the available width", () => {
    expect(mapScale(3, 3, 400)).toMatchObject({ metres: 100, pixels: 100 });
    expect(mapScale(3, 3, 390)).toMatchObject({ metres: 50, pixels: 50 });
    expect(mapScale(3, 3, 160)).toMatchObject({ metres: 20, pixels: 20 });
    for (const maxZoom of [0, 3, 12]) for (let zoom = -2; zoom <= maxZoom + 8; ++zoom) for (const width of [320, 390, 1440, 3840]) {
      const scale = mapScale(maxZoom, zoom, width);
      if (!scale) throw new Error("Expected a visible scale");
      expect(scale.pixels).toBeLessThanOrEqual(Math.min(120, width / 4) + 1e-9);
      expect(scale.pixels * 2 ** (maxZoom - zoom)).toBeCloseTo(scale.metres);
      expect(Number.parseFloat(scale.label) * 1000).toBeCloseTo(scale.metres);
      expect(scale.label).toMatch(/^\d+(\.\d+)? km$/);
      expect([1, 2, 5]).toContain(Math.round(scale.metres / 10 ** Math.floor(Math.log10(scale.metres))));
    }
  });

  it("withholds the scale for hidden maps or invalid projections", () => {
    for (const width of [0, -10, NaN]) expect(mapScale(3, 3, width)).toBeNull();
    for (const zoom of [NaN, Infinity, -Infinity]) expect(mapScale(3, zoom, 1440)).toBeNull();
  });
});
