package com.cyhdev.minecraft;

import java.io.IOException;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.border.WorldBorder;
import xyz.jpenilla.squaremap.api.MapWorld;
import xyz.jpenilla.squaremap.common.visibilitylimit.VisibilityLimitImpl;
import xyz.jpenilla.squaremap.common.visibilitylimit.VisibilityShape;

/** Compares the exported policy against the actual squaremap predicates without a running server. */
public final class PredictionVisibilityTest {
    private static int checks;
    private PredictionVisibilityTest() {}

    public static void main(String[] args) throws Exception {
        var circle = circle(8, -9, 12);
        var rectangle = rectangle(-15, -2, -4, 19);
        var border = new WorldBorder();
        border.setCenter(-1.25, 2.75);
        border.setSize(43.5);
        for (List<VisibilityShape> shapes : List.of(List.<VisibilityShape>of(), List.of(circle), List.of(rectangle), List.of(circle, rectangle))) {
            var actual = new VisibilityLimitImpl(null);
            actual.load(shapes);
            var policy = PredictionVisibility.capture(shapes, border);
            for (int z = -30; z <= 30; z++) {
                for (int x = -30; x <= 30; x++) {
                    check(allows(policy, x, z) == (actual.shouldRenderColumn(x, z) && border.isWithinBounds(x, z)),
                        "Exact column visibility and world-border intersection");
                }
            }
        }
        check(PredictionVisibility.capture(java.util.Collections.nCopies(64, circle), border).shapes().size() == 64, "64 shapes allowed");
        rejects(() -> PredictionVisibility.capture(java.util.Collections.nCopies(65, circle), border), "Shape count bound");
        rejects(() -> PredictionVisibility.capture(List.of(new UnknownShape()), border), "Unknown shape fails closed");
        rejects(() -> PredictionVisibility.capture(List.of(circle(0, 0, 46_341)), border), "Squaremap radius overflow rejected");
        rejects(() -> PredictionVisibility.rectangle(1, 0, 0, 1), "Reversed rectangle rejected");
        rejects(() -> PredictionVisibility.rectangle(-30_000_001, 0, 0, 1), "Coordinate bound");
        rejects(() -> PredictionVisibility.circle(0, 0, 0), "Zero radius rejected");
        rejects(() -> PredictionVisibility.borderBounds(Double.NaN, 0, 10, 10), "NaN border rejected");
        rejects(() -> PredictionVisibility.borderBounds(0, 0, Double.POSITIVE_INFINITY, 10), "Infinite border rejected");
        rejects(() -> PredictionVisibility.borderBounds(0.1, 0, 0.2, 1), "Border without integer columns rejected");
        var fractional = PredictionVisibility.borderBounds(-2.4, 1.2, 3.1, 8);
        check(fractional.equals(new WorldProtocol.Bounds(-2, 2, 3, 7)), "Fractional bounds use ceil and exclusive maximum");
        var rounded = PredictionVisibility.squaremapBorder(-1.8, 2.9, 5.1);
        check(rounded.equals(new WorldProtocol.RectangleVisibility("rectangle", -4, -1, 1, 4)), "Squaremap world-border rounding");
        check(PredictionVisibility.circle(30_000_000, -30_000_000, 46_340).radius() == 46_340, "Largest nonoverflowing circle");
        var distantCircle = circle(30_000_000, -30_000_000, 46_340);
        var distantPolicy = new PredictionVisibility.Policy(new WorldProtocol.Bounds(-30_000_000, -30_000_000, 30_000_000, 30_000_000),
            List.of(PredictionVisibility.circle(30_000_000, -30_000_000, 46_340)));
        for (int x : new int[] {-30_000_000, 0, 29_953_659, 29_953_660, 30_000_000}) {
            for (int z : new int[] {-30_000_000, -29_953_660, 0, 30_000_000}) {
                check(allows(distantPolicy, x, z) == distantCircle.shouldRenderColumn(null, x, z), "Circle arithmetic at coordinate limits");
            }
        }
        border.lerpSizeBetween(43.5, 20, 0, 1000);
        rejects(() -> PredictionVisibility.capture(List.of(), border), "Moving border fails closed");
        revisionsChangeWithPolicy();
        System.out.println("PredictionVisibilityTest: " + checks + " checks passed");
    }

