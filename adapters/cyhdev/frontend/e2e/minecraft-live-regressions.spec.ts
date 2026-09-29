import { expect, test, type Page, type Route } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

const tile = '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><rect width="512" height="512" fill="#71944d"/></svg>';
const fulfillTile = (route: Route) => route.fulfill({ contentType: "image/svg+xml", body: tile });

async function setup(page: Page) {
  await page.clock.install();
  await installApiMocks(page, "logged-out");
  await setUiPreferences(page, "en-US", "light");
  await installMinecraftMapMocks(page);
  await page.route("**/minecraft/map/images/**/*.png", fulfillTile);
  await page.route("https://mc-heads.net/avatar/**", fulfillTile);
}

test("manual terrain refresh waits for initial tiles and does not reuse a frozen-clock URL", async ({ page }) => {
  await setup(page);
  const initial: Route[] = [], refreshed: Route[] = [];
  const nonces = new Set<string>();
  let initialComplete = false, holdRefresh = true;
  await page.route("**/minecraft/map/tiles/**/*.png*", async route => {
    const nonce = new URL(route.request().url()).searchParams.get("refresh");
    if (!nonce && !initialComplete) { initial.push(route); return; }
    if (nonce) {
      nonces.add(nonce);
      if (holdRefresh) { refreshed.push(route); return; }
    }
    await fulfillTile(route);
  });
  await page.goto("/minecraft", { waitUntil: "domcontentloaded" });
  const refresh = page.getByRole("button", { name: "Refresh terrain", exact: true });
  await expect(refresh).toBeEnabled();
  await expect.poll(() => initial.length).toBeGreaterThan(0);
  await page.clock.pauseAt(new Date(Date.now() + 1000));
  await refresh.click();
  await expect(refresh).toBeDisabled();
  expect(nonces.size).toBe(0);
  initialComplete = true;
  await Promise.all(initial.map(fulfillTile));
  await expect.poll(() => refreshed.length).toBeGreaterThan(0);
  expect(nonces.size).toBe(1);
  await expect(refresh).toBeDisabled();
  holdRefresh = false;
  await Promise.all(refreshed.map(fulfillTile));
  await expect(refresh).toBeEnabled();
  await expect(page.getByRole("status").filter({ hasText: "Terrain refreshed" })).toBeVisible();
  await refresh.click();
  await expect.poll(() => nonces.size).toBe(2);
  await expect(refresh).toBeEnabled();
});

test("failed refreshed tiles report partial coverage and the next refresh can recover", async ({ page }) => {
  await setup(page);
  let unavailable = true;
  await page.route("**/minecraft/map/tiles/**/*.png*", async route => {
    if (unavailable && new URL(route.request().url()).searchParams.has("refresh")) { await route.fulfill({ status: 404 }); return; }
    await fulfillTile(route);
  });
  await page.goto("/minecraft");
  const refresh = page.getByRole("button", { name: "Refresh terrain", exact: true });
  await refresh.click();
  await expect(page.getByRole("status").filter({ hasText: /tiles unavailable/ })).toBeVisible();
  await expect(refresh).toBeEnabled();
  unavailable = false;
  await refresh.click();
  await expect(page.getByRole("status").filter({ hasText: /^Terrain refreshed$/ })).toBeVisible();
});

