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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanetChange {
    Style(PlanetStyle),
    Hour(f64),
    Cycle(bool),
    Settings {
        style: PlanetStyle,
        hour: f64,
        cycle: bool,
        day_minutes: u16,
    },
}
impl PlanetChange {
    pub fn from_request(text: &str) -> Option<Result<Self, Error>> {
        let text = text
            .trim()
            .trim_end_matches(['.', '!', '?'])
            .to_ascii_lowercase();
        Some(match text.as_str() {
            "use soft pastels" => Ok(Self::Style(PlanetStyle::SoftPastels)),
            "use bold fantasy" => Ok(Self::Style(PlanetStyle::BoldFantasy)),
            "pause day cycle" => Ok(Self::Cycle(false)),
            "start day cycle" => Ok(Self::Cycle(true)),
            _ => {
                let hour = text.strip_prefix("set time to ")?;
                hour.parse::<f64>()
                    .map(Self::Hour)
                    .map_err(|_| Error::Invalid("planet hour"))
            }
        })
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
    /// Apply a control or spoken request to the accepted clock and layout identity.
    pub fn changed(&self, change: PlanetChange, now_ms: u64) -> Result<Self, Error> {
        self.validate()?;
        let mut next = self.clone();
        let mut clock = |hour: Option<f64>, cycle: bool, day_minutes: u16| {
            if hour.is_some() || cycle != self.cycle_enabled || day_minutes != self.day_minutes {
                next.initial_hour = hour.unwrap_or_else(|| self.hour_at(now_ms));
                next.clock_anchor_unix_ms = now_ms;
            }
            next.cycle_enabled = cycle;
            next.day_minutes = day_minutes;
        };
        match change {
            PlanetChange::Style(style) => next.style = style,
            PlanetChange::Hour(hour) => clock(Some(hour), self.cycle_enabled, self.day_minutes),
            PlanetChange::Cycle(cycle) => clock(None, cycle, self.day_minutes),
            PlanetChange::Settings {
                style,
                hour,
                cycle,
                day_minutes,
            } => {
                clock(
                    (hour != self.initial_hour).then_some(hour),
                    cycle,
                    day_minutes,
                );
                next.style = style;
            }
        }
        next.validate()?;
        Ok(next)
    }
    pub fn time_label(&self) -> String {
        let minute = (self.initial_hour * 60.).round() as u32 % (24 * 60);
        format!("{:02}:{:02}", minute / 60, minute % 60)
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
        let night = hour < 6. || hour >= 19.;
        BTreeMap::from([
            (
                "sunAzimuth".into(),
                (hour * 15. + if night { 180. } else { 0. }).rem_euclid(360.),
            ),
            (
                "sunElevation".into(),
                if night { -elevation } else { elevation },
            ),
            ("sunIntensity".into(), day * 3.),
            ("moonIntensity".into(), (1. - day) * 3.),
            ("ambientIntensity".into(), 3.5 - day * 2.3),
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
