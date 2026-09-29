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
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Future;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import io.papermc.paper.registry.RegistryAccess;
import io.papermc.paper.registry.RegistryKey;
import org.bukkit.Bukkit;
import org.bukkit.ChunkSnapshot;
import org.bukkit.Material;
import org.bukkit.NamespacedKey;
import org.bukkit.World;
import org.bukkit.block.Biome;
import org.bukkit.plugin.java.JavaPlugin;
import xyz.jpenilla.squaremap.api.SquaremapProvider;
import xyz.jpenilla.squaremap.api.WorldIdentifier;

/** One bounded, read-only world query at a time, independent of player visibility controls. */
final class WorldDataBridge implements AutoCloseable {
    private final JavaPlugin plugin;
    private final Path socketPath;
    private final ScheduledThreadPoolExecutor deadlines;
    private final ServerSocketChannel listener;
    private final PredictionContext predictions = new PredictionContext();
    private volatile SocketChannel active;

    WorldDataBridge(JavaPlugin plugin, Path directory) throws IOException {
        this.plugin = plugin;
        this.socketPath = directory.resolve("world.sock");
        Files.deleteIfExists(socketPath);
        ServerSocketChannel channel = ServerSocketChannel.open(StandardProtocolFamily.UNIX);
        try {
            channel.bind(UnixDomainSocketAddress.of(socketPath), 1);
            Files.setPosixFilePermissions(socketPath, PosixFilePermissions.fromString("rw-------"));
        } catch (IOException | RuntimeException error) {
            closeResource(channel);
            Files.deleteIfExists(socketPath);
            throw error;
        }
        listener = channel;
        deadlines = new ScheduledThreadPoolExecutor(1, task -> {
            Thread thread = new Thread(task, "cyhdev-world-deadline");
            thread.setDaemon(true);
            return thread;
        });
        deadlines.setRemoveOnCancelPolicy(true);
    }

    void start() {
        Thread worker = new Thread(this::serve, "cyhdev-world-data");
        worker.setDaemon(true);
        worker.start();
    }

    private void serve() {
        while (listener.isOpen()) {
            try (SocketChannel client = listener.accept()) {
                active = client;
                long expires = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
                var deadline = deadlines.schedule(() -> closeResource(client), 15, TimeUnit.SECONDS);
                try {
                    WorldProtocol.Request request;
                    try {
                        request = WorldProtocol.parse(WorldProtocol.read(client));
                    } catch (IOException error) {
                        writeError(client, "invalid_request");
                        continue;
                    }
                    write(client, WorldProtocol.encode(execute(request, client, expires)));
                } catch (Exception error) {
                    if (error instanceof InterruptedException) Thread.currentThread().interrupt();
                    Throwable cause = error;
                    while (cause.getCause() != null) cause = cause.getCause();
                    writeError(client, cause instanceof PredictionContext.Unsupported ? "unsupported_prediction"
                        : error instanceof TimeoutException ? "timeout" : "unavailable");
                } finally {
                    deadline.cancel(false);
                    active = null;
                }
            } catch (IOException | RuntimeException error) {
                if (listener.isOpen()) plugin.getLogger().warning("World data connection failed: " + error.getClass().getSimpleName());
            }
        }
    }

    private record Context(World world, int minimumY, int maximumY, Path regionDirectory,
                           Material material, Map<Biome, String> biomes) {}