for (const width of [1440, 390]) {
  test(`player names, heads, and published vitals remain visible without hovering at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
    await setup(page);
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.text().includes("STRICT_READ_UNTRACKED")) errors.push(message.text()); });
    let moved = false;
    await page.route("**/minecraft/map/tiles/settings.json", route => route.fulfill({ json: { worlds: [{ name: "minecraft_overworld", display_name: "minecraft:overworld", type: "normal" }, { name: "minecraft_the_nether", display_name: "Ashen Realm", type: "nether" }] } }));
    await page.route("**/minecraft/map/tiles/players.json", route => route.fulfill({ json: { max: 100, players: [{ world: "minecraft_overworld", armor: 10, name: "EmeraldRange", x: moved ? 136 : 120, y: 66, health: 20, z: 104, uuid: "61dd44ba6b444b1bbbcb5a838082b3cd", yaw: 23 }] } }));
    await page.goto("/minecraft");
    const plate = page.locator(".minecraft-player-nameplate");
    await expect(plate).toContainText("EmeraldRange");
    await expect(plate).toBeVisible();
    const nameBox = await plate.locator(".minecraft-player-name").boundingBox();
    expect(nameBox).not.toBeNull();
    expect(nameBox!.width).toBeGreaterThan(60);
    expect(nameBox!.height).toBeLessThan(24);
    await expect(plate.getByAltText("Health: 20 of 20")).toBeVisible();
    await expect(plate.getByAltText("Armor: 10 of 20")).toBeVisible();
    await expect(page.locator(".minecraft-player-head")).toHaveAttribute("src", "https://mc-heads.net/avatar/61dd44ba6b444b1bbbcb5a838082b3cd/16");
    await expect(page.getByLabel("Dimension", { exact: true })).toContainText("Overworld");
    await expect(page.getByLabel("Dimension", { exact: true })).toContainText("Ashen Realm");
    const before = await page.locator(".minecraft-player-marker").boundingBox();
    expect(before).not.toBeNull();
    moved = true;
    await page.clock.runFor(5100);
    await expect.poll(async () => (await page.locator(".minecraft-player-marker").boundingBox())?.x).toBeGreaterThan(before!.x);
    await expect(plate).toHaveCount(1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: `/tmp/minecraft-live-${width}.png` });
    await page.getByLabel("Dimension", { exact: true }).selectOption("minecraft_the_nether");
    await expect(plate).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test(`waypoints draw above overlapping player banners and remain clickable at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
    await setup(page);
    await page.route("**/minecraft/map/tiles/players.json", route => route.fulfill({ json: { players: [{ name: "OverlapPlayerBanner", world: "minecraft_overworld", x: 128, z: 116 }] } }));
    await page.goto("/minecraft");
    const waypointPane = page.locator(".leaflet-minecraft-waypoints-pane");
    const marker = waypointPane.locator(".minecraft-waypoint-marker");
    const name = waypointPane.locator(".minecraft-waypoint-nameplate");
    const banner = page.locator(".minecraft-player-nameplate");
    await expect(name).toHaveText("Oakridge base");
    await expect(marker).toBeVisible();
    await expect(banner).toHaveText("OverlapPlayerBanner");
    await expect(waypointPane).toHaveCSS("z-index", "660");
    await expect(page.locator(".leaflet-minecraft-player-nameplates-pane")).toHaveCSS("z-index", "650");
    await expect(page.locator(".leaflet-popup-pane")).toHaveCSS("z-index", "700");
    const bannerBox = await banner.boundingBox();
    if (!bannerBox) throw new Error("Player banner is missing");
    for (const overlay of [marker, name]) {
      const box = await overlay.boundingBox();
      if (!box) throw new Error("Waypoint overlay is missing");
      expect(Math.min(box.x + box.width, bannerBox.x + bannerBox.width) - Math.max(box.x, bannerBox.x)).toBeGreaterThan(0);
      expect(Math.min(box.y + box.height, bannerBox.y + bannerBox.height) - Math.max(box.y, bannerBox.y)).toBeGreaterThan(0);
    }
    await page.screenshot({ path: `/tmp/minecraft-waypoint-overlap-${width}.png` });
    await marker.click();
    await expect(page.locator(".minecraft-waypoint-popup .leaflet-popup-content")).toHaveText("Oakridge base · 146, 72, 116 · Storage, beds, and the northern trail");
    await expect(banner).toBeVisible();
    await expect(name).toBeVisible();
  });
}

test("player names remain text and unavailable head images fall back to the local marker", async ({ page }) => {
  await setup(page);
  await page.route("https://mc-heads.net/avatar/**", route => route.abort());
  const name = '<img src=x onerror="alert(1)">';
  await page.route("**/minecraft/map/tiles/players.json", route => route.fulfill({ json: { players: [{ name, uuid: "61dd44ba6b444b1bbbcb5a838082b3cd", world: "minecraft_overworld", x: 120, z: 104 }] } }));
  await page.goto("/minecraft");
  await expect(page.locator(".minecraft-player-name")).toHaveText(name);
  await expect(page.locator(".minecraft-player-name img")).toHaveCount(0);
  await expect(page.locator(".minecraft-player-head")).toHaveAttribute("src", "/minecraft/map/images/icon/player.png");
});
