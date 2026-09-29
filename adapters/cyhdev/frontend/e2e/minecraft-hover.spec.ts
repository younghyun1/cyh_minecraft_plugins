import { expect, test, type Page, type Route } from "@playwright/test";
import type { MinecraftMapQuery } from "../src/generated";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { emptyMapData, installMinecraftMapMocks } from "./minecraft-fixtures";

type AreaQuery = Extract<MinecraftMapQuery, { kind: "area" }>;
const isInspection = (query: MinecraftMapQuery): query is AreaQuery => query.kind === "area" && query.width === 1 && query.height === 1;

async function setup(page: Page, predictions = false) {
  await page.clock.install();
  await installApiMocks(page, "logged-out");
  await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  if (predictions) await page.route("**/minecraft/map/tiles/**/*.png*", route => route.fulfill({ contentType: "image/svg+xml", body: '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"/>' }));
  await page.goto("/minecraft");
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await expect(page.getByRole("button", { name: "Survey selected area" })).toBeEnabled();
  if (!predictions) await page.getByLabel("Predicted biomes", { exact: true }).uncheck();
  await page.getByRole("button", { name: "Close map menu" }).click();
  await page.clock.pauseAt(new Date(Date.now() + 1000));
  return fixture;
}

/** The fixture starts at (128, 128) and zoom 4, with two screen pixels per block. */
async function screenPoint(page: Page, x: number, z: number) {
  const box = await page.locator(".minecraft-atlas-canvas").boundingBox();
  if (!box) throw new Error("Minecraft terrain is missing");
  // Leaflet rounds its pixel origin; odd viewport sizes put the center at floor(size / 2).
  return { x: box.x + Math.floor(box.width / 2) + (x - 128 + 0.5) * 2, y: box.y + Math.floor(box.height / 2) + (z - 128 + 0.5) * 2 };
}

async function hoverBlock(page: Page, x: number, z: number) {
  const point = await screenPoint(page, x, z);
  await page.mouse.move(point.x, point.y);
}

test("hover inspection debounces movement and reuses a bounded chunk response", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  const fixture = await setup(page);
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => { if (message.text().includes("STRICT_READ_UNTRACKED")) errors.push(message.text()); });
  expect(fixture.queries.filter(isInspection)).toEqual([]);
  await hoverBlock(page, 162, 130);
  await page.clock.runFor(250);
  await hoverBlock(page, 138, 130);
  await page.clock.runFor(499);
  expect(fixture.queries.filter(isInspection)).toEqual([]);
  await page.clock.runFor(2);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText(/Observed biome: plains/i);
  await expect(inspection).toContainText("4 × 4 block sample");
  expect(fixture.queries.filter(isInspection)).toEqual([{ kind: "area", world: "minecraft:overworld", chunk_x: 8, chunk_z: 8, width: 1, height: 1, y: null }]);
  await hoverBlock(page, 142, 130);
  await page.clock.runFor(1500);
  expect(fixture.queries.filter(isInspection)).toHaveLength(1);
  await expect(inspection).toContainText(/X 142 · Z 130/);
  await page.screenshot({ path: "/tmp/minecraft-hover-desktop.png" });
  await hoverBlock(page, 162, 130);
  await page.clock.runFor(550);
  await expect(inspection).toContainText(/Observed biome: forest/i);
  expect(fixture.queries.filter(isInspection)).toHaveLength(2);
  await page.mouse.move(0, 0);
  await expect(inspection).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("survey data remains inspectable with biome colors disabled", async ({ page }) => {
  const fixture = await setup(page);
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.getByRole("button", { name: "Survey selected area" }).click();
  await expect(page.getByText(/60 chunks read · 4 unavailable/)).toBeVisible();
  await page.getByLabel("Biome colors", { exact: true }).uncheck();
  await page.getByRole("button", { name: "Close map menu" }).click();
  await hoverBlock(page, 134, 130);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText(/Observed biome: plains/i);
  await expect(inspection).toContainText(/Surface Y \d+ at sample 132, 128/);
  await page.clock.runFor(1600);
  expect(fixture.queries.filter(isInspection)).toEqual([]);
});

