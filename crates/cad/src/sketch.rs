use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
}

impl CadSketchModel {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty() && self.entities.is_empty()
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

    pub fn validate(&self) -> CadResult<()> {
        for (plane_id, plane) in &self.planes {
            plane.validate()?;
            if plane.id != *plane_id {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch plane map key mismatch: key={plane_id} plane.id={}",
                        plane.id
                    ),
                });
            }
        }
        for (entity_id, entity) in &self.entities {
            entity.validate()?;
            if entity.id() != entity_id {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch entity map key mismatch: key={entity_id} entity.id={}",
                        entity.id()
                    ),
                });
            }
            if !self.planes.contains_key(entity.plane_id()) {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch entity {} references unknown plane {}",
                        entity.id(),
                        entity.plane_id()
                    ),
                });
            }
        }
        Ok(())
    }
}

fn validate_stable_id(value: &str, label: &str) -> CadResult<()> {
    if value.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: format!("{label} must not be empty"),
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(CadError::ParseFailed {
            reason: format!(
                "{label} contains invalid characters: {value} (allowed: [A-Za-z0-9._-])"
            ),
        });
    }
    Ok(())
}

fn validate_vec2_finite(value: [f64; 2], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

fn validate_vec3_finite(value: [f64; 3], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

#[cfg(test)]
mod tests {
    use super::{CadSketchEntity, CadSketchModel, CadSketchPlane};

    fn primary_plane() -> CadSketchPlane {
        CadSketchPlane {
            id: "plane.front".to_string(),
            name: "Front".to_string(),
            origin_mm: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
        }
    }

    #[test]
    fn sketch_model_serialization_is_deterministic_across_insertion_order() {
        let line = CadSketchEntity::Line {
            id: "entity.line.001".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [120.0, 0.0],
            anchor_ids: ["anchor.l.start".to_string(), "anchor.l.end".to_string()],
            construction: false,
        };
        let arc = CadSketchEntity::Arc {
            id: "entity.arc.001".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [60.0, 40.0],
            radius_mm: 20.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.a.center".to_string(),
                "anchor.a.start".to_string(),
                "anchor.a.end".to_string(),
            ],
            construction: false,
        };

        let mut left = CadSketchModel::default();
        left.insert_plane(primary_plane())
            .expect("left plane insert should succeed");
        left.insert_entity(line.clone())
            .expect("left line insert should succeed");
        left.insert_entity(arc.clone())
            .expect("left arc insert should succeed");

        let mut right = CadSketchModel::default();
        right
            .insert_plane(primary_plane())
            .expect("right plane insert should succeed");
        right
            .insert_entity(arc)
            .expect("right arc insert should succeed");
        right
            .insert_entity(line)
            .expect("right line insert should succeed");

        let left_json =
            serde_json::to_string(&left).expect("left model should serialize deterministically");
        let right_json =
            serde_json::to_string(&right).expect("right model should serialize deterministically");
        assert_eq!(left_json, right_json);
    }

    #[test]
    fn sketch_model_rejects_entities_that_reference_unknown_planes() {
        let mut model = CadSketchModel::default();
        let entity = CadSketchEntity::Point {
            id: "entity.point.001".to_string(),
            plane_id: "plane.missing".to_string(),
            position_mm: [5.0, 8.0],
            anchor_id: "anchor.p.001".to_string(),
            construction: true,
        };
        let result = model.insert_entity(entity);
        assert!(result.is_err(), "unknown plane reference must fail");
    }

    #[test]
    fn sketch_entity_validation_rejects_duplicate_anchor_ids() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(primary_plane())
            .expect("plane insert should succeed");
        let result = model.insert_entity(CadSketchEntity::Line {
            id: "entity.line.dup".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [50.0, 0.0],
            anchor_ids: ["anchor.dup".to_string(), "anchor.dup".to_string()],
            construction: false,
        });
        assert!(result.is_err(), "duplicate anchors must fail validation");
    }
}
