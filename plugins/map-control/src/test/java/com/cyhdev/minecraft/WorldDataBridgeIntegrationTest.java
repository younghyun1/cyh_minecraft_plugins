package com.cyhdev.minecraft;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.StandardProtocolFamily;
import java.net.UnixDomainSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

/** Opt-in checks against an isolated, empty Paper server, never the production server. */
public final class WorldDataBridgeIntegrationTest {
    private static int checks;

    private WorldDataBridgeIntegrationTest() {}

    public static void main(String[] args) throws Exception {
        if (!Boolean.getBoolean("cyhdev.minecraft.isolated") || args.length != 4) {
            throw new IllegalArgumentException("Requires -Dcyhdev.minecraft.isolated=true and <plugin-directory> <generated-chunk-x> <generated-chunk-z> <overworld-region-directory>");
        }
        Path directory = Path.of(args[0]).toAbsolutePath();
        Path worldSocket = directory.resolve("world.sock");
        Path controlSocket = directory.resolve("control.sock");
        int chunkX = Integer.parseInt(args[1]);
        int chunkZ = Integer.parseInt(args[2]);
        var saved = SavedStructures.read(Path.of(args[3]), chunkX, chunkZ);
        check(!saved.truncated(), "Saved fixture chunk must have readable structure metadata");
        check(Files.getPosixFilePermissions(directory).equals(PosixFilePermissions.fromString("rwx------")), "Private plugin directory");
        for (Path socket : new Path[] {worldSocket, controlSocket}) {
            check(Files.getPosixFilePermissions(socket).equals(PosixFilePermissions.fromString("rw-------")), "Private socket");
        }
        check(exchange(controlSocket, "STATUS\n").equals("OK\n"), "Isolated server must have no online players");
        JsonObject catalog = query(worldSocket, "{\"kind\":\"catalog\"}");
        check(catalog.get("kind").getAsString().equals("catalog") && catalog.get("world").isJsonNull(), "Catalog shape");
        check(!catalog.getAsJsonArray("blocks").isEmpty(), "Block catalog");
        JsonObject world = null;
        for (var entry : catalog.getAsJsonArray("worlds")) {
            if (entry.getAsJsonObject().get("id").getAsString().equals("minecraft:overworld")) world = entry.getAsJsonObject();
        }
        check(world != null, "Mapped overworld is required");
        if (world == null) throw new IOException("No mapped overworld");
        check(world.get("map_id").getAsString().equals("minecraft_overworld"), "Exact squaremap identifier");
        check(!catalog.has("seed") && !world.has("seed"), "Public catalog excludes private seed");
        String area = "{\"kind\":\"area\",\"world\":\"minecraft:overworld\",\"chunk_x\":" + chunkX
            + ",\"chunk_z\":" + chunkZ + ",\"width\":1,\"height\":1,\"y\":null}";
        JsonObject surface = query(worldSocket, area);
        check(surface.get("scanned_chunks").getAsInt() == 1 && surface.get("missing_chunks").getAsInt() == 0, "Generated chunk was captured");
        check(surface.getAsJsonArray("cells").size() == 16, "Sixteen biome/elevation samples per chunk");
        check(!surface.get("truncated").getAsBoolean(), "Saved fixture structure metadata is complete");
        for (var entry : surface.getAsJsonArray("cells")) {
            JsonObject cell = entry.getAsJsonObject();
            check(Math.floorDiv(cell.get("x").getAsInt(), 16) == chunkX && Math.floorDiv(cell.get("z").getAsInt(), 16) == chunkZ, "Cell lies in requested chunk");
            check(WorldProtocol.identifier(cell.get("biome").getAsString()), "Biome identifier");
        }
        int minY = world.get("min_y").getAsInt();
        int maxY = world.get("max_y").getAsInt();
        JsonObject sliced = query(worldSocket, area.replace("\"y\":null", "\"y\":" + minY));
        for (int i = 0; i < 16; i++) {
            check(sliced.getAsJsonArray("cells").get(i).getAsJsonObject().get("y").getAsInt()
                == surface.getAsJsonArray("cells").get(i).getAsJsonObject().get("y").getAsInt(), "Biome slice preserves surface elevation");
        }
        // In a fresh overworld the top three layers contain air. The cap must stop scanning on match 513.
        String blocks = "{\"kind\":\"blocks\",\"world\":\"minecraft:overworld\",\"chunk_x\":" + chunkX
            + ",\"chunk_z\":" + chunkZ + ",\"width\":1,\"height\":1,\"block\":\"minecraft:air\",\"min_y\":"
            + (maxY - 2) + ",\"max_y\":" + maxY + "}";
        JsonObject matches = query(worldSocket, blocks);
        check(matches.getAsJsonArray("matches").size() == 512 && matches.get("truncated").getAsBoolean(), "Block matches are capped explicitly");
        for (var entry : matches.getAsJsonArray("matches")) {
            int y = entry.getAsJsonObject().get("y").getAsInt();
            check(y >= maxY - 2 && y <= maxY, "Block match lies inside inclusive height range");
        }
        predictions(worldSocket, chunkX, chunkZ, Path.of(args[3]));
        JsonObject unknown = query(worldSocket,
            "{\"kind\":\"area\",\"world\":\"minecraft:overworld\",\"chunk_x\":1000000,\"chunk_z\":1000000,\"width\":1,\"height\":1,\"y\":null}");
        check(unknown.get("scanned_chunks").getAsInt() == 0 && unknown.get("missing_chunks").getAsInt() == 1
            && unknown.getAsJsonArray("cells").isEmpty(), "Unknown terrain remains ungenerated");
        check(query(worldSocket, "{\"kind\":\"catalog\",\"kind\":\"catalog\"}").get("error").getAsString().equals("invalid_request"), "Duplicate field rejected");
        JsonObject tooLarge = JsonParser.parseString(exchange(worldSocket, " ".repeat(WorldProtocol.REQUEST_BYTES))).getAsJsonObject();
        check(tooLarge.get("error").getAsString().equals("invalid_request"), "Oversized frame rejected");
        check(exchange(worldSocket, "{\"kind\":").isEmpty(), "Incomplete frame expires and closes");
        check(query(worldSocket, "{\"kind\":\"catalog\"}").get("kind").getAsString().equals("catalog"), "Worker recovers after timeout");
        check(exchange(controlSocket, "STATUS\n").equals("OK\n"), "Visibility socket still works");
        System.out.println("WorldDataBridgeIntegrationTest: " + checks + " checks passed");
    }

