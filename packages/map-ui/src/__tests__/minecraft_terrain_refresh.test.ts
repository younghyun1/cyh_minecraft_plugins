import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createTerrainRefresh } from "../components/minecraft/terrainRefresh";

describe("Minecraft terrain refresh", () => {
  beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(1000); });
  afterEach(() => { vi.useRealTimers(); });

  it("keeps a manual refresh queued while initial tiles load", () => {
    const reload = vi.fn(), report = vi.fn();
    const refresh = createTerrainRefresh({ loading: () => true, reload, report });
    refresh.request(); refresh.request();
    expect(reload).not.toHaveBeenCalled();
    expect(report).toHaveBeenLastCalledWith("refreshing");
    refresh.loaded();
    expect(reload).toHaveBeenCalledExactlyOnceWith(1000);
    refresh.loaded();
    expect(report).toHaveBeenLastCalledWith("complete");
    refresh.dispose();
  });

  it("uses distinct URLs at an identical or decreasing wall clock and coalesces pending work", () => {
    const reload = vi.fn();
    const refresh = createTerrainRefresh({ loading: () => false, reload, report: vi.fn() });
    refresh.request(); refresh.request(); refresh.request();
    expect(reload).toHaveBeenCalledTimes(1);
    refresh.loaded(); refresh.loaded();
    expect(reload.mock.calls.map(call => call[0])).toEqual([1000, 1001]);
    vi.setSystemTime(500); refresh.request();
    expect(reload.mock.calls.map(call => call[0])).toEqual([1000, 1001, 1002]);
    refresh.dispose();
  });

  it("reports missing tiles as partial and resets the failure state on retry", () => {
    const report = vi.fn();
    const refresh = createTerrainRefresh({ loading: () => false, reload: vi.fn(), report });
    refresh.request(); refresh.failed(); refresh.loaded();
    expect(report).toHaveBeenLastCalledWith("partial");
    refresh.request(); refresh.loaded();
    expect(report).toHaveBeenLastCalledWith("complete");
    refresh.dispose();
  });

  it("replaces stalled initial requests and releases a stalled refresh control after bounded waits", () => {
    const reload = vi.fn(), report = vi.fn();
    const refresh = createTerrainRefresh({ loading: () => true, reload, report });
    refresh.request(); vi.advanceTimersByTime(30_000);
    expect(reload).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(30_000);
    expect(report).toHaveBeenLastCalledWith("partial");
    refresh.dispose();
  });

  it("disposal cancels queued work and ignores late tile loads", () => {
    const reload = vi.fn(), report = vi.fn();
    const refresh = createTerrainRefresh({ loading: () => true, reload, report });
    refresh.request(); refresh.dispose(); refresh.loaded(); refresh.request(); vi.runAllTimers();
    expect(reload).not.toHaveBeenCalled();
    expect(report).toHaveBeenCalledExactlyOnceWith("refreshing");
  });
});
