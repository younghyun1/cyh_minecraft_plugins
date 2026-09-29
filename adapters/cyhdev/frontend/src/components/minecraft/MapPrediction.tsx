import { createEffect, createSignal, Show } from "solid-js";
import type { MapExplorer } from "./createMapExplorer";
import { coordinate } from "./mapMath";

export default function MapPrediction(props: { readonly explorer: MapExplorer }) {
  const [height, setHeight] = createSignal("64");
  createEffect(() => props.explorer.predictionY(), value => { setHeight(String(value)); });
  return <div class="minecraft-prediction-controls">
    <label class="minecraft-toggle"><input type="checkbox" checked={props.explorer.predictionsEnabled()} onChange={event => props.explorer.setPredictionsEnabled(event.currentTarget.checked)} /> Predicted biomes</label>
    <Show when={props.explorer.predictionsEnabled()}>
      <label>Tile format<select value={props.explorer.predictionFormat()} onChange={event => props.explorer.selectPredictionFormat(event.currentTarget.value === "png" ? "png" : "binary")}><option value="binary">Binary</option><option value="png">PNG</option></select></label>
      <label>Prediction Y<input type="number" value={height()} min={props.explorer.currentWorld()?.min_y ?? -64} max={props.explorer.currentWorld()?.max_y ?? 319} step="1" onInput={event => {
        setHeight(event.currentTarget.value);
        const y = coordinate(event.currentTarget.value), world = props.explorer.currentWorld();
        if (y !== null && world && y >= world.min_y && y <= world.max_y) props.explorer.setPredictionY(y);
      }} /></label>
      <Show when={props.explorer.predicting()}><small role="status">Loading predictions…</small></Show>
      <Show when={props.explorer.predictionNotice()}><p class="minecraft-map-warning" role="status">{props.explorer.predictionNotice()}</p></Show>
      <Show when={props.explorer.predictionPreset()}>{preset => <small class="minecraft-prediction-receipt">Predicted · Y {props.explorer.predictionY()} · {preset() === "large_biomes" ? "Large biomes" : preset() === "nether" ? "Nether" : preset() === "end" ? "The End" : "Default biomes"} · approximate</small>}</Show>
    </Show>
  </div>;
}
