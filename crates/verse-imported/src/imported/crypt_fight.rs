//! The cultist fight in the great crypt, played locally (`verse
//! --crypt-fight`).
//!
//! The rules, abilities, enemy AI, HUD, action bar, and respawn are the
//! ritual chamber's (`verse_world::play::Game`); the stage is
//! `verse_world::great_crypt`: the larger crypt from
//! `scripts/blender/great_crypt.py`, furnished with the crypt lab's props,
//! with acolytes chanting around a summoning circle, a High Priest, waves
//! from the side chapels, and Claude asleep on the dais until the ritual
//! completes.
//!
//! [`Fight::load`] builds the content pack (the original characters, two
//! new cultist looks, and the crypt's models, read from the repository's
//! `assets/verse`), [`Fight::compose`] assembles one frame for the shared
//! renderer, and [`run`] opens the window. The crypt's light is its own:
//! warm candles and braziers that flicker, colored cauldrons, cold moonlight
//! through the far window, and the circle's violet glow, which brightens as
//! the chant nears completion.
use super::{Instance, Renderer, chamber, lighting::Light, lighting::Lighting, overlay};
use crate::render::View;
use crate::ui::{Atlas, UiBatch};
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use verse_engine::assets::Pack;
use verse_engine::lighting::HeightFog;
use verse_world::great_crypt as crypt;
use verse_world::play::Game;

/// The repository's `assets/verse`, which the fight's content comes from.
#[must_use]
pub fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse")
}

/// The loaded fight: its authority, content, and overlay atlas.
pub struct Fight {
    pub game: Game,
    pub pack: Pack,
    pub atlas: Atlas,
    pub heights: BTreeMap<String, f32>,
    pub dir: PathBuf,
}

/// One composed frame: the camera, what to draw, the overlay, and the light.
pub struct Composed {
    pub view: View,
    pub instances: Vec<Instance>,
    pub ui: UiBatch,
    pub lighting: Lighting,
}

/// Builds the fight's content pack in `dir`: the original pack, the
/// Universal characters, the High Priest's and the acolytes' looks, and the
/// great crypt's models and layout.
///
/// # Errors
///
/// Returns a message when a source asset is missing or cannot be imported.
pub fn pack(dir: &Path) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let root = assets();
    let mut pack = super::original::generate(dir)?;
    super::characters::install(
        &mut pack,
        dir,
        &root.join("characters/quaternius"),
        "male-ranger",
    )?;
    // The High Priest in crimson, the acolytes in ash-gray robes.
    for (name, base, tint) in [
        ("cultist-leader", "universal-male-ranger", [0.86, 0.2, 0.13]),
        (
            "cultist-acolyte",
            "universal-female-peasant",
            [0.6, 0.57, 0.66],
        ),
    ] {
        let mut model = pack
            .models
            .get(base)
            .cloned()
            .ok_or_else(|| format!("The pack has no {base}"))?;
        for surface in &mut model.surfaces {
            surface.tint = tint;
        }
        pack.models.insert(name.into(), model);
    }
    verse_content::compiler::great_crypt::install(&mut pack, dir, &root.join("generated"))?;
    pack.source_revision = "verse-great-crypt-v1".into();
    pack.validate()?;
    Ok(pack)
}

