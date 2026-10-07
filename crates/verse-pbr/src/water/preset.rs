//! Water presets: how a body of water looks, from
//! `assets/verse/water/presets/*.toml`. The physics never reads one.
//!
//! A preset's absorption comes from one of Jerlov's water types (Nils G.
//! Jerlov, *Marine Optics*, Elsevier, 1976), which classify natural waters
//! by their diffuse attenuation coefficient `K_d(λ)`: oceanic types I, IA,
//! IB, II, and III, and coastal types 1 to 9, from the clearest open ocean to
//! turbid coastal water. [`Jerlov::absorption`] gives `K_d` at 650, 550, and
//! 450 nm for red, green, and blue, approximated to two figures from the
//! published curves; red is dominated by pure water's own absorption, about
//! 0.34 /m at 650 nm (Pope and Fry, *Applied Optics*, 1997). A preset may
//! instead give `absorption` itself. The in-scatter color, foam, roughness,
//! and shore band are our own tuning.

use std::sync::OnceLock;

use serde::Deserialize;

/// One of Jerlov's water types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jerlov {
    I,
    IA,
    IB,
    II,
    III,
    /// Coastal types 1, 3, 5, 7, and 9.
    Coastal(u8),
}

impl Jerlov {
    /// The type a preset names: `I`, `IA`, `IB`, `II`, `III`, or `C1`,
    /// `C3`, `C5`, `C7`, `C9`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "I" => Self::I,
            "IA" => Self::IA,
            "IB" => Self::IB,
            "II" => Self::II,
            "III" => Self::III,
            "C1" => Self::Coastal(1),
            "C3" => Self::Coastal(3),
            "C5" => Self::Coastal(5),
            "C7" => Self::Coastal(7),
            "C9" => Self::Coastal(9),
            _ => return None,
        })
    }

    /// `K_d` at 650, 550, and 450 nm, 1/m.
    #[must_use]
    pub fn absorption(self) -> [f32; 3] {
        match self {
            Self::I => [0.36, 0.064, 0.019],
            Self::IA => [0.37, 0.067, 0.022],
            Self::IB => [0.37, 0.069, 0.026],
            Self::II => [0.38, 0.077, 0.047],
            Self::III => [0.42, 0.10, 0.085],
            Self::Coastal(1) => [0.45, 0.12, 0.13],
            Self::Coastal(3) => [0.48, 0.16, 0.22],
            Self::Coastal(5) => [0.52, 0.22, 0.34],
            Self::Coastal(7) => [0.60, 0.31, 0.55],
            Self::Coastal(_) => [0.70, 0.43, 0.85],
        }
    }
}

/// How a body of water looks.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub jerlov: Option<Jerlov>,
    /// Absorption per meter, linear rgb.
    pub absorption: [f32; 3],
    /// In-scatter color, linear rgb.
    pub scatter: [f32; 3],
    /// How much foam forms, 0 to 1.
    pub foam: f32,
    /// The calm surface's roughness.
    pub roughness: f32,
    /// How wide the shore's foam band is, m.
    pub shore_band: f32,
    /// How strongly sunlight scatters through thin crests, 0 to 1.
    pub crest_scatter: f32,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "default".into(),
            description: "Clear water.".into(),
            jerlov: Some(Jerlov::II),
            absorption: Jerlov::II.absorption(),
            scatter: [0.012, 0.055, 0.07],
            foam: 0.6,
            roughness: 0.04,
            shore_band: 0.8,
            crest_scatter: 0.5,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    description: String,
    jerlov: Option<String>,
    absorption: Option<[f32; 3]>,
    scatter: [f32; 3],
    foam: f32,
    roughness: f32,
    shore_band: f32,
    crest_scatter: f32,
}

