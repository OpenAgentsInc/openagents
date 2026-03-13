#![cfg_attr(
    test,
    allow(
        clippy::all,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::pedantic,
        clippy::unwrap_used
    )
)]

pub mod renderer;
pub mod svg;
mod vector;

pub use renderer::{RenderMetrics, Renderer};
pub use svg::{SvgRasterized, SvgRenderer};
