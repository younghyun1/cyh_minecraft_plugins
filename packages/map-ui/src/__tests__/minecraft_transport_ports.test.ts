import { describe, expect, it, vi } from "vitest";
import { createSquaremapClient } from "../services/squaremap";

describe("squaremap transport port", () => {
  it("uses the supplied endpoint and transport without requiring website globals", async () => {
    const transport = vi.fn(async () => new Response(JSON.stringify({ worlds: [{ name: "world", display_name: "Overworld", type: "normal" }] })));
    const client = createSquaremapClient(transport, "/independent-map/tiles/"), signal = new AbortController().signal;
    expect(await client.worlds(signal)).toEqual([{ name: "world", displayName: "Overworld", type: "normal" }]);
    expect(transport).toHaveBeenCalledExactlyOnceWith("/independent-map/tiles/settings.json", { signal, cache: "no-store", credentials: "omit" });
    await expect(client.settings("../private", signal)).rejects.toThrow("world path");
    expect(transport).toHaveBeenCalledTimes(1);
  });

  it("cancels oversized metadata and releases the reader", async () => {
    const cancel = vi.fn(), stream = new ReadableStream<Uint8Array>({ start(controller) { controller.enqueue(new Uint8Array(1_048_577)); }, cancel });
    const client = createSquaremapClient(async () => new Response(stream), "/map");
    await expect(client.worlds(new AbortController().signal)).rejects.toThrow("size limit");
    expect(cancel).toHaveBeenCalledOnce(); expect(stream.locked).toBe(false);
  });
});
