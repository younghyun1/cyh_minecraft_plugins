export type TerrainRefreshState = "idle" | "refreshing" | "complete" | "partial";

type RefreshOptions = {
  readonly loading: () => boolean;
  readonly reload: (nonce: number) => void;
  readonly report: (state: TerrainRefreshState) => void;
};

/** Coalesce refreshes during loading; a stalled request cannot lock the refresh control. */
export function createTerrainRefresh(options: RefreshOptions) {
  let queued = false, running = false, failed = false, disposed = false, nonce = 0;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const clearTimeoutHandle = () => { clearTimeout(timeout); timeout = undefined; };
  const start = () => {
    queued = false; running = true; failed = false;
    clearTimeoutHandle();
    timeout = setTimeout(() => { running = false; options.report("partial"); }, 30_000);
    // The wall clock can repeat or move backwards; every refresh still needs a new URL.
    nonce = Math.max(Date.now(), nonce + 1);
    options.reload(nonce);
  };
  const loaded = () => {
    if (disposed) return;
    clearTimeoutHandle();
    if (queued) { start(); return; }
    if (running) { running = false; options.report(failed ? "partial" : "complete"); }
  };
  const request = () => {
    if (disposed) return;
    options.report("refreshing");
    if (running || options.loading()) {
      queued = true;
      clearTimeoutHandle();
      // Redrawing aborts Leaflet's stalled tile requests after this bounded wait.
      timeout = setTimeout(start, 30_000);
    } else start();
  };
  return { request, loaded, failed: () => { if (running) failed = true; }, dispose: () => { disposed = true; clearTimeoutHandle(); } };
}
