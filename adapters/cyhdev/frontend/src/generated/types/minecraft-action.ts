// Generated from rust-be-template OpenAPI. Do not edit by hand.


export type MinecraftAction = {
  readonly action: "message";
  readonly message: string;
} | {
  readonly action: "whitelist_add";
  readonly name: string;
} | {
  readonly action: "whitelist_remove";
  readonly name: string;
} | {
  readonly action: "whitelist_enable";
  readonly enabled: boolean;
} | {
  readonly action: "kick";
  readonly name: string;
} | {
  readonly action: "map_visibility";
  readonly hidden: boolean;
  readonly id: string;
} | {
  readonly action: "save";
} | {
  readonly action: "restart";
};
