use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kernel_math::Vec3;
use crate::{CadError, CadResult};

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

/// Motion result for fixed/revolute/slider joint evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CadJointMotion {
    Fixed {
        translation_mm: Vec3,
    },
    Revolute {
        translation_mm: Vec3,
        axis: Vec3,
        angle_deg: f64,
    },
    Slider {
        translation_mm: Vec3,
        axis: Vec3,
        offset_mm: f64,
    },
}

impl CadAssemblyJoint {
    pub fn solve_fixed_revolute_slider_motion(&self) -> CadResult<CadJointMotion> {
        let anchor_delta = vec3_sub(self.parent_anchor, self.child_anchor);
        match &self.kind {
            CadJointKind::Fixed => Ok(CadJointMotion::Fixed {
                translation_mm: anchor_delta,
            }),
            CadJointKind::Revolute { axis, .. } => Ok(CadJointMotion::Revolute {
                translation_mm: anchor_delta,
                axis: normalize_axis_or_z(*axis),
                angle_deg: self.state,
            }),
            CadJointKind::Slider { axis, .. } => {
                let normalized_axis = normalize_axis_or_z(*axis);
                let slider_offset = vec3_scale(normalized_axis, self.state);
                Ok(CadJointMotion::Slider {
                    translation_mm: vec3_add(anchor_delta, slider_offset),
                    axis: normalized_axis,
                    offset_mm: self.state,
                })
            }
            CadJointKind::Cylindrical { .. } | CadJointKind::Ball => Err(CadError::InvalidPolicy {
                reason: format!(
                    "joint {} is outside fixed/revolute/slider parity scope",
                    self.id
                ),
            }),
        }
    }
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

/// Resolved instance with effective part/material bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadResolvedPartInstance {
    pub instance_id: String,
    pub part_def_id: String,
    pub part_root: CadNodeId,
    pub name: Option<String>,
    pub effective_material: String,
    pub material_source: String,
    pub transform: Option<CadTransform3D>,
}

/// Resolution payload used by parity/eval layers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CadResolvedPartInstanceSet {
    pub resolved_instances: Vec<CadResolvedPartInstance>,
    pub unresolved_instance_ids: Vec<String>,
}

impl CadAssemblySchema {
    pub fn create_part_def(
        &mut self,
        id: impl Into<String>,
        root: CadNodeId,
        name: Option<String>,
        default_material: Option<String>,
    ) -> CadResult<String> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(CadError::InvalidParameter {
                name: "part_def.id".to_string(),
                reason: "part definition id must be non-empty".to_string(),
            });
        }
        if root == 0 {
            return Err(CadError::InvalidParameter {
                name: "part_def.root".to_string(),
                reason: "part definition root must be > 0".to_string(),
            });
        }
        if self.part_defs.contains_key(&id) {
            return Err(CadError::InvalidFeatureGraph {
                reason: format!("part definition already exists: {id}"),
            });
        }
        self.part_defs.insert(
            id.clone(),
            CadPartDef {
                id: id.clone(),
                name,
                root,
                default_material,
            },
        );
        Ok(id)
    }

    pub fn create_instance(
        &mut self,
        part_def_id: &str,
        name: Option<String>,
        transform: Option<CadTransform3D>,
    ) -> CadResult<String> {
        if !self.part_defs.contains_key(part_def_id) {
            return Err(CadError::InvalidParameter {
                name: "instance.part_def_id".to_string(),
                reason: format!("unknown part definition: {part_def_id}"),
            });
        }
        let id = self.next_instance_id(part_def_id);
        self.instances.push(CadPartInstance {
            id: id.clone(),
            part_def_id: part_def_id.to_string(),
            name,
            transform,
            material: None,
        });
        Ok(id)
    }

    pub fn rename_instance(&mut self, instance_id: &str, name: String) -> CadResult<()> {
        let instance = self.instance_mut(instance_id)?;
        instance.name = Some(name);
        Ok(())
    }

    pub fn set_instance_transform(
        &mut self,
        instance_id: &str,
        transform: CadTransform3D,
    ) -> CadResult<()> {
        let instance = self.instance_mut(instance_id)?;
        instance.transform = Some(transform);
        Ok(())
    }

    pub fn set_instance_material(
        &mut self,
        instance_id: &str,
        material: Option<String>,
    ) -> CadResult<()> {
        let instance = self.instance_mut(instance_id)?;
        instance.material = material;
        Ok(())
    }

    pub fn resolve_part_instances(&self) -> CadResolvedPartInstanceSet {
        let mut resolved_instances = Vec::new();
        let mut unresolved_instance_ids = Vec::new();

        for instance in &self.instances {
            let Some(part_def) = self.part_defs.get(&instance.part_def_id) else {
                unresolved_instance_ids.push(instance.id.clone());
                continue;
            };

            let (effective_material, material_source) = if let Some(material) = &instance.material {
                (material.clone(), "instance_override".to_string())
            } else if let Some(material) = &part_def.default_material {
                (material.clone(), "part_default".to_string())
            } else {
                ("default".to_string(), "fallback_default".to_string())
            };

            resolved_instances.push(CadResolvedPartInstance {
                instance_id: instance.id.clone(),
                part_def_id: instance.part_def_id.clone(),
                part_root: part_def.root,
                name: instance.name.clone(),
                effective_material,
                material_source,
                transform: instance.transform,
            });
        }

        resolved_instances.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
        unresolved_instance_ids.sort();
        unresolved_instance_ids.dedup();

        CadResolvedPartInstanceSet {
            resolved_instances,
            unresolved_instance_ids,
        }
    }

    fn next_instance_id(&self, part_def_id: &str) -> String {
        let mut next_index = 1_u64;
        loop {
            let candidate = format!("{part_def_id}-{next_index}");
            if self
                .instances
                .iter()
                .all(|instance| instance.id != candidate)
            {
                return candidate;
            }
            next_index = next_index.saturating_add(1);
        }
    }

    fn instance_mut(&mut self, instance_id: &str) -> CadResult<&mut CadPartInstance> {
        self.instances
            .iter_mut()
            .find(|instance| instance.id == instance_id)
            .ok_or_else(|| CadError::InvalidParameter {
                name: "instance.id".to_string(),
                reason: format!("unknown instance: {instance_id}"),
            })
    }
}

