import java.io.PrintWriter;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;

/** Offline climate projection oracle; no server, world, terrain or private seed is loaded. */
public final class PaperSurfaceFixture {
    private PaperSurfaceFixture() {}

    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("Expected fixture output path");
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var registries = VanillaRegistries.createWorldLookup();
        var settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS);
        var noise = registries.lookupOrThrow(Registries.NOISE);
        var biomes = MultiNoiseBiomeSourceParameterList.knownPresets()
            .get(MultiNoiseBiomeSourceParameterList.Preset.OVERWORLD);
        var caves = Set.of("minecraft:lush_caves", "minecraft:dripstone_caves",
            "minecraft:deep_dark", "minecraft:sulfur_caves");
        var surfaceBiomes = new Climate.ParameterList<>(biomes.values().stream()
            .filter(pair -> !caves.contains(pair.getSecond().identifier().toString())).toList());
        try (var output = new PrintWriter(Path.of(args[0]).toFile())) {
            output.println("seed\tlarge\tx\ty\tz\ttemperature\thumidity\tcontinentalness\terosion\tweirdness\tslice_depth\tslice_biome\tsurface_biome\tunfiltered_surface_biome");
            for (long seed : new long[] {1, -1, Long.MIN_VALUE, Long.MAX_VALUE}) {
                for (boolean large : new boolean[] {false, true}) {
                    var configuration = settings.getOrThrow(large
                        ? NoiseGeneratorSettings.LARGE_BIOMES : NoiseGeneratorSettings.OVERWORLD).value();
                    var sampler = RandomState.create(noise, seed, configuration)
                        .createClimateSampler(SamplerContext.EMPTY_UNCACHED);
                    var pairs = new HashSet<String>();
                    for (int row = 0; row < 48; row++) {
                        for (int column = 0; column < 48; column++) {
                            int x = -12291 + column * 512;
                            int z = -12289 + row * 512;
                            var point = sampler.sample(x >> 2, 0, z >> 2);
                            var surface = new Climate.TargetPoint(point.temperature(), point.humidity(),
                                point.continentalness(), point.erosion(), 0, point.weirdness());
                            var unfilteredSurfaceBiome = biomes.findValue(surface).identifier();
                            var surfaceBiome = surfaceBiomes.findValue(surface).identifier();
                            for (int y : new int[] {64, -32}) {
                                var slice = sampler.sample(x >> 2, y >> 2, z >> 2);
                                var sliceBiome = biomes.findValue(slice).identifier();
                                // Keep a regular diagnostic grid and every first biome pair. This
                                // includes rare cave transitions without filtering by Rust output.
                                if (pairs.add(sliceBiome + "/" + surfaceBiome + "/" + unfilteredSurfaceBiome)
                                        | (row % 8 == 0 && column % 8 == 0)) {
                                    output.printf("%d\t%b\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%s\t%s\t%s%n",
                                        seed, large, x, y, z, surface.temperature(), surface.humidity(),
                                        surface.continentalness(), surface.erosion(), surface.weirdness(),
                                        slice.depth(), sliceBiome, surfaceBiome, unfilteredSurfaceBiome);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
