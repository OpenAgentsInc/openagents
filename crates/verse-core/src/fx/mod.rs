//! Textured particle effects: flipbook sprites driven by effect
//! definitions, drawn in the physical renderer's scene pass.
//!
//! The pipeline (`docs/verse/particles.md`):
//!
//! - **Sprites** are sheets rendered by Blender scripts under
//!   `scripts/blender/fx/` into `assets/verse/fx/` ([`sheet`]).
//! - **Effects** are TOML files under `assets/verse/fx/effects/` naming
//!   their emitters: sheet, frames, rates, lifetimes, motion, curves over
//!   life, and blend ([`def`], [`library`]).
//! - **Running effects** live in a [`Particles`]: a zone starts one at a
//!   point, moves it if it trails something, stops it, steps it, and
//!   appends its [`Sprite`]s to the frame's [`crate::mesh::Mesh::sprites`].
//! - **The renderer** keeps a frame's sprites within the tier's [`budget`]
//!   and draws them back to front through one premultiplied-alpha pipeline
//!   that samples every sheet from one texture array ([`vertices`]).
//! - **Preview**: `cargo run --release -p verse --example fx_preview --
//!   EFFECT OUT_DIR` renders an effect over time into a contact sheet.

pub mod def;
pub mod library;
pub mod motes;
pub use verse_pbr::fx::{sheet, sprite};
pub mod system;

pub use def::Effect;
pub use library::Library;
pub use sprite::{
    Facing, Ribbon, RibbonPoint, Sprite, SpriteVertex, budget, vertices, vertices_with_ribbons,
};
pub use system::{Handle, Particles, Spawn, Style};

use std::sync::atomic::{AtomicU8, Ordering};
use verse_engine::quality::{Platform, Tier};

// Zero means that no renderer has published its adapter-selected tier yet.
static RENDER_TIER: AtomicU8 = AtomicU8::new(0);

/// Publishes the active renderer's selected tier for effect density.
pub fn set_render_tier(tier: Tier) {
    RENDER_TIER.store(tier_code(tier), Ordering::Release);
}

/// The adapter-selected tier, or the platform and operator's choice before
/// a renderer opens.
#[must_use]
pub fn render_tier() -> Tier {
    let published = RENDER_TIER.load(Ordering::Acquire);
    if (1..=3).contains(&published) {
        return selected_tier(published, None, Platform::current());
    }
    let requested = std::env::var("VERSE_QUALITY")
        .ok()
        .and_then(|v| Tier::parse(&v));
    selected_tier(published, requested, Platform::current())
}

fn tier_code(tier: Tier) -> u8 {
    match tier {
        Tier::Low => 1,
        Tier::Medium => 2,
        Tier::High => 3,
    }
}

fn selected_tier(published: u8, requested: Option<Tier>, platform: Platform) -> Tier {
    match published {
        1 => Tier::Low,
        2 => Tier::Medium,
        3 => Tier::High,
        _ => requested.unwrap_or(match platform {
            Platform::Desktop => Tier::High,
            Platform::Mobile | Platform::Web => Tier::Medium,
        }),
    }
}

#[cfg(test)]
mod render_tier_tests {
    use super::*;
    use verse_engine::quality::Probe;

    #[test]
    fn an_adapter_downgrade_overrides_the_requested_particle_density() {
        let requested = Some(Tier::High);
        let probe = Probe {
            platform: Platform::Desktop,
            gles: true,
            float_target: false,
            compute: false,
            samples: 1,
        };
        let effective = probe.select(requested);
        assert_eq!(
            selected_tier(tier_code(effective), requested, Platform::Desktop),
            Tier::Low
        );
        assert_eq!(
            selected_tier(tier_code(Tier::Medium), requested, Platform::Desktop),
            Tier::Medium
        );
        assert_eq!(
            selected_tier(0, Some(Tier::Low), Platform::Desktop),
            Tier::Low
        );
        assert_eq!(selected_tier(0, None, Platform::Web), Tier::Medium);
        assert_eq!(selected_tier(0, None, Platform::Desktop), Tier::High);
    }
}

#[cfg(test)]
mod tests;

/// Surface reached by a shared impact event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactSurface {
    Ground,
    Masonry,
}

/// A gameplay strike published once for its coordinated visual layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactEvent {
    pub at: glam::Vec3,
    pub normal: glam::Vec3,
    pub surface: ImpactSurface,
    pub intensity: f32,
    pub seed: u32,
}
