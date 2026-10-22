//! The Haunted House's prepared library (`docs/verse/games/the-haunted-house.md`,
//! "The prepared library").
//!
//! Like a parent who bought every decoration before the party, every likely
//! request already has a finished asset in the house's style, so a wish
//! needs only a placement. This crate holds:
//!
//! - [`PALETTE`]: the colour tokens documents name (`paper`, `pumpkin`,
//!   `licorice`, ...).
//! - [`catalog`]: every asset (card, mesh, or effect) with its aliases,
//!   footprint and surface, states, linked sounds, default trigger,
//!   intensity, and whether it is kid-safe; every sound with its source;
//!   the surfaces (a shader pattern and two palette tokens); the lighting
//!   presets; the arrival cues; and the skies. The data is
//!   `catalog.json` beside this crate, so an agent can read it.
//! - [`meshes`]: the low-poly mesh builders for the `mesh` assets, made in
//!   code like The Ice Cream Man's (`toon-mesh`).
//!
//! `haunt-rules` checks a house against this catalog, and `haunt-web` draws
//! from it. Nothing here does I/O, so it builds for `wasm32`.

pub mod meshes;

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The palette, as `(token, 0xRRGGBB, use)`. Shadows fall to `plum` or
/// `night_teal`, never neutral grey; outlines are `licorice`.
pub const PALETTE: [(&str, u32, &str); 11] = [
    ("paper", 0xF1E4C8, "highlights, ghosts, labels"),
    ("pumpkin", 0xE07A2E, "jack-o'-lanterns, accents, warm light"),
    ("pumpkin_deep", 0xB4501E, "pumpkin shading, rust"),
    ("licorice", 0x1B1714, "outlines, cats, silhouettes"),
    ("mustard", 0xC99A3B, "wallpaper, brass, candle bodies"),
    ("brick", 0x8E3B2A, "walls, carpet, ribbons"),
    ("dusk", 0xA9A3C4, "sky, distant mountains, moonlit stone"),
    ("plum", 0x4A3F5E, "shadows, vignette edge, night stone"),
    ("moss", 0x56664F, "ground, damp walls"),
    ("night_teal", 0x2B3346, "trees, deep shadow, water"),
    ("candle", 0xFFC56B, "candle and lantern light"),
];

/// A palette token's colour.
#[must_use]
pub fn colour(token: &str) -> Option<u32> {
    PALETTE
        .iter()
        .find(|(name, _, _)| *name == token)
        .map(|(_, hex, _)| *hex)
}

/// What an asset is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A die-cut image that turns to face the player.
    Card,
    /// A low-poly mesh made in code.
    Mesh,
    /// A light, particle, or sound effect with no model of its own.
    Effect,
}

/// Where an asset may sit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Floor,
    Wall,
    Ceiling,
    /// Outdoors only, far away (the moon).
    Sky,
}

impl Surface {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Floor => "floor",
            Self::Wall => "wall",
            Self::Ceiling => "ceiling",
            Self::Sky => "sky",
        }
    }
}

/// When a trigger fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum When {
    /// The player comes within `within` metres of the prop.
    Near,
    /// The player enters the area.
    Enter,
    /// The player reaches the middle of the area.
    EnterMiddle,
    /// The player looks at the prop.
    Look,
}

/// One thing a default trigger does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DefaultAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
}

/// The trigger an asset brings when it is placed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DefaultTrigger {
    pub when: When,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<f32>,
    #[serde(rename = "do")]
    pub actions: Vec<DefaultAction>,
}

/// One library asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub aliases: Vec<String>,
    pub kind: Kind,
    pub surface: Surface,
    /// Width (x), depth (z), and height (y), m, facing yaw 0 (north).
    pub footprint: [f32; 3],
    /// The first is the state it starts in.
    pub states: Vec<String>,
    pub sounds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<DefaultTrigger>,
    /// How scary, 0 to 5.
    pub intensity: u8,
    pub kid_safe: bool,
    pub about: String,
}

/// What a sound is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundKind {
    Music,
    /// An ambience bed that loops under an area.
    Bed,
    Effect,
}

/// One sound.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sound {
    pub id: String,
    pub kind: SoundKind,
    pub loops: bool,
    pub kid_safe: bool,
    /// Where it comes from: made in the page, or a file of ours or CC0
    /// with its origin.
    pub source: String,
}

/// A surface: a shader pattern in two palette colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceStyle {
    pub id: String,
    pub pattern: Pattern,
    pub colors: [String; 2],
    pub about: String,
}

/// The patterns the shell's shader draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Plain,
    Cobble,
    Wood,
    Stripe,
    Halftone,
    Brick,
    Tiles,
    Grass,
}

impl Pattern {
    /// The shader's number for it.
    #[must_use]
    pub fn index(self) -> u32 {
        self as u32
    }
}

