// Generated from rust-be-template OpenAPI. Do not edit by hand.

import type { MinecraftPlayer } from "./minecraft-player";

export type MinecraftStatus = {
  readonly players: ReadonlyArray<MinecraftPlayer>;
  readonly whitelist: ReadonlyArray<MinecraftPlayer>;
  readonly whitelist_enabled: boolean;
};
