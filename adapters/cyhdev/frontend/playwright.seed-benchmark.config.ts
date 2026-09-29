import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e", testMatch: "minecraft-seed-benchmark.spec.ts", timeout: 240_000, workers: 1, reporter: "line",
  use: { baseURL: "http://127.0.0.1:3000", trace: "retain-on-failure" },
  webServer: { command: "npm run dev -- --host 127.0.0.1", url: "http://127.0.0.1:3000", reuseExistingServer: true, timeout: 30_000 },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
