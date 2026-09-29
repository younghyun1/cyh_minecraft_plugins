import { createEffect, createSignal, onCleanup, onSettled, untrack } from "solid-js";
import type { MinecraftMapData, MinecraftMapQuery, MinecraftSeedTile } from "../../generated";
import { contractApi } from "../../services/account_api";
import { readSeedTile, readSeedTilePng } from "../../services/minecraft_seed_tile";
import { squaremap, type SquaremapPlayer, type SquaremapSettings, type SquaremapWorld } from "../../services/squaremap";
import { coordinate, scanOrigin, supportsSeedWorld, type MapPoint } from "./mapMath";
import { createMapInspection, createMapQueryGate } from "./mapInspectionState";
import { createSeedTiles } from "./seedTiles";

export type LoadedMap = { readonly world: SquaremapWorld; readonly settings: SquaremapSettings };
export const mapFailure = (cause: unknown) => cause instanceof Error ? cause.message : "The map request failed.";
type PredictionFormat = "binary" | "png";
const predictionFormatKey = "minecraft-prediction-format";
const storedPredictionFormat = (): PredictionFormat => {
  try { return window.localStorage.getItem(predictionFormatKey) === "png" ? "png" : "binary"; }
  catch { return "binary"; }
};
/** World changes invalidate pending results; hover reads share the manual query gate. */
export function createMapExplorer() {
  const [worlds, setWorlds] = createSignal<SquaremapWorld[]>([]);
  const [loaded, setLoaded] = createSignal<LoadedMap | null>(null);
  const [catalog, setCatalog] = createSignal<MinecraftMapData | null>(null);
  const [area, setArea] = createSignal<MinecraftMapData | null>(null);
  const [matches, setMatches] = createSignal<MinecraftMapData | null>(null);
  const [predictionPreset, setPredictionPreset] = createSignal<MinecraftSeedTile["preset"] | null>(null);
  const [predictionNotice, setPredictionNotice] = createSignal("");
  const [predicting, setPredicting] = createSignal(false);
  const [predictionsEnabled, setPredictionsEnabled] = createSignal(true);
  const [predictionY, setPredictionY] = createSignal(64);
  let transportFormat = storedPredictionFormat();
  const [predictionFormat, setPredictionFormat] = createSignal<PredictionFormat>(transportFormat);
  const [areaRegion, setAreaRegion] = createSignal<ReturnType<typeof scanOrigin> | null>(null);
  const [matchRegion, setMatchRegion] = createSignal<ReturnType<typeof scanOrigin> | null>(null);
  const [areaSlice, setAreaSlice] = createSignal<number | null>(null);
  const [matchedBlock, setMatchedBlock] = createSignal("");
  const [players, setPlayers] = createSignal<SquaremapPlayer[]>([]);
  const [point, setPoint] = createSignal<MapPoint>({ x: 0, z: 0 });
  const [selectedRegion, setSelectedRegion] = createSignal<ReturnType<typeof scanOrigin> | null>(null);
  const [view, setView] = createSignal<MapPoint>({ x: 0, z: 0 });
  const [busy, setBusy] = createSignal(false);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal("");
  const [catalogError, setCatalogError] = createSignal("");
  const [playerError, setPlayerError] = createSignal("");
  const [refresh, setRefresh] = createSignal(0);
  let active = true, epoch = 0, metadataPending = false, playersPending = false;
  const lifetime = new AbortController();
  let worldRequest: AbortController | null = null;
  let queryRequest: AbortController | null = null;
  let renderedAt: (point: MapPoint) => boolean = () => false;
  const currentWorld = () => catalog()?.worlds.find(world => world.map_id === loaded()?.world.name);
  const seedWorld = () => { const world = currentWorld(); return world && supportsSeedWorld(world.id) ? world.id : null; };
  const available = () => active && document.visibilityState === "visible";
  const gate = createMapQueryGate(available, value => { if (active) setBusy(value); });
  const inspection = createMapInspection({
    allowed: available,
    context: () => ({ world: currentWorld()?.id ?? null, area: area(), areaSlice: areaSlice(), matches: matches(), matchedBlock: matchedBlock(), seed: (point: MapPoint) => { const world = currentWorld(); return world && !renderedAt(point) ? seedTiles.sample(world.id, point, predictionY()) : null; } }),
    read: async (world, chunk_x, chunk_z) => (await contractApi.minecraftMapQuery({ body: { kind: "area", world, chunk_x, chunk_z, width: 1, height: 1, y: null } }, { signal: lifetime.signal })).data,
    schedule: job => gate.hover(job), clearQueued: () => gate.clearHover(),
  });
  const seedTiles = createSeedTiles({
    read: (query, signal) => transportFormat === "png" ? readSeedTilePng(query, signal) : readSeedTile(query, signal),
    changed: state => { if (active) { setPredictionNotice(state.notice); setPredicting(state.loading); setPredictionPreset(state.preset); } },
    updated: () => queueMicrotask(() => untrack(() => {
      const current = inspection.inspection(), world = currentWorld();
      if (current?.biome?.source === "predicted" || current?.status === "loading" && world && !renderedAt(current.point) && seedTiles.sample(world.id, current.point, predictionY())) inspection.refresh();
    })),
  });
  const selectPredictionFormat = (format: PredictionFormat) => {
    if (transportFormat === format) return;
    transportFormat = format; setPredictionFormat(format);
    // Invalidate permission, cached pixels, and in-flight replies together when changing decoders.
    inspection.invalidate(); seedTiles.invalidate();
    try { window.localStorage.setItem(predictionFormatKey, format); }
    catch { /* A disabled storage backend must not prevent a format change. */ }
  };
  const configureSeeds = () => {
    const world = currentWorld(), y = world ? Math.max(world.min_y, Math.min(world.max_y, predictionY())) : predictionY();
    if (y !== predictionY()) setPredictionY(y);
    seedTiles.configure(seedWorld(), y, available() && predictionsEnabled());
  };
  createEffect(() => [loaded(), catalog(), predictionsEnabled(), predictionY()] as const, () => untrack(configureSeeds));
  const setRenderedLookup = (lookup: (point: MapPoint) => boolean) => { renderedAt = lookup; inspection.refresh(); };

  const refreshPlayers = async () => {
    if (playersPending || !available()) return;
    playersPending = true;
    try {
      const result = await squaremap.players(lifetime.signal);
      if (active) { setPlayers(result); setPlayerError(""); }
    } catch {
      if (active) { setPlayers([]); setPlayerError("Player positions are unavailable."); }
    } finally { playersPending = false; }
  };

  const selectWorld = async (name: string, initialPoint?: MapPoint, candidates = worlds()) => {
    const world = candidates.find(item => item.name === name);
    if (!world) return;
    const revision = ++epoch;
    inspection.invalidate(); gate.invalidate();
    worldRequest?.abort();
    seedTiles.configure(null, predictionY(), false);
    const request = new AbortController(); worldRequest = request;
    setLoaded(null); setArea(null); setMatches(null); setAreaRegion(null); setMatchRegion(null); setSelectedRegion(null); setLoading(true); setError("");
    try {
      const settings = await squaremap.settings(world.name, request.signal);
      if (!active || revision !== epoch) return;
      const center = initialPoint ?? settings.spawn;
      const metadata = catalog()?.worlds.find(item => item.map_id === world.name);
      if (metadata) setPredictionY(value => Math.max(metadata.min_y, Math.min(metadata.max_y, value)));
      setPoint(center); setView(center); setLoaded({ world, settings });
    } catch (cause: unknown) {
      if (active && revision === epoch) setError(mapFailure(cause));
    } finally { if (active && revision === epoch) setLoading(false); }
  };

  const loadCatalog = () => gate.manual({ valid: () => active, run: async () => {
    setCatalogError("");
    try {
      const response = await contractApi.minecraftMapQuery({ body: { kind: "catalog" } }, { signal: lifetime.signal });
      if (active) setCatalog(response.data);
    } catch {
      if (active) { setCatalog(null); setCatalogError("World analysis is unavailable. Terrain and coordinate tools remain available."); }
    } finally { queueMicrotask(inspection.refresh); }
  } });
  const initialize = async () => {
    if (metadataPending) return;
    metadataPending = true; setLoading(true); setError("");
    void loadCatalog();
    try {
      const available = await squaremap.worlds(lifetime.signal);
      if (!active) return;
      setWorlds(available);
      const params = new URLSearchParams(window.location.search);
      const initialWorld = available.find(world => world.name === params.get("world")) ?? available[0];
      const x = coordinate(params.get("x") ?? ""), z = coordinate(params.get("z") ?? "");
      if (initialWorld) await selectWorld(initialWorld.name, x === null || z === null ? undefined : { x, z }, available);
      else { setLoading(false); setError("No rendered worlds are available yet."); }
      void refreshPlayers();
    } catch (cause: unknown) {
      if (active) { setLoading(false); setError(mapFailure(cause)); }
    } finally { metadataPending = false; }
  };

  const query = (body: MinecraftMapQuery) => {
    const revision = epoch;
    gate.manual({ valid: () => revision === epoch, run: async () => {
      const request = new AbortController(); queryRequest = request; setError("");
      try {
        const response = await contractApi.minecraftMapQuery({ body }, { signal: request.signal });
        if (!active || revision !== epoch) return;
        if (body.kind === "area") { setArea(response.data); setAreaRegion(body); setAreaSlice(body.y ?? null); }
        else if (body.kind === "blocks") { setMatches(response.data); setMatchRegion(body); setMatchedBlock(body.block); }
      } catch (cause: unknown) {
        if (active && revision === epoch) setError(mapFailure(cause));
      } finally { queueMicrotask(inspection.refresh); }
    } });
  };

  const scan = (y: number | null) => {
    const world = currentWorld();
    if (world) void query({ kind: "area", world: world.id, ...(selectedRegion() ?? scanOrigin(point(), 8)), y });
  };
  const searchBlocks = (block: string, min_y: number, max_y: number) => {
    const world = currentWorld();
    const region = selectedRegion() ?? scanOrigin(point(), 4);
    if (region.width > 4 || region.height > 4) { setError("Select an area no larger than 64 × 64 blocks for block search."); return; }
    if (world) void query({ kind: "blocks", world: world.id, ...region, block, min_y, max_y });
  };
  const selectPoint = (next: MapPoint) => { setSelectedRegion(null); setPoint(next); };
  const selectRegion = (region: ReturnType<typeof scanOrigin>) => {
    setSelectedRegion(region); setPoint({ x: region.chunk_x * 16 + region.width * 8, z: region.chunk_z * 16 + region.height * 8 });
  };
  const navigate = (next: MapPoint) => { selectPoint(next); setView({ ...next }); inspection.inspect(next); };
  const terrainRefreshing = () => { inspection.invalidate(); seedTiles.refresh(); };
  const refreshMap = () => { terrainRefreshing(); setRefresh(value => value + 1); void refreshPlayers(); };
  const onVisibility = () => {
    if (document.visibilityState !== "visible") { inspection.invalidate(); gate.invalidate(); }
    else if (!catalog() && !catalogError()) loadCatalog();
    configureSeeds();
  };
  onSettled(() => { void initialize(); });
  document.addEventListener("visibilitychange", onVisibility);
  const poll = window.setInterval(() => { if (document.visibilityState === "visible") void refreshPlayers(); }, 5000);
  onCleanup(() => { active = false; ++epoch; seedTiles.dispose(); inspection.invalidate(); gate.invalidate(); lifetime.abort(); worldRequest?.abort(); queryRequest?.abort(); window.clearInterval(poll); document.removeEventListener("visibilitychange", onVisibility); });
  return { worlds, loaded, catalog, area, matches, seedTiles, seedWorld, predictionFormat, selectPredictionFormat, predictionPreset, predictionNotice, predicting, predictionsEnabled, setPredictionsEnabled, predictionY, setPredictionY, setRenderedLookup, refreshInspection: inspection.refresh, areaRegion, matchRegion, areaSlice, matchedBlock, players, point, selectedRegion, selectRegion, view, busy, loading, error, catalogError, playerError, refresh, currentWorld, selectWorld, initialize, loadCatalog, scan, searchBlocks, terrainRefreshing, navigate, setPoint: selectPoint, refreshMap, inspect: inspection.inspect, inspection: inspection.inspection };
}

export type MapExplorer = ReturnType<typeof createMapExplorer>;