impl Fight {
    /// Builds the content in `dir` and starts the fight.
    ///
    /// # Errors
    ///
    /// Returns a message when the content or the fight cannot be built.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let pack = pack(dir)?;
        let atlas = chamber::original_portrait_atlas(dir, &pack)?;
        let heights = pack
            .models
            .iter()
            .map(|(id, m)| (id.clone(), m.height))
            .collect();
        Ok(Self {
            game: crypt::game(0)?,
            pack,
            atlas,
            heights,
            dir: dir.to_path_buf(),
        })
    }

    /// The crypt's static models, which the renderer merges once.
    #[must_use]
    pub fn static_instances(&self) -> Vec<Instance> {
        let mut out = chamber::static_instances(&self.pack, Vec3::ZERO);
        // The circle is drawn each frame, so its glow can follow the chant.
        out.retain(|i| i.model != CIRCLE_MODEL);
        for instance in &mut out {
            instance.emission = Vec3::ONE;
        }
        out
    }

    /// A renderer for the fight at `width` by `height` pixels.
    ///
    /// # Errors
    ///
    /// Returns a message when the GPU or the pack cannot be prepared.
    pub fn renderer(&self, width: u32, height: u32) -> Result<Renderer, String> {
        Renderer::new(
            self.pack.clone(),
            &self.dir,
            width.max(1),
            height.max(1),
            &self.atlas,
            &self.static_instances(),
        )
    }

    /// Composes the frame at `interpolation` between ticks, for a render
    /// target of `dimensions` pixels and an overlay `size` logical points
    /// wide and high. `cursor` is in overlay points; `camera` overrides the
    /// eye and target, as a capture does.
    ///
    /// # Errors
    ///
    /// Returns a message when the frame cannot be assembled.
    pub fn compose(
        &self,
        interpolation: f32,
        dimensions: [u32; 2],
        size: [f32; 2],
        cursor: [f32; 2],
        camera: Option<(Vec3, Vec3)>,
    ) -> Result<Composed, String> {
        let game = &self.game;
        let mut frame = game.interpolated_frame(interpolation)?;
        if let Some((eye, target)) = camera {
            frame.eye = eye;
            frame.target = target;
        }
        let view = View {
            view_proj: frame.view_projection(dimensions[0] as f32 / dimensions[1].max(1) as f32),
            eye: frame.eye,
        };
        let [width, height] = size;
        let mut ui = overlay::cinematic_with_focus(
            &self.atlas,
            &frame,
            &self.heights,
            view.view_proj,
            width,
            height,
            &BTreeMap::new(),
            game.player,
            frame
                .actors
                .iter()
                .find(|a| a.actor.id == game.selected)
                .and_then(|a| a.life),
        );
        overlay::damage_numbers(
            &mut ui,
            &self.atlas,
            game,
            &frame,
            &self.heights,
            view.view_proj,
            width,
            height,
        );
        let hover = overlay::action_at(cursor[0], cursor[1], width, height);
        overlay::action_bar(&mut ui, &self.atlas, game, width, height, hover);
        ritual_bar(&mut ui, &self.atlas, game, width);
        let mut instances = chamber::instances(&self.pack, &frame)?;
        let visuals = verse_world::visuals::Combat::extract(game);
        instances.extend(chamber::spell_instances_from_visuals(&visuals));
        instances.extend(chamber::blocker_instances(&self.pack, game));
        instances.extend(chamber::environment_instances(&self.pack, game));
        instances.extend(chamber::prop_instances(&self.pack, game, interpolation));
        instances.push(circle(game));
        let lighting = chamber::lighting_over(lighting(game), &visuals, game.player);
        Ok(Composed {
            view,
            instances,
            ui,
            lighting,
        })
    }
}

const CIRCLE_MODEL: &str = "crypt/summoning_circle";

/// How brightly the circle glows: rising with the chant, and pulsing once
/// Claude wakes.
fn circle_glow(game: &Game) -> f32 {
    let Some(ritual) = crypt::ritual(game) else {
        return 1.0;
    };
    let pulse = 0.5 + 0.5 * (game.time * 2.3).sin();
    if ritual.awakened.is_some() {
        1.6 + 0.6 * pulse
    } else {
        0.45 + 0.9 * ritual.fraction() + 0.15 * pulse * (ritual.chanters() > 0) as u8 as f32
    }
}

fn circle(game: &Game) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: CIRCLE_MODEL.into(),
        // The importer turned the model half a turn; it is round.
        transform: Mat4::from_translation(crypt::CIRCLE) * chamber::basis(),
        animation: 0.into(),
        time: game.time,
        animation_epoch: None,
        emission: Vec3::splat(circle_glow(game)),
    }
}

