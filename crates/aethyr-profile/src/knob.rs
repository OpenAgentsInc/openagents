//! `Detail`, knobs, curves, and floors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::UserPrefs;

/// One scalar the controller moves: 0 is every knob's floor, 1 is full.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Detail(f64);

impl Detail {
    pub const FLOOR: Self = Self(0.0);
    pub const FULL: Self = Self(1.0);

    /// `None` when `value` is not finite; otherwise clamped to 0..=1.
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then(|| Self(value.clamp(0.0, 1.0)))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// A setting `Detail` drives. World knobs first, then interface knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Knob {
    RenderScale,
    Aa,
    Occlusion,
    ClusterLodBias,
    CrowdTierDistance,
    ShadowQuality,
    FoliageDensity,
    BackdropBlur,
    UiShadows,
    Motion,
    UiEffects,
}

impl Knob {
    pub const ALL: [Self; 11] = [
        Self::RenderScale,
        Self::Aa,
        Self::Occlusion,
        Self::ClusterLodBias,
        Self::CrowdTierDistance,
        Self::ShadowQuality,
        Self::FoliageDensity,
        Self::BackdropBlur,
        Self::UiShadows,
        Self::Motion,
        Self::UiEffects,
    ];

    /// The CSS variable an interface knob writes; `None` for world knobs.
    pub fn css_variable(self) -> Option<&'static str> {
        match self {
            Self::BackdropBlur => Some("--detail-blur"),
            Self::UiShadows => Some("--detail-shadow"),
            Self::Motion => Some("--detail-motion"),
            Self::UiEffects => Some("--detail-effects"),
            _ => None,
        }
    }

    /// Whether a preference holds this knob at its floor.
    pub fn limited_by(self, prefs: &UserPrefs) -> bool {
        match self {
            Self::Motion | Self::UiEffects => prefs.reduced_motion,
            Self::BackdropBlur => prefs.reduced_transparency,
            _ => false,
        }
    }

    /// Knobs a preference can hold must declare a floor of 0, so that a
    /// preference and a floor never disagree.
    pub fn preference_governed(self) -> bool {
        matches!(self, Self::Motion | Self::UiEffects | Self::BackdropBlur)
    }
}

/// How a knob responds to `Detail`, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Curve {
    Linear,
    /// Holds low until `Detail` is high: `d²`.
    EaseIn,
    /// Rises early: `1 - (1 - d)²`.
    EaseOut,
    /// `count` equal steps above the floor, so the setting changes rarely.
    Steps {
        count: u32,
    },
}

impl Curve {
    pub fn at(self, detail: Detail) -> f64 {
        let d = detail.get();
        match self {
            Self::Linear => d,
            Self::EaseIn => d * d,
            Self::EaseOut => 1.0 - (1.0 - d) * (1.0 - d),
            Self::Steps { count } => {
                let n = f64::from(count.max(1));
                (d * n).floor() / n
            }
        }
    }
}

/// One knob's response: `floor` at `Detail` 0, `full` at 1. The floor is
/// hard: no `Detail` or preference produces a value below it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnobDecl {
    pub knob: Knob,
    pub curve: Curve,
    pub floor: f64,
    pub full: f64,
}

impl KnobDecl {
    pub fn value(&self, detail: Detail, prefs: &UserPrefs) -> KnobValue {
        let limited = self.knob.limited_by(prefs);
        let value = if limited {
            self.floor
        } else {
            let raw = self.floor + (self.full - self.floor) * self.curve.at(detail);
            crate::round6(raw.clamp(self.floor, self.full))
        };
        KnobValue {
            knob: self.knob,
            value,
            limited,
        }
    }
}

/// A knob's current setting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnobValue {
    pub knob: Knob,
    pub value: f64,
    /// A preference holds this knob at its floor.
    pub limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnobError {
    Duplicate(Knob),
    NotFinite(Knob),
    FloorAboveFull(Knob),
    NoSteps(Knob),
    /// A preference-governed knob declared a floor above 0.
    PreferenceFloor(Knob),
}

impl fmt::Display for KnobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(k) => write!(f, "knob {k:?} is declared twice"),
            Self::NotFinite(k) => write!(f, "knob {k:?} needs finite floor and full values"),
            Self::FloorAboveFull(k) => write!(f, "knob {k:?} has a floor above its full value"),
            Self::NoSteps(k) => write!(f, "knob {k:?} needs at least one step"),
            Self::PreferenceFloor(k) => {
                write!(f, "knob {k:?} follows a preference, so its floor must be 0")
            }
        }
    }
}

impl std::error::Error for KnobError {}

/// A validated set of knob declarations, as a zone manifest or the web
/// renderer declares them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<KnobDecl>", into = "Vec<KnobDecl>")]
pub struct KnobSet(Vec<KnobDecl>);

impl KnobSet {
    pub fn new(decls: Vec<KnobDecl>) -> Result<Self, KnobError> {
        let mut seen = Vec::with_capacity(decls.len());
        for decl in &decls {
            let k = decl.knob;
            if seen.contains(&k) {
                return Err(KnobError::Duplicate(k));
            }
            seen.push(k);
            if !decl.floor.is_finite() || !decl.full.is_finite() {
                return Err(KnobError::NotFinite(k));
            }
            if decl.floor > decl.full {
                return Err(KnobError::FloorAboveFull(k));
            }
            if matches!(decl.curve, Curve::Steps { count: 0 }) {
                return Err(KnobError::NoSteps(k));
            }
            if k.preference_governed() && decl.floor != 0.0 {
                return Err(KnobError::PreferenceFloor(k));
            }
        }
        Ok(Self(decls))
    }

    pub fn decls(&self) -> &[KnobDecl] {
        &self.0
    }

    pub fn values(&self, detail: Detail, prefs: &UserPrefs) -> Vec<KnobValue> {
        self.0
            .iter()
            .map(|decl| decl.value(detail, prefs))
            .collect()
    }
}

impl TryFrom<Vec<KnobDecl>> for KnobSet {
    type Error = KnobError;
    fn try_from(decls: Vec<KnobDecl>) -> Result<Self, KnobError> {
        Self::new(decls)
    }
}

impl From<KnobSet> for Vec<KnobDecl> {
    fn from(set: KnobSet) -> Self {
        set.0
    }
}

/// CSS custom properties for the interface knobs among `values`, plus
/// `--detail` itself, each written with three decimals.
pub fn css_variables(detail: Detail, values: &[KnobValue]) -> Vec<(String, String)> {
    let mut out = vec![("--detail".to_owned(), fixed3(detail.get()))];
    for value in values {
        if let Some(name) = value.knob.css_variable() {
            out.push((name.to_owned(), fixed3(value.value)));
        }
    }
    out
}

/// Three decimals, rounded half up; the TypeScript mirror formats the same
/// way so both write identical strings.
fn fixed3(value: f64) -> String {
    let thousandths = (value * 1000.0).round() as i64;
    let sign = if thousandths < 0 { "-" } else { "" };
    let abs = thousandths.unsigned_abs();
    format!("{sign}{}.{:03}", abs / 1000, abs % 1000)
}
