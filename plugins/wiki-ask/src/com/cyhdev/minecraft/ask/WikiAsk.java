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

/** Private gameplay answers; Bukkit state is read and written only on the server thread. */
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
            command.addAll(List.of("--index", path("wiki-index", false), "--codex", path("codex-binary", true),
                    "--codex-home", path("codex-home", false), "--work-dir", path("work-directory", false)));
            bridge = new BridgeClient(command);
        } catch (IllegalArgumentException exception) {
            getLogger().severe("Configure existing absolute bridge, index, Codex home, and work paths before enabling WikiAsk.");
            getServer().getPluginManager().disablePlugin(this);
            return;
        }
        running = true;
        getServer().getPluginManager().registerEvents(this, this);
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
        if (question.isEmpty() || question.codePointCount(0, question.length()) > 240
                || question.codePoints().anyMatch(Character::isISOControl)) {
            tell(player, "Usage: /ask <Minecraft question, up to 240 characters>");
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
            worker.execute(() -> answer(id, username, worldId, worldName, question));
            cooldowns.put(id, now);
            player.sendActionBar(Component.text("Checking the local wiki…", NamedTextColor.GRAY));
        } catch (RejectedExecutionException exception) {
            busy.set(false);
            tell(player, "Minecraft help is unavailable. Try again shortly.");
        }
        return true;
    }

    private void answer(UUID id, String username, UUID worldId, String worldName, String question) {
        var deadline = deadlines.schedule(bridge::close, 43, TimeUnit.SECONDS);
        String text;
        String source = "";
        try {
            String[] response = bridge.ask(id.toString(), username, worldId.toString(), worldName, question);
            text = response[0];
            source = response[1];
        } catch (IOException | RuntimeException exception) {
            bridge.close();
            text = "Minecraft help is temporarily unavailable. Try again shortly.";
            getLogger().warning("WikiAsk request failed; check the dedicated bridge configuration and Codex login.");
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
                    player.sendMessage(message);
                } finally { busy.set(false); }
            });
        } catch (RuntimeException exception) { busy.set(false); }
    }

    private static void tell(Player player, String message) {
        player.sendMessage(Component.text("[Ask] " + message, NamedTextColor.GRAY));
    }

    @EventHandler
    public void onQuit(PlayerQuitEvent event) { cooldowns.remove(event.getPlayer().getUniqueId()); }

    @Override
    public List<String> onTabComplete(CommandSender sender, Command command, String alias, String[] args) {
        return List.of();
    }

    @Override
    public void onDisable() {
        running = false;
        deadlines.shutdownNow();
        worker.shutdownNow();
        if (bridge != null) bridge.close();
        cooldowns.clear();
    }
}