/// The crypt's own light at `game`'s time: dim warm ambient, low fog, and
/// its lamps, which flicker like flames.
#[must_use]
pub fn lighting(game: &Game) -> Lighting {
    let warm = Vec3::new(1.0, 0.56, 0.24);
    let fire = Vec3::new(1.0, 0.46, 0.16);
    let mut lights = vec![
        // The circle first: it casts the chanters' shadows.
        (
            crypt::CIRCLE + Vec3::Y * 1.1,
            Vec3::new(0.62, 0.2, 1.0),
            70.0 * circle_glow(game),
            13.0,
            false,
        ),
        (Vec3::new(-3.3, 1.45, -11.9), fire, 70.0, 12.0, true),
        (Vec3::new(3.3, 1.45, -11.9), fire, 70.0, 12.0, true),
        (
            Vec3::new(0.0, 6.2, -19.6),
            Vec3::new(0.42, 0.52, 0.9),
            55.0,
            20.0,
            false,
        ),
        (Vec3::new(-2.95, 2.45, -15.05), warm, 26.0, 7.0, true),
        (Vec3::new(2.95, 2.45, -15.05), warm, 26.0, 7.0, true),
        (Vec3::new(0.0, 2.45, -20.9), warm, 30.0, 8.0, true),
        (
            Vec3::new(-7.6, 1.4, -18.2),
            Vec3::new(0.25, 1.0, 0.35),
            34.0,
            8.0,
            false,
        ),
        (
            Vec3::new(7.6, 1.4, -18.2),
            Vec3::new(1.0, 0.25, 0.12),
            34.0,
            8.0,
            false,
        ),
        (
            Vec3::new(7.7, 1.4, -11.4),
            Vec3::new(1.0, 0.62, 0.2),
            30.0,
            8.0,
            false,
        ),
        (Vec3::new(-3.6, 3.2, 13.1), warm, 30.0, 9.0, true),
        (Vec3::new(3.6, 3.2, 13.1), warm, 30.0, 9.0, true),
        (Vec3::new(0.9, 0.7, -5.4), warm, 14.0, 6.0, true),
        (Vec3::new(-1.2, 0.7, 3.4), warm, 14.0, 6.0, true),
        (Vec3::new(-8.9, 1.9, 3.1), warm, 22.0, 7.0, true),
        (Vec3::new(8.8, 1.9, 3.4), warm, 22.0, 7.0, true),
        (Vec3::new(-8.2, 0.7, 13.6), warm, 12.0, 6.0, true),
        (Vec3::new(8.4, 0.7, 13.2), warm, 12.0, 6.0, true),
    ];
    for side in [-1.0, 1.0] {
        for z in crypt::CHAPELS {
            lights.push((Vec3::new(side * 12.1, 1.8, z), warm, 26.0, 7.0, true));
        }
    }
    let t = game.time;
    let mut lighting = Lighting {
        ambient: Vec3::new(0.009, 0.008, 0.011),
        exposure: 1.0,
        fog: Vec3::new(0.011, 0.008, 0.012),
        density: 0.01,
        time: t,
        lights: Vec::new(),
        shadowed: 4,
        height_fog: Some(HeightFog {
            density: 0.06,
            base: 0.0,
            falloff: 0.5,
            start: 2.5,
            max_opacity: 0.6,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }),
    };
    for (i, (position, color, intensity, range, flame)) in lights.into_iter().enumerate() {
        // Flames gutter: two slow waves and a fast one, out of phase.
        let flicker = if flame {
            let k = i as f32 * 1.7;
            1.0 + 0.1 * (t * 7.3 + k).sin()
                + 0.07 * (t * 13.1 + k * 2.3).sin()
                + 0.05 * (t * 23.7 + k * 0.6).sin()
        } else {
            1.0
        };
        lighting.lights.push(Light {
            position,
            color,
            intensity: intensity * flicker * if i == 0 { 1.0 } else { 0.62 },
            range,
        });
    }
    lighting
}

/// The ritual's bar under the target frames: the chant's progress, how
/// many acolytes chant, and, once Claude wakes, how the ritual ended.
pub fn ritual_bar(ui: &mut UiBatch, atlas: &Atlas, game: &Game, width: f32) {
    let Some(ritual) = crypt::ritual(game) else {
        return;
    };
    let Some(font) = atlas.font("small").layout_at_scale(1.0) else {
        return;
    };
    let (w, h) = (300.0, 10.0);
    let x = width * 0.5 - w * 0.5;
    let y = 40.0;
    let fraction = ritual.fraction();
    let label = match ritual.awakened {
        Some(_) if ritual.empowered => {
            "The ritual is complete. Claude is awake and empowered".into()
        }
        Some(_) => "The ritual is broken. Claude is awake".into(),
        None => format!(
            "Ritual {:.0}%  ·  {} of {} acolytes chanting",
            fraction * 100.0,
            ritual.chanters(),
            crypt::CHANTERS.len()
        ),
    };
    ui.rect(
        atlas,
        x - 2.0,
        y - 2.0,
        w + 4.0,
        h + 4.0,
        [0.05, 0.03, 0.06, 0.85],
    );
    ui.rect(atlas, x, y, w, h, [0.16, 0.08, 0.2, 0.9]);
    let fill = if ritual.awakened.is_some() {
        1.0
    } else {
        fraction
    };
    ui.rect(atlas, x, y, w * fill, h, [0.62, 0.22, 0.95, 1.0]);
    let tx = width * 0.5 - font.measure(&label) * 0.5;
    overlay::outlined(ui, &font, tx, y + h + 14.0, &label, [0.86, 0.72, 1.0, 1.0]);
}

#[cfg(feature = "imported-desktop")]
mod window;
#[cfg(feature = "imported-desktop")]
pub use window::run;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crypt_lights_itself_within_budget() {
        let game = crypt::game(0).unwrap();
        let lighting = lighting(&game);
        assert!(lighting.lights.len() + 8 <= super::super::lighting::MAX_LIGHTS);
        let later = {
            let mut g = crypt::game(0).unwrap();
            g.time += 0.37;
            super::lighting(&g)
        };
        // Flames flicker; the moonlight holds.
        assert_ne!(lighting.lights[1].intensity, later.lights[1].intensity);
        assert_eq!(lighting.lights[3].intensity, later.lights[3].intensity);
    }

    #[test]
    fn the_circle_brightens_as_the_chant_nears_completion() {
        let mut game = crypt::game(0).unwrap();
        let early = circle_glow(&game);
        game.encounter
            .as_mut()
            .unwrap()
            .ritual
            .as_mut()
            .unwrap()
            .progress = crypt::RITUAL_SECONDS * 0.9;
        assert!(circle_glow(&game) > early + 0.5);
    }
}
