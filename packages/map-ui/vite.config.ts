import { defineConfig } from "vitest/config";
import solidPlugin from "@solidjs/vite-plugin";

export default defineConfig({
  plugins: [solidPlugin()],
  build: {
    target: "esnext",
    lib: { entry: "src/index.ts", formats: ["es"], fileName: "index", cssFileName: "map-ui" },
    rolldownOptions: { external: [/^(solid-js|@solidjs\/web|leaflet)(\/|$)/] },
  },
  test: { environment: "jsdom", include: ["src/**/*.test.{ts,tsx}"], transformMode: { web: [/\.[jt]sx?$/] } },
});