test("prediction inspection preserves its fixed-Y provenance without a terrain read", async ({ page }) => {
  const fixture = await setup(page, true);
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.getByLabel("Prediction Y", { exact: true }).fill("-16");
  await page.clock.runFor(1700);
  await expect(page.locator(".minecraft-prediction-receipt")).toContainText("Predicted · Y -16 · Large biomes");
  await page.getByRole("button", { name: "Close map menu" }).click();
  await hoverBlock(page, 156, 132);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText(/Predicted biome: forest/i);
  await expect(inspection).toContainText(/Y -16/);
  await expect(inspection).not.toContainText("Surface Y");
  await page.clock.runFor(1600);
  expect(fixture.queries.filter(isInspection)).toEqual([]);
});

test("exact searched blocks are identified without labelling nearby blocks", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Blocks", exact: true }).click();
  await page.getByRole("button", { name: "Search selected area" }).click();
  await expect(page.getByRole("heading", { name: "Matches (2)" })).toBeVisible();
  await page.getByRole("button", { name: "Close map menu" }).click();
  await hoverBlock(page, 132, 126);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText("Search match: diamond ore");
  await expect(inspection).toContainText("-20");
  await hoverBlock(page, 133, 126);
  await expect(inspection).not.toContainText("Search match:");
});

test("unavailable chunks are reported without inventing a biome or block", async ({ page }) => {
  await setup(page);
  await page.route("**/api/minecraft/map/query", async route => {
    const query = route.request().postDataJSON() as MinecraftMapQuery;
    if (!isInspection(query)) { await route.fallback(); return; }
    await route.fulfill({ json: { data: { ...emptyMapData("area"), world: query.world, missing_chunks: 1 } } });
  });
  await hoverBlock(page, 134, 130);
  await page.clock.runFor(550);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText("No generated terrain data");
  await expect(inspection).not.toContainText(/Observed biome:|Predicted biome:/);
});

test("late inspection replies cannot cross a dimension change", async ({ page }) => {
  await setup(page);
  let held: { route: Route; query: AreaQuery } | undefined;
  await page.route("**/api/minecraft/map/query", async route => {
    const query = route.request().postDataJSON() as MinecraftMapQuery;
    if (!isInspection(query)) { await route.fallback(); return; }
    if (query.world === "minecraft:overworld") { held = { route, query }; return; }
    await route.fulfill({ json: { data: { ...emptyMapData("area"), world: query.world, scanned_chunks: 1, cells: Array.from({ length: 16 }, (_, i) => ({ x: query.chunk_x * 16 + i % 4 * 4, z: query.chunk_z * 16 + Math.floor(i / 4) * 4, y: 64, biome: "minecraft:warped_forest" })) } } });
  });
  await hoverBlock(page, 134, 130);
  await page.clock.runFor(550);
  await expect.poll(() => held !== undefined).toBe(true);
  await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_nether");
  await expect(page.getByLabel("Dimension", { exact: true })).toHaveValue("minecraft_the_nether");
  if (!held) throw new Error("Expected a pending Overworld inspection");
  await held.route.fulfill({ json: { data: { ...emptyMapData("area"), world: held.query.world, scanned_chunks: 1, cells: [{ x: 132, z: 128, y: 70, biome: "minecraft:desert" }] } } });
  await hoverBlock(page, 134, 130);
  await page.clock.runFor(1600);
  const inspection = page.getByRole("region", { name: "Terrain inspection" });
  await expect(inspection).toContainText(/Observed biome: warped forest/i);
  await expect(inspection).not.toContainText(/desert/i);
});

test.describe("touch inspection", () => {
  test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
  test("tapping terrain exposes a compact readable biome panel", async ({ page }) => {
    const fixture = await setup(page);
    const point = await screenPoint(page, 134, 130);
    await page.touchscreen.tap(point.x, point.y);
    await page.clock.runFor(550);
    const inspection = page.getByRole("region", { name: "Terrain inspection" });
    await expect(inspection).toContainText(/Observed biome: plains/i);
    await expect(inspection).toContainText(/X 134 · Z 130/);
    expect(fixture.queries.filter(isInspection)).toHaveLength(1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    const box = await inspection.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(390);
    await page.screenshot({ path: "/tmp/minecraft-hover-mobile.png" });
  });
});