fn normalize_axis_or_z(axis: Vec3) -> Vec3 {
    axis.normalized().unwrap_or_else(Vec3::z)
}

fn vec3_add(left: Vec3, right: Vec3) -> Vec3 {
    Vec3::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

fn vec3_sub(left: Vec3, right: Vec3) -> Vec3 {
    Vec3::new(left.x - right.x, left.y - right.y, left.z - right.z)
}

fn vec3_scale(value: Vec3, scale: f64) -> Vec3 {
    Vec3::new(value.x * scale, value.y * scale, value.z * scale)
}

#[cfg(test)]
mod tests {
    use super::{
        CadAssemblyJoint, CadAssemblySchema, CadJointKind, CadJointMotion, CadPartDef,
        CadPartInstance, CadTransform3D,
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

    #[test]
    fn part_definition_and_instance_mutations_are_deterministic() {
        let mut schema = CadAssemblySchema::default();
        let base_id = schema
            .create_part_def(
                "base",
                1,
                Some("Base".to_string()),
                Some("powder_coat".to_string()),
            )
            .expect("create base part def");
        let arm_id = schema
            .create_part_def("arm", 2, Some("Arm".to_string()), Some("steel".to_string()))
            .expect("create arm part def");
        assert_eq!(base_id, "base");
        assert_eq!(arm_id, "arm");

        let arm_one = schema
            .create_instance("arm", Some("Arm One".to_string()), None)
            .expect("create arm instance one");
        let arm_two = schema
            .create_instance("arm", None, None)
            .expect("create arm instance two");
        let base_one = schema
            .create_instance("base", Some("Base One".to_string()), None)
            .expect("create base instance");
        assert_eq!(arm_one, "arm-1");
        assert_eq!(arm_two, "arm-2");
        assert_eq!(base_one, "base-1");

        schema
            .rename_instance("arm-2", "Arm Two".to_string())
            .expect("rename arm-2");
        schema
            .set_instance_transform(
                "arm-2",
                CadTransform3D {
                    translation: Vec3::new(5.0, 0.0, 0.0),
                    rotation: Vec3::new(0.0, 0.0, 45.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                },
            )
            .expect("set arm-2 transform");
        schema
            .set_instance_material("arm-2", Some("anodized".to_string()))
            .expect("set arm-2 material");
        schema
            .set_instance_material("base-1", None)
            .expect("clear base-1 material");

        // Simulate an externally loaded invalid instance to mirror vcad evaluate skip behavior.
        schema.instances.push(CadPartInstance {
            id: "orphan-1".to_string(),
            part_def_id: "missing".to_string(),
            name: None,
            transform: None,
            material: None,
        });

        let resolved = schema.resolve_part_instances();
        assert_eq!(
            resolved.unresolved_instance_ids,
            vec!["orphan-1".to_string()]
        );
        assert_eq!(resolved.resolved_instances.len(), 3);

        let arm_one = resolved
            .resolved_instances
            .iter()
            .find(|instance| instance.instance_id == "arm-1")
            .expect("arm-1 resolved");
        assert_eq!(arm_one.effective_material, "steel");
        assert_eq!(arm_one.material_source, "part_default");

        let arm_two = resolved
            .resolved_instances
            .iter()
            .find(|instance| instance.instance_id == "arm-2")
            .expect("arm-2 resolved");
        assert_eq!(arm_two.effective_material, "anodized");
        assert_eq!(arm_two.material_source, "instance_override");

        let base_one = resolved
            .resolved_instances
            .iter()
            .find(|instance| instance.instance_id == "base-1")
            .expect("base-1 resolved");
        assert_eq!(base_one.effective_material, "powder_coat");
        assert_eq!(base_one.material_source, "part_default");
    }

    #[test]
    fn instance_creation_requires_known_part_definition() {
        let mut schema = CadAssemblySchema::default();
        let error = schema
            .create_instance("missing", None, None)
            .expect_err("unknown part def should fail");
        assert!(
            error
                .to_string()
                .contains("unknown part definition: missing")
        );
    }

    #[test]
    fn part_definition_creation_rejects_duplicate_ids() {
        let mut schema = CadAssemblySchema::default();
        schema
            .create_part_def("base", 1, None, None)
            .expect("create base part def");
        let error = schema
            .create_part_def("base", 2, None, None)
            .expect_err("duplicate part def should fail");
        assert!(error.to_string().contains("part definition already exists"));
    }

    #[test]
    fn fixed_joint_motion_uses_anchor_delta() {
        let joint = CadAssemblyJoint {
            id: "joint.fixed.001".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(10.0, 5.0, 5.0),
            child_anchor: Vec3::new(0.0, 5.0, 5.0),
            kind: CadJointKind::Fixed,
            state: 0.0,
        };

        let motion = joint
            .solve_fixed_revolute_slider_motion()
            .expect("fixed motion");
        assert_eq!(
            motion,
            CadJointMotion::Fixed {
                translation_mm: Vec3::new(10.0, 0.0, 0.0),
            }
        );
    }

    #[test]
    fn revolute_joint_motion_normalizes_axis_with_fallback() {
        let joint = CadAssemblyJoint {
            id: "joint.revolute.001".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(10.0, 5.0, 5.0),
            child_anchor: Vec3::new(0.0, 5.0, 5.0),
            kind: CadJointKind::Revolute {
                axis: Vec3::new(0.0, 0.0, 0.0),
                limits: None,
            },
            state: 45.0,
        };

        let motion = joint
            .solve_fixed_revolute_slider_motion()
            .expect("revolute motion");
        assert_eq!(
            motion,
            CadJointMotion::Revolute {
                translation_mm: Vec3::new(10.0, 0.0, 0.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
                angle_deg: 45.0,
            }
        );
    }

    #[test]
    fn slider_joint_motion_applies_axis_displacement() {
        let joint = CadAssemblyJoint {
            id: "joint.slider.001".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(5.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Slider {
                axis: Vec3::new(1.0, 0.0, 0.0),
                limits: None,
            },
            state: 12.5,
        };

        let motion = joint
            .solve_fixed_revolute_slider_motion()
            .expect("slider motion");
        assert_eq!(
            motion,
            CadJointMotion::Slider {
                translation_mm: Vec3::new(17.5, 0.0, 0.0),
                axis: Vec3::new(1.0, 0.0, 0.0),
                offset_mm: 12.5,
            }
        );
    }

    #[test]
    fn cylindrical_joint_is_out_of_scope_for_frs_lane() {
        let joint = CadAssemblyJoint {
            id: "joint.cylindrical.001".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Cylindrical {
                axis: Vec3::new(0.0, 0.0, 1.0),
            },
            state: 0.0,
        };

        let error = joint
            .solve_fixed_revolute_slider_motion()
            .expect_err("cylindrical should be out of scope");
        assert!(
            error
                .to_string()
                .contains("outside fixed/revolute/slider parity scope")
        );
    }
}
