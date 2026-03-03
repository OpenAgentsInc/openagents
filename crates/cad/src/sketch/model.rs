use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::common::{validate_stable_id, validate_vec2_finite, validate_vec3_finite};
use super::constraints::CadSketchConstraint;
use crate::kernel_geom::SurfaceRecord;
use crate::kernel_primitives::BRepSolid;
use crate::kernel_topology::{FaceId, Orientation};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadSketchPlanePreset {
    Xy,
    Xz,
    Yz,
}

impl CadSketchPlanePreset {
    pub fn key(self) -> &'static str {
        match self {
            Self::Xy => "xy",
            Self::Xz => "xz",
            Self::Yz => "yz",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Xy => "XY",
            Self::Xz => "XZ",
            Self::Yz => "YZ",
        }
    }
}

impl CadSketchPlane {
    pub fn from_preset(preset: CadSketchPlanePreset) -> Self {
        let (normal, x_axis, y_axis) = match preset {
            CadSketchPlanePreset::Xy => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            CadSketchPlanePreset::Xz => ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            CadSketchPlanePreset::Yz => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        };
        Self {
            id: format!("plane.{}", preset.key()),
            name: preset.label().to_string(),
            origin_mm: [0.0, 0.0, 0.0],
            normal,
            x_axis,
            y_axis,
        }
    }

    pub fn xy() -> Self {
        Self::from_preset(CadSketchPlanePreset::Xy)
    }

    pub fn xz() -> Self {
        Self::from_preset(CadSketchPlanePreset::Xz)
    }

    pub fn yz() -> Self {
        Self::from_preset(CadSketchPlanePreset::Yz)
    }

    pub fn from_planar_face(brep: &BRepSolid, face_ref: &str) -> CadResult<Self> {
        let face_id = parse_face_ref(face_ref)?;
        Self::from_planar_face_with_identity(
            brep,
            face_ref,
            format!("plane.face.{}", face_id.0),
            format!("Face {}", face_id.0),
        )
    }

    pub fn from_planar_face_with_identity(
        brep: &BRepSolid,
        face_ref: &str,
        id: impl Into<String>,
        name: impl Into<String>,
    ) -> CadResult<Self> {
        let face_id = parse_face_ref(face_ref)?;
        let face = brep
            .topology
            .faces
            .get(&face_id)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!("sketch plane face_ref {face_ref} does not exist"),
            })?;
        let surface = brep
            .geometry
            .surfaces
            .get(face.surface_index)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!(
                    "sketch plane face_ref {face_ref} references missing surface {}",
                    face.surface_index
                ),
            })?;
        let plane = match surface {
            SurfaceRecord::Plane(plane) => plane,
            _ => {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "sketch plane face_ref {face_ref} must reference a planar face, found {:?}",
                        surface.kind()
                    ),
                });
            }
        };

        let mut normal = vec3_to_array(plane.normal_dir.into_inner());
        let x_axis = vec3_to_array(plane.x_dir.into_inner());
        let mut y_axis = vec3_to_array(plane.y_dir.into_inner());
        if face.orientation == Orientation::Reversed {
            normal = negate_vec3(normal);
            y_axis = negate_vec3(y_axis);
        }

        Ok(Self {
            id: id.into(),
            name: name.into(),
            origin_mm: point3_to_array(plane.origin),
            normal,
            x_axis,
            y_axis,
        })
    }

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

fn parse_face_ref(face_ref: &str) -> CadResult<FaceId> {
    let raw = face_ref
        .strip_prefix("face.")
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("sketch face ref must use face.<id> format, found {face_ref}"),
        })?;
    let face_id_value = raw.parse::<u64>().map_err(|error| CadError::ParseFailed {
        reason: format!("sketch face ref {face_ref} has invalid id: {error}"),
    })?;
    if face_id_value == 0 {
        return Err(CadError::ParseFailed {
            reason: format!("sketch face ref {face_ref} must use id >= 1"),
        });
    }
    Ok(FaceId(face_id_value))
}

fn point3_to_array(point: crate::kernel_math::Point3) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn vec3_to_array(vector: crate::kernel_math::Vec3) -> [f64; 3] {
    [vector.x, vector.y, vector.z]
}

fn negate_vec3(vector: [f64; 3]) -> [f64; 3] {
    [-vector[0], -vector[1], -vector[2]]
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
    Rectangle {
        id: String,
        plane_id: String,
        min_mm: [f64; 2],
        max_mm: [f64; 2],
        anchor_ids: [String; 4],
        construction: bool,
    },
    Circle {
        id: String,
        plane_id: String,
        center_mm: [f64; 2],
        radius_mm: f64,
        anchor_ids: [String; 2],
        construction: bool,
    },
    Spline {
        id: String,
        plane_id: String,
        control_points_mm: Vec<[f64; 2]>,
        anchor_ids: Vec<String>,
        closed: bool,
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
            Self::Rectangle { id, .. } => id,
            Self::Circle { id, .. } => id,
            Self::Spline { id, .. } => id,
            Self::Point { id, .. } => id,
        }
    }

    pub fn plane_id(&self) -> &str {
        match self {
            Self::Line { plane_id, .. } => plane_id,
            Self::Arc { plane_id, .. } => plane_id,
            Self::Rectangle { plane_id, .. } => plane_id,
            Self::Circle { plane_id, .. } => plane_id,
            Self::Spline { plane_id, .. } => plane_id,
            Self::Point { plane_id, .. } => plane_id,
        }
    }

    pub fn anchor_ids(&self) -> Vec<&str> {
        match self {
            Self::Line { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Arc { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Rectangle { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Circle { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
            Self::Spline { anchor_ids, .. } => anchor_ids.iter().map(String::as_str).collect(),
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
            Self::Rectangle { min_mm, max_mm, .. } => {
                validate_vec2_finite(*min_mm, "rectangle min_mm")?;
                validate_vec2_finite(*max_mm, "rectangle max_mm")?;
                if max_mm[0] <= min_mm[0] || max_mm[1] <= min_mm[1] {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "rectangle {} requires max_mm strictly greater than min_mm",
                            self.id()
                        ),
                    });
                }
            }
            Self::Circle {
                center_mm,
                radius_mm,
                ..
            } => {
                validate_vec2_finite(*center_mm, "circle center_mm")?;
                if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("circle {} radius_mm must be finite and > 0", self.id()),
                    });
                }
            }
            Self::Spline {
                control_points_mm,
                anchor_ids,
                closed,
                ..
            } => {
                if control_points_mm.len() < 2 {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "spline {} requires at least two control points",
                            self.id()
                        ),
                    });
                }
                if *closed && control_points_mm.len() < 3 {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "closed spline {} requires at least three control points",
                            self.id()
                        ),
                    });
                }
                if control_points_mm.len() != anchor_ids.len() {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "spline {} control_points_mm count must match anchor_ids count",
                            self.id()
                        ),
                    });
                }
                for point in control_points_mm {
                    validate_vec2_finite(*point, "spline control_points_mm")?;
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
