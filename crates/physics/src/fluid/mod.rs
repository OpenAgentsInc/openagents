//! Particle liquids (issue #11660).
//!
//! - [`pbf`]: a deterministic `f64` position-based fluid, the CPU reference
//!   that the GPU path in `verse-pbr` is checked against.
//!
//! Reimplemented from the papers, not from any code: Macklin and Müller,
//! "Position Based Fluids" (SIGGRAPH 2013) for the density solve, the
//! tensile term and XSPH viscosity; Akinci et al., "Versatile Rigid-Fluid
//! Coupling for Incompressible SPH" (SIGGRAPH 2012) for boundary particles
//! and two-way coupling; Akinci, Akinci and Teschner, "Versatile Surface
//! Tension and Adhesion for SPH Fluids" (SIGGRAPH Asia 2013) for cohesion,
//! curvature and adhesion. Genesis
//! (<https://github.com/Genesis-Embodied-AI/Genesis>, genesis-world
//! `df30b61a`, Apache-2.0)
//! ships a PBF liquid and an SPH surface tension; it is credited here as
//! the prompt for this module, and none of its code is translated.

pub mod pbf;

pub use pbf::{Fluid, Settings, Stats, Tank};
