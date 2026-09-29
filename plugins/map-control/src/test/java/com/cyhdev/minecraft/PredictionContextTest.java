package com.cyhdev.minecraft;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.bukkit.World;

/** Pure header and coverage tests; no running game server or generated terrain is needed. */
public final class PredictionContextTest {
    private static int checks;
    private PredictionContextTest() {}

    public static void main(String[] args) throws Exception {
        validatesDimensions();
        Path directory = Files.createTempDirectory("map-prediction-coverage-");
        try {
            check(SavedChunkCoverage.absent(directory, 0, 0), "Missing region");
            Path region = directory.resolve("r.-1.-1.mca");
            ByteBuffer header = ByteBuffer.allocate(8192);
            header.putInt(1023 * 4, 513);
            Files.write(region, header.array());
            check(!SavedChunkCoverage.absent(directory, -1, -1), "Saved allocation is uncertain, never absent");
            check(SavedChunkCoverage.absent(directory, -2, -1), "Negative chunk location");
            Files.write(region, new byte[4096]);
            rejects(() -> SavedChunkCoverage.absent(directory, -2, -1), "Incomplete header");
            Files.delete(region);
            Files.createSymbolicLink(region, directory.resolve("missing-target"));
            rejects(() -> SavedChunkCoverage.absent(directory, -1, -1), "Region symlink");
            Files.delete(region);
            rejects(() -> SavedChunkCoverage.absent(directory.resolve("missing"), 0, 0), "Unknown region directory");
            combinesConservatively(directory);
        } finally {
            try (var paths = Files.list(directory)) { for (Path path : paths.toList()) Files.deleteIfExists(path); }
            Files.delete(directory);
        }
        System.out.println("PredictionContextTest: " + checks + " checks passed");
    }

    private static void validatesDimensions() throws IOException {
        for (boolean seedProfile : List.of(false, true)) {
            check(PredictionContext.dimension("minecraft:overworld", World.Environment.NORMAL, -64, 320, seedProfile)
                == PredictionContext.Dimension.OVERWORLD, "Overworld in both protocols");
        }
        check(PredictionContext.dimension("minecraft:the_nether", World.Environment.NETHER, 0, 256, true)
            == PredictionContext.Dimension.NETHER, "Vanilla Nether tiles");
        check(PredictionContext.dimension("minecraft:the_end", World.Environment.THE_END, 0, 256, true)
            == PredictionContext.Dimension.END, "Vanilla End tiles");
        rejects(() -> PredictionContext.dimension("minecraft:the_nether", World.Environment.NETHER, 0, 256, false), "Legacy Nether rejected");
        rejects(() -> PredictionContext.dimension("minecraft:the_end", World.Environment.THE_END, 0, 256, false), "Legacy End rejected");
        rejects(() -> PredictionContext.dimension("custom:the_end", World.Environment.THE_END, 0, 256, true), "Custom dimension rejected");
        rejects(() -> PredictionContext.dimension("minecraft:the_end", World.Environment.NORMAL, 0, 256, true), "Mismatched environment rejected");
        rejects(() -> PredictionContext.dimension("minecraft:the_nether", World.Environment.NETHER, -64, 256, true), "Modified minimum height rejected");
        rejects(() -> PredictionContext.dimension("minecraft:the_nether", World.Environment.NETHER, 0, 128, true), "Modified build height rejected");
        rejects(() -> PredictionContext.dimension("minecraft:overworld", World.Environment.NORMAL, 0, 256, true), "Modified Overworld rejected");
    }

    private static void combinesConservatively(Path directory) throws Exception {
        var request = WorldProtocol.parse("{\"kind\":\"prediction_context\",\"world\":\"minecraft:overworld\",\"chunk_x\":-2,\"chunk_z\":-3,\"width\":3,\"height\":2,\"y\":64}");
        var before = snapshot(directory, "revision", List.of("absent", "absent", "unknown", "generated", "absent", "excluded"));
        var after = snapshot(directory, "revision", List.of("absent", "absent", "absent", "absent", "excluded", "absent"));
        var coverage = PredictionContext.combine(request, before, after, List.of(true, false, true, true, true, true));
        check(coverage.stream().map(WorldProtocol.Coverage::state).toList().equals(
            List.of("ungenerated", "unknown", "unknown", "generated", "excluded", "excluded")), "Conservative merge");
        check(coverage.get(5).chunk_x() == 0 && coverage.get(5).chunk_z() == -2, "Exact row-major coverage");
        rejects(() -> PredictionContext.combine(request, before, snapshot(directory, "changed", after.states()), List.of(true)), "Profile changed during read");
    }

    private static PredictionContext.Snapshot snapshot(Path directory, String revision, List<String> states) {
        return new PredictionContext.Snapshot("minecraft:overworld", "world-id", 1, "large_biomes", revision, directory,
            new PredictionPacks.Selection(directory, List.of("vanilla", "paper")), null, states);
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
