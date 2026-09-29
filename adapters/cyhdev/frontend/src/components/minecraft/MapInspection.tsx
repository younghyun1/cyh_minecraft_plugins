import { Show } from "solid-js";
import { displayName } from "./mapMath";
import type { MapInspection as Inspection } from "./mapInspectionState";

/** Keep pointer-driven updates out of live regions and leave terrain gestures unobstructed. */
export default function MapInspection(props: { readonly inspection: Inspection | null }) {
  return <Show when={props.inspection}>{inspection => <section class="minecraft-inspection" aria-label="Terrain inspection">
    <strong>X {inspection().point.x} · Z {inspection().point.z}</strong>
    <Show when={inspection().biome}>{biome => <>
      <p>{biome().source === "predicted" ? "Predicted" : "Observed"} biome: <b>{displayName(biome().name)}</b></p>
      <p class="minecraft-inspection-id">{biome().name}</p>
      <Show when={biome().surfaceY !== null}><p>Surface Y {biome().surfaceY} at sample {biome().sample.x}, {biome().sample.z}</p></Show>
      <p>{biome().step ?? 4} × {biome().step ?? 4} block sample<Show when={biome().y !== null}> · biome Y {biome().y}</Show><Show when={biome().y === null}> · surface biome</Show><Show when={biome().source === "predicted"}> · approximate</Show></p>
    </>}</Show>
    <Show when={inspection().block}>{block => <p>Search match: <b>{displayName(block().name)}</b> · Y {block().ys.slice(0, 8).join(", ")}<Show when={block().ys.length > 8}> (+{block().ys.length - 8} more)</Show></p>}</Show>
    <Show when={inspection().message}><p>{inspection().message}</p></Show>
  </section>}</Show>;
}
