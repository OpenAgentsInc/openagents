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

pub mod color;
pub mod curve;
pub mod geometry;
pub mod input;
pub mod scene;

pub use color::Hsla;
pub use curve::{CurvePrimitive, LineSegment};
pub use geometry::{Bounds, Edges, Point, Size};
pub use input::{Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey};
pub use scene::{
    GlyphInstance, GpuImageQuad, GpuLine, GpuQuad, GpuTextQuad, MESH_EDGE_FLAG_SELECTED,
    MESH_EDGE_FLAG_SILHOUETTE, MeshEdge, MeshPrimitive, MeshPrimitiveError, MeshTopology,
    MeshVertex, Quad, Scene, SvgQuad, TextRun,
};
