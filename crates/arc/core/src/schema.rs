use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// First version of the shared ARC Rust-native contract surface.
pub const ARC_CORE_SCHEMA_VERSION: u32 = 1;
/// What belongs in `schema` and what does not.
pub const SCHEMA_BOUNDARY_SUMMARY: &str = "Own ARC task/grid/example value types and shared identifiers. Do not absorb benchmark policy, transport behavior, solver search state, or reusable Psionic substrate.";
/// ARC tasks are 2D grids with edge sizes in the 1..=30 range.
pub const ARC_GRID_MAX_EDGE: u8 = 30;
/// ARC palette values stay in the canonical 0..=9 range.
pub const ARC_PALETTE_SIZE: u8 = 10;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArcTaskId(String);

impl ArcTaskId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn new(raw: impl Into<String>) -> Result<Self, ArcTaskIdError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ArcTaskIdError::Empty);
        }
        if trimmed.chars().any(char::is_whitespace) {
            return Err(ArcTaskIdError::ContainsWhitespace(trimmed.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl fmt::Display for ArcTaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ArcTaskId {
    type Err = ArcTaskIdError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::new(raw)
    }
}

impl Serialize for ArcTaskId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcTaskId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcTaskIdError {
    #[error("ARC task id must not be empty")]
    Empty,
    #[error("ARC task id must not contain whitespace: {0}")]
    ContainsWhitespace(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcGrid {
    width: u8,
    height: u8,
    cells: Vec<u8>,
}

impl ArcGrid {
    pub fn new(width: u8, height: u8, cells: Vec<u8>) -> Result<Self, ArcGridError> {
        if width == 0 || width > ARC_GRID_MAX_EDGE {
            return Err(ArcGridError::InvalidWidth(width));
        }
        if height == 0 || height > ARC_GRID_MAX_EDGE {
            return Err(ArcGridError::InvalidHeight(height));
        }

        let expected_len = usize::from(width) * usize::from(height);
        if cells.len() != expected_len {
            return Err(ArcGridError::MismatchedCellCount {
                expected: expected_len,
                actual: cells.len(),
            });
        }

        if let Some(color) = cells.iter().copied().find(|cell| *cell >= ARC_PALETTE_SIZE) {
            return Err(ArcGridError::InvalidColor(color));
        }

        Ok(Self {
            width,
            height,
            cells,
        })
    }

    #[must_use]
    pub fn width(&self) -> u8 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u8 {
        self.height
    }

    #[must_use]
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn cell(&self, x: u8, y: u8) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.cells.get(index).copied()
    }
}

impl Serialize for ArcGrid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ArcGridWire {
            width: self.width,
            height: self.height,
            cells: self.cells.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArcGrid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArcGridWire::deserialize(deserializer)?;
        Self::new(wire.width, wire.height, wire.cells).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcGridError {
    #[error("ARC grid width must be in the 1..={ARC_GRID_MAX_EDGE} range, got {0}")]
    InvalidWidth(u8),
    #[error("ARC grid height must be in the 1..={ARC_GRID_MAX_EDGE} range, got {0}")]
    InvalidHeight(u8),
    #[error("ARC grid cell count mismatch: expected {expected}, got {actual}")]
    MismatchedCellCount { expected: usize, actual: usize },
    #[error("ARC grid color must be in the 0..=9 range, got {0}")]
    InvalidColor(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcExample {
    pub input: ArcGrid,
    pub output: ArcGrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcTask {
    pub id: ArcTaskId,
    pub train: Vec<ArcExample>,
    pub test: Vec<ArcGrid>,
}

impl ArcTask {
    pub fn new(
        id: ArcTaskId,
        train: Vec<ArcExample>,
        test: Vec<ArcGrid>,
    ) -> Result<Self, ArcTaskError> {
        if train.is_empty() {
            return Err(ArcTaskError::MissingTrainExamples);
        }
        if test.is_empty() {
            return Err(ArcTaskError::MissingTestInputs);
        }
        Ok(Self { id, train, test })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcTaskError {
    #[error("ARC task must include at least one train example")]
    MissingTrainExamples,
    #[error("ARC task must include at least one test input")]
    MissingTestInputs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcGridWire {
    width: u8,
    height: u8,
    cells: Vec<u8>,
}
