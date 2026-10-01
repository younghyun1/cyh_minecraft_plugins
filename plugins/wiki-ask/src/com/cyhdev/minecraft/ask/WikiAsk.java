package com.cyhdev.minecraft.ask;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.SynchronousQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.event.ClickEvent;
import net.kyori.adventure.text.event.HoverEvent;
import net.kyori.adventure.text.format.NamedTextColor;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerQuitEvent;
import org.bukkit.plugin.java.JavaPlugin;

/** Shared per-world gameplay chat; Bukkit state stays on the server thread. */
public final class WikiAsk extends JavaPlugin implements Listener {
    private final Map<UUID, Long> cooldowns = new HashMap<>();
    private final AtomicBoolean busy = new AtomicBoolean();
    private final ThreadPoolExecutor worker = new ThreadPoolExecutor(1, 1, 0, TimeUnit.SECONDS,
            new SynchronousQueue<>(), Thread.ofPlatform().name("wiki-ask-worker").daemon().factory());
    private final ScheduledExecutorService deadlines = Executors.newSingleThreadScheduledExecutor(
            Thread.ofPlatform().name("wiki-ask-deadline").daemon().factory());
    private BridgeClient bridge;
    private volatile boolean running;

    @Override
    public void onEnable() {
        saveDefaultConfig();
        try {
            List<String> command = new ArrayList<>();
            command.add(path("bridge-binary", true));
            command.add("serve");
            command.addAll(List.of("--index", path("wiki-index", false), "--corpus", path("wiki-corpus", false), "--codex", path("codex-binary", true),
                    "--codex-home", path("codex-home", false), "--work-dir", path("work-directory", false)));
            bridge = new BridgeClient(command);
        } catch (IllegalArgumentException exception) {
            getLogger().severe("Configure existing absolute bridge, corpus, index, Codex home, and work paths before enabling WikiAsk.");
            getServer().getPluginManager().disablePlugin(this);
            return;
        }
        running = true;
        getServer().getPluginManager().registerEvents(this, this);
        busy.set(true);
        worker.execute(() -> {
            var deadline = deadlines.schedule(bridge::close, 43, TimeUnit.SECONDS);
            try {
                bridge.prepare();
            } catch (IOException | RuntimeException exception) {
                bridge.close();
                logFailure("startup", exception);
            } finally {
                deadline.cancel(false);
                busy.set(false);
            }
        });
    }

    private String path(String key, boolean executable) {
        String configured = getConfig().getString(key, "");
        Path path = Path.of(configured);
        if (configured.isBlank() || !path.isAbsolute() || !Files.exists(path)
                || (executable ? !Files.isRegularFile(path) || !Files.isExecutable(path) : !Files.isDirectory(path))) {
            throw new IllegalArgumentException("Invalid path");
        }
        return path.toString();
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (!(sender instanceof Player player)) {
            sender.sendMessage(Component.text("Use /ask in game."));
            return true;
        }
        if (!running || !player.hasPermission("wikiask.use")) return true;
        String question = String.join(" ", args).trim();
        String action = BridgeClient.action(args);
        if (!action.equals("ask") && !WorldChat.canControl(player)) {
            tell(player, "Only server operators can clear or compact conversations.");
            return true;
        }
        if (question.isEmpty() || question.codePointCount(0, question.length()) > 240
                || question.codePoints().anyMatch(Character::isISOControl)) {
            tell(player, "Usage: /ask <question> | clear | compact");
            return true;
        }
        UUID id = player.getUniqueId();
        String username = player.getName();
        UUID worldId = player.getWorld().getUID();
        String worldName = player.getWorld().getName();
        long now = System.nanoTime();
        Long last = cooldowns.get(id);
        if (last != null && now - last < TimeUnit.SECONDS.toNanos(10)) {
            tell(player, "Wait a few seconds before asking again.");
            return true;
        }
        if (cooldowns.size() >= 4096 && !cooldowns.containsKey(id)) {
            cooldowns.entrySet().removeIf(entry -> now - entry.getValue() >= TimeUnit.SECONDS.toNanos(10));
            if (cooldowns.size() >= 4096) { tell(player, "Minecraft help is busy. Try again shortly."); return true; }
        }
        if (!busy.compareAndSet(false, true)) {
            tell(player, "Minecraft help is busy. Try again shortly.");
            return true;
        }
        try {
            worker.execute(() -> answer(id, username, worldId, worldName, question, action));
            cooldowns.put(id, now);
            WorldChat.send(getServer().getOnlinePlayers(), worldId, WorldChat.question(username, question));
            player.sendActionBar(Component.text(action.equals("ask") ? "Checking the local wiki…" : "Updating this world's conversation…", NamedTextColor.GRAY));
        } catch (RejectedExecutionException exception) {
            busy.set(false);
            tell(player, "Minecraft help is unavailable. Try again shortly.");
        }
        return true;
    }