/// The presets built into this crate, by file name.
const FILES: [(&str, &str); 7] = [
    (
        "cove",
        include_str!("../../../../assets/verse/water/presets/cove.toml"),
    ),
    (
        "lake",
        include_str!("../../../../assets/verse/water/presets/lake.toml"),
    ),
    (
        "marsh",
        include_str!("../../../../assets/verse/water/presets/marsh.toml"),
    ),
    (
        "ocean",
        include_str!("../../../../assets/verse/water/presets/ocean.toml"),
    ),
    (
        "pond",
        include_str!("../../../../assets/verse/water/presets/pond.toml"),
    ),
    (
        "pool",
        include_str!("../../../../assets/verse/water/presets/pool.toml"),
    ),
    (
        "river",
        include_str!("../../../../assets/verse/water/presets/river.toml"),
    ),
];

impl Preset {
    /// Parses a preset file named `name`.
    ///
    /// # Errors
    /// The text is not a preset, names no known Jerlov type, gives neither
    /// a type nor an absorption, or holds a value out of range.
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| format!("Water preset {name}: {e}"))?;
        let jerlov = file
            .jerlov
            .as_deref()
            .map(|j| {
                Jerlov::parse(j).ok_or_else(|| format!("Water preset {name}: no Jerlov type {j}"))
            })
            .transpose()?;
        let absorption = file
            .absorption
            .or_else(|| jerlov.map(Jerlov::absorption))
            .ok_or_else(|| format!("Water preset {name} gives no Jerlov type or absorption"))?;
        let preset = Self {
            name: name.into(),
            description: file.description,
            jerlov,
            absorption,
            scatter: file.scatter,
            foam: file.foam,
            roughness: file.roughness,
            shore_band: file.shore_band,
            crest_scatter: file.crest_scatter,
        };
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        let positive = |v: &f32| v.is_finite() && *v >= 0.0;
        if !(preset.absorption.iter().all(positive)
            && preset.scatter.iter().all(positive)
            && unit(preset.foam)
            && unit(preset.roughness)
            && unit(preset.crest_scatter)
            && positive(&preset.shore_band))
        {
            return Err(format!("Water preset {name} has a value out of range"));
        }
        Ok(preset)
    }

    /// Every built-in preset, parsed once.
    ///
    /// # Panics
    /// When a built-in file does not parse, which a test rules out.
    #[must_use]
    pub fn all() -> &'static [Preset] {
        static ALL: OnceLock<Vec<Preset>> = OnceLock::new();
        ALL.get_or_init(|| {
            FILES
                .iter()
                .map(|(name, text)| {
                    Self::parse(name, text).expect("a built-in water preset parses")
                })
                .collect()
        })
    }

    /// The built-in preset named `name` (`ocean`, `cove`, `pool`, `lake`,
    /// `pond`, `river`, or `marsh`).
    #[must_use]
    pub fn named(name: &str) -> Option<&'static Preset> {
        Self::all().iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in preset parses, and clearer types absorb less blue.
    #[test]
    fn presets_parse_and_jerlov_types_darken_in_order() {
        assert_eq!(Preset::all().len(), FILES.len());
        for p in Preset::all() {
            assert!(p.jerlov.is_some(), "{} names a Jerlov type", p.name);
        }
        let order = [
            Jerlov::I,
            Jerlov::IA,
            Jerlov::IB,
            Jerlov::II,
            Jerlov::III,
            Jerlov::Coastal(1),
            Jerlov::Coastal(3),
            Jerlov::Coastal(5),
            Jerlov::Coastal(7),
            Jerlov::Coastal(9),
        ];
        for pair in order.windows(2) {
            let (a, b) = (pair[0].absorption(), pair[1].absorption());
            assert!(b.iter().zip(&a).all(|(b, a)| b >= a), "{pair:?}");
        }
        // Open-ocean water passes blue best; turbid coastal water, green.
        let ocean = Jerlov::I.absorption();
        assert!(ocean[2] < ocean[1] && ocean[1] < ocean[0]);
        let coastal = Jerlov::Coastal(9).absorption();
        assert!(coastal[1] < coastal[2]);
        assert!(Preset::parse("bad", "description = \"x\"").is_err());
        assert!(Preset::named("cove").is_some_and(|p| p.jerlov == Some(Jerlov::II)));
    }
}
