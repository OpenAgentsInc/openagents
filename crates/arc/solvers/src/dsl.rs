use std::fmt;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Human-readable ownership summary for the ARC solver crate.
pub const ARC_SOLVER_BOUNDARY_SUMMARY: &str = "arc-solvers owns ARC-specific DSL/interpreter semantics and later solver-lane policy above arc-core contracts";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcDslTier {
    TierA,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcProgramMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub tier: ArcDslTier,
}

impl Default for ArcProgramMetadata {
    fn default() -> Self {
        Self {
            label: None,
            tier: ArcDslTier::TierA,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArcSymbol(String);

impl ArcSymbol {
    pub fn new(raw: impl Into<String>) -> Result<Self, ArcSymbolError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ArcSymbolError::Empty);
        }

        let mut chars = trimmed.chars();
        let Some(first) = chars.next() else {
            return Err(ArcSymbolError::Empty);
        };
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(ArcSymbolError::InvalidStart(first));
        }
        for ch in chars {
            if !ch.is_ascii_alphanumeric() && ch != '_' {
                return Err(ArcSymbolError::InvalidCharacter(ch));
            }
        }

        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArcSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ArcSymbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcSymbolError {
    #[error("ARC symbol names must not be empty")]
    Empty,
    #[error("ARC symbol names must start with a letter or underscore, got `{0}`")]
    InvalidStart(char),
    #[error("ARC symbol names must use ASCII letters, digits, or underscores, got `{0}`")]
    InvalidCharacter(char),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcGridBinding {
    pub name: ArcSymbol,
    pub value: ArcGridExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcProgram {
    pub input_symbol: ArcSymbol,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<ArcGridBinding>,
    pub body: ArcGridExpr,
    #[serde(default)]
    pub metadata: ArcProgramMetadata,
}

impl ArcProgram {
    #[must_use]
    pub fn new(input_symbol: ArcSymbol, body: ArcGridExpr) -> Self {
        Self {
            input_symbol,
            bindings: Vec::new(),
            body,
            metadata: ArcProgramMetadata::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcGridExpr {
    Input,
    Var {
        name: ArcSymbol,
    },
    Empty {
        width: u8,
        height: u8,
        fill: u8,
    },
    Sequence {
        steps: Vec<ArcGridExpr>,
    },
    CropToSelector {
        source: Box<ArcGridExpr>,
        selector: ArcObjectSelector,
    },
    PaintSelector {
        base: Box<ArcGridExpr>,
        source: Box<ArcGridExpr>,
        selector: ArcObjectSelector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recolor: Option<u8>,
        transform: ArcObjectTransform,
    },
    RotateQuarterTurns {
        source: Box<ArcGridExpr>,
        quarter_turns: u8,
    },
    ReflectHorizontal {
        source: Box<ArcGridExpr>,
    },
    ReflectVertical {
        source: Box<ArcGridExpr>,
    },
    Recolor {
        source: Box<ArcGridExpr>,
        from: u8,
        to: u8,
    },
    IfAnyObjects {
        source: Box<ArcGridExpr>,
        selector: ArcObjectSelector,
        then_branch: Box<ArcGridExpr>,
        else_branch: Box<ArcGridExpr>,
    },
    Let {
        name: ArcSymbol,
        value: Box<ArcGridExpr>,
        body: Box<ArcGridExpr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcObjectSelector {
    All,
    ByColor { color: u8 },
    Largest,
    Smallest,
    TopLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcObjectTransform {
    Identity,
    Translate { dx: i8, dy: i8 },
    RotateQuarterTurns { quarter_turns: u8 },
    ReflectHorizontal,
    ReflectVertical,
}
