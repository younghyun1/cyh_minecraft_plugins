// Generated from rust-be-template OpenAPI. Do not edit by hand.

import type { MinecraftPredictedBiome } from "./minecraft-predicted-biome";
import type { MinecraftPredictionCoverage } from "./minecraft-prediction-coverage";
import type { MinecraftPredictionPreset } from "./minecraft-prediction-preset";

export type MinecraftPrediction = {
  readonly cells: ReadonlyArray<MinecraftPredictedBiome>;
  readonly coverage: ReadonlyArray<MinecraftPredictionCoverage>;
  readonly expires_at_ms: number;
  readonly generator_revision: string;
  readonly min_x: number;
  readonly min_z: number;
  readonly preset: MinecraftPredictionPreset;
  readonly sampled_at_ms: number;
  readonly step: number;
  readonly world: string;
  readonly y: number;
};
