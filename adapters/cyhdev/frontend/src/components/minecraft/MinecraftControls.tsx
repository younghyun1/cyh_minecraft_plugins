import { createSignal, For, onSettled, Show } from "solid-js";
import type { MinecraftAction, MinecraftStatus } from "../../generated";
import { contractApi } from "../../services/account_api";
import { t } from "../../state/i18n";

/** Requests are explicit and never retried because a lost response may follow a successful action. */
export default function MinecraftControls() {
  const [status, setStatus] = createSignal<MinecraftStatus | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [receipt, setReceipt] = createSignal("");
  const [message, setMessage] = createSignal("");
  const [name, setName] = createSignal("");

  const failure = (value: unknown) => value instanceof Error ? value.message : t("minecraft.failed");
  const refresh = async () => {
    if (busy()) return;
    setBusy(true);
    setError("");
    try {
      const response = await contractApi.minecraftStatus();
      setStatus(response.data);
    } catch (cause: unknown) {
      setStatus(null);
      setError(failure(cause));
    } finally { setBusy(false); }
  };

  const execute = async (body: MinecraftAction) => {
    if (busy()) return;
    setBusy(true);
    setError("");
    setReceipt("");
    try {
      await contractApi.minecraftAction({ body });
      setReceipt(body.action === "restart" ? t("minecraft.restarting") : t("minecraft.accepted"));
      if (body.action === "message") setMessage("");
      if (body.action === "whitelist_add") setName("");
      if (body.action === "restart") setStatus(null);
      else {
        try { setStatus((await contractApi.minecraftStatus()).data); }
        catch (cause: unknown) { setStatus(null); setError(failure(cause)); }
      }
    } catch (cause: unknown) { setError(failure(cause)); }
    finally { setBusy(false); }
  };

  onSettled(() => { void refresh(); });

  return (
    <section class="minecraft-controls" aria-label={t("minecraft.controls")} aria-busy={busy() ? "true" : "false"}>
      <Show when={error()}><p role="alert">{error()}</p></Show>
      <Show when={receipt()}><p role="status">{receipt()}</p></Show>
      <button type="button" disabled={busy()} onClick={() => void refresh()}>{t("common.refresh")}</button>
      <form onSubmit={(event) => { event.preventDefault(); void execute({ action: "message", message: message() }); }}>
        <label for="minecraft-message">{t("minecraft.message")}</label>
        <input id="minecraft-message" value={message()} maxlength={512} required disabled={busy()} onInput={(event) => setMessage(event.currentTarget.value)} />
        <button type="submit" disabled={busy() || !message().trim()}>{t("minecraft.send")}</button>
      </form>
      <h3>{t("minecraft.whitelist")}</h3>
      <Show when={status()}>{(current) => <>
        <p>{current().whitelist_enabled ? t("minecraft.enabled") : t("minecraft.disabled")}</p>
        <button type="button" disabled={busy()} onClick={() => {
          if (window.confirm(t("minecraft.whitelist_confirm"))) void execute({ action: "whitelist_enable", enabled: !current().whitelist_enabled });
        }}>{current().whitelist_enabled ? t("minecraft.disable") : t("minecraft.enable")}</button>
        <ul><For each={current().whitelist}>{(player) => <li>
          <span>{player.name}</span>
          <button type="button" disabled={busy()} onClick={() => {
            if (window.confirm(`${t("minecraft.remove_confirm")} ${player.name}`)) void execute({ action: "whitelist_remove", name: player.name });
          }}>{t("minecraft.remove")}</button>
        </li>}</For></ul>
      </>}</Show>
      <form onSubmit={(event) => { event.preventDefault(); void execute({ action: "whitelist_add", name: name() }); }}>
        <label for="minecraft-name">{t("minecraft.player_name")}</label>
        <input id="minecraft-name" value={name()} pattern="[A-Za-z0-9_]{1,16}" maxlength={16} required disabled={busy()} onInput={(event) => setName(event.currentTarget.value)} />
        <button type="submit" disabled={busy() || !/^[A-Za-z0-9_]{1,16}$/.test(name())}>{t("minecraft.add")}</button>
      </form>
      <h3>{t("minecraft.players")}</h3>
      <p>{t("minecraft.map_help")}</p>
      <Show when={status()}>{(current) => <ul><For each={current().players} fallback={<li>{t("minecraft.no_players")}</li>}>{(player) => <li>
        <span><span>{player.name}</span><small class="minecraft-visibility">{player.map_hidden == null ? t("minecraft.map_unknown") : player.map_hidden ? t("minecraft.map_hidden") : t("minecraft.map_allowed")}</small></span>
        <button type="button" disabled={busy() || player.map_hidden == null}
          aria-label={`${player.map_hidden ? t("minecraft.map_show") : t("minecraft.map_hide")}: ${player.name}`}
          onClick={() => {
            if (player.map_hidden == null) return;
            if (player.map_hidden && !window.confirm(`${t("minecraft.map_show_confirm")} ${player.name}`)) return;
            void execute({ action: "map_visibility", id: player.id, hidden: !player.map_hidden });
          }}>{player.map_hidden ? t("minecraft.map_show") : t("minecraft.map_hide")}</button>
        <button type="button" disabled={busy()} onClick={() => {
          if (window.confirm(`${t("minecraft.kick_confirm")} ${player.name}`)) void execute({ action: "kick", name: player.name });
        }}>{t("minecraft.kick")}</button>
      </li>}</For></ul>}</Show>
      <div class="minecraft-server-actions">
        <button type="button" disabled={busy()} onClick={() => void execute({ action: "save" })}>{t("minecraft.save")}</button>
        <button type="button" disabled={busy()} onClick={() => {
          if (window.confirm(t("minecraft.restart_confirm"))) void execute({ action: "restart" });
        }}>{t("minecraft.restart")}</button>
      </div>
    </section>
  );
}
