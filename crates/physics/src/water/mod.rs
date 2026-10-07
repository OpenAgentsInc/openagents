//! Water: bodies of water, their gameplay surface, currents, and the forces
//! they put on rigid bodies.
//!
//! - [`WaterBody`]: a pond, river, waterfall, ocean, pool, puddle, or marsh,
//!   as an [`Outline`] (a polygon, a river [`Course`] with widths, or
//!   everywhere), a [`Level`] (constant, or a profile along the course), a
//!   density, up to eight Gerstner terms ([`WaveSet`]) with an optional
//!   spectrum seed, and an optional [`FlowGrid`].
//! - [`Surface`]: the gameplay surface, level plus Gerstner waves on a
//!   clock folded on a whole-tick period, sampled in `f64` for height,
//!   normal, surface velocity, current, and density.
//! - [`Water`] and [`WaterSet`]: point queries over a zone's water through
//!   a uniform grid.
//! - [`submerged`]: submerged volume, center of buoyancy, and wetted area
//!   of a sphere, capsule, or cuboid.
//! - [`apply`]: buoyancy, drag relative to the current, and angular
//!   damping as forces for the next step, each a named ledger [`Term`].
//! - [`FlowGrid::river`]: divergence-free potential flow along a course
//!   around obstacles, from the stream function.
//!
//! Nothing here renders or reads files; the look of water lives elsewhere
//! and never feeds back into these forces. See `docs/verse/water.md`.

mod apply;
mod body;
mod flow;
mod set;
mod submerge;
mod surface;

pub use apply::{Push, Settings, Term, apply, apply_where, apply_with, record};
pub use body::{
    Course, FRESH, Kind, Level, Outline, SALT, Sample, Station, Surface, WaterBody, WaterId,
};
pub use flow::{FlowGrid, Obstacle};
pub use set::{Water, WaterSet};
pub use submerge::{Plane, Submersion, area, below, submerged, volume};
pub use surface::{Displacement, G, INVERSIONS, MAX_WAVES, Phases, Wave, WaveSet};

#[cfg(test)]
mod tests;
