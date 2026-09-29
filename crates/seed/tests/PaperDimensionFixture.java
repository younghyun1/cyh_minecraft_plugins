import java.io.PrintWriter;
import java.nio.file.Path;
import java.util.Locale;
import net.minecraft.core.Holder;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.densityfunction.DensityFunction;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.densityfunction.generator.EndIslandFunction;
import net.minecraft.world.level.levelgen.synth.Noise;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Offline synthetic-seed oracle using Paper's noise classes; no server or world is created. */
public final class PaperDimensionFixture {
    private PaperDimensionFixture() {}

    @SuppressWarnings("deprecation") // These are the exact legacy entry points RandomState uses.
    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("Expected fixture output path");
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        try (var output = new PrintWriter(Path.of(args[0]).toFile())) {
            output.println("seed\tx\tz\ttemperature\tvegetation\tend_erosion");
            for (long seed : new long[] {1, -1, Long.MIN_VALUE, Long.MAX_VALUE}) {
                var temperature = NormalNoise.createParity(-7, 1, 1)
                    .createForLegacyNetherBiome(new LegacyRandomSource(seed));
                var vegetation = NormalNoise.createParity(-7, 1, 1)
                    .createForLegacyNetherBiome(new LegacyRandomSource(seed + 1));
                var end = new EndIslandFunction().compileSampler(new DensityFunction.CompileContext() {
                    @Override public Noise createNoiseSampler(Holder<NormalNoise> noise) {
                        throw new UnsupportedOperationException("End island fixture uses no octave noise");
                    }
                    @Override public RandomSource createRandom(Identifier id) {
                        throw new UnsupportedOperationException("End island fixture uses its legacy seed");
                    }
                    @Override public RandomSource createEndIslandRandom() {
                        return new LegacyRandomSource(seed);
                    }
                });
                int[] coordinates = {-30000000, -1048576, -8192, -2049, -1024, -17, -1,
                    0, 15, 16, 1023, 1024, 2048, 8192, 1048576, 30000000};
                for (int z : coordinates) {
                    for (int x : coordinates) {
                        int quartX = (x >> 2) << 2;
                        int quartZ = (z >> 2) << 2;
                        int centerX = ((x >> 4) * 2 + 1) * 8;
                        int centerZ = ((z >> 4) * 2 + 1) * 8;
                        output.printf(Locale.ROOT, "%d\t%d\t%d\t%.9g\t%.9g\t%.9g%n", seed, x, z,
                            temperature.get(quartX * 0.25, 0, quartZ * 0.25), vegetation.get(quartX * 0.25, 0, quartZ * 0.25),
                            end.sampleValue(SamplerContext.EMPTY_UNCACHED, centerX, 0, centerZ));
                    }
                }
            }
        }
    }
}
