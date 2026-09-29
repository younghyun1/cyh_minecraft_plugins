import { expect, test } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

for (const format of ["binary", "png"] as const) {
  test(`${format} defaults to surface predictions and preserves explicit underground height`, async ({ page }) => {
    await page.setViewportSize({ width: format === "png" ? 390 : 1440, height: 900 });
    await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    await page.route("**/minecraft/map/tiles/**/*.png*", route => route.fulfill({ contentType: "image/svg+xml", body: '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"/>' }));
    await page.goto("/minecraft?x=4096&z=4096");
    const ready = page.locator('.minecraft-seed-tile[data-ready="true"]'), menu = page.getByRole("button", { name: "Layers", exact: true });
    await expect(ready.first()).toBeVisible();
    expect(fixture.predictions.every(query => query.world === "minecraft:overworld" && query.y === null)).toBe(true);
    await menu.click();
    await expect(page.getByRole("combobox", { name: "Biome view", exact: true })).toHaveValue("surface");
    await expect(page.getByLabel("Prediction Y", { exact: true })).toHaveCount(0);
    if (format === "png") {
      const png = page.waitForResponse(response => response.url().endsWith("seed-tile.png") && response.ok());
      await page.getByRole("combobox", { name: "Tile format", exact: true }).selectOption("png"); await png;
    }
    await expect(page.locator(".minecraft-prediction-receipt")).toContainText("Predicted · Surface · Large biomes");
    if (format === "binary") await page.screenshot({ path: "../target/minecraft-tests/surface-mode.png" });
    await page.getByRole("button", { name: "Close map menu" }).click();
    const box = await page.locator(".minecraft-atlas-canvas").boundingBox(); if (!box) throw new Error("Missing map");
    const hover = () => page.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2 + 20);
    const inspection = page.getByRole("region", { name: "Terrain inspection" });
    await hover(); await expect(inspection).toContainText("Predicted biome:"); await expect(inspection).toContainText("surface biome");
    await expect(inspection).not.toContainText("Surface Y"); await expect(inspection).not.toContainText("biome Y");
    await menu.click();
    await page.getByRole("combobox", { name: "Biome view", exact: true }).selectOption("underground");
    await expect(page.getByLabel("Prediction Y", { exact: true })).toHaveValue("64");
    await page.getByLabel("Prediction Y", { exact: true }).fill("-16");
    await expect.poll(() => fixture.predictions.some(query => query.y === -16)).toBe(true);
    await expect(page.locator(".minecraft-prediction-receipt")).toContainText("Predicted · Y -16");
    if (format === "binary") await page.screenshot({ path: "../target/minecraft-tests/underground-mode.png" });
    await page.getByRole("button", { name: "Close map menu" }).click();
    await hover(); await expect(inspection).toContainText("biome Y -16"); await expect(inspection).not.toContainText("surface biome");
    await menu.click();
    await page.getByRole("combobox", { name: "Biome view", exact: true }).selectOption("surface");
    await expect(page.getByLabel("Prediction Y", { exact: true })).toHaveCount(0);
    await expect(page.locator(".minecraft-prediction-receipt")).toContainText("Predicted · Surface");
    await page.getByRole("combobox", { name: "Biome view", exact: true }).selectOption("underground");
    await expect(page.getByLabel("Prediction Y", { exact: true })).toHaveValue("-16");
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(format === "png" ? 390 : 1440);
  });
}
