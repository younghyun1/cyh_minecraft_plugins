//! Paper-compatible End island arithmetic over Pumpkin's seeded simplex primitive.

use pumpkin_data::chunk::Biome;
use pumpkin_util::{
    noise::simplex::SimplexNoiseSampler,
    random::{RandomImpl, legacy_rand::LegacyRand},
};

pub(crate) struct EndSampler {
    simplex: SimplexNoiseSampler,
    chunks: Box<[Option<ChunkBiome>; 64]>,
}

#[derive(Clone, Copy)]
struct ChunkBiome {
    x: i32,
    z: i32,
    biome: &'static Biome,
}

impl EndSampler {
    /// End noise uses the legacy generator after the vanilla fixed draw offset.
    pub(crate) fn new(seed: i64) -> Self {
        let mut random = LegacyRand::from_seed(seed as u64);
        random.skip(17292);
        Self {
            simplex: SimplexNoiseSampler::new(&mut random),
            chunks: Box::new([None; 64]),
        }
    }

    /// Java 26.3 evaluates these operations separately in f32. Pumpkin's End
    /// adapter uses fused multiply-add, changing island radii at far coordinates.
    pub(crate) fn erosion(&self, block_x: i32, block_z: i32) -> f32 {
        let x = block_x / 8;
        let z = block_z / 8;
        let mut height = -100.0f32;
        for dx in -12..=12 {
            for dz in -12..=12 {
                let island_x = i64::from(x / 2 + dx);
                let island_z = i64::from(z / 2 + dz);
                if island_x * island_x + island_z * island_z <= 4096
                    || (self.simplex.sample_2d(island_x as f64, island_z as f64) as f32) >= -0.9
                {
                    continue;
                }
                let radius = ((island_x as f32).abs() * 3439.0 + (island_z as f32).abs() * 147.0)
                    % 13.0
                    + 9.0;
                let offset_x = (x % 2 - dx * 2) as f32;
                let offset_z = (z % 2 - dz * 2) as f32;
                let candidate = 100.0 - (offset_x * offset_x + offset_z * offset_z).sqrt() * radius;
                height = height.max(candidate.clamp(-100.0, 80.0));
            }
        }
        (height - 8.0) / 128.0
    }

    /// Biomes are constant within each End chunk and independent of Y.
    pub(crate) fn biome(&mut self, x: i32, z: i32) -> &'static Biome {
        let section_x = x >> 4;
        let section_z = z >> 4;
        // A native tile asks for each chunk 16 times. This fixed direct-mapped
        // row cache avoids repeating its 625-point island search without growth.
        let slot = section_x.rem_euclid(64) as usize;
        if let Some(cached) = self.chunks[slot]
            && (cached.x, cached.z) == (section_x, section_z)
        {
            return cached.biome;
        }
        let biome = self.chunk_biome(section_x, section_z);
        self.chunks[slot] = Some(ChunkBiome {
            x: section_x,
            z: section_z,
            biome,
        });
        biome
    }

    fn chunk_biome(&self, section_x: i32, section_z: i32) -> &'static Biome {
        // Java widens before squaring; the upstream supplier overflows i32 here.
        if i64::from(section_x).pow(2) + i64::from(section_z).pow(2) <= 4096 {
            return &Biome::THE_END;
        }
        let erosion = self.erosion((section_x * 2 + 1) * 8, (section_z * 2 + 1) * 8);
        if erosion > 0.25 {
            &Biome::END_HIGHLANDS
        } else if erosion >= -0.0625 {
            &Biome::END_MIDLANDS
        } else if erosion < -0.21875 {
            &Biome::SMALL_END_ISLANDS
        } else {
            &Biome::END_BARRENS
        }
    }
}
