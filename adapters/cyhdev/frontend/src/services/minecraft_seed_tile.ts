import { ApiContractError, type MinecraftSeedTileQuery } from "../generated";
import { decodeSeedTileBinary, SEED_TILE_CONTENT_TYPE, SEED_TILE_MAX_BYTES } from "../components/minecraft/seedTileBinary";
import { decodeSeedTilePng, SEED_PNG_MAX_BYTES } from "../components/minecraft/seedTilePng";
import { apiFetch } from "./api";

/** Binary success has a bounded decoder; errors retain the ordinary HTTP status contract. */
export async function readSeedTile(query: MinecraftSeedTileQuery, signal: AbortSignal) {
  return decodeSeedTileBinary(await readSeedBytes(query, signal, "/api/minecraft/map/seed-tile.bin", SEED_TILE_CONTENT_TYPE, SEED_TILE_MAX_BYTES));
}

export async function readSeedTilePng(query: MinecraftSeedTileQuery, signal: AbortSignal) {
  return decodeSeedTilePng(await readSeedBytes(query, signal, "/api/minecraft/map/seed-tile.png", "image/png", SEED_PNG_MAX_BYTES), signal);
}

async function readSeedBytes(query: MinecraftSeedTileQuery, signal: AbortSignal, endpoint: string, contentType: string, limit: number) {
  const response = await apiFetch(endpoint, { method: "POST", headers: { "Content-Type": "application/json", Accept: contentType }, body: JSON.stringify(query), signal });
  if (!response.ok) { await response.body?.cancel(); throw new ApiContractError(response.status, "Seed map is temporarily unavailable."); }
  if (response.headers.get("content-type")?.split(";")[0]?.trim() !== contentType || !response.body) { await response.body?.cancel(); throw new Error("Invalid seed tile response."); }
  // Fetch already decoded HTTP content encoding; this fixed buffer bounds decompressed bytes and chunk overhead.
  const reader = response.body.getReader(), bytes = new Uint8Array(limit); let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read(); if (done) break;
      if (value.length > bytes.length - size) { await reader.cancel(); throw new Error("Seed tile response is too large."); }
      bytes.set(value, size); size += value.length;
    }
  } finally { reader.releaseLock(); }
  return bytes.buffer.slice(0, size);
}
