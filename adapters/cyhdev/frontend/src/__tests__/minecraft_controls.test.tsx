import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Minecraft from "../pages/minecraft";
import MinecraftControls from "../components/minecraft/MinecraftControls";
import { contractApi } from "../services/account_api";
import { setAuthenticated, setSuperuser } from "../state/auth";
import { EN_US_DEFAULT_TEXTS } from "../i18n/defaults/en-us";
import { setLocaleSignal, setTexts } from "../state/i18n";

const transport = vi.hoisted(() => ({ status: vi.fn(), action: vi.fn() }));
vi.mock("../services/account_api", () => ({ contractApi: { minecraftStatus: transport.status, minecraftAction: transport.action } }));

describe("Minecraft controls", () => {
  beforeEach(() => {
    setAuthenticated(false);
    setSuperuser(false);
    setLocaleSignal("en-US");
    setTexts(EN_US_DEFAULT_TEXTS);
    transport.status.mockReset().mockResolvedValue({ data: { players: [{ id: "00000000-0000-4000-8000-000000000001", name: "Alex", map_hidden: false }], whitelist: [], whitelist_enabled: true } });
    transport.action.mockReset().mockResolvedValue({ data: { acknowledged: true } });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("does not load controls or request private state for visitors", () => {
    render(() => <Minecraft />);
    expect(screen.queryByText("Server controls")).toBeNull();
    expect(contractApi.minecraftStatus).not.toHaveBeenCalled();
  });

  it("keeps the public map free of controls even for superusers", () => {
    setAuthenticated(true);
    setSuperuser(true);
    render(() => <Minecraft />);
    expect(screen.queryByRole("button", { name: "Server controls" })).toBeNull();
    expect(contractApi.minecraftStatus).not.toHaveBeenCalled();
  });

  it("requires restart confirmation and sends only the typed action", async () => {
    render(() => <MinecraftControls />);
    await screen.findByText("Alex");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByRole("button", { name: "Restart server" }));
    expect(transport.action).not.toHaveBeenCalled();
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Restart server" }));
    await waitFor(() => expect(transport.action).toHaveBeenCalledWith({ body: { action: "restart" } }));
    await screen.findByText(/Shutdown acknowledged/);
    expect(transport.action).toHaveBeenCalledTimes(1);
  });

  it("hides by UUID and requires confirmation before revealing a hidden player", async () => {
    const id = "00000000-0000-4000-8000-000000000001";
    render(() => <MinecraftControls />);
    await screen.findByText("Alex");
    transport.status.mockResolvedValue({ data: { players: [{ id, name: "Alex", map_hidden: true }], whitelist: [], whitelist_enabled: true } });
    fireEvent.click(screen.getByRole("button", { name: "Hide from map: Alex" }));
    await waitFor(() => expect(transport.action).toHaveBeenCalledWith({ body: { action: "map_visibility", id, hidden: true } }));
    await screen.findByText("Hidden from map");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByRole("button", { name: "Allow on map: Alex" }));
    expect(transport.action).toHaveBeenCalledTimes(1);
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Allow on map: Alex" }));
    await waitFor(() => expect(transport.action).toHaveBeenLastCalledWith({ body: { action: "map_visibility", id, hidden: false } }));
  });

  it("does not treat an unavailable visibility bridge as a visible player", async () => {
    transport.status.mockResolvedValue({ data: { players: [{ id: "a", name: "Alex", map_hidden: null }], whitelist: [], whitelist_enabled: true } });
    render(() => <MinecraftControls />);
    await screen.findByText("Map visibility unavailable");
    expect((screen.getByRole("button", { name: "Hide from map: Alex" }) as HTMLButtonElement).disabled).toBe(true);
    expect(transport.action).not.toHaveBeenCalled();
  });

  it("keeps a failed message for review and does not retry it", async () => {
    transport.action.mockRejectedValue(new Error("Check its state before retrying."));
    render(() => <MinecraftControls />);
    await screen.findByText("Alex");
    const input = screen.getByLabelText("Global message") as HTMLInputElement;
    fireEvent.input(input, { target: { value: "Hello everyone" } });
    await waitFor(() => expect((screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    await screen.findByRole("alert");
    expect(input.value).toBe("Hello everyone");
    expect(transport.action).toHaveBeenCalledTimes(1);
    expect(transport.action).toHaveBeenCalledWith({ body: { action: "message", message: "Hello everyone" } });
  });
});
