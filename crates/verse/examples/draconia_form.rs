//! Draconia as a zone-pack form, for scenes drawn through the zone renderer
//! (the peacock scene): loads the Everglade pack, adds Draconia to it in
//! memory with her Universal clips and a baked "rise" clip whose hair lifts
//! in an upward gust, and poses her through `Beast`, the way a scene does.
//!
//! Usage: draconia_form [MODEL.glb]
//!
//! MODEL defaults to `~/work/film-out/draconia/build/draconia-long.neutral.glb`
//! (the long, loose style; `draconia.neutral.glb` is the updo). It prints
//! the form's size and where the hair's tip is at rest and in the gust,
//! which a test of the whole path wants to see rise. See
//! docs/verse/draconia/parity.md ("Current asset for scenes").
use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::imported::{characters, flat};
use verse::zones::everglade::player::Beast;
use verse::zones::everglade_pack;
use verse_engine::assets::Pack;
use verse_engine::hair_chains::{self, Spring, Wind};

/// The engine clip ID of the baked rise.
const RISE: u16 = 901;

/// An upward gust, as a guardian's flash throws her hair up: calm, then a
/// rush from below from `start` that peaks at 0.25 s and fades by 1.4 s.
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
            center: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

fn main() -> Result<(), String> {
    let model_path = std::env::args().nth(1).map_or_else(
        || {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("work/film-out/draconia/build/draconia-long.neutral.glb")
        },
        PathBuf::from,
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let zone = root.join(everglade_pack::PACK_DIRECTORY).join(format!(
        "{}.{}",
        everglade_pack::PACK_SHA256,
        everglade_pack::PACK_EXTENSION
    ));
    let mut zone = everglade_pack::ZonePack::load_local(&zone)?;
    let dir = std::env::temp_dir().join(format!("verse-draconia-form-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut engine = Pack {
        inventory: None,
        version: 1,
        source_revision: "draconia-form".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    flat::white_texture(&mut engine, &dir)?;
    // 1024-pixel maps: the zone renderer draws base colour only.
    let mut draconia = characters::film_character(
        &mut engine,
        &dir,
        &model_path,
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
        2.0,
        30.0,
        2.0,
        Spring {
            stiffness: 25.0,
            ..Default::default()
        },
        &spheres,
        rising_gust(0.2),
    )?;
    draconia.clips.push(rise);
    let mut clips: Vec<(&str, u16)> = everglade_pack::compile::PLAYER_CLIPS
        .iter()
        .copied()
        .filter(|(_, id)| draconia.clips.iter().any(|c| c.id == *id))
        .collect();
    clips.push(("rise", RISE));
    let form = everglade_pack::film::add_character(
        &mut zone,
        &engine,
        &dir,
        &draconia,
        "film/draconia",
        &clips,
        1024,
    )?;
    println!(
        "form {}: {} joints, {} triangles, clips {:?}",
        form.name,
        form.joints.len(),
        form.triangles(),
        form.clips
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
    );
    let mut beast = Beast::new(&zone, &form)?;
    let tip = |beast: &Beast| {
        beast
            .vertices()
            .iter()
            .map(|v| Vec3::from(v.pos))
            .fold(f32::MAX, |m, p| m.min(p.y))
    };
    let _ = tip;
    let high = |beast: &Beast| {
        // The hair's lowest point behind her: the long fall's tips.
        beast
            .vertices()
            .iter()
            .map(|v| Vec3::from(v.pos))
            .filter(|p| p.z < -0.05)
            .fold(f32::MAX, |m, p| m.min(p.y))
    };
    for t in [0.0f32, 0.3, 0.5, 0.8, 1.2, 1.8] {
        if !beast.pose_clip(Vec3::ZERO, 0.0, 1.0, "rise", t, false, 1.0 / 30.0) {
            return Err("no rise clip".into());
        }
        println!("t {t:.1}: lowest hair behind her at {:.3} m", high(&beast));
    }
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
