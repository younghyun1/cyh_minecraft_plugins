import { expect, test } from "@playwright/test";
import { installApiMocks, setUiPreferences } from "./fixtures";
import { installMinecraftMapMocks } from "./minecraft-fixtures";

for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  test(`Minecraft map loads only on its page at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await installApiMocks(page, "superuser");
    await setUiPreferences(page, "en-US", "dark");
    await installMinecraftMapMocks(page);
    let requests = 0;
    let privateRequests = 0;
    let hidden = false;
    const actions: unknown[] = [];
    await page.route("**/api/admin/minecraft", async (route) => {
      privateRequests += 1;
      await route.fulfill({ json: { data: { players: [{ id: "00000000-0000-0000-0000-000000000001", name: "Alex", map_hidden: hidden }], whitelist: [], whitelist_enabled: true } } });
    });
    await page.route("**/api/admin/minecraft/actions", async (route) => {
      actions.push(route.request().postDataJSON());
      if (route.request().postDataJSON().action === "map_visibility") hidden = route.request().postDataJSON().hidden;
      await route.fulfill({ json: { data: { acknowledged: true } } });
    });
    await page.route("**/minecraft/map/", async (route) => {
      requests += 1;
      await route.fulfill({ contentType: "text/html", body: "<h1>World map</h1>" });
    });
    await page.goto("/");
    await expect(page.locator(".site-header")).toBeVisible();
    expect(requests).toBe(0);
    if (viewport.width < 768) {
      await page.getByRole("button", { name: "Open sidebar menu" }).click();
    }
    await page.getByRole("button", { name: /^Projects/ }).click();
    await page.getByRole("link", { name: "Minecraft Map" }).click();
    await expect(page).toHaveURL(/\/minecraft$/);
    await expect(page.getByLabel("Dimension", { exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "World atlas" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Original map", exact: true })).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Interactive Minecraft terrain map" })).toBeVisible();
    expect(requests).toBe(0);
    const bounds = await page.locator(".minecraft-page").boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds!.width).toBeLessThanOrEqual(viewport.width);
    expect(bounds!.height).toBeGreaterThan(viewport.height * 0.7);
    expect(bounds!.y).toBeGreaterThanOrEqual(44);
    expect(bounds!.y + bounds!.height).toBeLessThan(viewport.height);
    expect(privateRequests).toBe(0);
    await expect(page.getByLabel("Global message")).toHaveCount(0);
    await expect(page.locator("iframe")).toHaveCount(0);
    await page.goto("/admin/operations");
    expect(privateRequests).toBe(0);
    await page.locator('a[href="/admin/minecraft"]:visible').click();
    await expect(page).toHaveURL(/\/admin\/minecraft$/);
    await expect(page.locator('a[href="/admin/minecraft"]:visible')).toBeInViewport();
    await expect(page.getByText("Alex", { exact: true })).toBeVisible();
    await page.getByLabel("Global message").fill("Hello everyone");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.getByRole("status")).toHaveText("Minecraft acknowledged the request.");
    expect(actions).toEqual([{ action: "message", message: "Hello everyone" }]);
    await page.getByRole("button", { name: "Hide from map: Alex" }).click();
    await expect(page.getByText("Hidden from map", { exact: true })).toBeVisible();
    expect(actions.at(-1)).toEqual({ action: "map_visibility", id: "00000000-0000-0000-0000-000000000001", hidden: true });
    const panel = await page.locator(".minecraft-admin").boundingBox();
    expect(panel!.x).toBeGreaterThanOrEqual(0);
    expect(panel!.x + panel!.width).toBeLessThanOrEqual(viewport.width);
    expect(requests).toBe(0);
    await page.getByRole("button", { name: "Save world" }).scrollIntoViewIfNeeded();
    await expect(page.getByRole("button", { name: "Save world" })).toBeInViewport();
    await page.screenshot({ path: test.info().outputPath("minecraft-controls.png") });
  });
}
