import { createEffect, createSignal, Show } from "solid-js";
import type { MapExplorer } from "./createMapExplorer";
import { coordinate, distance, WORLD_LIMIT, type MapPoint } from "./mapMath";

type Props = {
  readonly explorer: MapExplorer;
  readonly measuring: boolean;
  readonly start: MapPoint | null;
  readonly onMeasure: (enabled: boolean) => void;
};

export default function MapTravel(props: Props) {
  const [x, setX] = createSignal("0"), [z, setZ] = createSignal("0");
  const [copied, setCopied] = createSignal("");
  const isNether = () => props.explorer.loaded()?.world.type === "nether";
  const isOverworld = () => props.explorer.loaded()?.world.type === "normal";
  const converted = () => ({ x: Math.floor(props.explorer.point().x * (isNether() ? 8 : 1 / 8)), z: Math.floor(props.explorer.point().z * (isNether() ? 8 : 1 / 8)) });
  createEffect(() => props.explorer.point(), point => { setX(String(point.x)); setZ(String(point.z)); setCopied(""); });
  const copyLink = async () => {
    const url = new URL("/minecraft", window.location.origin);
    url.searchParams.set("world", props.explorer.loaded()?.world.name ?? "");
    url.searchParams.set("x", String(props.explorer.point().x)); url.searchParams.set("z", String(props.explorer.point().z));
    try { await navigator.clipboard.writeText(url.toString()); setCopied("Location link copied."); }
    catch { setCopied("Clipboard is unavailable. Select and copy the link below."); }
    return url.toString();
  };
  const shareLink = () => `/minecraft?world=${encodeURIComponent(props.explorer.loaded()?.world.name ?? "")}&x=${props.explorer.point().x}&z=${props.explorer.point().z}`;
  return <section class="minecraft-tool-panel" aria-labelledby="minecraft-travel-title">
    <h2 id="minecraft-travel-title">Plan your journey</h2>
    <form onSubmit={event => { event.preventDefault(); const nextX = coordinate(x()), nextZ = coordinate(z()); if (nextX !== null && nextZ !== null) props.explorer.navigate({ x: nextX, z: nextZ }); }}>
      <div class="minecraft-coordinate-fields"><label>Go to X<input type="number" value={x()} min={-WORLD_LIMIT} max={WORLD_LIMIT} step="1" required onInput={event => setX(event.currentTarget.value)} /></label><label>Go to Z<input type="number" value={z()} min={-WORLD_LIMIT} max={WORLD_LIMIT} step="1" required onInput={event => setZ(event.currentTarget.value)} /></label></div>
      <button type="submit">Go to coordinates</button>
    </form>
    <div class="minecraft-button-row"><button type="button" onClick={() => { const spawn = props.explorer.loaded()?.settings.spawn; if (spawn) props.explorer.navigate(spawn); }}>Go to spawn</button><button type="button" onClick={() => void copyLink()}>Copy location link</button></div>
    <Show when={copied()}><p role="status">{copied()} <a href={shareLink()}>Open this location</a></p></Show>
    <h3>Distance ruler</h3>
    <button type="button" aria-pressed={props.measuring ? "true" : "false"} onClick={() => props.onMeasure(!props.measuring)}>{props.measuring ? "Clear measurement" : "Measure from here"}</button>
    <Show when={props.start && props.measuring}><p class="minecraft-distance">{distance(props.start ?? props.explorer.point(), props.explorer.point()).toFixed(1)} blocks<small>Horizontal, straight-line distance</small></p></Show>
    <Show when={isNether() || isOverworld()}><h3>{isNether() ? "Overworld" : "Nether"} coordinates</h3><p class="minecraft-portal-coordinate">X {converted().x} · Z {converted().z}</p></Show>
  </section>;
}
