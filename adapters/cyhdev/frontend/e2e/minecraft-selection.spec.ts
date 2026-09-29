import { expect, test, type Page } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

async function screenPoint(page: Page, x: number, z: number) {
  const box = await page.locator(".minecraft-atlas-canvas").boundingBox();
  if (!box) throw new Error("Minecraft map is missing");
  return { x: box.x + Math.floor(box.width / 2) + (x - 128 + 0.5) * 2, y: box.y + Math.floor(box.height / 2) + (z - 128 + 0.5) * 2 };
}

for (const width of [1440, 390]) {
  test(`drag selection surveys its chunk bounds and keeps closed-menu waypoints visible at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
    await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.text().includes("STRICT_READ_UNTRACKED")) errors.push(message.text()); });
    await page.goto("/minecraft");
    await expect(page.getByRole("region", { name: "Map menu" })).toHaveCount(0);
    await expect(page.locator(".leaflet-tile-loaded").first()).toBeVisible();
    const waypoint = await screenPoint(page, 146, 116);
    await page.mouse.move(waypoint.x, waypoint.y);
    await expect(page.locator(".leaflet-tooltip").filter({ hasText: "Oakridge base" })).toBeVisible();
    const labelBox = await page.locator(".leaflet-tooltip").filter({ hasText: "Oakridge base" }).boundingBox();
    expect(labelBox).not.toBeNull(); expect(labelBox!.width).toBeGreaterThan(120); expect(labelBox!.height).toBeLessThan(100);
    await expect(page.getByRole("button", { name: "Places", exact: true })).toHaveAttribute("aria-expanded", "false");

    const select = page.getByRole("button", { name: "Select area", exact: true });
    await select.click();
    await expect(select).toHaveAttribute("aria-pressed", "true");
    const start = await screenPoint(page, 96, 96), end = await screenPoint(page, 127, 143);
    await page.mouse.move(start.x, start.y); await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 8 }); await page.mouse.up();
    await expect(select).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByRole("button", { name: "Layers", exact: true })).toHaveAttribute("aria-expanded", "true");
    await expect(page.locator(".minecraft-selection-readout")).toContainText("32 × 48 blocks selected");
    await expect(page.locator(".minecraft-selection-readout")).toContainText("X 112 / Z 120");
    await page.getByRole("button", { name: "Survey selected area" }).click();
    await expect(page.getByText(/6 chunks read · 0 unavailable/)).toBeVisible();
    expect(fixture.queries).toContainEqual({ kind: "area", world: "minecraft:overworld", chunk_x: 6, chunk_z: 6, width: 2, height: 3, y: null });
    await page.getByRole("button", { name: "Close map menu" }).click();
    const center = await screenPoint(page, 112, 120);
    await page.mouse.move(center.x, center.y);
    const inspection = page.getByRole("region", { name: "Terrain inspection" });
    await expect(inspection).toContainText("Observed biome: plains");
    const detailBox = await inspection.boundingBox(), selectionBox = await page.locator(".minecraft-selection-readout").boundingBox();
    expect(detailBox).not.toBeNull(); expect(selectionBox).not.toBeNull();
    expect(detailBox!.y + detailBox!.height).toBeLessThan(selectionBox!.y);
    await expect(page.locator(".minecraft-map-key")).not.toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: `/tmp/minecraft-selection-${width}.png` });
    expect(errors).toEqual([]);
  });
}

test.describe("touch area selection", () => {
  test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });
  test("a touch drag selects reversed bounds without panning", async ({ page, browserName }) => {
    test.skip(browserName !== "chromium", "Multi-point touch dispatch uses Chromium's test protocol.");
    await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    await page.goto("/minecraft");
    const select = page.getByRole("button", { name: "Select area", exact: true });
    await select.tap();
    const start = await screenPoint(page, 120, 136), end = await screenPoint(page, 100, 104);
    const client = await page.context().newCDPSession(page);
    // A completed native gesture avoids raw touch dispatch suppressing the next tap in Chromium.
    await client.send("Input.synthesizeScrollGesture", { x: start.x, y: start.y, xDistance: end.x - start.x, yDistance: end.y - start.y, gestureSourceType: "touch", preventFling: true });
    await client.detach();
    await expect(select).toHaveAttribute("aria-pressed", "false");
    await expect(page.locator(".minecraft-selection-readout")).toContainText("32 × 48 blocks selected");
    await expect(page.locator(".minecraft-selection-readout")).toContainText("X 112 / Z 120");
    await page.getByRole("button", { name: "Survey selected area" }).tap();
    await expect(page.getByText(/6 chunks read · 0 unavailable/)).toBeVisible();
    expect(fixture.queries).toContainEqual({ kind: "area", world: "minecraft:overworld", chunk_x: 6, chunk_z: 6, width: 2, height: 3, y: null });
  });
});
