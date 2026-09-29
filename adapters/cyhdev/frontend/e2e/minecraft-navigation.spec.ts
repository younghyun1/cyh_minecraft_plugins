import { expect, test, type Page } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

async function setup(page: Page, maxZoom = 3) {
  await installApiMocks(page, "logged-out");
  await setUiPreferences(page, "en-US", "light");
  await installMinecraftMapMocks(page);
  await page.route("**/minecraft/map/tiles/*/settings.json", route => route.fulfill({ json: { zoom: { max: maxZoom, def: maxZoom, extra: 2 }, spawn: { x: 128, z: 128 } } }));
  await page.route("**/minecraft/map/tiles/players.json", route => route.fulfill({ json: { players: ["minecraft_overworld", "minecraft_the_nether", "minecraft_the_end"].map(world => ({ name: "Origin", world, x: 0, z: 0 })) } }));
  await page.goto("/minecraft?x=4096&z=-8192");
  await expect(page.locator(".minecraft-atlas-canvas")).toHaveAttribute("data-zoom", String(maxZoom));
}

async function zoomTo(page: Page, target: number) {
  const map = page.locator(".minecraft-atlas-canvas");
  let zoom = Number(await map.getAttribute("data-zoom"));
  while (zoom !== target) {
    await page.getByRole("button", { name: zoom < target ? "Zoom in" : "Zoom out", exact: true }).click();
    zoom += zoom < target ? 1 : -1;
    await expect(map).toHaveAttribute("data-zoom", String(zoom));
  }
}

for (const width of [1440, 390]) {
  test(`Home centers every dimension without changing zoom at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await setup(page);
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    const dimension = page.getByLabel("Dimension", { exact: true });
    for (const [index, world] of ["minecraft_overworld", "minecraft_the_nether", "minecraft_the_end"].entries()) {
      await dimension.selectOption(world);
      await zoomTo(page, 4);
      const home = page.getByRole("button", { name: "Home", exact: true });
      await expect(home).toHaveCount(1);
      await expect(home).toBeInViewport();
      if (index === 0) { await home.focus(); await page.keyboard.press("Enter"); }
      else if (index === 1) { await home.focus(); await page.keyboard.press("Space"); }
      else await home.click();
      await expect(dimension).toHaveValue(world);
      await expect(page.locator(".minecraft-atlas-canvas")).toHaveAttribute("data-zoom", "4");
      await expect.poll(async () => {
        const map = await page.locator(".minecraft-atlas-canvas").boundingBox();
        const marker = await page.locator(".minecraft-player-marker").boundingBox();
        return map && marker ? Math.hypot(marker.x + marker.width / 2 - map.x - map.width / 2, marker.y + marker.height / 2 - map.y - map.height / 2) : Infinity;
      }).toBeLessThan(2);
      await expect(page.locator(".minecraft-map-scale")).toHaveCount(1);
      const zoomControl = await page.locator(".leaflet-control-zoom").boundingBox();
      const dimensionControl = await page.locator(".minecraft-world-picker").boundingBox();
      expect(Math.abs((zoomControl?.y ?? Infinity) - (dimensionControl?.y ?? -Infinity))).toBeLessThan(1);
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    expect(errors).toEqual([]);
    await page.screenshot({ path: test.info().outputPath(`minecraft-navigation-${width}.png`) });
  });
}

for (const maxZoom of [3, 5]) {
  test(`centered metre and kilometre scale matches block distances and resize with native zoom ${maxZoom}`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await setup(page, maxZoom);
    const scale = page.locator(".minecraft-map-scale-line");
    const labels = page.locator(".minecraft-map-scale-label");
    const label = labels.last();
    const verify = async (zoom: number) => {
      await expect(label).toHaveText(/^\d+(?:\.\d+)? (?:m|km)$/);
      const distance = await label.innerText();
      const metres = Number.parseFloat(distance) * (distance.endsWith("km") ? 1000 : 1);
      const box = await scale.boundingBox();
      const map = await page.locator(".minecraft-atlas-canvas").boundingBox();
      expect(box).not.toBeNull();
      expect(Math.abs((box?.x ?? Infinity) + (box?.width ?? 0) / 2 - (map?.x ?? 0) - (map?.width ?? 0) / 2)).toBeLessThan(1);
      expect((map?.y ?? 0) + (map?.height ?? 0) - (box?.y ?? 0) - (box?.height ?? 0)).toBeLessThan(60);
      // The line's full CSS width represents the printed physical distance.
      expect(Math.abs((box?.width ?? 0) * 2 ** (maxZoom - zoom) - metres)).toBeLessThan(2 ** (maxZoom - zoom) + 0.01);
      await expect(page.locator(".minecraft-map-scale-mark")).toHaveCount(5);
      await expect(labels).toHaveCount(5);
    };
    for (const zoom of [maxZoom, maxZoom + 2, maxZoom - 1, -2, maxZoom]) {
      await zoomTo(page, zoom);
      await verify(zoom);
    }
    await expect(label).toHaveText("500 m");
    await page.setViewportSize({ width: 320, height: 700 });
    await expect(label).toHaveText("200 m");
    await verify(maxZoom);
    await expect(scale).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(320);
  });
}