/// A lighting preset's base light.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub ambient: String,
    pub level: f32,
    pub about: String,
}

/// An arrival cue: sounds behind the player after they arrive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: String,
    pub sounds: Vec<String>,
    pub about: String,
}

/// A sky.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sky {
    pub id: String,
    pub about: String,
}

/// The whole library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub assets: Vec<Asset>,
    pub sounds: Vec<Sound>,
    pub surfaces: Vec<SurfaceStyle>,
    pub presets: Vec<Preset>,
    pub cues: Vec<Cue>,
    pub skies: Vec<Sky>,
}

/// The checked-in catalog's text.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

/// The library.
///
/// # Panics
/// The checked-in catalog doesn't read; its tests catch that.
#[must_use]
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(CATALOG_JSON).expect("catalog.json reads"))
}

impl Catalog {
    #[must_use]
    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// The asset an id or an alias names, ignoring case and spaces.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Asset> {
        let key = |s: &str| {
            s.to_ascii_lowercase()
                .replace(['-', '\'', '_'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let wanted = key(name);
        self.asset(name).or_else(|| {
            self.assets
                .iter()
                .find(|a| key(&a.id) == wanted || a.aliases.iter().any(|x| key(x) == wanted))
        })
    }

    #[must_use]
    pub fn sound(&self, id: &str) -> Option<&Sound> {
        self.sounds.iter().find(|s| s.id == id)
    }

    #[must_use]
    pub fn surface(&self, id: &str) -> Option<&SurfaceStyle> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    #[must_use]
    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    #[must_use]
    pub fn cue(&self, id: &str) -> Option<&Cue> {
        self.cues.iter().find(|c| c.id == id)
    }

    #[must_use]
    pub fn sky(&self, id: &str) -> Option<&Sky> {
        self.skies.iter().find(|s| s.id == id)
    }

    /// The library as text for an agent (`haunt describe --library`): every
    /// asset with its kind, size, surface, states, sounds, default trigger
    /// and intensity, then the sounds, surfaces, presets and cues.
    #[must_use]
    pub fn describe(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "THE PREPARED LIBRARY");
        for kind in [Kind::Card, Kind::Mesh, Kind::Effect] {
            let title = match kind {
                Kind::Card => "Cards (die-cut images that face you)",
                Kind::Mesh => "Meshes (low-poly models)",
                Kind::Effect => "Effects",
            };
            let _ = writeln!(out, "\n{title}:");
            for a in self.assets.iter().filter(|a| a.kind == kind) {
                let [w, d, h] = a.footprint;
                let _ = write!(
                    out,
                    "- {} ({}): {}. {w} x {d} m, {h} m tall, on the {}. States: {}.",
                    a.id,
                    a.aliases.join(", "),
                    a.about,
                    a.surface.word(),
                    a.states.join(", "),
                );
                if !a.sounds.is_empty() {
                    let _ = write!(out, " Sounds: {}.", a.sounds.join(", "));
                }
                if let Some(t) = &a.trigger {
                    let when = match t.when {
                        When::Near => format!("within {} m", t.within.unwrap_or(2.0)),
                        When::Enter => "on entering the area".into(),
                        When::EnterMiddle => "at the middle of the area".into(),
                        When::Look => "when looked at".into(),
                    };
                    let what: Vec<String> = t
                        .actions
                        .iter()
                        .map(|x| {
                            x.sfx
                                .as_ref()
                                .map(|s| format!("plays {s}"))
                                .or_else(|| x.state.as_ref().map(|s| format!("goes {s}")))
                                .or_else(|| x.effect.as_ref().map(|e| format!("shows {e}")))
                                .unwrap_or_default()
                        })
                        .collect();
                    let _ = write!(out, " By default {when} it {}.", what.join(" and "));
                }
                let _ = writeln!(
                    out,
                    " Intensity {}{}.",
                    a.intensity,
                    if a.kid_safe { ", kid-safe" } else { "" }
                );
            }
        }
        let _ = writeln!(out, "\nSounds:");
        for s in &self.sounds {
            let kind = match s.kind {
                SoundKind::Music => "music",
                SoundKind::Bed => "ambience bed",
                SoundKind::Effect => "effect",
            };
            let _ = writeln!(out, "- {} ({kind}): {}.", s.id, s.source);
        }
        let _ = writeln!(out, "\nSurfaces:");
        for s in &self.surfaces {
            let _ = writeln!(out, "- {}: {}.", s.id, s.about);
        }
        let _ = writeln!(out, "\nLighting presets:");
        for p in &self.presets {
            let _ = writeln!(out, "- {}: {}.", p.id, p.about);
        }
        let _ = writeln!(out, "\nArrival cues:");
        for c in &self.cues {
            let _ = writeln!(out, "- {}: {}.", c.id, c.about);
        }
        let _ = writeln!(out, "\nPalette:");
        for (name, hex, usage) in PALETTE {
            let _ = writeln!(out, "- {name} #{hex:06X}: {usage}.");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalog_reads_and_its_ids_are_unique() {
        let c = catalog();
        let mut seen = BTreeSet::new();
        for a in &c.assets {
            assert!(seen.insert(a.id.clone()), "{} twice", a.id);
            assert!(!a.states.is_empty(), "{}", a.id);
            assert!(a.intensity <= 5);
            assert!(a.footprint.iter().all(|x| *x > 0.0), "{}", a.id);
            for s in &a.sounds {
                assert!(c.sound(s).is_some(), "{} names sound {s}", a.id);
            }
            if let Some(t) = &a.trigger {
                for x in &t.actions {
                    if let Some(s) = &x.sfx {
                        assert!(c.sound(s).is_some(), "{} plays {s}", a.id);
                    }
                    if let Some(s) = &x.state {
                        assert!(a.states.contains(s), "{} has no state {s}", a.id);
                    }
                    if let Some(e) = &x.effect {
                        assert!(
                            c.asset(e).is_some_and(|e| e.kind == Kind::Effect),
                            "{} shows {e}",
                            a.id
                        );
                    }
                }
            }
        }
        for s in &c.surfaces {
            for token in &s.colors {
                assert!(colour(token).is_some(), "{} uses {token}", s.id);
            }
        }
        for p in &c.presets {
            assert!(colour(&p.ambient).is_some(), "{}", p.id);
        }
        for cue in &c.cues {
            for s in &cue.sounds {
                assert!(c.sound(s).is_some(), "{} plays {s}", cue.id);
            }
        }
        for s in &c.sounds {
            assert!(!s.source.is_empty() && s.kid_safe, "{}", s.id);
        }
    }

    #[test]
    fn the_spec_set_is_all_here() {
        let c = catalog();
        for id in [
            "werewolf",
            "vampire",
            "witch",
            "black-cat",
            "bat-swarm",
            "sheet-ghost",
            "skeleton",
            "mummy",
            "zombie-hand",
            "giant-spider",
            "crow",
            "owl",
            "scarecrow",
            "pumpkin-man",
            "headless-rider",
            "rat",
            "jack-o-lantern",
            "candle",
            "candelabra",
            "cauldron",
            "coffin",
            "tombstone",
            "cobweb",
            "portrait",
            "grandfather-clock",
            "gramophone",
            "armchair",
            "rocking-chair",
            "broom",
            "potion-shelf",
            "skull",
            "chains",
            "cage",
            "chandelier",
            "cracked-mirror",
            "piano",
            "bookshelf",
            "hanging-sheet",
            "banging-door",
            "lightning-window",
            "hay-bale",
            "corn-stalks",
            "iron-fence",
            "gnarled-tree",
            "moon",
            "ground-fog",
            "lightning-flash",
            "light-flicker",
            "dripping",
            "glowing-eyes",
            "floating-object",
            "door-slam",
            "footprints",
        ] {
            assert!(c.asset(id).is_some(), "{id}");
        }
        for id in [
            "creak-door",
            "footsteps-upstairs",
            "howl-wolf",
            "thunder",
            "crow",
            "owl",
            "hiss-cat",
            "laugh-evil",
            "cackle-witch",
            "chains",
            "heartbeat",
            "wind",
            "rain",
            "organ-sting",
            "scream-kid",
            "whisper",
            "chime-clock",
            "bubbling",
            "flutter-bat",
            "music-box",
            "bassline-minor",
            "clock-tick",
            "fireplace",
            "cauldron",
            "thunder-distant",
        ] {
            assert!(c.sound(id).is_some(), "{id}");
        }
        for id in [
            "moonlight",
            "candle",
            "lantern-only",
            "lightning",
            "jack-o-glow",
            "blackout",
        ] {
            assert!(c.preset(id).is_some(), "{id}");
        }
    }

    #[test]
    fn aliases_find_their_asset() {
        let c = catalog();
        assert_eq!(c.find("Wolf Man").map(|a| a.id.as_str()), Some("werewolf"));
        assert_eq!(
            c.find("jack o lantern").map(|a| a.id.as_str()),
            Some("jack-o-lantern")
        );
        assert_eq!(c.find("ghost").map(|a| a.id.as_str()), Some("sheet-ghost"));
        assert!(c.find("dragon").is_none());
    }

    #[test]
    fn the_library_reads_as_plain_text() {
        let text = catalog().describe();
        assert!(text.contains("- werewolf (wolfman, wolf man, wolf)"));
        assert!(text.contains("By default within 3 m it plays howl-wolf and goes lunge."));
        assert!(
            oa_copy::violations(&text, &[]).is_empty(),
            "{:?}",
            oa_copy::violations(&text, &[])
        );
    }
}
