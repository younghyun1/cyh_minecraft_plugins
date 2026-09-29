// Generated from rust-be-template OpenAPI. Do not edit by hand.

import type {
  ApiResponse,
  MinecraftAction,
  MinecraftActionResult,
  MinecraftMapData,
  MinecraftMapQuery,
  MinecraftPrediction,
  MinecraftPredictionQuery,
  MinecraftSeedTile,
  MinecraftSeedTileQuery,
  MinecraftStatus,
  MinecraftWaypoint,
  MinecraftWaypointInput,
} from "../api-types";
import {
  appendQuery,
  interpolatePath,
  requestHeaders,
  requestJson,
  type ApiRequestOptions,
  type ApiTransport,
} from "../runtime";

export function createMinecraftClient(transport: ApiTransport) {
  return {
    createMinecraftMapWaypoint: async (input: {
      readonly body: MinecraftWaypointInput;
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/admin/minecraft/map/waypoints";
      const url = path;
      return requestJson<ApiResponse<MinecraftWaypoint>>(transport, url, {
        method: "POST",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
    deleteMinecraftMapWaypoint: async (input: {
      readonly path: {
        readonly waypoint_id: string;
      };
    }, options: ApiRequestOptions = {}) => {
      const path = interpolatePath("/api/admin/minecraft/map/waypoints/{waypoint_id}", input.path);
      const url = path;
      return requestJson<ApiResponse<MinecraftActionResult>>(transport, url, {
        method: "DELETE",
        headers: requestHeaders(options.headers, false),
        signal: options.signal,
      });
    },
    minecraftAction: async (input: {
      readonly body: MinecraftAction;
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/admin/minecraft/actions";
      const url = path;
      return requestJson<ApiResponse<MinecraftActionResult>>(transport, url, {
        method: "POST",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
    minecraftMapPrediction: async (input: {
      readonly body: MinecraftPredictionQuery;
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/minecraft/map/prediction";
      const url = path;
      return requestJson<ApiResponse<MinecraftPrediction>>(transport, url, {
        method: "POST",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
    minecraftMapQuery: async (input: {
      readonly body: MinecraftMapQuery;
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/minecraft/map/query";
      const url = path;
      return requestJson<ApiResponse<MinecraftMapData>>(transport, url, {
        method: "POST",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
    minecraftMapSeedTile: async (input: {
      readonly body: MinecraftSeedTileQuery;
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/minecraft/map/seed-tile";
      const url = path;
      return requestJson<ApiResponse<MinecraftSeedTile>>(transport, url, {
        method: "POST",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
    minecraftMapWaypoints: async (input: {
      readonly query: {
        readonly world: string;
      };
    }, options: ApiRequestOptions = {}) => {
      const path = "/api/minecraft/map/waypoints";
      const url = appendQuery(path, input.query);
      return requestJson<ApiResponse<ReadonlyArray<MinecraftWaypoint>>>(transport, url, {
        method: "GET",
        headers: requestHeaders(options.headers, false),
        signal: options.signal,
      });
    },
    minecraftStatus: async (options: ApiRequestOptions = {}) => {
      const path = "/api/admin/minecraft";
      const url = path;
      return requestJson<ApiResponse<MinecraftStatus>>(transport, url, {
        method: "GET",
        headers: requestHeaders(options.headers, false),
        signal: options.signal,
      });
    },
    updateMinecraftMapWaypoint: async (input: {
      readonly body: MinecraftWaypointInput;
      readonly path: {
        readonly waypoint_id: string;
      };
    }, options: ApiRequestOptions = {}) => {
      const path = interpolatePath("/api/admin/minecraft/map/waypoints/{waypoint_id}", input.path);
      const url = path;
      return requestJson<ApiResponse<MinecraftWaypoint>>(transport, url, {
        method: "PATCH",
        headers: requestHeaders(options.headers, true),
        signal: options.signal,
        body: JSON.stringify(input.body),
      });
    },
  } as const;
}
