//! Draconia for the film's zone-renderer scenes: the face work's skinned
//! build (`docs/verse/draconia/parity.md`, "Current asset for scenes")
//! added to the Everglade pack in memory as the form `film/draconia`, with
//! her Universal clips and a baked `rise` whose hair lifts in an upward
//! gust, as `crates/verse/examples/draconia_form.rs` does.

use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::imported::{characters, flat};
use verse::zones::everglade_pack::{self, Character, ZonePack};
use verse_engine::assets::Pack;
use verse_engine::hair_chains::{self, Spring, Wind};

/// The engine clip ID of the baked rise.
const RISE: u16 = 901;

/// A shot file's path to her glb, with `~/` expanded.
pub fn path(spec: &str) -> PathBuf {
    match spec.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(spec),
    }
}

/// An upward gust, as the guardian's flash throws her hair up: calm, then
/// a rush from below from `start` that peaks 0.25 s later and fades by
/// 1.4 s.
fn rising_gust(start: f32) -> impl Fn(f32) -> Wind {
    move |t| {
        let u = t - start;
        let k = if u < 0.0 {
            0.0
        } else if u < 0.25 {
            u / 0.25
        } else {
            (1.0 - (u - 0.25) / 1.15).max(0.0)
        };
        Wind {
            velocity: Vec3::new(0.0, 16.0, -3.0) * k,
            gust: 0.3,
            gust_rate: 2.0,
            turbulence: 1.5 * k + 0.2,
            outward: 6.0 * k,
            center: Vec3::ZERO,
        }
    }
}

/// Adds Draconia from `glb` to `zone` and returns her form, with a `rise`
/// clip whose gust starts `gust_at` seconds in.
pub fn form(zone: &mut ZonePack, glb: &Path, gust_at: f32) -> Result<Character, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("verse-film-draconia-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut engine = Pack {
        inventory: None,
        version: 1,
        source_revision: "film-draconia".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    flat::white_texture(&mut engine, &dir)?;
    let mut draconia = characters::film_character(
        &mut engine,
        &dir,
        glb,
        &root.join("assets/verse/characters/quaternius"),
        characters::ImportOptions {
            max_texture_edge: 1024,
        },
    )?;
    let spheres = hair_chains::humanoid_spheres(&draconia);
    let rise = hair_chains::bake(
        &draconia,
        0,
        RISE,
        3.6,
        30.0,
        2.0,
        Spring {
            stiffness: 25.0,
            ..Default::default()
        },
        &spheres,
        rising_gust(gust_at),
    )?;
    draconia.clips.push(rise);
    let mut clips: Vec<(&str, u16)> = everglade_pack::compile::PLAYER_CLIPS
        .iter()
        .copied()
        .filter(|(_, id)| draconia.clips.iter().any(|c| c.id == *id))
        .collect();
    clips.push(("rise", RISE));
    let form = everglade_pack::film::add_character(
        zone,
        &engine,
        &dir,
        &draconia,
        "film/draconia",
        &clips,
        1024,
    )?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(form)
}
