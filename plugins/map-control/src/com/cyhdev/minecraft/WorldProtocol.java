package com.cyhdev.minecraft;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.Strictness;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
import java.io.IOException;
import java.io.StringReader;
import java.nio.ByteBuffer;
import java.nio.channels.ReadableByteChannel;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;

/** The local world-data protocol has no commands, mutations, or client-selected paths. */
final class WorldProtocol {
    static final int REQUEST_BYTES = 4096;
    static final int RESPONSE_BYTES = 2 * 1024 * 1024;
    static final int MAX_WORLDS = 16;
    static final int MAX_BLOCKS = 4096;
    static final int MAX_CELLS = 1024;
    static final int MAX_STRUCTURES = 256;
    static final int MAX_MATCHES = 512;
    static final int MIN_Y = -2032;
    static final int MAX_Y = 2031;
    private static final long BORDER = 30_000_000L;
    private static final Pattern IDENTIFIER = Pattern.compile("[a-z0-9_.-]+:[a-z0-9_./-]+");
    private static final Pattern INTEGER = Pattern.compile("-?(0|[1-9][0-9]*)");
    private static final Set<String> STRINGS = Set.of("kind", "world", "block");
    private static final Set<String> NUMBERS = Set.of("chunk_x", "chunk_z", "width", "height", "y", "min_y", "max_y");
    private static final Set<String> AREA_FIELDS = Set.of("kind", "world", "chunk_x", "chunk_z", "width", "height", "y");
    private static final Set<String> BLOCK_FIELDS = Set.of("kind", "world", "chunk_x", "chunk_z", "width", "height", "block", "min_y", "max_y");
    private static final Gson JSON = new GsonBuilder().serializeNulls().disableHtmlEscaping().create();

    private WorldProtocol() {}

    record Request(String kind, String world, int chunk_x, int chunk_z, int width, int height,
                   Integer y, String block, int min_y, int max_y) {}
    record WorldInfo(String id, String name, String map_id, int min_y, int max_y) {}
    record Cell(int x, int z, int y, String biome) {}
    record Structure(String kind, int min_x, int min_y, int min_z, int max_x, int max_y, int max_z) {}
    record Match(int x, int y, int z) {}
    sealed interface WireResponse permits Response, PredictionResponse, SeedProfileResponse {}
    record Response(String kind, String world, long sampled_at_ms, int scanned_chunks, int missing_chunks,
                    boolean truncated, List<WorldInfo> worlds, List<String> blocks, List<Cell> cells,
                    List<Structure> structures, List<Match> matches) implements WireResponse {}
    record Coverage(int chunk_x, int chunk_z, String state) {}
    /** This response belongs only on the same-user socket; it must never be forwarded to a browser. */
    record PredictionResponse(String kind, String world, String world_id, long sampled_at_ms, long seed,
                              String preset, String profile_revision, List<Coverage> coverage) implements WireResponse {
        @Override public String toString() { return "PredictionResponse[private profile]"; }
    }
    record Bounds(int min_x, int min_z, int max_x, int max_z) {}
    sealed interface VisibilityShape permits RectangleVisibility, CircleVisibility {}
    record RectangleVisibility(String kind, int min_x, int min_z, int max_x, int max_z) implements VisibilityShape {}
    record CircleVisibility(String kind, int center_x, int center_z, int radius) implements VisibilityShape {}
    /** Private seed metadata only; visibility is a union intersected with the world border. */
    record SeedProfileResponse(String kind, String world, String world_id, long sampled_at_ms, long seed,
                               String preset, String profile_revision, Bounds world_border,
                               List<VisibilityShape> visibility) implements WireResponse {
        @Override public String toString() { return "SeedProfileResponse[private profile]"; }
    }

    static boolean identifier(String value) {
        return value != null && value.length() <= 128 && IDENTIFIER.matcher(value).matches();
    }

