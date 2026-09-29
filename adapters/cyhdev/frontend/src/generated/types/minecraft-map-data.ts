// Generated from rust-be-template OpenAPI. Do not edit by hand.

import type { MinecraftMapCell } from "./minecraft-map-cell";
import type { MinecraftMapKind } from "./minecraft-map-kind";
import type { MinecraftMapMatch } from "./minecraft-map-match";
import type { MinecraftMapStructure } from "./minecraft-map-structure";
import type { MinecraftMapWorld } from "./minecraft-map-world";

export type MinecraftMapData = {
  readonly blocks: ReadonlyArray<string>;
  readonly cells: ReadonlyArray<MinecraftMapCell>;
  readonly kind: MinecraftMapKind;
  readonly matches: ReadonlyArray<MinecraftMapMatch>;
  readonly missing_chunks: number;
  readonly sampled_at_ms: number;
  readonly scanned_chunks: number;
  readonly structures: ReadonlyArray<MinecraftMapStructure>;
  readonly truncated: boolean;
  readonly world?: string | null;
  readonly worlds: ReadonlyArray<MinecraftMapWorld>;
};
