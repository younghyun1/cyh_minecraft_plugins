import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import type { MinecraftMapData } from "../../generated";
import type { MapExplorer } from "./createMapExplorer";
import { biomeColor, coordinate, displayName } from "./mapMath";

export type LayerControls = {
  readonly biome: () => string;
  readonly setBiome: (value: string) => void;
  readonly biomes: () => boolean;
  readonly setBiomes: (value: boolean) => void;
  readonly elevation: () => boolean;
  readonly setElevation: (value: boolean) => void;
  readonly structures: () => boolean;
  readonly setStructures: (value: boolean) => void;
  readonly grid: () => boolean;
  readonly setGrid: (value: boolean) => void;
};

export function ScanReceipt(props: { readonly result: MinecraftMapData }) {
  return <p class="minecraft-scan-receipt">Sampled {new Date(props.result.sampled_at_ms).toLocaleTimeString()} · {props.result.scanned_chunks} chunks read · {props.result.missing_chunks} unavailable
    <Show when={props.result.truncated}><strong> Partial results: some data was unavailable or exceeded the work or result limit.</strong></Show>
  </p>;
}

export function MapAnalysis(props: { readonly explorer: MapExplorer; readonly layers: LayerControls }) {
  const [height, setHeight] = createSignal("");
  const biomes = createMemo(() => [...new Set(props.explorer.area()?.cells.map(cell => cell.biome) ?? [])].sort());
  const heights = createMemo(() => props.explorer.area()?.cells.map(cell => cell.y) ?? []);
  const validHeight = () => height() === "" || (coordinate(height()) !== null && Number(height()) >= (props.explorer.currentWorld()?.min_y ?? -64) && Number(height()) <= (props.explorer.currentWorld()?.max_y ?? 319));
  return <section class="minecraft-tool-panel" aria-labelledby="minecraft-survey-title">
    <h2 id="minecraft-survey-title">Survey the landscape</h2>
    <form onSubmit={event => { event.preventDefault(); props.explorer.scan(height() === "" ? null : coordinate(height())); }}>
      <label>Biome sampling height<input type="number" placeholder="Surface" value={height()} min={props.explorer.currentWorld()?.min_y} max={props.explorer.currentWorld()?.max_y} step="1" onInput={event => setHeight(event.currentTarget.value)} /></label>
      <button class="minecraft-primary-action" type="submit" disabled={props.explorer.busy() || !props.explorer.currentWorld() || !validHeight()}>{props.explorer.busy() ? "Surveying…" : "Survey selected area"}</button>
    </form>
    <fieldset class="minecraft-layer-options"><legend>Map layers</legend>
      <label><input type="checkbox" checked={props.layers.biomes()} onChange={event => props.layers.setBiomes(event.currentTarget.checked)} /> Biome colors</label>
      <label><input type="checkbox" checked={props.layers.elevation()} onChange={event => props.layers.setElevation(event.currentTarget.checked)} /> Surface elevation</label>
      <label><input type="checkbox" checked={props.layers.structures()} onChange={event => props.layers.setStructures(event.currentTarget.checked)} /> Structure bounds</label>
      <label><input type="checkbox" checked={props.layers.grid()} onChange={event => props.layers.setGrid(event.currentTarget.checked)} /> Chunk grid</label>
    </fieldset>
    <Show when={props.explorer.area()}>{result => <>
      <ScanReceipt result={result()} />
      <small>Biome sample: {props.explorer.areaSlice() === null ? "surface" : `Y ${props.explorer.areaSlice()}`}</small>
      <label>Highlight biome<select value={props.layers.biome()} onChange={event => props.layers.setBiome(event.currentTarget.value)}><option value="">All sampled biomes</option><For each={biomes()}>{biome => <option value={biome}>{displayName(biome)}</option>}</For></select></label>
      <Show when={!props.layers.elevation()}><ul class="minecraft-biome-legend"><For each={biomes()}>{biome => <li><span aria-hidden="true" style={{ "background-color": biomeColor(biome) }} />{displayName(biome)}</li>}</For></ul></Show>
      <Show when={props.layers.elevation() && heights().length > 0}><div class="minecraft-height-legend"><span>Y {Math.min(...heights())}</span><span aria-hidden="true" /><span>Y {Math.max(...heights())}</span></div></Show>
      <h3>Structures ({result().structures.length})</h3>
      <ul class="minecraft-result-list"><For each={result().structures} fallback={<li>No structures returned for this survey.</li>}>{structure => <li><button class="minecraft-place-link" type="button" onClick={() => props.explorer.navigate({ x: Math.floor((structure.min_x + structure.max_x) / 2), z: Math.floor((structure.min_z + structure.max_z) / 2) })}>{displayName(structure.kind)}<small>{structure.min_x}, {structure.min_y}, {structure.min_z}</small></button></li>}</For></ul>
    </>}</Show>
  </section>;
}

export function MapBlockSearch(props: { readonly explorer: MapExplorer }) {
  const [block, setBlock] = createSignal("minecraft:diamond_ore");
  const [minY, setMinY] = createSignal("-64");
  const [maxY, setMaxY] = createSignal("64");
  createEffect(() => props.explorer.currentWorld(), world => { setMinY(String(world?.min_y ?? -64)); setMaxY(String(Math.min(world?.max_y ?? 319, 64))); });
  const valid = () => {
    const min = coordinate(minY()), max = coordinate(maxY()), world = props.explorer.currentWorld();
    return world !== undefined && min !== null && max !== null && min >= world.min_y && max <= world.max_y && max >= min && max - min < 512 && props.explorer.catalog()?.blocks.includes(block()) === true;
  };
  return <section class="minecraft-tool-panel" aria-labelledby="minecraft-block-title">
    <h2 id="minecraft-block-title">Find blocks nearby</h2>
    <form onSubmit={event => { event.preventDefault(); if (valid()) props.explorer.searchBlocks(block(), Number(minY()), Number(maxY())); }}>
      <label>Block type<input list="minecraft-block-types" value={block()} maxlength={128} onInput={event => setBlock(event.currentTarget.value)} required /></label>
      <datalist id="minecraft-block-types"><For each={props.explorer.catalog()?.blocks ?? []}>{value => <option value={value} />}</For></datalist>
      <div class="minecraft-coordinate-fields"><label>Minimum Y<input type="number" value={minY()} min={props.explorer.currentWorld()?.min_y} max={props.explorer.currentWorld()?.max_y} step="1" required onInput={event => setMinY(event.currentTarget.value)} /></label><label>Maximum Y<input type="number" value={maxY()} min={props.explorer.currentWorld()?.min_y} max={props.explorer.currentWorld()?.max_y} step="1" required onInput={event => setMaxY(event.currentTarget.value)} /></label></div>
      <button class="minecraft-primary-action" type="submit" disabled={props.explorer.busy() || !valid()}>{props.explorer.busy() ? "Searching…" : "Search selected area"}</button>
    </form>
    <Show when={props.explorer.matches()}>{result => <>
      <ScanReceipt result={result()} /><h3>{displayName(props.explorer.matchedBlock())}</h3><h3>Matches ({result().matches.length})</h3>
      <ul class="minecraft-result-list"><For each={result().matches} fallback={<li>No matches returned in the scanned chunks.</li>}>{match => <li><button class="minecraft-place-link" type="button" onClick={() => props.explorer.navigate(match)}>X {match.x} · Y {match.y} · Z {match.z}</button></li>}</For></ul>
    </>}</Show>
  </section>;
}
