//! A finite personal courtyard recipe stored as an existing editable source file.
use crate::{
    Error, MAX_SAFE_INTEGER, OriginalImport, ProjectDocument, WorldEntity, WorldNamespace,
    WorldTemplate, project_hash,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const PLANET_SOURCE: &str = "planet-v1.json";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlanetStyle {
    SoftPastels,
    BoldFantasy,
}
impl PlanetStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::SoftPastels => "Soft pastels",
            Self::BoldFantasy => "Bold fantasy",
        }
    }
    pub fn effect(self) -> String {
        let color = match self {
            Self::SoftPastels => "mix(base, vec3<f32>(0.75, 0.82, 0.86), 0.32)",
            Self::BoldFantasy => {
                "clamp(base * vec3<f32>(1.16, 1.03, 0.88), vec3<f32>(0.0), vec3<f32>(1.0))"
            }
        };
        format!(
            "fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> {{\n  return {color};\n}}"
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanetRecipe {
    pub schema: String,
    pub seed: u32,
    pub side_metres: u16,
    pub spawn: [f64; 3],
    pub style: PlanetStyle,
    pub initial_hour: f64,
    pub cycle_enabled: bool,
    pub day_minutes: u16,
    pub clock_anchor_unix_ms: u64,
}
impl PlanetRecipe {
    pub fn new(seed: u32, now_ms: u64) -> Self {
        Self {
            schema: "openagents.verse.planet.v1".into(),
            seed,
            side_metres: 64,
            spawn: [0., 1.62, 12.],
            style: PlanetStyle::SoftPastels,
            initial_hour: 8.,
            cycle_enabled: true,
            day_minutes: 10,
            clock_anchor_unix_ms: now_ms,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != "openagents.verse.planet.v1"
            || self.side_metres != 64
            || self.spawn != [0., 1.62, 12.]
            || !self.initial_hour.is_finite()
            || !(0.0..24.0).contains(&self.initial_hour)
            || !(2..=60).contains(&self.day_minutes)
            || self.clock_anchor_unix_ms > MAX_SAFE_INTEGER
        {
            return Err(Error::Invalid("personal planet recipe"));
        }
        Ok(())
    }
    pub fn hour_at(&self, now_ms: u64) -> f64 {
        let elapsed = if self.cycle_enabled {
            now_ms.saturating_sub(self.clock_anchor_unix_ms) as f64 * 24.
                / (self.day_minutes as f64 * 60_000.)
        } else {
            0.
        };
        (self.initial_hour + elapsed).rem_euclid(24.)
    }
    /// Numeric lighting values stay within the existing bounded environment controls.
    pub fn lighting_at(&self, now_ms: u64) -> BTreeMap<String, f64> {
        let hour = self.hour_at(now_ms);
        let elevation = (65. * ((hour - 6.) * std::f64::consts::TAU / 24.).sin()).clamp(-20., 80.);
        let day = (elevation / 15.).clamp(0., 1.);
        BTreeMap::from([
            ("sunAzimuth".into(), (hour * 15.).rem_euclid(360.)),
            ("sunElevation".into(), elevation),
            ("sunIntensity".into(), day * 3.),
            ("moonIntensity".into(), (1. - day) * 1.5),
            ("ambientIntensity".into(), 0.65 + day * 0.55),
            ("skyIntensity".into(), 0.45 + day * 0.65),
            ("skyBackgroundIntensity".into(), 0.45 + day * 0.55),
            ("fogDensity".into(), 0.003),
        ])
    }
    pub fn source(&self) -> Result<OriginalImport, Error> {
        self.validate()?;
        let text =
            serde_json::to_string(self).map_err(|_| Error::Invalid("personal planet recipe"))?;
        Ok(OriginalImport {
            name: PLANET_SOURCE.into(),
            sha256: project_hash(text.as_bytes()),
            text,
        })
    }
    pub fn from_source(source: &OriginalImport) -> Result<Self, Error> {
        if source.name != PLANET_SOURCE
            || source.text.len() > 24 * 1024
            || project_hash(source.text.as_bytes()) != source.sha256
        {
            return Err(Error::Invalid("personal planet source"));
        }
        let recipe: Self = serde_json::from_str(&source.text)
            .map_err(|_| Error::Invalid("personal planet source"))?;
        recipe.validate()?;
        Ok(recipe)
    }
    pub fn from_document(d: &ProjectDocument) -> Result<Self, Error> {
        d.validate()?;
        let sources: Vec<_> = d
            .originals
            .iter()
            .filter(|s| s.name == PLANET_SOURCE)
            .collect();
        if d.namespace != WorldNamespace::PersonalPlanet || sources.len() != 1 {
            return Err(Error::Invalid("personal planet source"));
        }
        let recipe = Self::from_source(sources[0])?;
        if !d.rooms.is_empty()
            || !d.removed_walls.is_empty()
            || d.entities.values().any(|e| {
                e.position[0].abs() > 28.
                    || e.position[2].abs() > 28.
                    || !(0.0..=12.).contains(&e.position[1])
                    || !(0.5..=2.).contains(&e.scale)
            })
        {
            return Err(Error::Invalid("personal planet bounds"));
        }
        Ok(recipe)
    }
    pub fn document(&self, template: WorldTemplate) -> Result<ProjectDocument, Error> {
        self.validate()?;
        let mut random = (self.seed as u64) ^ 0x9e3779b97f4a7c15;
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            (random % 10_000) as f64 / 10_000.
        };
        let mut entities = BTreeMap::new();
        // Six small seating groups leave the entrance and center clear for walking.
        for (group, (cx, cz)) in [
            (-16., -16.),
            (0., -16.),
            (16., -16.),
            (-16., 8.),
            (16., 8.),
            (0., 24.),
        ]
        .into_iter()
        .enumerate()
        {
            let x = cx + next() - 0.5;
            let z = cz + next() - 0.5;
            let turn = if next() < 0.5 {
                0.0_f64
            } else {
                std::f64::consts::FRAC_PI_2
            };
            for (part, (asset, dx, dz, yaw)) in [
                ("table_large", 0., 0., 0.),
                ("bench", 0., -2., 0.),
                ("bench", 0., 2., std::f64::consts::PI),
                ("barrel", 3., 3., 0.),
            ]
            .into_iter()
            .enumerate()
            {
                entities.insert(
                    format!("planet-prop-{group}-{part}"),
                    WorldEntity {
                        asset: format!("quaternius:fantasy-standard:{asset}"),
                        position: [
                            x + dx * turn.cos() + dz * turn.sin(),
                            0.,
                            z - dx * turn.sin() + dz * turn.cos(),
                        ],
                        yaw: turn + yaw,
                        scale: 1.,
                    },
                );
            }
        }
        let effect = self.style.effect();
        let d = ProjectDocument {
            schema: crate::PROJECT_SCHEMA.into(),
            namespace: WorldNamespace::PersonalPlanet,
            template,
            source_revision: 0,
            entities,
            rooms: BTreeMap::new(),
            removed_walls: vec![],
            materials: BTreeMap::new(),
            environment: self.lighting_at(self.clock_anchor_unix_ms),
            source_draft: effect.clone(),
            active_source: effect,
            layers: vec![],
            originals: vec![self.source()?],
        };
        d.validate()?;
        Self::from_document(&d)?;
        Ok(d)
    }
}
