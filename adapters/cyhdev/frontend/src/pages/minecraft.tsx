import { createSignal, For, Match, Show, Switch, untrack } from "solid-js";
import type { MinecraftWaypoint } from "../generated";
import MapCanvas, { type TerrainRefreshState } from "../components/minecraft/MapCanvas";
import MapInspection from "../components/minecraft/MapInspection";
import { MapAnalysis, MapBlockSearch, type LayerControls } from "../components/minecraft/MapAnalysis";
import MapTravel from "../components/minecraft/MapTravel";
import MapWaypoints from "../components/minecraft/MapWaypoints";
import MapPrediction from "../components/minecraft/MapPrediction";
import { createMapExplorer, type MapExplorer } from "../components/minecraft/createMapExplorer";
import type { MapPoint } from "../components/minecraft/mapMath";
import "../styles/minecraft.css";

export default function Minecraft() {
  const explorer = createMapExplorer();
  return (
    <section class="minecraft-page minecraft-atlas" aria-label="Minecraft map">
        <Show when={explorer.loading()}><div class="minecraft-map-notice" role="status">Loading map…</div></Show>
        <Show when={explorer.error()}><div class="minecraft-map-notice" role="alert">{explorer.error()} <Show when={!explorer.loaded()}><button type="button" onClick={() => void explorer.initialize()}>Retry map</button></Show></div></Show>
        <Show when={explorer.loaded()} keyed>{_loaded => <ExplorerWorkspace explorer={explorer} />}</Show>
    </section>
  );
}