    private WorldProtocol.WireResponse execute(WorldProtocol.Request request, SocketChannel client, long expires) throws Exception {
        long sampledAt = System.currentTimeMillis();
        if (request.kind().equals("catalog")) return sync(() -> catalog(sampledAt), client, expires);
        Context context = sync(() -> prepare(request), client, expires);
        if (request.kind().equals("prediction_context")) return prediction(request, context, sampledAt, client, expires);
        if (request.kind().equals("seed_profile")) return seedProfile(request, context, client, expires);
        List<WorldProtocol.Cell> cells = new ArrayList<>();
        LinkedHashSet<WorldProtocol.Structure> structures = new LinkedHashSet<>();
        List<WorldProtocol.Match> matches = new ArrayList<>();
        int scanned = 0;
        int missing = 0;
        boolean truncated = false;
        for (int dz = 0; dz < request.height(); dz++) {
            for (int dx = 0; dx < request.width(); dx++) {
                checkActive(client, expires);
                int x = request.chunk_x() + dx;
                int z = request.chunk_z() + dz;
                ChunkSnapshot snapshot = capture(context, request.kind().equals("area"), x, z, client, expires);
                if (snapshot == null) {
                    missing++;
                    continue;
                }
                scanned++;
                if (request.kind().equals("area")) {
                    sampleArea(snapshot, context, request.y(), cells);
                    try {
                        SavedStructures.Result saved = SavedStructures.read(context.regionDirectory(), x, z);
                        truncated |= saved.truncated();
                        for (WorldProtocol.Structure structure : saved.structures()) {
                            if (structures.size() < WorldProtocol.MAX_STRUCTURES) structures.add(structure);
                            else if (!structures.contains(structure)) truncated = true;
                        }
                    } catch (IOException error) {
                        // A concurrent world save can replace region data; keep terrain results and show incomplete structures.
                        truncated = true;
                    }
                } else if (findBlocks(snapshot, context.material(), request, matches, client, expires)) {
                    truncated = true;
                    return response(request, sampledAt, scanned, missing, truncated, cells, structures, matches);
                }
            }
        }
        return response(request, sampledAt, scanned, missing, truncated, cells, structures, matches);
    }

    private WorldProtocol.PredictionResponse prediction(WorldProtocol.Request request, Context context, long sampledAt,
                                                        SocketChannel client, long expires) throws Exception {
        PredictionContext.Snapshot before = sync(() -> predictions.capture(context.world(), request), client, expires);
        String packsFingerprint = predictionPacks(before, expires);
        List<Boolean> absent = new ArrayList<>(request.width() * request.height());
        for (int dz = 0; dz < request.height(); dz++) {
            for (int dx = 0; dx < request.width(); dx++) {
                checkActive(client, expires);
                boolean missing = false;
                if (before.states().get(absent.size()).equals("absent")) {
                    try { missing = SavedChunkCoverage.absent(context.regionDirectory(), request.chunk_x() + dx, request.chunk_z() + dz); }
                    catch (IOException error) { /* Unreadable or changing storage means unknown coverage. */ }
                }
                absent.add(missing);
            }
        }
        PredictionContext.Snapshot after = sync(() -> predictions.capture(context.world(), request), client, expires);
        if (!packsFingerprint.equals(predictionPacks(after, expires))) throw new PredictionContext.Unsupported();
        checkActive(client, expires);
        return new WorldProtocol.PredictionResponse("prediction_context", request.world(), after.worldId(), sampledAt,
            after.seed(), after.preset(), predictions.verifiedRevision(after, packsFingerprint),
            PredictionContext.combine(request, before, after, absent));
    }

    private static String predictionPacks(PredictionContext.Snapshot snapshot, long expires) throws PredictionContext.Unsupported {
        try { return PredictionPacks.fingerprint(snapshot.packs(), expires); }
        catch (IOException error) { throw new PredictionContext.Unsupported(); }
    }

    private WorldProtocol.SeedProfileResponse seedProfile(WorldProtocol.Request request, Context context,
                                                          SocketChannel client, long expires) throws Exception {
        PredictionContext.Snapshot before = sync(() -> predictions.capture(context.world(), request), client, expires);
        String fingerprint = predictionPacks(before, expires);
        if (!fingerprint.equals(predictionPacks(before, expires))) throw new PredictionContext.Unsupported();
        // The final policy capture follows disk inspection so its freshness is not extended by slow reads.
        PredictionContext.Snapshot after = sync(() -> predictions.capture(context.world(), request), client, expires);
        PredictionContext.requireSameProfile(before, after);
        checkActive(client, expires);
        var visibility = after.visibility();
        if (visibility == null) throw new PredictionContext.Unsupported();
        return new WorldProtocol.SeedProfileResponse("seed_profile", request.world(), after.worldId(), System.currentTimeMillis(),
            after.seed(), after.preset(), predictions.verifiedRevision(after, fingerprint), visibility.worldBorder(), visibility.shapes());
    }

