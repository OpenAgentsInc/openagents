#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! Deterministic ARC engine primitives for local package execution and replay.
//!
//! `arc-engine` owns ARC-specific level, sprite, camera, and action-transition
//! behavior for local runs. It must stay below client, benchmark, and solver
//! policy while exposing one deterministic execution owner for later crates.

pub mod package;
pub mod runtime;

pub use package::{
    ARC_ENGINE_SCHEMA_VERSION, ArcBlockingMode, ArcCamera, ArcGamePackage, ArcInteractionMode,
    ArcInteractionTrigger, ArcLevelDefinition, ArcLevelEffect, ArcLevelTarget, ArcPoint, ArcSprite,
    ArcSpriteInstance, ArcStateGate, load_game_package,
};
pub use runtime::{
    ArcEngine, ArcEngineError, ArcEngineGameState, ArcEngineState, ArcEngineStepOutcome,
};

/// Human-readable ownership summary for this crate.
pub const CRATE_ROLE: &str =
    "Deterministic ARC game execution, sprite/camera logic, and replay-safe action stepping";

/// Stable boundary summary for downstream ARC crates.
pub const ENGINE_BOUNDARY_SUMMARY: &str = "arc-engine owns package loading, deterministic local state transitions, frame rendering, and replay-safe local execution";