function ExplorerWorkspace(props: { readonly explorer: MapExplorer }) {
  const explorer = untrack(() => props.explorer);
  let menuOpener: HTMLButtonElement | undefined;
  const [tab, setTab] = createSignal<"layers" | "blocks" | "places" | "travel" | null>(null);
  const closeMenu = () => { setTab(null); menuOpener?.focus(); };
  const [refreshState, setRefreshState] = createSignal<TerrainRefreshState>("idle");
  const [selecting, setSelecting] = createSignal(false);
  const [biome, setBiome] = createSignal("");
  const [biomes, setBiomes] = createSignal(true), [elevation, setElevation] = createSignal(false);
  const [structures, setStructures] = createSignal(true), [grid, setGrid] = createSignal(false);
  const [measuring, setMeasuring] = createSignal(false), [measureStart, setMeasureStart] = createSignal<MapPoint | null>(null);
  const [waypoints, setWaypoints] = createSignal<readonly MinecraftWaypoint[]>([]);
  const layers: LayerControls = { biome, setBiome, biomes, setBiomes, elevation, setElevation, structures, setStructures, grid, setGrid };
  const tabs = [{ id: "layers", label: "Layers", icon: "▧" }, { id: "blocks", label: "Blocks", icon: "▣" }, { id: "places", label: "Places", icon: "⚑" }, { id: "travel", label: "Travel", icon: "◇" }] as const;
  return <div class="minecraft-atlas-workspace">
    <div class="minecraft-atlas-stage">
      <nav class="minecraft-tool-tabs" aria-label="Map tools"><For each={tabs}>{item => <button type="button" aria-label={item.label} title={item.label} aria-expanded={tab() === item.id ? "true" : "false"} aria-controls="minecraft-map-menu" onClick={event => { menuOpener = event.currentTarget; setSelecting(false); setTab(value => value === item.id ? null : item.id); }}><span aria-hidden="true">{item.icon}</span></button>}</For><button type="button" aria-label="Select area" title="Select area" aria-pressed={selecting() ? "true" : "false"} onClick={event => { menuOpener = event.currentTarget; setTab(null); setSelecting(value => !value); }}><span aria-hidden="true">▱</span></button></nav>
      <section id="minecraft-map-menu" class="minecraft-map-menu" hidden={tab() === null} aria-label="Map menu" onKeyDown={event => { if (event.key === "Escape") closeMenu(); }}>
        <button type="button" class="minecraft-menu-close" aria-label="Close map menu" onClick={closeMenu}>×</button>
        <div class="minecraft-tool-scroll">
        <Show when={explorer.catalogError()}><p class="minecraft-map-warning">{explorer.catalogError()} <button type="button" disabled={explorer.busy()} onClick={() => void explorer.loadCatalog()}>Retry analysis</button></p></Show>
        <Switch>
          <Match when={tab() === "layers"}><MapPrediction explorer={explorer} /><MapAnalysis explorer={explorer} layers={layers} /></Match>
          <Match when={tab() === "blocks"}><MapBlockSearch explorer={explorer} /></Match>
          <Match when={tab() === "travel"}><MapTravel explorer={explorer} measuring={measuring()} start={measureStart()} onMeasure={enabled => { setMeasuring(enabled); setMeasureStart(enabled ? { ...explorer.point() } : null); }} /></Match>
        </Switch>
        <div hidden={tab() !== "places"}><Show when={explorer.currentWorld()} keyed>{world => <MapWaypoints world={world.id} minY={world.min_y} maxY={world.max_y} point={explorer.point()} onChange={setWaypoints} onNavigate={explorer.navigate} />}</Show><Show when={!explorer.currentWorld()}><p class="minecraft-map-warning">Waypoints need world metadata. Retry world analysis to load them.</p></Show></div>
        </div>
      </section>
      <Show when={explorer.loaded()} keyed>{loaded => <MapCanvas mapId={loaded.world.name} settings={loaded.settings} point={explorer.point()} view={explorer.view()} area={explorer.area()} matches={explorer.matches()} seedTiles={explorer.seedTiles} seedWorld={explorer.seedWorld()} predictionY={explorer.predictionY()} predictionsEnabled={explorer.predictionsEnabled()} onRenderedLookup={explorer.setRenderedLookup} onInspectionRefresh={explorer.refreshInspection} areaRegion={explorer.areaRegion()} matchRegion={explorer.matchRegion()} waypoints={waypoints()} players={explorer.players().filter(player => player.world === loaded.world.name)} biome={biome()} biomeLayer={biomes()} elevation={elevation()} structures={structures()} grid={grid()} measuring={measuring()} measureStart={measureStart()} refresh={explorer.refresh()} onPoint={explorer.setPoint} onInspect={explorer.inspect} onTerrainRefresh={explorer.terrainRefreshing} onRefreshState={setRefreshState} selecting={selecting()} region={explorer.selectedRegion()} onSelect={region => { explorer.selectRegion(region); setSelecting(false); setTab("layers"); }} />}</Show>
      <div class="minecraft-map-controls">
        <label class="minecraft-world-picker"><span>Dimension</span><select aria-label="Dimension" value={explorer.loaded()?.world.name ?? ""} disabled={explorer.worlds().length === 0} onChange={event => void explorer.selectWorld(event.currentTarget.value)}><For each={explorer.worlds()}>{world => <option value={world.name}>{world.displayName}</option>}</For></select></label>
        <button type="button" aria-label="Refresh terrain" disabled={refreshState() === "refreshing"} onClick={explorer.refreshMap}>{refreshState() === "refreshing" ? "Refreshing…" : "Refresh terrain"}</button>
      </div>
      <Show when={refreshState() !== "idle"}><p class="minecraft-refresh-status" role="status">{refreshState() === "refreshing" ? "Refreshing terrain…" : refreshState() === "partial" ? "Terrain refreshed; some tiles unavailable" : "Terrain refreshed"}</p></Show>
      <div class="minecraft-selection-readout"><strong>X {explorer.point().x} <span>/</span> Z {explorer.point().z}</strong><Show when={explorer.selectedRegion()}>{region => <small>{region().width * 16} × {region().height * 16} blocks selected</small>}</Show><Show when={selecting()}><small>Drag to select an area</small></Show></div>
      <MapInspection inspection={explorer.inspection()} />
      <div class="minecraft-map-key"><span class="minecraft-key-waypoint">● Waypoint</span><span class="minecraft-key-player">● Player</span><span class="minecraft-key-match">● Block match</span><Show when={explorer.predictionsEnabled() && explorer.predictionPreset()}><span class="minecraft-key-prediction">▧ Predicted · Y {explorer.predictionY()}</span></Show></div>
      <Show when={explorer.playerError()}><p class="minecraft-player-warning">{explorer.playerError()}</p></Show>
    </div>
  </div>;
}
