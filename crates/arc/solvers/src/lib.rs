#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC solver-domain crate owning the typed DSL and pure interpreter seed.
//!
//! `arc-solvers` sits above shared ARC contracts and below later verifier,
//! lane, and arbiter policy. It must not absorb benchmark scoring truth, app
//! UX, or reusable Psionic substrate.

pub mod dsl;
pub mod interpreter;

pub use dsl::{
    ARC_SOLVER_BOUNDARY_SUMMARY, ArcDslTier, ArcGridBinding, ArcGridExpr, ArcObjectSelector,
    ArcObjectTransform, ArcProgram, ArcProgramMetadata, ArcSymbol, ArcSymbolError,
};
pub use interpreter::{ArcInterpreter, ArcInterpreterError};

/// Stable role summary for downstream ARC crates.
pub const CRATE_ROLE: &str =
    "ARC solver DSL, pure interpreter, and later lane/verifier/arbiter substrate";
