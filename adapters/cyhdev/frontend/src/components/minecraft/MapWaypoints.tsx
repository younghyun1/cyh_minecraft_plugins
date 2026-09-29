import { createSignal, For, onCleanup, onSettled, Show, untrack } from "solid-js";
import type { MinecraftWaypoint, MinecraftWaypointInput } from "../../generated";
import { contractApi } from "../../services/account_api";
import { isSuperuser } from "../../state/auth";
import { mapFailure } from "./createMapExplorer";
import { coordinate, WORLD_LIMIT, type MapPoint } from "./mapMath";

type Props = {
  readonly world: string;
  readonly point: MapPoint;
  readonly minY: number;
  readonly maxY: number;
  readonly onChange: (waypoints: readonly MinecraftWaypoint[]) => void;
  readonly onNavigate: (point: MapPoint) => void;
};

export default function MapWaypoints(props: Props) {
  const world = untrack(() => props.world);
  const [items, setItems] = createSignal<readonly MinecraftWaypoint[]>([]);
  const [busy, setBusy] = createSignal(false);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal("");
  const [editor, setEditor] = createSignal(false);
  const [editing, setEditing] = createSignal<string | null>(null);
  const [name, setName] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [x, setX] = createSignal("0"), [y, setY] = createSignal("64"), [z, setZ] = createSignal("0");
  let active = true, pending = false;
  const controller = new AbortController();
  const publish = (next: readonly MinecraftWaypoint[]) => { setItems(next); props.onChange(next); };
  const load = async () => {
    if (pending) return;
    pending = true; setLoading(true); setError("");
    try {
      const response = await contractApi.minecraftMapWaypoints({ query: { world } }, { signal: controller.signal });
      if (active) publish(response.data);
    } catch (cause: unknown) { if (active) setError(mapFailure(cause)); }
    finally { pending = false; if (active) setLoading(false); }
  };
  const edit = (waypoint?: MinecraftWaypoint) => {
    setEditing(waypoint?.id ?? null); setName(waypoint?.name ?? ""); setDescription(waypoint?.description ?? "");
    setX(String(waypoint?.x ?? props.point.x)); setY(String(waypoint?.y ?? Math.max(props.minY, Math.min(props.maxY, 64)))); setZ(String(waypoint?.z ?? props.point.z)); setEditor(true);
  };
  const save = async () => {
    if (pending || isSuperuser() !== true) return;
    const nextX = coordinate(x()), nextY = coordinate(y()), nextZ = coordinate(z());
    if (nextX === null || nextY === null || nextZ === null || nextY < props.minY || nextY > props.maxY || !name().trim()) return;
    const body: MinecraftWaypointInput = { world: props.world, name: name().trim(), description: description().trim(), x: nextX, y: nextY, z: nextZ };
    const id = editing(); pending = true; setBusy(true); setError("");
    try {
      const response = id === null ? await contractApi.createMinecraftMapWaypoint({ body }) : await contractApi.updateMinecraftMapWaypoint({ path: { waypoint_id: id }, body });
      if (!active) return;
      publish(id === null ? [...items(), response.data] : items().map(item => item.id === id ? response.data : item));
      setEditor(false);
    } catch (cause: unknown) { if (active) setError(`${mapFailure(cause)} Refresh the list before retrying if the result is uncertain.`); }
    finally { pending = false; if (active) setBusy(false); }
  };
  const remove = async (waypoint: MinecraftWaypoint) => {
    if (pending || isSuperuser() !== true || !window.confirm(`Delete the public waypoint “${waypoint.name}”?`)) return;
    pending = true; setBusy(true); setError("");
    try {
      await contractApi.deleteMinecraftMapWaypoint({ path: { waypoint_id: waypoint.id } });
      if (active) { publish(items().filter(item => item.id !== waypoint.id)); if (editing() === waypoint.id) setEditor(false); }
    } catch (cause: unknown) { if (active) setError(mapFailure(cause)); }
    finally { pending = false; if (active) setBusy(false); }
  };
  onSettled(() => { void load(); });
  onCleanup(() => { active = false; controller.abort(); });
  return <section class="minecraft-tool-panel" aria-labelledby="minecraft-waypoint-title" aria-busy={busy() || loading() ? "true" : "false"}>
    <h2 id="minecraft-waypoint-title">Named waypoints</h2>
    <Show when={error()}><p role="alert">{error()}</p></Show>
    <div class="minecraft-button-row"><button type="button" disabled={busy() || loading()} onClick={() => void load()}>Refresh waypoints</button><Show when={isSuperuser() === true}><button type="button" disabled={busy() || loading()} onClick={() => edit()}>Add waypoint</button></Show></div>
    <Show when={loading()}><p>Loading waypoints…</p></Show>
    <ul class="minecraft-result-list"><For each={items()} fallback={<Show when={!loading()}><li>No waypoints in this dimension.</li></Show>}>{item => <li>
      <button type="button" class="minecraft-place-link" onClick={() => props.onNavigate(item)}>{item.name}<small>{item.x}, {item.y}, {item.z}</small></button>
      <Show when={item.description}><p>{item.description}</p></Show>
      <Show when={isSuperuser() === true}><div class="minecraft-button-row"><button type="button" disabled={busy()} onClick={() => edit(item)} aria-label={`Edit waypoint ${item.name}`}>Edit</button><button type="button" disabled={busy()} onClick={() => void remove(item)} aria-label={`Delete waypoint ${item.name}`}>Delete</button></div></Show>
    </li>}</For></ul>
    <Show when={editor() && isSuperuser() === true}><form class="minecraft-waypoint-form" onSubmit={event => { event.preventDefault(); void save(); }}>
      <h3>{editing() ? "Edit waypoint" : "Add a public waypoint"}</h3>
      <label>Name<input value={name()} maxlength={80} required disabled={busy()} onInput={event => setName(event.currentTarget.value)} /></label>
      <label>Description<textarea value={description()} maxlength={500} disabled={busy()} onInput={event => setDescription(event.currentTarget.value)} /></label>
      <div class="minecraft-coordinate-fields">
        <label>X<input type="number" value={x()} min={-WORLD_LIMIT} max={WORLD_LIMIT} step="1" required disabled={busy()} onInput={event => setX(event.currentTarget.value)} /></label>
        <label>Y<input type="number" value={y()} min={props.minY} max={props.maxY} step="1" required disabled={busy()} onInput={event => setY(event.currentTarget.value)} /></label>
        <label>Z<input type="number" value={z()} min={-WORLD_LIMIT} max={WORLD_LIMIT} step="1" required disabled={busy()} onInput={event => setZ(event.currentTarget.value)} /></label>
      </div>
      <button type="button" disabled={busy()} onClick={() => { setX(String(props.point.x)); setZ(String(props.point.z)); }}>Use selected X / Z</button>
      <div class="minecraft-button-row"><button type="submit" disabled={busy()}>{busy() ? "Saving…" : "Save waypoint"}</button><button type="button" disabled={busy()} onClick={() => setEditor(false)}>Cancel</button></div>
    </form></Show>
  </section>;
}
