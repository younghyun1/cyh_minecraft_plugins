// Generated from rust-be-template OpenAPI. Do not edit by hand.

import type { MinecraftSeedPreset } from "./minecraft-seed-preset";

export type MinecraftSeedTile = {
  readonly expires_at_ms: number;
  readonly generator_revision: string;
  readonly height: number;
  readonly indices: ReadonlyArray<number | null>;
  readonly level: number;
  readonly min_x: number;
  readonly min_z: number;
  readonly palette: ReadonlyArray<string>;
  readonly preset: MinecraftSeedPreset;
  readonly profile_epoch: string;
  readonly sampled_at_ms: number;
  readonly step: number;
  readonly tile_x: number;
  readonly tile_z: number;
  readonly width: number;
  readonly world: string;
  readonly y: number;
};