    private void answer(UUID id, String username, UUID worldId, String worldName, String question, String action) {
        var deadline = deadlines.schedule(bridge::close, action.equals("ask") ? 43 : 60, TimeUnit.SECONDS);
        String text;
        String source = "";
        try {
            String[] response = bridge.request(id.toString(), username, worldId.toString(), worldName,
                    action.equals("ask") ? question : "", action);
            text = response[0];
            source = response[1];
        } catch (IOException | RuntimeException exception) {
            bridge.close();
            text = "Minecraft help is temporarily unavailable. Try again shortly.";
            logFailure("request", exception);
        } finally {
            deadline.cancel(false);
        }
        String answer = text;
        String url = source;
        if (!running) { busy.set(false); return; }
        try {
            getServer().getScheduler().runTask(this, () -> {
                try {
                    Player player = getServer().getPlayer(id);
                    if (player == null || !player.isOnline() || !player.hasPermission("wikiask.use")
                            || !player.getWorld().getUID().equals(worldId)) return;
                    Component message = Component.text("[Ask] ", NamedTextColor.AQUA).append(Component.text(answer, NamedTextColor.WHITE));
                    if (!url.isEmpty()) {
                        message = message.append(Component.text(" [Wiki]", NamedTextColor.GRAY)
                                .clickEvent(ClickEvent.openUrl(url))
                                .hoverEvent(HoverEvent.showText(Component.text("Minecraft Wiki contributors · CC BY-NC-SA 3.0 · summarized"))));
                    }
                    WorldChat.send(getServer().getOnlinePlayers(), worldId, message);
                } finally { busy.set(false); }
            });
        } catch (RuntimeException exception) { busy.set(false); }
    }

    private static void tell(Player player, String message) {
        player.sendMessage(Component.text("[Ask] " + message, NamedTextColor.GRAY));
    }

    private void logFailure(String operation, Exception exception) {
        // Only bridge-owned IOException messages are eligible; parser exceptions may contain content.
        String detail = exception instanceof IOException && exception.getMessage() != null ? BridgeClient.plain(exception.getMessage())
                : exception.getClass().getSimpleName();
        getLogger().warning("WikiAsk " + operation + " failed: " + detail);
    }

    @EventHandler
    public void onQuit(PlayerQuitEvent event) { cooldowns.remove(event.getPlayer().getUniqueId()); }

    @Override
    public List<String> onTabComplete(CommandSender sender, Command command, String alias, String[] args) {
        if (WorldChat.canControl(sender) && args.length == 1) {
            String prefix = args[0].toLowerCase(java.util.Locale.ROOT);
            return List.of("clear", "compact").stream().filter(value -> value.startsWith(prefix)).toList();
        }
        return List.of();
    }

    @Override
    public void onDisable() {
        running = false;
        deadlines.shutdownNow();
        worker.shutdownNow();
        if (bridge != null) bridge.shutdown();
        cooldowns.clear();
    }
}
