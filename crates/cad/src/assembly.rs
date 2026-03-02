use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kernel_math::Vec3;

/// Stable node id type used by assembly part definitions.
pub type CadNodeId = u64;

/// Joint limits are encoded as `[min, max]`.
pub type CadJointLimits = (f64, f64);

/// Simple 3D transform payload aligned with vcad assembly schemas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CadTransform3D {
    pub translation: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl Default for CadTransform3D {
    fn default() -> Self {
        Self {
            translation: Vec3::new(0.0, 0.0, 0.0),
            rotation: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

impl CadTransform3D {
    pub fn identity() -> Self {
        Self::default()
    }
}

/// Assembly joint variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CadJointKind {
    Fixed,
    Revolute {
        axis: Vec3,
        #[serde(skip_serializing_if = "Option::is_none")]
        limits: Option<CadJointLimits>,
    },
    Slider {
        axis: Vec3,
        #[serde(skip_serializing_if = "Option::is_none")]
        limits: Option<CadJointLimits>,
    },
    Cylindrical {
        axis: Vec3,
    },
    Ball,
}

/// Reusable part definition for assemblies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadPartDef {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub root: CadNodeId,
    #[serde(rename = "defaultMaterial", skip_serializing_if = "Option::is_none")]
    pub default_material: Option<String>,
}

/// Placed part instance with optional transform/material overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadPartInstance {
    pub id: String,
    #[serde(rename = "partDefId")]
    pub part_def_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<CadTransform3D>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
}

/// Alias matching vcad naming used in some callers.
pub type CadInstance = CadPartInstance;

/// Joint instance connecting two part instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadAssemblyJoint {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "parentInstanceId")]
    pub parent_instance_id: Option<String>,
    #[serde(rename = "childInstanceId")]
    pub child_instance_id: String,
    #[serde(rename = "parentAnchor")]
    pub parent_anchor: Vec3,
    #[serde(rename = "childAnchor")]
    pub child_anchor: Vec3,
    pub kind: CadJointKind,
    pub state: f64,
}

/// Assembly payload attached to a CAD document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CadAssemblySchema {
    #[serde(rename = "partDefs", skip_serializing_if = "BTreeMap::is_empty")]
    pub part_defs: BTreeMap<String, CadPartDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<CadPartInstance>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<CadAssemblyJoint>,
    #[serde(rename = "groundInstanceId", skip_serializing_if = "Option::is_none")]
    pub ground_instance_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        CadAssemblyJoint, CadAssemblySchema, CadJointKind, CadPartDef, CadPartInstance,
        CadTransform3D,
    };
    use crate::kernel_math::Vec3;
    use std::collections::BTreeMap;

    #[test]
    fn transform_identity_defaults_are_stable() {
        let transform = CadTransform3D::identity();
        assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(transform.rotation, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(transform.scale, Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn joint_kind_serialization_matches_vcad_tags() {
        let fixed = serde_json::to_string(&CadJointKind::Fixed).expect("serialize fixed");
        assert!(fixed.contains("\"type\":\"Fixed\""));

        let revolute = serde_json::to_string(&CadJointKind::Revolute {
            axis: Vec3::new(0.0, 0.0, 1.0),
            limits: Some((-180.0, 180.0)),
        })
        .expect("serialize revolute");
        assert!(revolute.contains("\"type\":\"Revolute\""));
        assert!(revolute.contains("\"limits\""));

        let slider = serde_json::to_string(&CadJointKind::Slider {
            axis: Vec3::new(1.0, 0.0, 0.0),
            limits: None,
        })
        .expect("serialize slider");
        assert!(slider.contains("\"type\":\"Slider\""));
        assert!(!slider.contains("\"limits\""));
    }

    #[test]
    fn assembly_schema_round_trip_preserves_camel_case_fields() {
        let mut part_defs = BTreeMap::new();
        part_defs.insert(
            "base".to_string(),
            CadPartDef {
                id: "base".to_string(),
                name: Some("Base".to_string()),
                root: 11,
                default_material: Some("aluminum".to_string()),
            },
        );
        part_defs.insert(
            "arm".to_string(),
            CadPartDef {
                id: "arm".to_string(),
                name: Some("Arm".to_string()),
                root: 22,
                default_material: None,
            },
        );

        let schema = CadAssemblySchema {
            part_defs,
            instances: vec![
                CadPartInstance {
                    id: "base_inst".to_string(),
                    part_def_id: "base".to_string(),
                    name: Some("Ground Base".to_string()),
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "arm_inst".to_string(),
                    part_def_id: "arm".to_string(),
                    name: Some("Arm Instance".to_string()),
                    transform: Some(CadTransform3D {
                        translation: Vec3::new(0.0, 0.0, 10.0),
                        rotation: Vec3::new(0.0, 0.0, 0.0),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: Some("steel".to_string()),
                },
            ],
            joints: vec![CadAssemblyJoint {
                id: "joint.revolute.001".to_string(),
                name: Some("Base-Arm".to_string()),
                parent_instance_id: Some("base_inst".to_string()),
                child_instance_id: "arm_inst".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 10.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Revolute {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    limits: Some((-90.0, 90.0)),
                },
                state: 0.0,
            }],
            ground_instance_id: Some("base_inst".to_string()),
        };

        let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
        let restored: CadAssemblySchema = serde_json::from_str(&json).expect("parse schema");
        assert_eq!(restored, schema);
        assert!(json.contains("\"partDefs\""));
        assert!(json.contains("\"partDefId\""));
        assert!(json.contains("\"parentInstanceId\""));
        assert!(json.contains("\"childInstanceId\""));
        assert!(json.contains("\"parentAnchor\""));
        assert!(json.contains("\"childAnchor\""));
        assert!(json.contains("\"groundInstanceId\""));
        assert!(json.contains("\"defaultMaterial\""));
    }
}
