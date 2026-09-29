package com.cyhdev.minecraft;

import java.io.IOException;
import java.net.StandardProtocolFamily;
import java.net.UnixDomainSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.UUID;
import java.util.concurrent.Future;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import org.bukkit.Bukkit;
import org.bukkit.plugin.java.JavaPlugin;
import xyz.jpenilla.squaremap.api.SquaremapProvider;

/** Same-user IPC only; squaremap remains the sole owner of persistent visibility. */
public final class MapControl extends JavaPlugin {
    private ServerSocketChannel listener;
    private volatile SocketChannel active;
    private ScheduledThreadPoolExecutor deadlines;
    private Path socketPath;
    private WorldDataBridge worldData;

    @Override
    public void onEnable() {
        try {
            Path directory = getDataFolder().toPath().toAbsolutePath();
            Files.createDirectories(directory);
            Files.setPosixFilePermissions(directory, PosixFilePermissions.fromString("rwx------"));
            socketPath = directory.resolve("control.sock");
            // The private directory is owned by this plugin. Remove only its previous socket.
            Files.deleteIfExists(socketPath);
            listener = ServerSocketChannel.open(StandardProtocolFamily.UNIX);
            listener.bind(UnixDomainSocketAddress.of(socketPath), 1);
            Files.setPosixFilePermissions(socketPath, PosixFilePermissions.fromString("rw-------"));
            deadlines = new ScheduledThreadPoolExecutor(1, task -> {
                Thread thread = new Thread(task, "cyhdev-map-deadline");
                thread.setDaemon(true);
                return thread;
            });
            deadlines.setRemoveOnCancelPolicy(true);
            worldData = new WorldDataBridge(this, directory);
            worldData.start();
            Thread worker = new Thread(this::serve, "cyhdev-map-control");
            worker.setDaemon(true);
            worker.start();
        } catch (IOException | RuntimeException error) {
            getLogger().severe("Map control socket could not start: " + error.getClass().getSimpleName());
            Bukkit.getPluginManager().disablePlugin(this);
        }
    }

    private void serve() {
        // One request at a time, one deadline, no executor queue of player mutations.
        while (listener.isOpen()) {
            try (SocketChannel client = listener.accept()) {
                active = client;
                long expires = System.nanoTime() + TimeUnit.SECONDS.toNanos(3);
                var deadline = deadlines.schedule(() -> close(client), 3, TimeUnit.SECONDS);
                try {
                    String request = readRequest(client);
                    Future<String> result = Bukkit.getScheduler().callSyncMethod(this, () -> {
                        if (System.nanoTime() >= expires || !client.isOpen()) return "ERROR\n";
                        return execute(request);
                    });
                    try {
                        write(client, result.get(Math.max(1, expires - System.nanoTime()), TimeUnit.NANOSECONDS));
                    } finally {
                        // A timed-out queued operation must not execute on a later server tick.
                        result.cancel(false);
                    }
                } catch (Exception error) {
                    if (error instanceof InterruptedException) Thread.currentThread().interrupt();
                    try { write(client, "ERROR\n"); } catch (IOException ignored) { /* Peer timed out. */ }
                } finally {
                    deadline.cancel(false);
                    active = null;
                }
            } catch (IOException | RuntimeException error) {
                if (listener.isOpen()) getLogger().warning("Map control connection failed: " + error.getClass().getSimpleName());
            }
        }
    }

    /** Bukkit and squaremap calls run only on the server thread. */
    private String execute(String request) {
        var manager = SquaremapProvider.get().playerManager();
        if (request.equals("STATUS")) {
            var players = Bukkit.getOnlinePlayers();
            if (players.size() > 1000) return "ERROR\n";
            StringBuilder response = new StringBuilder("OK\n");
            for (var player : players) {
                UUID id = player.getUniqueId();
                response.append(id).append(' ').append(manager.hidden(id) ? '1' : '0').append('\n');
            }
            return response.toString();
        }
        String[] parts = request.split(" ", -1);
        if (parts.length != 2 || !(parts[0].equals("HIDE") || parts[0].equals("SHOW"))) return "ERROR\n";
        UUID id;
        try { id = UUID.fromString(parts[1]); }
        catch (IllegalArgumentException error) { return "ERROR\n"; }
        if (!id.toString().equals(parts[1]) || Bukkit.getPlayer(id) == null) return "ERROR\n";
        boolean hidden = parts[0].equals("HIDE");
        manager.hidden(id, hidden, true);
        return manager.hidden(id) == hidden ? "OK\n" : "ERROR\n";
    }

    private static String readRequest(SocketChannel client) throws IOException {
        ByteBuffer bytes = ByteBuffer.allocate(64);
        while (bytes.hasRemaining()) {
            int count = client.read(bytes);
            if (count < 0) throw new IOException("Incomplete request");
            for (int i = 0; i < bytes.position(); i++) {
                if (bytes.get(i) == '\n') {
                    if (i != bytes.position() - 1) throw new IOException("Trailing request data");
                    return new String(bytes.array(), 0, i, StandardCharsets.US_ASCII);
                }
            }
        }
        throw new IOException("Request too large");
    }

    private static void write(SocketChannel client, String response) throws IOException {
        ByteBuffer bytes = StandardCharsets.US_ASCII.encode(response);
        while (bytes.hasRemaining()) client.write(bytes);
    }

    private static void close(AutoCloseable resource) {
        if (resource == null) return;
        try { resource.close(); } catch (Exception ignored) { /* Shutdown is best effort. */ }
    }

    @Override
    public void onDisable() {
        close(worldData);
        close(listener);
        close(active);
        if (deadlines != null) deadlines.shutdownNow();
        if (socketPath != null) {
            try { Files.deleteIfExists(socketPath); }
            catch (IOException error) { getLogger().warning("Map control socket cleanup failed"); }
        }
    }
}
