import { createEffect, createSignal, onCleanup, onSettled, untrack } from "solid-js";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import type { MinecraftMapData, MinecraftWaypoint } from "../../contracts";
import type { SquaremapPlayer, SquaremapSettings } from "../../services/squaremap";
import { biomeColor, displayName, elevationColor, selectionRegion, WORLD_LIMIT, type scanOrigin, type MapPoint } from "./mapMath";
import { createSeedTileLayer, createTerrainBoundary, minimumMapZoom } from "./seedTileLayer";
import { type SeedTiles } from "./seedTiles";
import { createTerrainRefresh, type TerrainRefreshState } from "./terrainRefresh";
import { createMapControls } from "./mapControls";

export type { TerrainRefreshState } from "./terrainRefresh";

export type MapAssets = {
  readonly terrainTileUrl: (mapId: string, refresh?: number) => string;
  readonly playerFallbackUrl: string;
  readonly playerHeadUrl?: (uuid: string) => string;
  readonly playerVitalUrl: (kind: "health" | "armor", value: number) => string;
};

export type MapCanvasProps = {
  readonly assets: MapAssets;
  readonly mapId: string;
  readonly settings: SquaremapSettings;
  readonly point: MapPoint;
  readonly view: MapPoint;
  readonly area: MinecraftMapData | null;
  readonly matches: MinecraftMapData | null;
  readonly seedTiles: SeedTiles;
  readonly seedWorld: string | null;
  readonly predictionY: number | null;
  readonly predictionsEnabled: boolean;
  readonly onRenderedLookup: (lookup: (point: MapPoint) => boolean) => void;
  readonly onInspectionRefresh: () => void;
  readonly areaRegion: ReturnType<typeof scanOrigin> | null;
  readonly matchRegion: ReturnType<typeof scanOrigin> | null;
  readonly waypoints: readonly MinecraftWaypoint[];
  readonly players: readonly SquaremapPlayer[];
  readonly biome: string;
  readonly biomeLayer: boolean;
  readonly elevation: boolean;
  readonly structures: boolean;
  readonly grid: boolean;
  readonly measuring: boolean;
  readonly measureStart: MapPoint | null;
  readonly refresh: number;
  readonly onPoint: (point: MapPoint) => void;
  readonly onInspect: (point: MapPoint | null) => void;
  readonly onTerrainRefresh: () => void;
  readonly onRefreshState: (state: TerrainRefreshState) => void;
  readonly selecting?: boolean;
  readonly region?: ReturnType<typeof selectionRegion> | null;
  readonly onSelect?: (region: ReturnType<typeof selectionRegion>) => void;
};

/** All plugin text enters Leaflet through DOM text nodes, never HTML strings. */
function label(value: string): HTMLElement {
  const node = document.createElement("span");
  node.textContent = value;
  return node;
}

