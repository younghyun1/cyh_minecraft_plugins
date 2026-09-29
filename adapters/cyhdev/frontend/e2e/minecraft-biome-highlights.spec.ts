import { expect, test, type Page } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

async function setup(page: Page, width: number) {
  await page.setViewportSize({ width, height: 900 });
  await installApiMocks(page, "logged-out");
  await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  await page.goto("/minecraft");
  await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.locator(".minecraft-biome-picker summary").click();
  return fixture;
}

async function alphaAt(page: Page, x: number): Promise<number> {
  const tile = page.locator('.minecraft-biome-highlight-tile[data-tile-x="0"][data-tile-z="0"][data-level="0"]');
  await expect(tile).toHaveAttribute("data-ready", "true");
  return tile.evaluate((node, pixelX) => {
    const context = (node as HTMLCanvasElement).getContext("2d");
    if (!context) throw new Error("Highlight canvas is unavailable");
    return context.getImageData(pixelX, 4, 1, 1).data[3] ?? 0;
  }, x);
}

for (const width of [1440, 390]) {
  test(`selected biomes highlight cached prediction tiles at ${width}px`, async ({ page }) => {
    const fixture = await setup(page, width);
    await expect(page.locator(".minecraft-biome-highlight-tile")).toHaveCount(0);
    await page.getByRole("checkbox", { name: "plains", exact: true }).check();
    expect(await alphaAt(page, 4)).toBeGreaterThan(0);
    expect(await alphaAt(page, 60)).toBe(0);
    const initialRequests = fixture.predictions.filter(query => query.tile_x === 0 && query.tile_z === 0 && query.level === 0).length;
    await page.getByRole("checkbox", { name: "forest", exact: true }).check();
    expect(await alphaAt(page, 60)).toBeGreaterThan(0);
    await page.getByRole("checkbox", { name: "plains", exact: true }).uncheck();
    expect(await alphaAt(page, 4)).toBe(0);
    expect(await alphaAt(page, 60)).toBeGreaterThan(0);
    expect(fixture.predictions.filter(query => query.tile_x === 0 && query.tile_z === 0 && query.level === 0)).toHaveLength(initialRequests);
    await expect(page.locator('.leaflet-minecraft-biome-highlights-pane')).toHaveCSS("z-index", "410");
    await page.screenshot({ path: test.info().outputPath(`biome-highlights-${width}.png`) });
    await page.getByLabel("Tile format").selectOption("png");
    expect(await alphaAt(page, 4)).toBe(0);
    expect(await alphaAt(page, 60)).toBeGreaterThan(0);
    await page.getByRole("button", { name: "Zoom out", exact: true }).click();
    await expect(page.locator(".minecraft-atlas-canvas")).toHaveAttribute("data-zoom", "3");
    await page.getByRole("button", { name: "Zoom out", exact: true }).click();
    await expect(page.locator(".minecraft-atlas-canvas")).toHaveAttribute("data-zoom", "2");
    await expect(page.locator('.minecraft-biome-highlight-tile[data-level="1"][data-ready="true"]').first()).toBeVisible();
    await page.getByRole("button", { name: "Clear highlights" }).click();
    await expect(page.locator(".minecraft-biome-highlight-tile")).toHaveCount(0);
  });
}

test("surveyed biomes highlight with predictions disabled, and the picker follows dimensions", async ({ page }) => {
  await setup(page, 1440);
  await page.getByLabel("Predicted biomes", { exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "forest", exact: true }).check();
  await page.getByRole("button", { name: "Survey selected area" }).click();
  const observed = page.locator(".leaflet-minecraft-biome-highlights-pane canvas.leaflet-zoom-animated");
  await expect(observed).toBeAttached();
  await expect.poll(() => observed.evaluate(node => {
    const canvas = node as HTMLCanvasElement, context = canvas.getContext("2d");
    if (!context) return false;
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    for (let index = 3; index < pixels.length; index += 4) if (pixels[index] !== 0) return true;
    return false;
  })).toBe(true);
  await expect(page.locator(".minecraft-biome-highlight-tile")).toHaveCount(0);
  await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_nether");
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.locator(".minecraft-biome-picker summary").click();
  await expect(page.getByRole("checkbox", { name: "warped forest", exact: true })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "forest", exact: true })).toHaveCount(0);
  await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_end");
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.locator(".minecraft-biome-picker summary").click();
  await expect(page.getByRole("checkbox", { name: "end highlands", exact: true })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "warped forest", exact: true })).toHaveCount(0);
});
