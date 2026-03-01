use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::common::{validate_stable_id, validate_vec2_finite, validate_vec3_finite};
use super::constraints::CadSketchConstraint;
use crate::{CadError, CadResult};

/// Stable sketch plane definition in model-space millimeters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSketchPlane {
    pub id: String,
    pub name: String,
    pub origin_mm: [f64; 3],
    pub normal: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
}

impl CadSketchPlane {
    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(&self.id, "sketch plane id")?;
        if self.name.trim().is_empty() {
            return Err(CadError::ParseFailed {
                reason: "sketch plane name must not be empty".to_string(),
            });
        }
        validate_vec3_finite(self.origin_mm, "sketch plane origin_mm")?;
        validate_vec3_finite(self.normal, "sketch plane normal")?;
        validate_vec3_finite(self.x_axis, "sketch plane x_axis")?;
        validate_vec3_finite(self.y_axis, "sketch plane y_axis")?;
        Ok(())
    }
}

/// Stable sketch entity model for Wave 2 kickoff.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadSketchEntity {
    Line {
        id: String,
        plane_id: String,
        start_mm: [f64; 2],
        end_mm: [f64; 2],
        anchor_ids: [String; 2],
        construction: bool,
    },
    Arc {
        id: String,
        plane_id: String,
        center_mm: [f64; 2],
        radius_mm: f64,
        start_deg: f64,
        end_deg: f64,
        anchor_ids: [String; 3],
        construction: bool,
    },
    Point {
        id: String,
        plane_id: String,
        position_mm: [f64; 2],
        anchor_id: String,
        construction: bool,
    },
}

impl CadSketchEntity {
    pub fn id(&self) -> &str {
        match self {
            Self::Line { id, .. } => id,
            Self::Arc { id, .. } => id,
            Self::Point { id, .. } => id,
        }
    }

    pub fn plane_id(&self) -> &str {
        match self {
            Self::Line { plane_id, .. } => plane_id,
            Self::Arc { plane_id, .. } => plane_id,
            Self::Point { plane_id, .. } => plane_id,
        }
    }

    pub fn anchor_ids(&self) -> Vec<&str> {
        match self {
            Self::Line { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Arc { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Point { anchor_id, .. } => vec![anchor_id.as_str()],
        }
    }

    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(self.id(), "sketch entity id")?;
        validate_stable_id(self.plane_id(), "sketch entity plane_id")?;
        for anchor in self.anchor_ids() {
            validate_stable_id(anchor, "sketch anchor id")?;
        }

        let anchors = self.anchor_ids();
        let mut unique = BTreeSet::<&str>::new();
        for anchor in &anchors {
            unique.insert(anchor);
        }
        if unique.len() != anchors.len() {
            return Err(CadError::ParseFailed {
                reason: format!("sketch entity {} has duplicate anchor ids", self.id()),
            });
        }

        match self {
            Self::Line {
                start_mm, end_mm, ..
            } => {
                validate_vec2_finite(*start_mm, "line start_mm")?;
                validate_vec2_finite(*end_mm, "line end_mm")?;
                if start_mm == end_mm {
                    return Err(CadError::ParseFailed {
                        reason: format!("line {} start/end points must differ", self.id()),
                    });
                }
            }
            Self::Arc {
                center_mm,
                radius_mm,
                start_deg,
                end_deg,
                ..
            } => {
                validate_vec2_finite(*center_mm, "arc center_mm")?;
                if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("arc {} radius_mm must be finite and > 0", self.id()),
                    });
                }
                if !start_deg.is_finite() || !end_deg.is_finite() {
                    return Err(CadError::ParseFailed {
                        reason: format!("arc {} angle bounds must be finite", self.id()),
                    });
                }
            }
            Self::Point { position_mm, .. } => {
                validate_vec2_finite(*position_mm, "point position_mm")?;
            }
        }
        Ok(())
    }
}

/// Deterministic sketch model persisted in `.apcad`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadSketchModel {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub planes: BTreeMap<String, CadSketchPlane>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entities: BTreeMap<String, CadSketchEntity>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub constraints: BTreeMap<String, CadSketchConstraint>,
}

impl CadSketchModel {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty() && self.entities.is_empty() && self.constraints.is_empty()
    }

    pub fn insert_plane(&mut self, plane: CadSketchPlane) -> CadResult<()> {
        plane.validate()?;
        self.planes.insert(plane.id.clone(), plane);
        self.validate()
    }

    pub fn insert_entity(&mut self, entity: CadSketchEntity) -> CadResult<()> {
        entity.validate()?;
        self.entities.insert(entity.id().to_string(), entity);
        self.validate()
    }

    pub fn insert_constraint(&mut self, constraint: CadSketchConstraint) -> CadResult<()> {
        constraint.validate()?;
        self.constraints
            .insert(constraint.id().to_string(), constraint);
        self.validate()
    }
}
