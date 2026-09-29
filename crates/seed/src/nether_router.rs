//! Adapt the pinned Nether router to Pumpkin's existing legacy noise initializer.

use pumpkin_data::{
    noise_parameter::DoublePerlinNoiseParameters,
    noise_router::{BaseMultiNoiseRouter, BaseNoiseFunctionComponent, ShiftedNoiseData},
};

// At this revision ordinary Noise nodes use Xoroshiro initialization even for
// Nether temperature/vegetation. Zero-shift nodes are numerically equivalent,
// but select Pumpkin's legacy seed/seed+1 initialization matching Paper 26.3.
pub(crate) const NETHER_MULTI_NOISE: BaseMultiNoiseRouter = BaseMultiNoiseRouter {
    full_component_stack: &[
        BaseNoiseFunctionComponent::Constant { value: 0.0 },
        BaseNoiseFunctionComponent::ShiftedNoise {
            shift_x_index: 0,
            shift_y_index: 0,
            shift_z_index: 0,
            data: &ShiftedNoiseData {
                noise_id: DoublePerlinNoiseParameters::NETHER_TEMPERATURE,
                xz_scale: 0.25,
                y_scale: 0.0,
            },
        },
        BaseNoiseFunctionComponent::ShiftedNoise {
            shift_x_index: 0,
            shift_y_index: 0,
            shift_z_index: 0,
            data: &ShiftedNoiseData {
                noise_id: DoublePerlinNoiseParameters::NETHER_VEGETATION,
                xz_scale: 0.25,
                y_scale: 0.0,
            },
        },
    ],
    temperature: 1,
    vegetation: 2,
    continents: 0,
    erosion: 0,
    depth: 0,
    ridges: 0,
};