export default function MapCanvas(props: MapCanvasProps) {
  let element: HTMLDivElement | undefined;
  let map: L.Map | undefined;
  let tiles: L.TileLayer | undefined;
  let observer: ResizeObserver | undefined;
  let refreshTimer: number | undefined;
  let seedLayer: ReturnType<typeof createSeedTileLayer> | undefined;
  let boundary: ReturnType<typeof createTerrainBoundary> | undefined;
  let terrainRefresh: ReturnType<typeof createTerrainRefresh> | undefined;
  let removeControls: (() => void) | undefined;
  let dragStart: MapPoint | null = null, dragEnd: MapPoint | null = null, dragPointer: number | null = null, suppressClickUntil = 0;
  const [ready, setReady] = createSignal(false);
  const sampled = L.layerGroup(), markers = L.layerGroup(), playersLayer = L.layerGroup(), selection = L.layerGroup(), grid = L.layerGroup();
  const waypointRenderer = L.svg({ pane: "minecraft-waypoints" });
  const dragged = L.layerGroup();
  // The parent keys this component by its loaded world, so projection settings are immutable here.
  const settings = untrack(() => props.settings), worldMapId = untrack(() => props.mapId), assets = untrack(() => props.assets);
  const scale = 2 ** settings.maxZoom;
  const position = (point: MapPoint): L.LatLngTuple => [-point.z / scale, point.x / scale];
  const bounds = (x: number, z: number, endX: number, endZ: number): L.LatLngBoundsExpression => [position({ x, z }), position({ x: endX, z: endZ })];
  const invalidate = () => {
    if (!map || !element) return;
    map.setMinZoom(minimumMapZoom(element.clientWidth, element.clientHeight, settings.maxZoom));
    map.invalidateSize({ animate: false });
  };
  const clearInspection = () => untrack(() => props.onInspect(null));
  const eventPoint = (event: MouseEvent): MapPoint | null => {
    if (!map) return null;
    const cursor = map.mouseEventToLatLng(event);
    const x = Math.floor(cursor.lng * scale), z = Math.floor(-cursor.lat * scale);
    return Math.abs(x) <= WORLD_LIMIT && Math.abs(z) <= WORLD_LIMIT ? { x, z } : null;
  };
  // Leaflet canvas events throttle movement and snap small marker hits to their centers.
  const inspectCursor = (event: MouseEvent) => untrack(() => {
    if (props.selecting || event.buttons !== 0 || event.target instanceof Element && event.target.closest(".leaflet-control")) { clearInspection(); return; }
    props.onInspect(eventPoint(event));
  });
  const drawDrag = (region: ReturnType<typeof selectionRegion>) => {
    dragged.clearLayers();
    L.rectangle(bounds(region.chunk_x * 16, region.chunk_z * 16, (region.chunk_x + region.width) * 16, (region.chunk_z + region.height) * 16), { color: "#ffffff", weight: 2, fillColor: "#ffcd6e", fillOpacity: 0.15, interactive: false }).addTo(dragged);
  };
  const beginSelection = (event: PointerEvent) => untrack(() => {
    if (!props.selecting || !event.isPrimary || event.button !== 0 || event.target instanceof Element && event.target.closest(".leaflet-control")) return;
    const point = eventPoint(event);
    if (!point) return;
    event.preventDefault(); dragStart = point; dragEnd = point; dragPointer = event.pointerId;
    element?.setPointerCapture(event.pointerId); clearInspection(); drawDrag(selectionRegion(point, point));
  });
  const moveSelection = (event: PointerEvent) => {
    if (dragPointer !== event.pointerId || !dragStart) return;
    const point = eventPoint(event);
    if (point) { dragEnd = point; drawDrag(selectionRegion(dragStart, point)); }
  };
  const endSelection = (event: PointerEvent) => untrack(() => {
    if (dragPointer !== event.pointerId || !dragStart || !dragEnd) return;
    const region = selectionRegion(dragStart, eventPoint(event) ?? dragEnd);
    dragStart = null; dragEnd = null; dragPointer = null; suppressClickUntil = Date.now() + 150;
    if (element?.hasPointerCapture(event.pointerId)) element.releasePointerCapture(event.pointerId);
    props.onSelect?.(region);
  });
  const cancelSelection = () => { dragStart = null; dragEnd = null; dragPointer = null; dragged.clearLayers(); };

  const drawGrid = () => {
    grid.clearLayers();
    if (!map || !untrack(() => props.grid)) return;
    const visible = map.getBounds();
    const minX = Math.max(-WORLD_LIMIT, Math.floor(visible.getWest() * scale / 16) * 16);
    const maxX = Math.min(WORLD_LIMIT, Math.ceil(visible.getEast() * scale / 16) * 16);
    const minZ = Math.max(-WORLD_LIMIT, Math.floor(-visible.getNorth() * scale / 16) * 16);
    const maxZ = Math.min(WORLD_LIMIT, Math.ceil(-visible.getSouth() * scale / 16) * 16);
    if ((maxX - minX) / 16 + (maxZ - minZ) / 16 > 128) return;
    const style = { color: "#fff0ba", opacity: 0.4, weight: 1, interactive: false };
    for (let x = minX; x <= maxX; x += 16) L.polyline([position({ x, z: minZ }), position({ x, z: maxZ })], style).addTo(grid);
    for (let z = minZ; z <= maxZ; z += 16) L.polyline([position({ x: minX, z }), position({ x: maxX, z })], style).addTo(grid);
  };

  onSettled(() => {
    if (!element) return;
    map = L.map(element, { crs: L.CRS.Simple, attributionControl: false, preferCanvas: true, minZoom: minimumMapZoom(element.clientWidth, element.clientHeight, settings.maxZoom), maxZoom: settings.maxZoom + settings.extraZoom, zoomControl: true });
    map.setView(position(untrack(() => props.view)), settings.defaultZoom);
    removeControls = createMapControls(map, settings.maxZoom, clearInspection);
    const reportZoom = () => { if (element && map) element.dataset.zoom = String(map.getZoom()); };
    map.on("zoomend", reportZoom); reportZoom();
    // Leaflet's own empty-image URL means canceled, so it never completes missing tiles.
    const missingTile = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>')}`;
    tiles = L.tileLayer(assets.terrainTileUrl(worldMapId), { tileSize: 512, minZoom: -2, minNativeZoom: 0, maxNativeZoom: settings.maxZoom, noWrap: true, keepBuffer: 1, errorTileUrl: missingTile });
    terrainRefresh = createTerrainRefresh({ loading: () => tiles?.isLoading() ?? false, reload: nonce => { tiles?.setUrl(assets.terrainTileUrl(worldMapId, nonce)); }, report: state => untrack(() => props.onRefreshState(state)) });
    tiles.on("load", terrainRefresh.loaded).on("tileerror", terrainRefresh.failed).addTo(map);
    map.createPane("minecraft-seeds");
    map.createPane("minecraft-frontier");
    map.createPane("minecraft-player-nameplates");
    map.createPane("minecraft-waypoints");
    boundary = createTerrainBoundary(map, tiles, settings.maxZoom, point => untrack(() => props.seedWorld ? props.seedTiles.sample(props.seedWorld, point, props.predictionY) !== null : false), () => untrack(props.onInspectionRefresh));
    untrack(() => props.onRenderedLookup(point => boundary?.rendered(point) ?? false));
    sampled.addTo(map); grid.addTo(map); markers.addTo(map); playersLayer.addTo(map); selection.addTo(map); dragged.addTo(map);
    map.on("click", (event: L.LeafletMouseEvent) => untrack(() => {
      if (props.selecting || Date.now() < suppressClickUntil) return;
      const point = eventPoint(event.originalEvent);
      if (point) { props.onPoint(point); props.onInspect(point); }
    }));
    // Leaving or moving the map cancels a stationary inspection; panning itself never scans.
    element.addEventListener("mousemove", inspectCursor);
    element.addEventListener("mouseleave", clearInspection);
    element.addEventListener("pointerdown", beginSelection);
    element.addEventListener("pointermove", moveSelection);
    element.addEventListener("pointerup", endSelection);
    element.addEventListener("pointercancel", cancelSelection);
    map.on("dragstart zoomstart", clearInspection);
    map.on("moveend", drawGrid);
    observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(invalidate);
    observer?.observe(element);
    refreshTimer = window.setInterval(() => {
      if (document.visibilityState === "visible" && tiles && !tiles.isLoading()) {
        untrack(() => props.onTerrainRefresh());
        terrainRefresh?.request();
      }
    }, 30_000);
    setReady(true);
  });
  onCleanup(() => { removeControls?.(); window.clearInterval(refreshTimer); terrainRefresh?.dispose(); seedLayer?.dispose(); boundary?.dispose(); element?.removeEventListener("mousemove", inspectCursor); element?.removeEventListener("mouseleave", clearInspection); element?.removeEventListener("pointerdown", beginSelection); element?.removeEventListener("pointermove", moveSelection); element?.removeEventListener("pointerup", endSelection); element?.removeEventListener("pointercancel", cancelSelection); observer?.disconnect(); map?.remove(); map = undefined; });

  createEffect(() => [ready(), props.selecting, props.region] as const, ([mounted, selecting, region]) => {
    if (!mounted || !map) return;
    if (selecting) { map.dragging.disable(); map.touchZoom.disable(); } else { map.dragging.enable(); map.touchZoom.enable(); cancelSelection(); }
    element?.classList.toggle("minecraft-selecting", selecting === true);
    if (region) drawDrag(region);
  });

  createEffect(() => [ready(), props.view] as const, ([mounted, point]) => {
    if (mounted) map?.panTo(position(point), { animate: false });
  });
  createEffect(() => [ready(), props.grid] as const, () => untrack(drawGrid));
  createEffect(() => [ready(), props.refresh] as const, ([mounted, refresh]) => {
    if (mounted && refresh > 0) terrainRefresh?.request();
  });
  createEffect(() => [ready(), props.seedWorld, props.predictionsEnabled, props.predictionY] as const, ([mounted, world, enabled, y]) => {
    seedLayer?.dispose(); seedLayer = undefined;
    boundary?.setEnabled(enabled && world !== null);
    if (!mounted || !map || !world || !enabled) return;
    seedLayer = createSeedTileLayer({ map, maxZoom: settings.maxZoom, store: untrack(() => props.seedTiles), world, y, changed: () => boundary?.refresh() });
    seedLayer.layer.addTo(map);
  });
  createEffect(() => [ready(), props.area, props.biome, props.biomeLayer, props.elevation] as const, ([mounted, area, biome, showBiomes, elevation]) => {
    if (!mounted) return;
    sampled.clearLayers();
    if (!area || (!showBiomes && !elevation)) return;
    const heights = area.cells.map(cell => cell.y);
    const min = Math.min(...heights), max = Math.max(...heights);
    for (const cell of area.cells) {
      const selected = !biome || cell.biome === biome;
      const color = elevation ? elevationColor(cell.y, min, max) : biomeColor(cell.biome);
      L.rectangle(bounds(cell.x, cell.z, cell.x + 4, cell.z + 4), { stroke: false, fillColor: selected ? color : "#111111", fillOpacity: selected ? 0.65 : 0.75 })
        .bindTooltip(label(`${displayName(cell.biome)} · surface Y ${cell.y} · ${cell.x}, ${cell.z}`)).addTo(sampled);
    }
  });
  createEffect(() => [ready(), props.area, props.matches, props.waypoints, props.structures] as const, ([mounted, area, matches, waypoints, structures]) => {
    if (!mounted) return;
    markers.clearLayers();
    if (structures) for (const structure of area?.structures ?? []) {
      L.rectangle(bounds(structure.min_x, structure.min_z, structure.max_x + 1, structure.max_z + 1), { color: "#ffe19c", weight: 2, fillOpacity: 0.1 })
        .bindTooltip(label(`${displayName(structure.kind)} · Y ${structure.min_y} to ${structure.max_y}`)).addTo(markers);
    }
    for (const match of matches?.matches ?? []) L.circleMarker(position(match), { radius: 4, color: "#111111", weight: 1, fillColor: "#68e4ef", fillOpacity: 1 })
      .bindTooltip(label(`Block · ${match.x}, ${match.y}, ${match.z}`)).addTo(markers);
    for (const waypoint of waypoints) L.circleMarker(position(waypoint), { pane: "minecraft-waypoints", renderer: waypointRenderer, className: "minecraft-waypoint-marker", radius: 7, color: "#20130d", weight: 2, fillColor: "#ffcd6e", fillOpacity: 1 })
      .bindTooltip(label(waypoint.name), { permanent: true, direction: "right", offset: [9, 0], pane: "minecraft-waypoints", className: "minecraft-waypoint-nameplate", opacity: 1 })
      .bindPopup(label(`${waypoint.name} · ${waypoint.x}, ${waypoint.y}, ${waypoint.z}${waypoint.description ? ` · ${waypoint.description}` : ""}`), { className: "minecraft-waypoint-popup" }).addTo(markers);
  });
  createEffect(() => [ready(), props.players] as const, ([mounted, players]) => {
    if (!mounted) return;
    playersLayer.clearLayers();
    if (!settings.playerTracker.enabled) return;
    for (const player of players) {
      const head = document.createElement("img");
      head.className = "minecraft-player-head"; head.alt = ""; head.width = 20; head.height = 20;
      head.referrerPolicy = "no-referrer";
      const fallback = assets.playerFallbackUrl;
      head.src = settings.playerTracker.heads && player.uuid && assets.playerHeadUrl ? assets.playerHeadUrl(player.uuid) : fallback;
      head.addEventListener("error", () => { head.src = fallback; }, { once: true });
      const marker = L.marker(position(player), { icon: L.divIcon({ html: head, className: "minecraft-player-marker", iconSize: [20, 20], iconAnchor: [10, 10] }), keyboard: false, title: player.name });
      if (settings.playerTracker.nameplates) {
        const details = document.createElement("div"); details.className = "minecraft-player-details";
        const name = label(player.name); name.className = "minecraft-player-name"; details.append(name);
        for (const [kind, value, enabled] of [["health", player.health, settings.playerTracker.health], ["armor", player.armor, settings.playerTracker.armor]] as const) {
          if (!enabled || value === null) continue;
          const stat = document.createElement("img"); stat.className = "minecraft-player-vital"; stat.src = assets.playerVitalUrl(kind, value); stat.alt = `${kind === "health" ? "Health" : "Armor"}: ${value} of 20`; stat.width = 80; stat.height = 8;
          details.append(stat);
        }
        marker.bindTooltip(details, { permanent: true, direction: "right", offset: [12, 0], pane: "minecraft-player-nameplates", className: "minecraft-player-nameplate", opacity: 1 });
      }
      marker.addTo(playersLayer);
    }
  });
  createEffect(() => [ready(), props.point, props.measuring, props.measureStart, props.areaRegion, props.matchRegion] as const, ([mounted, point, measuring, start, areaRegion, matchRegion]) => {
    if (!mounted) return;
    selection.clearLayers();
    L.circleMarker(position(point), { radius: 6, color: "#ffffff", weight: 2, fill: false, interactive: false }).addTo(selection);
    for (const [region, color, title] of [[areaRegion, "#ffcd6e", "Surveyed area"], [matchRegion, "#68e4ef", "Block search area"]] as const) {
      if (region) L.rectangle(bounds(region.chunk_x * 16, region.chunk_z * 16, (region.chunk_x + region.width) * 16, (region.chunk_z + region.height) * 16), { color, weight: 2, fill: false })
        .bindTooltip(label(title)).addTo(selection);
    }
    if (measuring && start) L.polyline([position(start), position(point)], { color: "#ffffff", dashArray: "5 5", weight: 2, interactive: false }).addTo(selection);
  });

  return <div class="minecraft-atlas-canvas" ref={node => { element = node; }} role="region" aria-label="Interactive Minecraft terrain map" />;
}