    private static void predictions(Path socket, int generatedX, int generatedZ, Path region) throws IOException {
        String request = "{\"kind\":\"prediction_context\",\"world\":\"minecraft:overworld\",\"chunk_x\":1000000,\"chunk_z\":1000000,\"width\":8,\"height\":8,\"y\":64}";
        Path missingRegion = region.resolve("r.31250.31250.mca");
        check(Files.notExists(missingRegion), "Distant fixture region absent before prediction");
        JsonObject first = query(socket, request);
        check(first.get("kind").getAsString().equals("prediction_context"), "Private context response");
        check(first.get("seed").getAsLong() == 1, "Synthetic fixture seed");
        check(first.get("preset").getAsString().equals(System.getProperty("cyhdev.minecraft.fixturePreset", "default")), "Effective noise preset");
        check(first.get("world_id").getAsString().matches("[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"), "World UUID");
        check(first.get("profile_revision").getAsString().matches("[a-f0-9]{64}"), "Opaque profile revision");
        check(first.getAsJsonArray("coverage").size() == 64, "Exhaustive bounded chunk coverage");
        for (int i = 0; i < 64; i++) {
            JsonObject chunk = first.getAsJsonArray("coverage").get(i).getAsJsonObject();
            check(chunk.get("chunk_x").getAsInt() == 1000000 + i % 8 && chunk.get("chunk_z").getAsInt() == 1000000 + i / 8,
                "Coverage coordinates");
            check(chunk.get("state").getAsString().equals("ungenerated"), "No saved or pending chunk");
        }
        JsonObject second = query(socket, request);
        check(first.get("profile_revision").equals(second.get("profile_revision")), "Unchanged profile revision");
        String generated = request.replace("\"chunk_x\":1000000", "\"chunk_x\":" + generatedX)
            .replace("\"chunk_z\":1000000", "\"chunk_z\":" + generatedZ).replace("\"width\":8", "\"width\":1").replace("\"height\":8", "\"height\":1");
        String state = query(socket, generated).getAsJsonArray("coverage").get(0).getAsJsonObject().get("state").getAsString();
        check(state.equals("generated") || state.equals("unknown"), "Existing chunk never receives predictions");
        check(query(socket, request.replace("minecraft:overworld", "minecraft:the_nether")).get("error").getAsString().equals("unsupported_prediction"), "Unsupported dimension");
        check(Files.notExists(missingRegion), "Prediction did not generate distant region");
    }

    private static JsonObject query(Path socket, String request) throws IOException {
        return JsonParser.parseString(exchange(socket, request + "\n")).getAsJsonObject();
    }

    private static String exchange(Path socket, String request) throws IOException {
        try (SocketChannel client = SocketChannel.open(StandardProtocolFamily.UNIX);
            var deadlines = Executors.newSingleThreadScheduledExecutor()) {
            var deadline = deadlines.schedule(() -> closeClient(client), 20, TimeUnit.SECONDS);
            try {
                client.connect(UnixDomainSocketAddress.of(socket));
                ByteBuffer input = StandardCharsets.UTF_8.encode(request);
                while (input.hasRemaining()) client.write(input);
                ByteBuffer chunk = ByteBuffer.allocate(8192);
                var output = new ByteArrayOutputStream();
                for (int count; (count = client.read(chunk)) != -1;) {
                    if (output.size() + count > WorldProtocol.RESPONSE_BYTES) throw new IOException("Integration response exceeded bound");
                    output.write(chunk.array(), 0, count);
                    chunk.clear();
                }
                return output.toString(StandardCharsets.UTF_8);
            } finally {
                deadline.cancel(false);
            }
        }
    }

    private static void check(boolean condition, String message) {
        checks++;
        if (!condition) throw new AssertionError(message);
    }

    private static void closeClient(SocketChannel client) {
        try { client.close(); } catch (IOException ignored) { /* Test deadline cleanup. */ }
    }
}