    private static WorldProtocol.Response response(WorldProtocol.Request request, long sampledAt, int scanned, int missing,
                                                    boolean truncated, List<WorldProtocol.Cell> cells,
                                                    LinkedHashSet<WorldProtocol.Structure> structures, List<WorldProtocol.Match> matches) {
        return new WorldProtocol.Response(request.kind(), request.world(), sampledAt, scanned, missing, truncated,
            List.of(), List.of(), cells, List.copyOf(structures), matches);
    }

    /** Only currently enabled squaremap worlds may be queried, including after a configuration reload. */
    private Context prepare(WorldProtocol.Request request) throws IOException {
        if (SquaremapProvider.get().getWorldIfEnabled(WorldIdentifier.parse(request.world())).isEmpty()) throw new IOException("World is not mapped");
        NamespacedKey key = NamespacedKey.fromString(request.world());
        World world = key == null ? null : Bukkit.getWorld(key);
        if (world == null) throw new IOException("World is unavailable");
        int minimum = world.getMinHeight();
        int maximum = world.getMaxHeight() - 1;
        if (request.y() != null && (request.y() < minimum || request.y() > maximum)) throw new IOException("Biome height outside world");
        Material material = null;
        if (request.kind().equals("blocks")) {
            material = Material.matchMaterial(request.block());
            if (material == null || material.isLegacy() || !material.isBlock() || !material.getKey().toString().equals(request.block())) {
                throw new IOException("Unknown block");
            }
            if (request.min_y() < minimum || request.max_y() > maximum) throw new IOException("Block height outside world");
        }
        Map<Biome, String> biomes = new HashMap<>();
        if (request.kind().equals("area")) {
            var biomeRegistry = RegistryAccess.registryAccess().getRegistry(RegistryKey.BIOME);
            for (Biome biome : biomeRegistry) {
                NamespacedKey biomeKey = biomeRegistry.getKey(biome);
                if (biomeKey == null || !WorldProtocol.identifier(biomeKey.toString()) || biomes.size() >= 4096) throw new IOException("Invalid biome registry");
                biomes.put(biome, biomeKey.toString());
            }
        }
        return new Context(world, minimum, maximum, world.getWorldPath().resolve("region"), material, Map.copyOf(biomes));
    }

    private static WorldProtocol.Response catalog(long sampledAt) throws IOException {
        List<WorldProtocol.WorldInfo> worlds = new ArrayList<>();
        boolean truncated = false;
        for (var mapWorld : SquaremapProvider.get().mapWorlds()) {
            if (worlds.size() >= WorldProtocol.MAX_WORLDS) { truncated = true; break; }
            String id = mapWorld.identifier().asString();
            NamespacedKey key = NamespacedKey.fromString(id);
            World world = key == null ? null : Bukkit.getWorld(key);
            if (world == null || !WorldProtocol.identifier(id)) continue;
            // squaremap's levelWebName uses this exact transformation for config and tile directories.
            worlds.add(new WorldProtocol.WorldInfo(id, world.getName(), id.replace(':', '_'), world.getMinHeight(), world.getMaxHeight() - 1));
        }
        worlds.sort(Comparator.comparing(WorldProtocol.WorldInfo::id));
        List<String> blocks = new ArrayList<>();
        for (Material material : Material.values()) {
            if (material.isLegacy() || !material.isBlock()) continue;
            if (blocks.size() >= WorldProtocol.MAX_BLOCKS) { truncated = true; break; }
            String id = material.getKey().toString();
            if (!WorldProtocol.identifier(id)) throw new IOException("Invalid block registry");
            blocks.add(id);
        }
        blocks.sort(String::compareTo);
        return new WorldProtocol.Response("catalog", null, sampledAt, 0, 0, truncated, worlds, blocks, List.of(), List.of(), List.of());
    }