    static Request parse(String line) throws IOException {
        Map<String, String> strings = new HashMap<>();
        Map<String, Integer> numbers = new HashMap<>();
        Set<String> fields = new HashSet<>();
        try (JsonReader reader = new JsonReader(new StringReader(line))) {
            reader.setStrictness(Strictness.STRICT);
            reader.beginObject();
            while (reader.hasNext()) {
                String name = reader.nextName();
                if (!fields.add(name)) throw new IOException("Duplicate field");
                if (STRINGS.contains(name) && reader.peek() == JsonToken.STRING) {
                    strings.put(name, reader.nextString());
                } else if (NUMBERS.contains(name) && reader.peek() == JsonToken.NUMBER) {
                    String value = reader.nextString();
                    if (!INTEGER.matcher(value).matches()) throw new IOException("Expected integer");
                    numbers.put(name, Integer.valueOf(value));
                } else if (name.equals("y") && reader.peek() == JsonToken.NULL) {
                    reader.nextNull();
                } else {
                    throw new IOException("Unknown field or invalid type");
                }
            }
            reader.endObject();
            if (reader.peek() != JsonToken.END_DOCUMENT) throw new IOException("Trailing JSON");
        } catch (IllegalArgumentException | IllegalStateException error) {
            throw new IOException("Invalid JSON request", error);
        }
        String kind = strings.get("kind");
        if ("catalog".equals(kind)) {
            if (!fields.equals(Set.of("kind"))) throw new IOException("Unexpected catalog fields");
            return new Request(kind, null, 0, 0, 0, 0, null, null, 0, 0);
        }
        if ("seed_profile".equals(kind)) {
            if (!fields.equals(Set.of("kind", "world")) || !identifier(strings.get("world"))) throw new IOException("Invalid seed profile request");
            return new Request(kind, strings.get("world"), 0, 0, 0, 0, null, null, 0, 0);
        }
        boolean prediction = "prediction_context".equals(kind);
        boolean area = "area".equals(kind) || prediction;
        if (!area && !"blocks".equals(kind)) throw new IOException("Unknown query kind");
        // Omitting the optional biome slice means sample each column's surface biome.
        if (area && !prediction) fields.add("y");
        if (!fields.equals(area ? AREA_FIELDS : BLOCK_FIELDS)) throw new IOException("Missing query fields");
        String world = strings.get("world");
        String block = strings.get("block");
        if (!identifier(world) || (!area && !identifier(block))) throw new IOException("Invalid identifier");
        int chunkX = numbers.get("chunk_x");
        int chunkZ = numbers.get("chunk_z");
        int width = numbers.get("width");
        int height = numbers.get("height");
        int maximum = area ? 8 : 4;
        if (width < 1 || height < 1 || width > maximum || height > maximum) throw new IOException("Area too large");
        if (!insideBorder(chunkX, width) || !insideBorder(chunkZ, height)) throw new IOException("Outside world border");
        Integer y = numbers.get("y");
        if (prediction && y == null) throw new IOException("Prediction requires a fixed biome height");
        int minimumY = area ? 0 : numbers.get("min_y");
        int maximumY = area ? 0 : numbers.get("max_y");
        if (y != null && (y < MIN_Y || y > MAX_Y)) throw new IOException("Invalid biome height");
        if (!area && (minimumY < MIN_Y || maximumY > MAX_Y || maximumY < minimumY || (long) maximumY - minimumY + 1 > 512)) {
            throw new IOException("Invalid block height range");
        }
        return new Request(kind, world, chunkX, chunkZ, width, height, y, block, minimumY, maximumY);
    }

    private static boolean insideBorder(int chunk, int count) {
        long first = (long) chunk * 16;
        long last = ((long) chunk + count) * 16 - 1;
        return first >= -BORDER && last <= BORDER;
    }

    static String read(ReadableByteChannel client) throws IOException {
        ByteBuffer bytes = ByteBuffer.allocate(REQUEST_BYTES);
        int checked = 0;
        while (bytes.hasRemaining()) {
            if (client.read(bytes) < 0) throw new IOException("Incomplete request");
            while (checked < bytes.position()) {
                if (bytes.get(checked) == '\n') {
                    if (checked != bytes.position() - 1) throw new IOException("Trailing request data");
                    try {
                        return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes.array(), 0, checked)).toString();
                    } catch (CharacterCodingException error) {
                        throw new IOException("Invalid UTF-8", error);
                    }
                }
                checked++;
            }
        }
        throw new IOException("Request too large");
    }

    static ByteBuffer encode(WireResponse response) throws IOException {
        byte[] bytes = (JSON.toJson(response) + "\n").getBytes(StandardCharsets.UTF_8);
        if (bytes.length > RESPONSE_BYTES) throw new IOException("Response too large");
        return ByteBuffer.wrap(bytes);
    }
}
