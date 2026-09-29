import { expect, test } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { initialWaypoint, installMinecraftMapMocks } from "./minecraft-fixtures";

for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
  test(`explorer surveys terrain, searches blocks, and fits ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await installApiMocks(page, "logged-out");
    await setUiPreferences(page, "en-US", "light");
    const fixture = await installMinecraftMapMocks(page);
    const errors: string[] = []; page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.text().includes("STRICT_READ_UNTRACKED")) errors.push(message.text()); });
    await page.goto("/minecraft");
    await expect(page.locator(".leaflet-tile-loaded").first()).toBeVisible();
    await expect(page.getByRole("region", { name: "Map menu" })).toHaveCount(0);
    await page.getByRole("button", { name: "Layers", exact: true }).click();
    await page.getByLabel("Predicted biomes", { exact: true }).uncheck();
    await expect(page.getByRole("button", { name: "Survey selected area" })).toBeEnabled();
    expect(fixture.queries.map(query => query.kind)).toEqual(["catalog"]);
    await page.getByRole("button", { name: "Survey selected area" }).click();
    await expect(page.getByText(/60 chunks read · 4 unavailable/)).toBeVisible();
    await page.getByText("Highlight biomes", { exact: true }).click();
    await page.getByRole("checkbox", { name: "forest", exact: true }).check();
    await expect(page.getByRole("checkbox", { name: "forest", exact: true })).toBeChecked();
    await page.getByLabel("Surface elevation").check();
    await expect(page.locator(".minecraft-height-legend")).toBeVisible();
    await page.getByLabel("Surface elevation").uncheck();
    await page.getByRole("button", { name: "Clear highlights" }).click();
    await page.getByText(/60 chunks read · 4 unavailable/).scrollIntoViewIfNeeded();
    await page.screenshot({ path: `../target/minecraft-tests/explorer-${viewport.width}.png` });
    const documentWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    expect(documentWidth).toBeLessThanOrEqual(viewport.width);
    const map = await page.locator(".minecraft-atlas-canvas").boundingBox();
    expect(map).not.toBeNull(); expect(map!.height).toBeGreaterThanOrEqual(180); expect(map!.x + map!.width).toBeLessThanOrEqual(viewport.width);

    await page.getByRole("button", { name: "Blocks", exact: true }).click();
    await page.getByLabel("Minimum Y").fill("-32");
    await page.getByLabel("Maximum Y").fill("0");
    await page.getByRole("button", { name: "Search selected area" }).click();
    await expect(page.getByRole("heading", { name: "Matches (2)" })).toBeVisible();
    await expect(page.getByText(/Partial results/)).toBeVisible();
    expect(fixture.queries.at(-1)).toMatchObject({ kind: "blocks", world: "minecraft:overworld", width: 4, height: 4, min_y: -32, max_y: 0 });
    await page.getByRole("button", { name: "X 132 · Y -20 · Z 126", exact: true }).click();
    await expect(page.locator(".minecraft-selection-readout")).toContainText("X 132 / Z 126");

    await page.getByRole("button", { name: "Travel", exact: true }).click();
    await page.getByLabel("Go to X").fill("-80"); await page.getByLabel("Go to Z").fill("160");
    await page.getByRole("button", { name: "Go to coordinates" }).click();
    await expect(page.locator(".minecraft-portal-coordinate")).toHaveText("X -10 · Z 20");
    await page.getByRole("button", { name: "Measure from here" }).click();
    await page.getByRole("button", { name: "Close map menu" }).click();
    await page.locator(".minecraft-atlas-canvas").click({ position: { x: viewport.width / 2, y: 180 } });
    await page.getByRole("button", { name: "Travel", exact: true }).click();
    await expect.poll(async () => Number.parseFloat(await page.locator(".minecraft-distance").textContent() ?? "0")).toBeGreaterThan(0);
    await page.getByRole("button", { name: "Places", exact: true }).click();
    await expect(page.getByRole("button", { name: /Oakridge base/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "Add waypoint" })).toHaveCount(0);
    expect(fixture.writes).toEqual([]);
    expect(errors).toEqual([]);
  });
}

test("administrators create, edit, and delete public waypoints", async ({ page }) => {
  await installApiMocks(page, "superuser"); await setUiPreferences(page, "en-US", "dark");
  const fixture = await installMinecraftMapMocks(page);
  await page.goto("/minecraft");
  await page.getByRole("button", { name: "Places", exact: true }).click();
  await page.getByRole("button", { name: "Add waypoint" }).click();
  await page.getByLabel("Name", { exact: true }).fill("South farm");
  await page.getByLabel("Description", { exact: true }).fill("Wheat and carrots");
  await page.getByRole("button", { name: "Save waypoint" }).click();
  await expect.poll(() => fixture.writes.length).toBe(1);
  expect(fixture.writes[0]).toMatchObject({ method: "POST", body: { name: "South farm", world: "minecraft:overworld", x: 128, z: 128 } });
  await expect(page.locator(".minecraft-waypoint-nameplate").filter({ hasText: "South farm" })).toBeVisible();
  await page.getByRole("button", { name: "Edit waypoint South farm" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Village farm");
  await page.getByRole("button", { name: "Save waypoint" }).click();
  await expect.poll(() => fixture.writes.length).toBe(2);
  expect(fixture.writes[1]).toMatchObject({ method: "PATCH", body: { name: "Village farm" } });
  await expect(page.locator(".minecraft-waypoint-nameplate").filter({ hasText: "South farm" })).toHaveCount(0);
  await expect(page.locator(".minecraft-waypoint-nameplate").filter({ hasText: "Village farm" })).toBeVisible();
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Delete waypoint Village farm" }).first().click();
  await expect.poll(() => fixture.writes.length).toBe(3);
  expect(fixture.writes[2]?.method).toBe("DELETE");
  await expect(page.locator(".minecraft-waypoint-nameplate").filter({ hasText: "Village farm" })).toHaveCount(0);
});

test("public waypoint nameplates preserve literal text and follow their dimension", async ({ page }) => {
  await installApiMocks(page, "logged-out"); await setUiPreferences(page, "en-US", "light");
  const fixture = await installMinecraftMapMocks(page);
  const waypoint = { ...initialWaypoint, name: '<b>Camp</b> & "Home"', description: '<img src="x" onerror="alert(1)"> supplies' };
  await page.route("**/api/minecraft/map/waypoints?*", route => route.fulfill({ json: { data: new URL(route.request().url()).searchParams.get("world") === waypoint.world ? [waypoint] : [] } }));
  await page.goto("/minecraft");
  const nameplate = page.locator(".minecraft-waypoint-nameplate");
  await expect(nameplate).toBeVisible();
  await expect(nameplate).toHaveText(waypoint.name);
  await expect(nameplate.locator("b, img")).toHaveCount(0);
  await expect(nameplate).toHaveCSS("pointer-events", "none");
  await expect(page.getByRole("region", { name: "Map menu" })).toHaveCount(0);

  const canvas = page.locator(".minecraft-atlas-canvas"), box = await canvas.boundingBox();
  if (!box) throw new Error("Minecraft map is missing");
  await canvas.click({ position: { x: Math.floor(box.width / 2) + (waypoint.x - 128) * 2, y: Math.floor(box.height / 2) + (waypoint.z - 128) * 2 } });
  const popup = page.locator(".minecraft-waypoint-popup .leaflet-popup-content");
  await expect(popup).toHaveText(`${waypoint.name} · 146, 72, 116 · ${waypoint.description}`);
  await expect(popup.locator("b, img")).toHaveCount(0);
  await page.getByRole("button", { name: "Close popup" }).click();
  await expect(nameplate).toBeVisible();

  await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_nether");
  await expect(nameplate).toHaveCount(0);
  await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_overworld");
  await expect(nameplate).toBeVisible();
  await expect(nameplate).toHaveText(waypoint.name);
  expect(fixture.writes).toEqual([]);
});