    private static void revisionsChangeWithPolicy() throws IOException {
        var generator = new PredictionContext();
        var packs = new PredictionPacks.Selection(Path.of("/synthetic"), List.of("vanilla"));
        var first = new PredictionVisibility.Policy(new WorldProtocol.Bounds(-10, -10, 10, 10), List.of());
        var second = new PredictionVisibility.Policy(new WorldProtocol.Bounds(-9, -10, 10, 10), List.of());
        var before = new PredictionContext.Snapshot("minecraft:overworld", "test", 1, "large_biomes", "base",
            Path.of("/synthetic"), packs, first, List.of());
        var after = new PredictionContext.Snapshot("minecraft:overworld", "test", 1, "large_biomes", "base",
            Path.of("/synthetic"), packs, second, List.of());
        check(!generator.verifiedRevision(before, "packs").equals(generator.verifiedRevision(after, "packs")), "Visibility policy changes opaque revision");
        rejects(() -> PredictionContext.requireSameProfile(before, after), "Policy change during capture rejected");
    }

    private static boolean allows(PredictionVisibility.Policy policy, int x, int z) {
        var border = policy.worldBorder();
        if (x < border.min_x() || x > border.max_x() || z < border.min_z() || z > border.max_z()) return false;
        if (policy.shapes().isEmpty()) return true;
        for (var shape : policy.shapes()) {
            if (shape instanceof WorldProtocol.RectangleVisibility r && x >= r.min_x() && x <= r.max_x() && z >= r.min_z() && z <= r.max_z()) return true;
            if (shape instanceof WorldProtocol.CircleVisibility c) {
                long dx = (long) x - c.center_x(), dz = (long) z - c.center_z();
                if (dx * dx + dz * dz <= (long) c.radius() * c.radius()) return true;
            }
        }
        return false;
    }

    private static VisibilityShape circle(int x, int z, int radius) throws ReflectiveOperationException {
        var constructor = Class.forName("xyz.jpenilla.squaremap.common.visibilitylimit.CircleShape").getDeclaredConstructor(int.class, int.class, int.class);
        constructor.setAccessible(true);
        return (VisibilityShape) constructor.newInstance(x, z, radius);
    }

    private static VisibilityShape rectangle(int minX, int minZ, int maxX, int maxZ) throws ReflectiveOperationException {
        var constructor = Class.forName("xyz.jpenilla.squaremap.common.visibilitylimit.RectangleShape").getDeclaredConstructor(BlockPos.class, BlockPos.class);
        constructor.setAccessible(true);
        return (VisibilityShape) constructor.newInstance(new BlockPos(minX, 0, minZ), new BlockPos(maxX, 0, maxZ));
    }

    private static final class UnknownShape implements VisibilityShape {
        @Override public boolean shouldRenderChunk(MapWorld world, int x, int z) { return true; }
        @Override public boolean shouldRenderRegion(MapWorld world, int x, int z) { return true; }
        @Override public boolean shouldRenderColumn(MapWorld world, int x, int z) { return true; }
        @Override public int countChunksInRegion(MapWorld world, int x, int z) { return 1024; }
    }

    @FunctionalInterface private interface Checked { void run() throws Exception; }
    private static void rejects(Checked operation, String message) throws IOException {
        boolean rejected = false;
        try { operation.run(); } catch (PredictionContext.Unsupported expected) { rejected = true; }
        catch (Exception error) { throw new IOException("Unexpected test failure", error); }
        check(rejected, message);
    }

    private static void check(boolean condition, String message) {
        checks++;
        if (!condition) throw new AssertionError(message);
    }
}
