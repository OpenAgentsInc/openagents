//! The renderer's half of Verse's particle effects: the sprite sheets and
//! the sprite quads the physical pipeline draws. Effects, emitters, and the
//! simulation live in `verse::fx`; read `docs/verse/particles.md`.

pub mod fire;
pub mod sheet;
pub mod sprite;

pub use sprite::{
    Facing, Ribbon, RibbonPoint, Sprite, SpriteVertex, budget, vertices, vertices_with_ribbons,
};
