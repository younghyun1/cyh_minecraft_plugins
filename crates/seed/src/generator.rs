//! Initialize only the biome router, without a server, chunks or world storage.

use pumpkin_data::{
    chunk::{NETHER_BIOME_SOURCE, OVERWORLD_BIOME_SOURCE},
    noise_router::{
        BaseNoiseFunctionComponent, END_BASE_NOISE_ROUTER, LARGE_BIOMES_BASE_NOISE_ROUTER,
        OVERWORLD_BASE_NOISE_ROUTER,
    },
};
use pumpkin_world::generation::{
    GlobalRandomConfig,
    noise::router::{
        density_volume::DensityVolume,
        multi_noise_sampler::MultiNoiseSampler,
        proto_noise_router::{ProtoMultiNoiseRouter, ProtoNoiseRouters},
    },
};

use crate::{
    Cell, Dimension, Error, PredictionRequest, PredictionResponse, end_sampler::EndSampler,
};

/// Exact source and generated-data revision used by this executable.
pub const GENERATOR_REVISION: &str =
    "pumpkin-4426d1113a211e6018a2db416e33b6b8a7802614-java26.3-surface1";

pub(crate) enum Sampler<'a> {
    MultiNoise(MultiNoiseSampler<'a>),
    End(EndSampler),
}

impl<'a> Sampler<'a> {
    /// Keep dimension-specific state bounded and owned by the persistent worker.
    pub(crate) fn new(router: &'a ProtoMultiNoiseRouter, dimension: Dimension, seed: i64) -> Self {
        match dimension {
            Dimension::End => Self::End(EndSampler::new(seed)),
            Dimension::Overworld | Dimension::Nether => {
                Self::MultiNoise(MultiNoiseSampler::generate(router))
            }
        }
    }
}

/// Construct only the selected dimension's seed-specific biome noise router.
pub(crate) fn router(
    seed: i64,
    dimension: Dimension,
    large_biomes: bool,
    surface: bool,
) -> ProtoMultiNoiseRouter {
    let base = match (dimension, large_biomes) {
        (Dimension::Overworld, true) => &LARGE_BIOMES_BASE_NOISE_ROUTER.multi_noise,
        (Dimension::Overworld, false) => &OVERWORLD_BASE_NOISE_ROUTER.multi_noise,
        (Dimension::Nether, _) => &crate::nether_router::NETHER_MULTI_NOISE,
        (Dimension::End, _) => &END_BASE_NOISE_ROUTER.multi_noise,
    };
    let random = GlobalRandomConfig::new(seed as u64, dimension != Dimension::Overworld);
    let mut components =
        ProtoNoiseRouters::generate_proto_stack(base.full_component_stack, &random).into_vec();
    let depth = if surface {
        // Cubiomes' SAMPLE_NO_DEPTH projects climate onto depth zero. A separate
        // constant output skips the unused terrain-depth spline without changing
        // any shared temperature, humidity, continentalness, erosion or ridges.
        components.extend(
            ProtoNoiseRouters::generate_proto_stack(
                &[BaseNoiseFunctionComponent::Constant { value: 0.0 }],
                &random,
            )
            .into_vec(),
        );
        components.len() - 1
    } else {
        base.depth
    };
    ProtoMultiNoiseRouter {
        full_component_stack: components.into_boxed_slice(),
        temperature: base.temperature,
        vegetation: base.vegetation,
        continents: base.continents,
        erosion: base.erosion,
        depth,
        ridges: base.ridges,
    }
}

/// Predict a bounded surface or fixed-height grid, including the large-biomes preset.
pub fn predict(request: &PredictionRequest) -> Result<PredictionResponse, Error> {
    request.validate()?;
    let router = router(
        request.seed,
        request.dimension,
        request.large_biomes,
        request.y.is_none(),
    );
    let mut sampler = Sampler::new(&router, request.dimension, request.seed);
    sample(request, &mut sampler, || false)
}

/// Reuse seeded state on its owning CPU thread; cancellation is checked between rows.
pub(crate) fn sample(
    request: &PredictionRequest,
    sampler: &mut Sampler<'_>,
    cancelled: impl Fn() -> bool,
) -> Result<PredictionResponse, Error> {
    request.validate()?;
    if cancelled() {
        return Err(Error::Cancelled);
    }
    let y = request.y.unwrap_or(0);

    // Aligned quart grids can use Pumpkin's volume path. Other grids must round
    // each coordinate separately, since a block step need not be a quart step.
    if let Sampler::MultiNoise(sampler) = sampler
        && request.step.is_multiple_of(4)
        && request.min_x % 4 == 0
        && request.min_z % 4 == 0
        && let Ok(step) = i32::try_from(request.step)
    {
        sampler.fill_volume(DensityVolume::new(
            request.width as usize,
            1,
            request.height as usize,
            request.min_x,
            (y >> 2) << 2,
            request.min_z,
            step,
            4,
            step,
        ));
    }

    let mut cells = Vec::with_capacity((request.width * request.height) as usize);
    for row in 0..request.height {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        for column in 0..request.width {
            let x = i64::from(request.min_x) + i64::from(column) * i64::from(request.step);
            let z = i64::from(request.min_z) + i64::from(row) * i64::from(request.step);
            let x = i32::try_from(x).map_err(|_| Error::Bounds)?;
            let z = i32::try_from(z).map_err(|_| Error::Bounds)?;
            let biome = match sampler {
                Sampler::End(sampler) => sampler.biome(x, z),
                Sampler::MultiNoise(sampler) => {
                    let point = sampler.sample(x >> 2, y >> 2, z >> 2);
                    let source = match request.dimension {
                        Dimension::Nether => &NETHER_BIOME_SOURCE,
                        Dimension::Overworld | Dimension::End => &OVERWORLD_BIOME_SOURCE,
                    };
                    // Clearing the previous leaf keeps equal-distance ties stable
                    // across overlapping viewports and request order.
                    if request.y.is_none() {
                        crate::surface_biome::get(&point.convert_to_list())?
                    } else {
                        source.get(&point.convert_to_list(), &mut None)
                    }
                }
            };
            cells.push(Cell {
                x,
                z,
                biome: format!("minecraft:{}", biome.registry_id),
            });
        }
    }
    Ok(PredictionResponse {
        generator_revision: GENERATOR_REVISION.to_owned(),
        large_biomes: request.large_biomes,
        cells,
    })
}

#[cfg(test)]
#[path = "generator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "surface_tests.rs"]
mod surface_tests;