    private ChunkSnapshot capture(Context context, boolean area, int x, int z, SocketChannel client, long expires) throws Exception {
        CompletableFuture<ChunkSnapshot> result = new CompletableFuture<>();
        // Even already loaded chunks are captured at most once per tick; only one load can be pending.
        var scheduled = Bukkit.getScheduler().runTaskLater(plugin, () -> {
            try {
                checkActive(client, expires);
                if (SquaremapProvider.get().getWorldIfEnabled(WorldIdentifier.parse(context.world().key().asString())).isEmpty()) {
                    throw new IOException("World is no longer mapped");
                }
                context.world().getChunkAtAsync(x, z, false, false).whenComplete((chunk, error) -> {
                    if (error != null) { result.completeExceptionally(error); return; }
                    try {
                        checkActive(client, expires);
                        if (SquaremapProvider.get().getWorldIfEnabled(WorldIdentifier.parse(context.world().key().asString())).isEmpty()) {
                            throw new IOException("World is no longer mapped");
                        }
                        result.complete(chunk == null || !chunk.isGenerated() ? null : chunk.getChunkSnapshot(area, area, false, false));
                    } catch (Exception failure) {
                        result.completeExceptionally(failure);
                    }
                });
            } catch (Exception error) {
                result.completeExceptionally(error);
            }
        }, 1L);
        try {
            return result.get(remaining(client, expires), TimeUnit.NANOSECONDS);
        } finally {
            scheduled.cancel();
            result.cancel(false);
        }
    }

    private static void sampleArea(ChunkSnapshot snapshot, Context context, Integer sliceY, List<WorldProtocol.Cell> cells) throws IOException {
        for (int z = 0; z < 16; z += 4) {
            for (int x = 0; x < 16; x += 4) {
                int surface = Math.max(context.minimumY(), Math.min(context.maximumY(), snapshot.getHighestBlockYAt(x, z)));
                String biome = context.biomes().get(snapshot.getBiome(x, sliceY == null ? surface : sliceY, z));
                if (biome == null || cells.size() >= WorldProtocol.MAX_CELLS) throw new IOException("Invalid biome snapshot");
                cells.add(new WorldProtocol.Cell(snapshot.getX() * 16 + x, snapshot.getZ() * 16 + z, surface, biome));
            }
        }
    }

    /** Return true only after finding an additional match beyond the response cap. */
    private static boolean findBlocks(ChunkSnapshot snapshot, Material material, WorldProtocol.Request request,
                                      List<WorldProtocol.Match> matches, SocketChannel client, long expires) throws Exception {
        for (int y = request.min_y(); y <= request.max_y(); y++) {
            checkActive(client, expires);
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    if (snapshot.getBlockType(x, y, z) != material) continue;
                    if (matches.size() == WorldProtocol.MAX_MATCHES) return true;
                    matches.add(new WorldProtocol.Match(snapshot.getX() * 16 + x, y, snapshot.getZ() * 16 + z));
                }
            }
        }
        return false;
    }

    private <T> T sync(Callable<T> operation, SocketChannel client, long expires) throws Exception {
        Future<T> future = Bukkit.getScheduler().callSyncMethod(plugin, () -> {
            checkActive(client, expires);
            return operation.call();
        });
        try { return future.get(remaining(client, expires), TimeUnit.NANOSECONDS); }
        finally { future.cancel(false); }
    }

    private static long remaining(SocketChannel client, long expires) throws IOException, TimeoutException {
        checkActive(client, expires);
        return Math.max(1, expires - System.nanoTime());
    }

    private static void checkActive(SocketChannel client, long expires) throws IOException, TimeoutException {
        if (System.nanoTime() >= expires) throw new TimeoutException("World query expired");
        if (!client.isOpen()) throw new IOException("World query disconnected");
    }

    private static void write(SocketChannel client, ByteBuffer bytes) throws IOException {
        while (bytes.hasRemaining()) client.write(bytes);
    }

    private static void writeError(SocketChannel client, String error) {
        try { write(client, StandardCharsets.UTF_8.encode("{\"error\":\"" + error + "\"}\n")); }
        catch (IOException ignored) { /* The request deadline or peer may already have closed the socket. */ }
    }

    private static void closeResource(AutoCloseable resource) {
        if (resource == null) return;
        try { resource.close(); } catch (Exception ignored) { /* Shutdown is best effort. */ }
    }

    @Override
    public void close() {
        closeResource(listener);
        closeResource(active);
        deadlines.shutdownNow();
        try { Files.deleteIfExists(socketPath); }
        catch (IOException error) { plugin.getLogger().warning("World data socket cleanup failed"); }
    }
}
