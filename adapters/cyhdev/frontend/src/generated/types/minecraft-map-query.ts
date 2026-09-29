// Generated from rust-be-template OpenAPI. Do not edit by hand.


export type MinecraftMapQuery = {
  readonly kind: "catalog";
} | {
  readonly chunk_x: number;
  readonly chunk_z: number;
  readonly height: number;
  readonly kind: "area";
  readonly width: number;
  readonly world: string;
  readonly y?: number | null;
} | {
  readonly block: string;
  readonly chunk_x: number;
  readonly chunk_z: number;
  readonly height: number;
  readonly kind: "blocks";
  readonly max_y: number;
  readonly min_y: number;
  readonly width: number;
  readonly world: string;
};
