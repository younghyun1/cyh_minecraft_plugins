import { expect, test } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

for (const width of [1440, 390]) {
  test(`continuous seed tiles pan and zoom without clearing loaded neighbors at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
    await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.text().includes("STRICT_READ_UNTRACKED")) errors.push(message.text()); });
    await page.goto("/minecraft");
    const ready = page.locator('.minecraft-seed-tile[data-ready="true"]');
    await expect(ready.first()).toBeVisible();
    await expect.poll(() => fixture.predictions.length).toBeGreaterThan(4);
    expect(fixture.predictions.every(query => query.world === "minecraft:overworld" && query.y === null)).toBe(true);
    expect(fixture.predictions.some(query => query.tile_x < 0 || query.tile_z < 0)).toBe(true);
    await expect(page.locator(".minecraft-prediction-cell")).toHaveCount(0);
    await expect(page.locator(".leaflet-minecraft-seeds-pane")).toHaveCSS("z-index", "190");
    await expect(page.locator(".leaflet-tile-pane")).toHaveCSS("z-index", "200");
    const original = await ready.first().elementHandle(), box = await page.locator(".minecraft-atlas-canvas").boundingBox();
    if (!box) throw new Error("Missing terrain canvas");
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down(); await page.mouse.move(box.x + box.width / 2 - 140, box.y + box.height / 2, { steps: 8 }); await page.mouse.up();
    expect(await original?.evaluate(node => node.isConnected)).toBe(true);
    await expect(ready.first()).toBeVisible();
    await page.getByRole("button", { name: "Zoom out" }).click();
    await expect(page.locator(".minecraft-atlas-canvas")).toHaveAttribute("data-zoom", "3");
    await page.getByRole("button", { name: "Zoom out" }).click();
    await expect.poll(() => fixture.predictions.some(query => query.level > 0)).toBe(true);
    await expect(ready.first()).toBeVisible();
    await page.getByRole("button", { name: "Layers", exact: true }).click();
    await expect(page.getByLabel("Predicted biomes", { exact: true })).toBeChecked();
    await page.getByRole("combobox", { name: "Biome view", exact: true }).selectOption("underground");
    await page.getByLabel("Prediction Y", { exact: true }).fill("-16");
    await expect(page.locator(".minecraft-prediction-receipt")).toContainText("Predicted · Y -16 · Large biomes");
    await expect.poll(() => fixture.predictions.some(query => query.y === -16)).toBe(true);
    await page.getByRole("button", { name: "Close map menu" }).click();
    await page.screenshot({ path: `../target/minecraft-tests/continuous-seed-${width}.png` });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_nether");
    await expect(ready.first()).toBeVisible();
    await expect.poll(() => fixture.predictions.some(query => query.world === "minecraft:the_nether" && query.y === 0)).toBe(true);
    expect(errors).toEqual([]);
  });
}

test("permission refresh retains valid imagery and removes it at expiry when renewal fails", async ({ page }) => {
  await page.clock.install();
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  await page.goto("/minecraft");
  const ready = page.locator('.minecraft-seed-tile[data-ready="true"]');
  await expect(ready.first()).toBeVisible();
  await page.clock.pauseAt(new Date(Date.now() + 1000));
  await page.route("**/api/minecraft/map/seed-tile.{bin,png}", route => route.fulfill({ status: 503, json: { error_code: 89, message: "Unavailable" } }));
  const count = fixture.predictions.length;
  await page.clock.runFor(11000);
  await expect(ready.first()).toBeVisible();
  await page.clock.runFor(5000);
  await expect(ready).toHaveCount(0);
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await expect(page.getByText("Seed map is temporarily unavailable.")).toBeVisible();
  await page.getByLabel("Predicted biomes", { exact: true }).uncheck();
  await page.clock.runFor(20000);
  expect(fixture.predictions.length).toBe(count);
  await expect(page.locator(".minecraft-seed-tile")).toHaveCount(0);
});

for (const [map, world, biome, preset] of [["minecraft_overworld", "minecraft:overworld", "forest", "Large biomes"], ["minecraft_the_nether", "minecraft:the_nether", "warped forest", "Nether"], ["minecraft_the_end", "minecraft:the_end", "end", "The End"]] as const) {
  test(`${preset} predicts by default with dimension-specific hover biomes`, async ({ page }) => {
    await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    await page.route("**/minecraft/map/tiles/**/*.png*", route => route.fulfill({ contentType: "image/svg+xml", body: '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"/>' }));
    await page.goto(`/minecraft?world=${map}&x=4096&z=4096`);
    await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
    await expect.poll(() => fixture.predictions.some(query => query.world === world)).toBe(true);
    const box = await page.locator(".minecraft-atlas-canvas").boundingBox(); if (!box) throw new Error("Missing terrain canvas");
    await page.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2 + 20);
    const inspection = page.getByRole("region", { name: "Terrain inspection" });
    await expect(inspection).toContainText("Predicted biome:"); await expect(inspection).toContainText(biome);
    await page.getByRole("button", { name: "Layers", exact: true }).click();
    await expect(page.getByLabel("Predicted biomes", { exact: true })).toBeChecked();
    await expect(page.locator(".minecraft-prediction-receipt")).toContainText(preset);
  });
}

test("negative zoom keeps native terrain, its frontier, and observed hover above coarse predictions", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  await page.route("**/minecraft/map/tiles/**/*.png*", route => {
    const parts = /\/(\d+)\/(-?\d+)_(-?\d+)\.png/.exec(route.request().url());
    if (!parts) return route.fallback();
    const step = 2 ** (3 - Number(parts[1])), min = Number(parts[2]) * 512 * step;
    const width = Math.max(0, Math.min(512, (192 - min) / step));
    return route.fulfill({ contentType: "image/svg+xml", body: `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><rect width="${width}" height="512" fill="#799452"/></svg>` });
  });
  await page.goto("/minecraft");
  const map = page.locator(".minecraft-atlas-canvas");
  await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
  for (let zoom = 3; zoom >= -2; --zoom) { await page.getByRole("button", { name: "Zoom out" }).click(); await expect(map).toHaveAttribute("data-zoom", String(zoom)); }
  await expect.poll(() => fixture.predictions.some(query => query.level === 5)).toBe(true);
  const nativeTiles = page.locator('.leaflet-tile-pane img.leaflet-tile-loaded[src*="/0/"]');
  await expect(nativeTiles.first()).toBeVisible();
  await expect.poll(() => nativeTiles.count()).toBeGreaterThan(16);
  expect(await nativeTiles.count()).toBeLessThanOrEqual(256);
  await expect.poll(async () => (await nativeTiles.first().boundingBox())?.width).toBe(128);
  expect(await page.locator(".leaflet-tile-pane img").evaluateAll(nodes => nodes.every(node => /\/tiles\/[^/]+\/\d+\//.test((node as HTMLImageElement).src)))).toBe(true);
  const frontier = page.locator(".leaflet-minecraft-frontier-pane canvas");
  await expect.poll(() => frontier.evaluate((node: HTMLCanvasElement) => node.getContext("2d")?.getImageData(0, 0, node.width, node.height).data.some((value, i) => i % 4 === 3 && value > 0))).toBe(true);
  const box = await map.boundingBox(); if (!box) throw new Error("Missing terrain canvas");
  await page.mouse.move(box.x + box.width / 2 - 30, box.y + box.height / 2 + 20);
  await expect(page.getByRole("region", { name: "Terrain inspection" })).toContainText("Observed biome:");
  await page.mouse.move(box.x + box.width / 2 + 80, box.y + box.height / 2 + 20);
  await expect(page.getByRole("region", { name: "Terrain inspection" })).toContainText("Predicted biome:");
  await page.mouse.move(1, 1);
  await page.screenshot({ path: "../target/minecraft-tests/continuous-seed-negative-zoom.png" });
  await page.getByRole("button", { name: "Travel", exact: true }).click();
  await page.getByLabel("Go to X", { exact: true }).fill("-2000000"); await page.getByLabel("Go to Z", { exact: true }).fill("2000000");
  await page.getByRole("button", { name: "Go to coordinates", exact: true }).click();
  await expect.poll(() => fixture.predictions.some(query => query.level === 5 && query.tile_x < -240 && query.tile_z > 240)).toBe(true);
  expect(fixture.predictions.every(query => query.level >= 0 && query.level <= 12)).toBe(true);
  await page.getByRole("button", { name: "Close map menu" }).click();
});

test("actual terrain remains above seed colors and supplies observed hover data", async ({ page }) => {
  await page.clock.install();
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  await page.goto("/minecraft");
  await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
  const box = await page.locator(".minecraft-atlas-canvas").boundingBox(); if (!box) throw new Error("Missing terrain canvas");
  await page.mouse.move(box.x + Math.floor(box.width / 2) + 13, box.y + Math.floor(box.height / 2) + 5);
  await page.clock.runFor(1600);
  await expect(page.getByRole("region", { name: "Terrain inspection" })).toContainText("Observed biome:");
  expect(fixture.queries.some(query => query.kind === "area" && query.width === 1)).toBe(true);
});

test("frontier has no tile seams, hides while zooming, and stays visible while panning", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  await installMinecraftMapMocks(page);
  await page.route("**/minecraft/map/tiles/**/*.png*", route => {
    const parts = /\/(\d+)\/(-?\d+)_(-?\d+)\.png/.exec(route.request().url());
    if (!parts) return route.fallback();
    const step = 2 ** (3 - Number(parts[1])), min = Number(parts[2]) * 512 * step;
    const width = Math.max(0, Math.min(512, (192 - min) / step));
    return route.fulfill({ contentType: "image/svg+xml", body: `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><defs><pattern id="grass" width="16" height="16" patternUnits="userSpaceOnUse"><rect width="16" height="16" fill="#799452"/><rect width="6" height="5" fill="#859d60"/></pattern></defs><rect width="${width}" height="512" fill="url(#grass)"/></svg>` });
  });
  await page.goto("/minecraft");
  await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
  const frontier = page.locator(".leaflet-minecraft-frontier-pane canvas");
  await expect(frontier).toHaveCount(1);
  await expect.poll(() => frontier.evaluate((node: HTMLCanvasElement) => {
    const data = node.getContext("2d")?.getImageData(0, 0, node.width, node.height).data;
    if (!data) return 0;
    const columns = new Set<number>();
    for (let i = 3; i < data.length; i += 4) if ((data[i] ?? 0) > 0) columns.add(Math.floor(i / 4) % node.width);
    return columns.size;
  })).toBeLessThanOrEqual(2);
  await expect.poll(() => frontier.evaluate((node: HTMLCanvasElement) => node.getContext("2d")?.getImageData(0, 0, node.width, node.height).data.some((value, i) => i % 4 === 3 && value > 0))).toBe(true);
  await expect.poll(() => page.locator(".minecraft-seed-tile").evaluateAll(nodes => nodes.every(node => node.getAttribute("data-ready") === "true" && getComputedStyle(node).opacity === "1"))).toBe(true);
  await page.mouse.move(1, 1);
  await page.screenshot({ path: "../target/minecraft-tests/continuous-seed-frontier-1440.png" });
  const pane = page.locator(".leaflet-minecraft-frontier-pane"), map = page.locator(".minecraft-atlas-canvas");
  await page.getByRole("button", { name: "Zoom out" }).click();
  await expect(pane).toHaveCSS("visibility", "hidden");
  await expect(map).toHaveAttribute("data-zoom", "3");
  await expect(pane).toHaveCSS("visibility", "visible");
  await expect.poll(() => frontier.evaluate((node: HTMLCanvasElement) => node.getContext("2d")?.getImageData(0, 0, node.width, node.height).data.some((value, i) => i % 4 === 3 && value > 0))).toBe(true);
  const box = await map.boundingBox(); if (!box) throw new Error("Missing map");
  const frontierX = () => frontier.evaluate((node: HTMLCanvasElement) => {
    const pixels = node.getContext("2d")?.getImageData(0, 0, node.width, node.height).data;
    const first = pixels?.findIndex((value, i) => i % 4 === 3 && value > 0) ?? -1;
    const rect = node.getBoundingClientRect();
    return first < 0 ? -Infinity : rect.x + Math.floor(first / 4) % node.width * rect.width / node.width;
  });
  await expect.poll(async () => Math.abs(await frontierX() - (box.x + box.width / 2 + 64))).toBeLessThanOrEqual(2);
  const mapPane = page.locator(".leaflet-map-pane");
  const mapPaneX = () => mapPane.evaluate(node => node.getBoundingClientRect().x);
  const beforePan = await mapPaneX();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down(); await page.mouse.move(box.x + box.width / 2 - 80, box.y + box.height / 2, { steps: 8 });
  await expect(pane).toHaveCSS("visibility", "visible"); await expect(map).toHaveAttribute("data-zoom", "3");
  await page.mouse.up(); await expect(pane).toHaveCSS("visibility", "visible");
  // Leaflet's drag threshold and inertia alter pointer displacement; the frontier
  // must track the actual map translation without disappearing at the same zoom.
  await expect.poll(async () => await mapPaneX() - beforePan).toBeLessThan(-40);
  await expect.poll(async () => Math.abs(await frontierX() - (box.x + box.width / 2 + 64 + await mapPaneX() - beforePan))).toBeLessThanOrEqual(2);
});

test("failed tiles finish their lifecycle so subsequent zooms can load", async ({ page }) => {
  await page.clock.install();
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  await installMinecraftMapMocks(page);
  let failing = true;
  await page.route("**/api/minecraft/map/seed-tile.{bin,png}", route => failing ? route.fulfill({ status: 503, json: { error_code: 89 } }) : route.fallback());
  await page.goto("/minecraft");
  await expect(page.locator(".minecraft-seed-tile").first()).toBeAttached();
  await page.clock.runFor(13000);
  await expect(page.locator(".minecraft-seed-tile.leaflet-tile-loaded").first()).toBeAttached();
  failing = false;
  await page.getByRole("button", { name: "Zoom out" }).click(); await page.clock.runFor(500);
  await page.getByRole("button", { name: "Zoom out" }).click(); await page.clock.runFor(500);
  await expect(page.locator('.minecraft-seed-tile[data-ready="true"]').first()).toBeVisible();
});

test("Layers switches binary and PNG tiles, clears cached imagery, and remembers the format", async ({ page }) => {
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  await installMinecraftMapMocks(page);
  const requests = { bin: 0, png: 0 };
  page.on("request", request => { if (request.url().endsWith("seed-tile.bin")) ++requests.bin; if (request.url().endsWith("seed-tile.png")) ++requests.png; });
  await page.goto("/minecraft");
  const ready = page.locator('.minecraft-seed-tile[data-ready="true"]');
  await expect(ready.first()).toBeVisible();
  expect(requests.bin).toBeGreaterThan(0); expect(requests.png).toBe(0);
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  const format = page.getByRole("combobox", { name: "Tile format", exact: true });
  await expect(format).toHaveValue("binary");
  await format.selectOption("png");
  await expect.poll(() => requests.png).toBeGreaterThan(0);
  await expect(ready.first()).toBeVisible();
  await page.waitForLoadState("networkidle");
  await page.reload();
  await expect(ready.first()).toBeVisible();
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await expect(format).toHaveValue("png");
  const before = requests.bin;
  await format.selectOption("binary");
  await expect.poll(() => requests.bin).toBeGreaterThan(before);
  await expect(ready.first()).toBeVisible();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("minecraft-prediction-format"))).toBe("binary");
});
