//! Water: bodies of water, their gameplay surface, currents, and the forces
//! they put on rigid bodies.
//!
//! - [`WaterBody`]: a pond, river, waterfall, ocean, pool, puddle, or marsh,
//!   as an [`Outline`] (a polygon, a river [`Course`] with widths, or
//!   everywhere), a [`Level`] (constant, or a profile along the course), a
//!   density, up to eight Gerstner terms ([`WaveSet`]) with an optional
//!   spectrum seed, and an optional [`FlowGrid`].
//! - [`Surface`]: the gameplay surface, level plus Gerstner waves on a
//!   clock folded on a whole-tick period, plus an ocean's spectral band,
//!   sampled in `f64` for height, normal, surface velocity, current, and
//!   density.
//! - [`Spectrum`] and [`Synth`]: a seeded JONSWAP spectrum with
//!   directional spreading, synthesized by FFT into looping cascades; the
//!   lowest is the gameplay band ([`spectrum::field`]).
//! - [`Water`] and [`WaterSet`]: point queries over a zone's water through
//!   a uniform grid.
//! - [`submerged`]: submerged volume, center of buoyancy, and wetted area
//!   of a sphere, capsule, or cuboid.
//! - [`apply`]: buoyancy, drag relative to the current, and angular
//!   damping as forces for the next step, each a named ledger [`Term`].
//! - [`FlowGrid::river`]: divergence-free potential flow along a course
//!   around obstacles, from the stream function.
//! - [`medium`]: a character's medium (ground, wading, swimming, diving)
//!   and a swimmer's float line.
//! - [`event`]: host events on water (a level change, ice, a dam), each
//!   with a start tick, and the world tick clients derive from their
//!   clocks ([`tick_at`]).
//! - [`weather`]: each zone's deterministic weather schedule from its
//!   climate, seed, and the world tick, spell weather over it, the ground's
//!   wetness and puddles, and rain's bounded rise of ponds and streams.
//!
//! Nothing here renders or reads files; the look of water lives elsewhere
//! and never feeds back into these forces. See `docs/verse/water.md`.

mod apply;
mod body;
pub mod event;
pub mod fft;
mod flow;
pub mod medium;
mod set;
pub mod spectrum;
mod submerge;
mod surface;
pub mod weather;

pub use apply::{Push, Settings, Term, apply, apply_scaled, apply_where, apply_with, record};
pub use body::{
    Course, FRESH, Kind, Level, Outline, SALT, Sample, Station, Surface, WaterBody, WaterId,
};
pub use event::{Effect, Event, Evented, Eventful, Events, TICK_HZ, tick_at};
pub use flow::{FlowGrid, Obstacle};
pub use medium::{Medium, Stroke};
pub use set::{Water, WaterSet};
pub use spectrum::{Cascade, Field, Spectrum, Synth, Tile};
pub use submerge::{Plane, Submersion, area, below, submerged, volume};
pub use surface::{Displacement, G, INVERSIONS, MAX_WAVES, Phases, Wave, WaveSet};
pub use weather::{
    Climate, Ground, Overlay, OverlayKind, Schedule, State as WeatherState, Weather,
};

#[cfg(test)]
mod spectrum_tests;
#[cfg(test)]
mod tests;
