import { expect, test } from "@playwright/test";
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { gzipSync } from "node:zlib";
import type { SeedBenchmarkOptions } from "./seed-benchmark-harness";
import type { MinecraftSeedTileQuery } from "../src/contracts";

type Fixture = { name: string; query: MinecraftSeedTileQuery; binary: string; png: string };
const key = (preset: string, query: MinecraftSeedTileQuery) => `${preset}:${query.world}:${query.level}:${query.tile_x}:${query.tile_z}:${query.y}`;

test("real sampler tiles compare native PNG and binary canvas pan latency", async ({ page }, info) => {
  const smoke = process.env.SEED_BENCHMARK_SMOKE === "1", networks = smoke ? ["local"] : ["local", "1mbps-80ms"];
  const directory = process.env.SEED_BENCHMARK_CORPUS ?? resolve("../../target/browser-tiles");
  const manifest = JSON.parse(await readFile(resolve(directory, "manifest.json"), "utf8")) as Fixture[];
  const fixtures = new Map<string, { binary: Buffer; png: Buffer; pngGzip: Buffer }>();
  for (const fixture of manifest.filter(item => item.name.startsWith("grid/"))) {
    const binary = await readFile(resolve(directory, fixture.binary)), png = await readFile(resolve(directory, fixture.png));
    fixtures.set(key(fixture.name.split("/")[1] ?? "", fixture.query), { binary: gzipSync(binary), png, pngGzip: gzipSync(png) });
  }
  expect(fixtures.size).toBe(360);
  let missingFixtures = 0;
  const server = createServer(async (request, response) => {
    response.setHeader("Access-Control-Allow-Origin", "*"); response.setHeader("Access-Control-Allow-Headers", "Content-Type,X-Benchmark-Preset,X-Benchmark-Wire"); response.setHeader("Access-Control-Allow-Methods", "POST,OPTIONS"); response.setHeader("Access-Control-Expose-Headers", "Content-Length"); response.setHeader("Access-Control-Max-Age", "600"); response.setHeader("Cache-Control", "no-store");
    if (request.method === "OPTIONS") { response.writeHead(204); response.end(); return; }
    let body = ""; for await (const part of request) { body += String(part); if (body.length > 2048) { response.writeHead(413); response.end(); return; } }
    let query: MinecraftSeedTileQuery;
    try { query = JSON.parse(body) as MinecraftSeedTileQuery; } catch { response.writeHead(400); response.end(); return; }
    const preset = String(request.headers["x-benchmark-preset"]), wire = String(request.headers["x-benchmark-wire"]), fixture = fixtures.get(key(preset, query));
    if (!fixture) { ++missingFixtures; response.writeHead(404); response.end(); return; }
    const bytes = wire === "binary" ? fixture.binary : wire === "png-gzip" ? fixture.pngGzip : fixture.png;
    response.setHeader("Content-Type", wire === "binary" ? "application/vnd.cyhdev.biome-tile" : "image/png"); response.setHeader("Content-Length", bytes.length);
    if (wire !== "png") response.setHeader("Content-Encoding", "gzip");
    response.end(bytes);
  });
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  const address = server.address(); if (!address || typeof address === "string") throw new Error("Missing benchmark fixture port");
  const endpoint = `http://127.0.0.1:${address.port}/tiles`;
  type Run = { network: string; preset: string; wire: SeedBenchmarkOptions["wire"]; repeat: number; stages: Awaited<ReturnType<Window["runSeedBenchmark"]>> };
  const runs: Run[] = [], session = await page.context().newCDPSession(page);
  try {
    await page.goto("/e2e/seed-benchmark.html");
    await page.waitForFunction(() => typeof window.runSeedBenchmark === "function");
    await session.send("Network.enable");
    for (const network of networks) {
      await session.send("Network.emulateNetworkConditions", { offline: false, latency: network === "local" ? 0 : 80, downloadThroughput: network === "local" ? -1 : 131_072, uploadThroughput: network === "local" ? -1 : 65_536 });
      for (let repeat = 0; repeat < (smoke ? 1 : 3); ++repeat) for (const preset of smoke ? ["default"] : ["default", "large_biomes", "nether", "end"]) {
        const formats = repeat % 2 === 0 ? ["binary", "png", "png-gzip"] as const : ["png-gzip", "png", "binary"] as const;
        for (const wire of formats) {
          const world = preset === "nether" ? "minecraft:the_nether" : preset === "end" ? "minecraft:the_end" : "minecraft:overworld";
          const stages = await page.evaluate(options => window.runSeedBenchmark(options), { endpoint, preset, world, wire });
          expect(stages.every(stage => stage.hoverHits === 512)).toBe(true);
          expect(stages.find(stage => stage.stage === "revisit")?.requests).toBe(0);
          expect(stages.every(stage => stage.firstFrameMs >= 0)).toBe(true);
          expect(missingFixtures).toBe(0);
          runs.push({ network, preset, wire, repeat, stages });
        }
      }
    }
  } finally { await session.detach(); await new Promise<void>((done, reject) => server.close(error => error ? reject(error) : done())); }
  const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0;
  const summary = networks.flatMap(network => ["binary", "png", "png-gzip"].flatMap(wire => ["cold", "pan", "revisit", "zoom-out"].map(stage => {
    const samples = runs.filter(run => run.network === network && run.wire === wire).flatMap(run => run.stages.filter(value => value.stage === stage));
    const metrics = Object.fromEntries(["firstFrameMs", "completeFrameMs", "decodeMs", "drawMs", "hoverMs", "wireBytes", "requests"].map(name => [name, median(samples.map(value => value[name as keyof typeof value] as number))]));
    return { network, wire, stage, samples: samples.length, ...metrics, stalls: samples.reduce((total, value) => total + value.stalls, 0), maxFrameMs: Math.max(...samples.map(value => value.maxFrameMs)) };
  })));
  const report = { browser: info.project.name, corpus: "seed1/default-large-nether-end/balanced", viewport: { width: 1024, height: 512 }, note: "First/complete frame are the first new rAF after visible tile listeners finish, not display-photon measurements. Same production codecs/cache/Leaflet renderer; fetch buffers replace the small bounded stream-copy helper. Date.now frozen for fixture TTL only; performance.now and animation frames remain real. Binary and png-gzip use native HTTP gzip; decoded binary bodies must start with CYBM; png uses raw image response. Prefetch is enabled and settles between stages. Decode timings sum elapsed async operations, not CPU time. Values describe local fixture delivery, excluding server sampling and encoding.", summary, runs };
  await writeFile(resolve(directory, smoke ? "browser-smoke.json" : "browser-benchmark.json"), JSON.stringify(report, null, 2) + "\n");
  await info.attach("browser-benchmark", { body: JSON.stringify(report, null, 2), contentType: "application/json" });
  console.log(JSON.stringify(summary, null, 2));
});
