package com.cyhdev.minecraft;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Function-only compatibility must not admit data that changes generation or filters vanilla data. */
public final class PredictionPacksTest {
    private static final String METADATA = "{\"pack\":{\"description\":\"Spawn chunks\",\"min_format\":121,\"max_format\":121}}";
    private static int checks;
    private PredictionPacksTest() {}

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("map-prediction-packs-");
        try {
            Path pack = directory.resolve("spawn");
            Path function = pack.resolve("data/spawn/function/load.mcfunction");
            Files.createDirectories(function.getParent());
            Files.writeString(pack.resolve("pack.mcmeta"), METADATA);
            Files.writeString(function, "forceload add 0 0\n");
            Path loadTag = pack.resolve("data/minecraft/tags/function/load.json");
            Files.createDirectories(loadTag.getParent());
            Files.writeString(loadTag, "{\"values\":[\"spawn:load\"]}");
            var selection = new PredictionPacks.Selection(directory, List.of("vanilla", "paper", "file/spawn"));
            String original = fingerprint(selection);
            check(original.length() == 64 && original.equals(fingerprint(selection)), "Stable function-only pack");
            check(original.equals(fingerprint(new PredictionPacks.Selection(directory,
                List.of("file/spawn", "paper", "vanilla")))), "Pack order independent");
            Files.writeString(function, "forceload add 16 16\n");
            check(!original.equals(fingerprint(selection)), "Function edits invalidate profile");
            check(fingerprint(new PredictionPacks.Selection(directory.resolve("missing"), List.of("vanilla", "paper"))).length() == 64,
                "Built-in packs do not require a directory");
            rejects(() -> PredictionPacks.fingerprint(selection, System.nanoTime() - 1), "Expired query");

            for (String resource : List.of("worldgen/biome/custom.json", "worldgen/noise_settings/overworld.json",
                    "dimension/overworld.json", "dimension_type/overworld.json", "tags/worldgen/biome/overworld.json",
                    "recipe/custom.json", "tags/function/load.json.mcmeta")) {
                Path extra = pack.resolve("data/spawn").resolve(resource);
                Files.createDirectories(extra.getParent());
                Files.writeString(extra, "{}");
                rejects(() -> fingerprint(selection), "Reject generation/unknown resource: " + resource);
                removeTree(pack.resolve("data/spawn").resolve(resource.split("/")[0]));
            }
            Path unknown = pack.resolve("data/spawn/worldgen");
            Files.createDirectory(unknown);
            rejects(() -> fingerprint(selection), "Unknown empty directory");
            Files.delete(unknown);

            for (String metadata : List.of(
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":121},\"filter\":{}}",
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":121},\"overlays\":{}}",
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":121},\"features\":{}}",
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":121,\"unknown\":1}}",
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":121,\"pack_format\":121}}",
                    "{\"pack\":{\"description\":{},\"pack_format\":121}}",
                    "{\"pack\":{\"description\":\"x\",\"pack_format\":\"121\"}}",
                    "{\"pack\":{\"description\":\"x\"}}", METADATA + "{}")) {
                Files.writeString(pack.resolve("pack.mcmeta"), metadata);
                rejects(() -> fingerprint(selection), "Ambiguous pack metadata");
            }
            Files.writeString(pack.resolve("pack.mcmeta"), " ".repeat(8193));
            rejects(() -> fingerprint(selection), "Metadata byte bound");
            Files.writeString(pack.resolve("pack.mcmeta"), METADATA);
            Files.write(function, new byte[65537]);
            rejects(() -> fingerprint(selection), "Function byte bound");
            Files.writeString(function, "forceload add 0 0\n");

            Path linked = function.getParent().resolve("linked.mcfunction");
            Files.createSymbolicLink(linked, function);
            rejects(() -> fingerprint(selection), "Resource symlink");
            Files.delete(linked);
            Path linkedDirectory = pack.resolve("data/linked");
            Files.createSymbolicLink(linkedDirectory, pack.resolve("data/spawn"));
            rejects(() -> fingerprint(selection), "Namespace symlink");
            Files.delete(linkedDirectory);
            Files.createSymbolicLink(directory.resolve("linked"), pack);
            rejects(() -> fingerprint(new PredictionPacks.Selection(directory, List.of("vanilla", "file/linked"))), "Pack symlink");
            Files.delete(directory.resolve("linked"));

            for (String name : List.of("file/../spawn", "file/.", "file/..", "file/spawn.zip", "plugin/custom", "file/missing")) {
                rejects(() -> fingerprint(new PredictionPacks.Selection(directory, List.of("vanilla", name))), "Unsupported pack: " + name);
            }
            rejects(() -> fingerprint(new PredictionPacks.Selection(directory, List.of("paper"))), "Vanilla required");
            rejects(() -> fingerprint(new PredictionPacks.Selection(directory, List.of("vanilla", "vanilla"))), "Duplicate packs");
            rejects(() -> fingerprint(new PredictionPacks.Selection(directory, java.util.Collections.nCopies(9, "vanilla"))), "Pack count bound");

            Path deep = function.getParent().resolve("a/b/c/d/e/f/g/deep.mcfunction");
            Files.createDirectories(deep.getParent());
            Files.writeString(deep, "return 0");
            rejects(() -> fingerprint(selection), "Resource depth bound");
            removeTree(function.getParent().resolve("a"));
            Path many = function.getParent().resolve("many");
            Files.createDirectory(many);
            for (int i = 0; i < 257; i++) Files.writeString(many.resolve(i + ".mcfunction"), "return 0");
            rejects(() -> fingerprint(selection), "Entry count bound");
            removeTree(many);
            Path large = function.getParent().resolve("large");
            Files.createDirectory(large);
            for (int i = 0; i < 17; i++) Files.write(large.resolve(i + ".mcfunction"), new byte[65536]);
            rejects(() -> fingerprint(selection), "Combined pack byte bound");
            removeTree(large);
            Files.delete(pack.resolve("pack.mcmeta"));
            rejects(() -> fingerprint(selection), "Metadata required");
        } finally { removeTree(directory); }
        System.out.println("PredictionPacksTest: " + checks + " checks passed");
    }

    private static String fingerprint(PredictionPacks.Selection selection) throws IOException {
        return PredictionPacks.fingerprint(selection, System.nanoTime() + 5_000_000_000L);
    }

    private static void removeTree(Path root) throws IOException {
        try (var paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
        }
    }

    @FunctionalInterface private interface Checked { void run() throws IOException; }
    private static void rejects(Checked operation, String message) throws IOException {
        boolean rejected = false;
        try { operation.run(); } catch (IOException expected) { rejected = true; }
        check(rejected, message);
    }

    private static void check(boolean condition, String message) {
        checks++;
        if (!condition) throw new AssertionError(message);
    }
}
