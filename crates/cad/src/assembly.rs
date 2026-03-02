use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

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

/// Motion result for assembly joint evaluation.
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
    Cylindrical {
        translation_mm: Vec3,
        axis: Vec3,
        angle_deg: f64,
    },
    Ball {
        translation_mm: Vec3,
        axis: Vec3,
        angle_deg: f64,
    },
}

/// Joint state semantic resolution for deterministic state updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadJointStateSemantics {
    pub requested_state: f64,
    pub effective_state: f64,
    pub limits: Option<CadJointLimits>,
    pub was_clamped: bool,
    pub physics_state: f64,
    pub dof: usize,
    pub state_unit: String,
}

impl CadAssemblyJoint {
    pub fn joint_dof(&self) -> usize {
        match self.kind {
            CadJointKind::Fixed => 0,
            CadJointKind::Revolute { .. }
            | CadJointKind::Slider { .. }
            | CadJointKind::Cylindrical { .. } => 1,
            CadJointKind::Ball => 3,
        }
    }

    pub fn joint_limits(&self) -> Option<CadJointLimits> {
        match self.kind {
            CadJointKind::Revolute { limits, .. } | CadJointKind::Slider { limits, .. } => limits,
            CadJointKind::Fixed | CadJointKind::Cylindrical { .. } | CadJointKind::Ball => None,
        }
    }

    pub fn convert_state_to_physics_units(&self, state: f64) -> f64 {
        match self.kind {
            CadJointKind::Revolute { .. }
            | CadJointKind::Cylindrical { .. }
            | CadJointKind::Ball => state.to_radians(),
            CadJointKind::Slider { .. } => state / 1000.0,
            CadJointKind::Fixed => 0.0,
        }
    }

    pub fn convert_state_from_physics_units(&self, physics_state: f64) -> f64 {
        match self.kind {
            CadJointKind::Revolute { .. }
            | CadJointKind::Cylindrical { .. }
            | CadJointKind::Ball => physics_state.to_degrees(),
            CadJointKind::Slider { .. } => physics_state * 1000.0,
            CadJointKind::Fixed => 0.0,
        }
    }

    pub fn resolve_state_semantics(&self, requested_state: f64) -> CadJointStateSemantics {
        let limits = self.joint_limits().map(normalize_limits);
        let unclamped_state = match self.kind {
            CadJointKind::Fixed => 0.0,
            CadJointKind::Revolute { .. }
            | CadJointKind::Slider { .. }
            | CadJointKind::Cylindrical { .. }
            | CadJointKind::Ball => requested_state,
        };
        let effective_state = limits
            .map(|(lower, upper)| unclamped_state.clamp(lower, upper))
            .unwrap_or(unclamped_state);
        let was_clamped = (effective_state - unclamped_state).abs() > 1e-12;

        CadJointStateSemantics {
            requested_state,
            effective_state,
            limits,
            was_clamped,
            physics_state: self.convert_state_to_physics_units(effective_state),
            dof: self.joint_dof(),
            state_unit: self.state_unit_label().to_string(),
        }
    }

    pub fn set_state_with_limits(&mut self, requested_state: f64) -> CadJointStateSemantics {
        let semantics = self.resolve_state_semantics(requested_state);
        self.state = semantics.effective_state;
        semantics
    }

    fn state_unit_label(&self) -> &'static str {
        match self.kind {
            CadJointKind::Slider { .. } => "mm",
            CadJointKind::Fixed => "fixed",
            CadJointKind::Revolute { .. }
            | CadJointKind::Cylindrical { .. }
            | CadJointKind::Ball => "deg",
        }
    }

    pub fn solve_motion(&self) -> CadJointMotion {
        let anchor_delta = vec3_sub(self.parent_anchor, self.child_anchor);
        match &self.kind {
            CadJointKind::Fixed => CadJointMotion::Fixed {
                translation_mm: clean_vec3(anchor_delta),
            },
            CadJointKind::Revolute { axis, .. } => {
                let normalized_axis = normalize_axis_or_z(*axis);
                let rotated_child =
                    axis_angle_rotate(normalized_axis, self.state, self.child_anchor);
                CadJointMotion::Revolute {
                    translation_mm: clean_vec3(vec3_sub(self.parent_anchor, rotated_child)),
                    axis: clean_vec3(normalized_axis),
                    angle_deg: self.state,
                }
            }
            CadJointKind::Slider { axis, .. } => {
                let normalized_axis = normalize_axis_or_z(*axis);
                let slider_offset = vec3_scale(normalized_axis, self.state);
                CadJointMotion::Slider {
                    translation_mm: clean_vec3(vec3_add(anchor_delta, slider_offset)),
                    axis: clean_vec3(normalized_axis),
                    offset_mm: self.state,
                }
            }
            CadJointKind::Cylindrical { axis } => {
                let normalized_axis = normalize_axis_or_z(*axis);
                let rotated_child =
                    axis_angle_rotate(normalized_axis, self.state, self.child_anchor);
                CadJointMotion::Cylindrical {
                    translation_mm: clean_vec3(vec3_sub(self.parent_anchor, rotated_child)),
                    axis: clean_vec3(normalized_axis),
                    angle_deg: self.state,
                }
            }
            CadJointKind::Ball => {
                let z_axis = Vec3::z();
                let rotated_child = axis_angle_rotate(z_axis, self.state, self.child_anchor);
                CadJointMotion::Ball {
                    translation_mm: clean_vec3(vec3_sub(self.parent_anchor, rotated_child)),
                    axis: clean_vec3(z_axis),
                    angle_deg: self.state,
                }
            }
        }
    }

    pub fn solve_fixed_revolute_slider_motion(&self) -> CadResult<CadJointMotion> {
        match self.solve_motion() {
            motion @ CadJointMotion::Fixed { .. }
            | motion @ CadJointMotion::Revolute { .. }
            | motion @ CadJointMotion::Slider { .. } => Ok(motion),
            CadJointMotion::Cylindrical { .. } | CadJointMotion::Ball { .. } => {
                Err(CadError::InvalidPolicy {
                    reason: format!(
                        "joint {} is outside fixed/revolute/slider parity scope",
                        self.id
                    ),
                })
            }
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

/// Result payload for deleting an instance with joint cleanup.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CadInstanceDeletionSummary {
    pub deleted_instance_id: String,
    pub removed_joint_ids: Vec<String>,
    pub cleared_ground: bool,
}

/// Assembly pane selection/editing state used by UI layers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CadAssemblyUiState {
    pub selected_instance_id: Option<String>,
    pub selected_joint_id: Option<String>,
    pub last_error: Option<String>,
}

impl CadAssemblySchema {
    pub fn to_json(&self) -> CadResult<String> {
        serde_json::to_string(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadAssemblySchema json: {error}"),
        })
    }

    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadAssemblySchema pretty json: {error}"),
        })
    }

    pub fn from_json(payload: &str) -> CadResult<Self> {
        serde_json::from_str(payload).map_err(|error| CadError::Serialization {
            reason: format!("failed to parse CadAssemblySchema json: {error}"),
        })
    }

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

    pub fn solve_forward_kinematics(&self) -> BTreeMap<String, CadTransform3D> {
        let mut results = BTreeMap::new();
        if self.instances.is_empty() {
            return results;
        }

        let mut joint_tree: HashMap<&str, (&CadAssemblyJoint, Option<&str>)> = HashMap::new();
        let mut children_by_parent: HashMap<Option<&str>, Vec<&str>> = HashMap::new();
        children_by_parent.insert(None, Vec::new());

        for joint in &self.joints {
            let parent_id = joint.parent_instance_id.as_deref();
            joint_tree.insert(&joint.child_instance_id, (joint, parent_id));
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(&joint.child_instance_id);
        }

        let child_ids: HashSet<&str> = self
            .joints
            .iter()
            .map(|joint| joint.child_instance_id.as_str())
            .collect();
        let root_instances: Vec<&CadPartInstance> = self
            .instances
            .iter()
            .filter(|instance| !child_ids.contains(instance.id.as_str()))
            .collect();
        for instance in &root_instances {
            results.insert(
                instance.id.clone(),
                clean_transform(instance.transform.unwrap_or_else(CadTransform3D::identity)),
            );
        }

        let mut queue: VecDeque<Option<&str>> = VecDeque::new();
        queue.push_back(None);
        for instance in &root_instances {
            queue.push_back(Some(&instance.id));
        }

        let mut visited: HashSet<Option<&str>> = HashSet::new();
        visited.insert(None);

        let instance_by_id: HashMap<&str, &CadPartInstance> = self
            .instances
            .iter()
            .map(|instance| (instance.id.as_str(), instance))
            .collect();

        while let Some(parent_id) = queue.pop_front() {
            let children = children_by_parent
                .get(&parent_id)
                .cloned()
                .unwrap_or_default();

            for child_id in children {
                if visited.contains(&Some(child_id)) {
                    continue;
                }
                visited.insert(Some(child_id));

                let Some((joint, edge_parent_id)) = joint_tree.get(child_id) else {
                    continue;
                };
                let Some(instance) = instance_by_id.get(child_id) else {
                    continue;
                };

                let parent_world = edge_parent_id
                    .and_then(|parent| results.get(parent))
                    .copied()
                    .unwrap_or_else(CadTransform3D::identity);
                let joint_transform = compute_joint_fk_transform(joint);
                let local_transform = instance.transform.unwrap_or_else(CadTransform3D::identity);
                let jointed = compose_transforms(joint_transform, local_transform);
                let world = compose_transforms(parent_world, jointed);

                results.insert(child_id.to_string(), clean_transform(world));
                queue.push_back(Some(child_id));
            }
        }

        results
    }

    pub fn set_ground_instance(&mut self, instance_id: &str) -> CadResult<()> {
        if !self
            .instances
            .iter()
            .any(|instance| instance.id == instance_id)
        {
            return Err(CadError::InvalidParameter {
                name: "ground_instance_id".to_string(),
                reason: format!("unknown instance: {instance_id}"),
            });
        }
        self.ground_instance_id = Some(instance_id.to_string());
        Ok(())
    }

    pub fn delete_joint(&mut self, joint_id: &str) -> CadResult<()> {
        let Some(index) = self.joints.iter().position(|joint| joint.id == joint_id) else {
            return Err(CadError::InvalidParameter {
                name: "joint.id".to_string(),
                reason: format!("unknown joint: {joint_id}"),
            });
        };
        self.joints.remove(index);
        Ok(())
    }

    pub fn delete_instance(&mut self, instance_id: &str) -> CadResult<CadInstanceDeletionSummary> {
        let Some(instance_index) = self
            .instances
            .iter()
            .position(|instance| instance.id == instance_id)
        else {
            return Err(CadError::InvalidParameter {
                name: "instance.id".to_string(),
                reason: format!("unknown instance: {instance_id}"),
            });
        };

        self.instances.remove(instance_index);

        let mut removed_joint_ids: Vec<String> = self
            .joints
            .iter()
            .filter(|joint| {
                joint.child_instance_id == instance_id
                    || joint
                        .parent_instance_id
                        .as_deref()
                        .is_some_and(|parent_id| parent_id == instance_id)
            })
            .map(|joint| joint.id.clone())
            .collect();
        self.joints.retain(|joint| {
            joint.child_instance_id != instance_id
                && joint
                    .parent_instance_id
                    .as_deref()
                    .map_or(true, |parent_id| parent_id != instance_id)
        });
        removed_joint_ids.sort();
        removed_joint_ids.dedup();

        let cleared_ground = self
            .ground_instance_id
            .as_deref()
            .is_some_and(|ground_id| ground_id == instance_id);
        if cleared_ground {
            self.ground_instance_id = None;
        }

        Ok(CadInstanceDeletionSummary {
            deleted_instance_id: instance_id.to_string(),
            removed_joint_ids,
            cleared_ground,
        })
    }

    pub fn set_joint_state_with_limits(
        &mut self,
        joint_id: &str,
        requested_state: f64,
    ) -> CadResult<CadJointStateSemantics> {
        let joint = self.joint_mut(joint_id)?;
        Ok(joint.set_state_with_limits(requested_state))
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

    fn joint_mut(&mut self, joint_id: &str) -> CadResult<&mut CadAssemblyJoint> {
        self.joints
            .iter_mut()
            .find(|joint| joint.id == joint_id)
            .ok_or_else(|| CadError::InvalidParameter {
                name: "joint.id".to_string(),
                reason: format!("unknown joint: {joint_id}"),
            })
    }
}

impl CadAssemblyUiState {
    pub fn to_json(&self) -> CadResult<String> {
        serde_json::to_string(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadAssemblyUiState json: {error}"),
        })
    }

    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize CadAssemblyUiState pretty json: {error}"),
        })
    }

    pub fn from_json(payload: &str) -> CadResult<Self> {
        serde_json::from_str(payload).map_err(|error| CadError::Serialization {
            reason: format!("failed to parse CadAssemblyUiState json: {error}"),
        })
    }

    pub fn select_instance(
        &mut self,
        schema: &CadAssemblySchema,
        instance_id: &str,
    ) -> CadResult<()> {
        if !schema
            .instances
            .iter()
            .any(|instance| instance.id == instance_id)
        {
            return Err(CadError::InvalidParameter {
                name: "instance.id".to_string(),
                reason: format!("unknown instance: {instance_id}"),
            });
        }
        self.selected_instance_id = Some(instance_id.to_string());
        self.last_error = None;
        Ok(())
    }

    pub fn select_joint(&mut self, schema: &CadAssemblySchema, joint_id: &str) -> CadResult<()> {
        if !schema.joints.iter().any(|joint| joint.id == joint_id) {
            return Err(CadError::InvalidParameter {
                name: "joint.id".to_string(),
                reason: format!("unknown joint: {joint_id}"),
            });
        }
        self.selected_joint_id = Some(joint_id.to_string());
        self.last_error = None;
        Ok(())
    }

    pub fn rename_selected_instance(
        &mut self,
        schema: &mut CadAssemblySchema,
        name: String,
    ) -> CadResult<()> {
        let Some(instance_id) = self.selected_instance_id.as_deref() else {
            return Err(CadError::InvalidParameter {
                name: "ui.selected_instance_id".to_string(),
                reason: "no selected assembly instance".to_string(),
            });
        };
        schema.rename_instance(instance_id, name)?;
        self.last_error = None;
        Ok(())
    }

    pub fn set_selected_joint_state(
        &mut self,
        schema: &mut CadAssemblySchema,
        requested_state: f64,
    ) -> CadResult<CadJointStateSemantics> {
        let Some(joint_id) = self.selected_joint_id.as_deref() else {
            return Err(CadError::InvalidParameter {
                name: "ui.selected_joint_id".to_string(),
                reason: "no selected assembly joint".to_string(),
            });
        };
        let semantics = schema.set_joint_state_with_limits(joint_id, requested_state)?;
        self.last_error = None;
        Ok(semantics)
    }

    pub fn sync_with_schema(&mut self, schema: &CadAssemblySchema) {
        if self
            .selected_instance_id
            .as_deref()
            .is_some_and(|instance_id| {
                !schema
                    .instances
                    .iter()
                    .any(|instance| instance.id == instance_id)
            })
        {
            self.selected_instance_id = None;
        }
        if self
            .selected_joint_id
            .as_deref()
            .is_some_and(|joint_id| !schema.joints.iter().any(|joint| joint.id == joint_id))
        {
            self.selected_joint_id = None;
        }
    }
}

fn normalize_axis_or_z(axis: Vec3) -> Vec3 {
    axis.normalized().unwrap_or_else(Vec3::z)
}

fn deg_to_rad(value_deg: f64) -> f64 {
    value_deg * std::f64::consts::PI / 180.0
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

fn axis_angle_rotate(axis: Vec3, angle_deg: f64, value: Vec3) -> Vec3 {
    let axis = normalize_axis_or_z(axis);
    let angle = deg_to_rad(angle_deg);
    let c = angle.cos();
    let s = angle.sin();

    let first = vec3_scale(value, c);
    let second = vec3_scale(axis.cross(value), s);
    let third = vec3_scale(axis, axis.dot(value) * (1.0 - c));
    clean_vec3(vec3_add(vec3_add(first, second), third))
}

fn clean_vec3(value: Vec3) -> Vec3 {
    Vec3::new(
        clean_scalar(value.x),
        clean_scalar(value.y),
        clean_scalar(value.z),
    )
}

fn clean_scalar(value: f64) -> f64 {
    if value.abs() < 1e-12 { 0.0 } else { value }
}

fn normalize_limits(limits: CadJointLimits) -> CadJointLimits {
    let (lower, upper) = limits;
    if lower <= upper {
        (lower, upper)
    } else {
        (upper, lower)
    }
}

type Mat3 = [[f64; 3]; 3];

fn euler_to_matrix(angles: Vec3) -> Mat3 {
    let rx = deg_to_rad(angles.x);
    let ry = deg_to_rad(angles.y);
    let rz = deg_to_rad(angles.z);

    let (cx, sx) = (rx.cos(), rx.sin());
    let (cy, sy) = (ry.cos(), ry.sin());
    let (cz, sz) = (rz.cos(), rz.sin());

    [
        [cy * cz, sx * sy * cz - cx * sz, cx * sy * cz + sx * sz],
        [cy * sz, sx * sy * sz + cx * cz, cx * sy * sz - sx * cz],
        [-sy, sx * cy, cx * cy],
    ]
}

fn mat_vec3(matrix: &Mat3, value: Vec3) -> Vec3 {
    Vec3::new(
        matrix[0][0] * value.x + matrix[0][1] * value.y + matrix[0][2] * value.z,
        matrix[1][0] * value.x + matrix[1][1] * value.y + matrix[1][2] * value.z,
        matrix[2][0] * value.x + matrix[2][1] * value.y + matrix[2][2] * value.z,
    )
}

fn mat_mul(left: &Mat3, right: &Mat3) -> Mat3 {
    let mut result = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            result[i][j] =
                left[i][0] * right[0][j] + left[i][1] * right[1][j] + left[i][2] * right[2][j];
        }
    }
    result
}

fn matrix_to_euler(matrix: &Mat3) -> Vec3 {
    let sy = -matrix[2][0];
    let cy = (matrix[0][0] * matrix[0][0] + matrix[1][0] * matrix[1][0]).sqrt();

    if cy > 1e-6 {
        Vec3::new(
            matrix[2][1].atan2(matrix[2][2]) * 180.0 / std::f64::consts::PI,
            sy.atan2(cy) * 180.0 / std::f64::consts::PI,
            matrix[1][0].atan2(matrix[0][0]) * 180.0 / std::f64::consts::PI,
        )
    } else {
        Vec3::new(
            (-matrix[1][2]).atan2(matrix[1][1]) * 180.0 / std::f64::consts::PI,
            sy.atan2(cy) * 180.0 / std::f64::consts::PI,
            0.0,
        )
    }
}

fn axis_angle_to_matrix(axis: Vec3, angle_deg: f64) -> Mat3 {
    let normalized_axis = normalize_axis_or_z(axis);
    let angle = deg_to_rad(angle_deg);
    let c = angle.cos();
    let s = angle.sin();
    let t = 1.0 - c;
    let (x, y, z) = (normalized_axis.x, normalized_axis.y, normalized_axis.z);

    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

fn compose_transforms(outer: CadTransform3D, inner: CadTransform3D) -> CadTransform3D {
    let scale = Vec3::new(
        outer.scale.x * inner.scale.x,
        outer.scale.y * inner.scale.y,
        outer.scale.z * inner.scale.z,
    );

    let outer_rot = euler_to_matrix(outer.rotation);
    let inner_rot = euler_to_matrix(inner.rotation);
    let composed_rot = mat_mul(&outer_rot, &inner_rot);
    let rotation = matrix_to_euler(&composed_rot);

    let scaled_inner_trans = Vec3::new(
        outer.scale.x * inner.translation.x,
        outer.scale.y * inner.translation.y,
        outer.scale.z * inner.translation.z,
    );
    let rotated_inner_trans = mat_vec3(&outer_rot, scaled_inner_trans);
    let translation = vec3_add(outer.translation, rotated_inner_trans);

    clean_transform(CadTransform3D {
        translation,
        rotation,
        scale,
    })
}

fn compute_joint_fk_transform(joint: &CadAssemblyJoint) -> CadTransform3D {
    match &joint.kind {
        CadJointKind::Fixed => CadTransform3D {
            translation: clean_vec3(vec3_sub(joint.parent_anchor, joint.child_anchor)),
            rotation: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        },
        CadJointKind::Revolute { axis, .. } => {
            let normalized_axis = normalize_axis_or_z(*axis);
            let rotation_matrix = axis_angle_to_matrix(normalized_axis, joint.state);
            let rotation = clean_vec3(matrix_to_euler(&rotation_matrix));
            let rotated_child = mat_vec3(&rotation_matrix, joint.child_anchor);
            let translation = clean_vec3(vec3_sub(joint.parent_anchor, rotated_child));
            CadTransform3D {
                translation,
                rotation,
                scale: Vec3::new(1.0, 1.0, 1.0),
            }
        }
        CadJointKind::Slider { axis, .. } => {
            let normalized_axis = normalize_axis_or_z(*axis);
            let slide_offset = vec3_scale(normalized_axis, joint.state);
            CadTransform3D {
                translation: clean_vec3(vec3_add(
                    vec3_sub(joint.parent_anchor, joint.child_anchor),
                    slide_offset,
                )),
                rotation: Vec3::new(0.0, 0.0, 0.0),
                scale: Vec3::new(1.0, 1.0, 1.0),
            }
        }
        CadJointKind::Cylindrical { axis } => {
            let normalized_axis = normalize_axis_or_z(*axis);
            let rotation_matrix = axis_angle_to_matrix(normalized_axis, joint.state);
            let rotation = clean_vec3(matrix_to_euler(&rotation_matrix));
            let rotated_child = mat_vec3(&rotation_matrix, joint.child_anchor);
            let translation = clean_vec3(vec3_sub(joint.parent_anchor, rotated_child));
            CadTransform3D {
                translation,
                rotation,
                scale: Vec3::new(1.0, 1.0, 1.0),
            }
        }
        CadJointKind::Ball => {
            let z_axis = Vec3::z();
            let rotation_matrix = axis_angle_to_matrix(z_axis, joint.state);
            let rotation = clean_vec3(matrix_to_euler(&rotation_matrix));
            let rotated_child = mat_vec3(&rotation_matrix, joint.child_anchor);
            let translation = clean_vec3(vec3_sub(joint.parent_anchor, rotated_child));
            CadTransform3D {
                translation,
                rotation,
                scale: Vec3::new(1.0, 1.0, 1.0),
            }
        }
    }
}

fn clean_transform(transform: CadTransform3D) -> CadTransform3D {
    CadTransform3D {
        translation: clean_vec3(transform.translation),
        rotation: clean_vec3(transform.rotation),
        scale: clean_vec3(transform.scale),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadAssemblyJoint, CadAssemblySchema, CadAssemblyUiState, CadJointKind, CadJointMotion,
        CadJointStateSemantics, CadPartDef, CadPartInstance, CadTransform3D,
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
    fn assembly_schema_json_helpers_round_trip_is_deterministic() {
        let schema = CadAssemblySchema {
            part_defs: BTreeMap::from([
                (
                    "base".to_string(),
                    CadPartDef {
                        id: "base".to_string(),
                        name: Some("Base".to_string()),
                        root: 1,
                        default_material: Some("aluminum".to_string()),
                    },
                ),
                (
                    "arm".to_string(),
                    CadPartDef {
                        id: "arm".to_string(),
                        name: Some("Arm".to_string()),
                        root: 2,
                        default_material: None,
                    },
                ),
            ]),
            instances: vec![
                CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: Some("Base".to_string()),
                    transform: Some(CadTransform3D::identity()),
                    material: None,
                },
                CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: Some("Arm".to_string()),
                    transform: Some(CadTransform3D {
                        translation: Vec3::new(10.0, 0.0, 0.0),
                        rotation: Vec3::new(0.0, 0.0, 0.0),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: Some("steel".to_string()),
                },
            ],
            joints: vec![CadAssemblyJoint {
                id: "joint.hinge".to_string(),
                name: Some("Hinge".to_string()),
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Revolute {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    limits: Some((-90.0, 90.0)),
                },
                state: 0.0,
            }],
            ground_instance_id: Some("base-1".to_string()),
        };

        let compact = schema.to_json().expect("serialize compact schema");
        let pretty = schema.to_pretty_json().expect("serialize pretty schema");
        let compact_restored =
            CadAssemblySchema::from_json(&compact).expect("parse compact schema");
        let pretty_restored = CadAssemblySchema::from_json(&pretty).expect("parse pretty schema");

        assert_eq!(compact_restored, schema);
        assert_eq!(pretty_restored, schema);
        assert!(compact.contains("\"partDefs\""));
        assert!(compact.contains("\"groundInstanceId\""));
    }

    #[test]
    fn assembly_ui_state_json_helpers_round_trip_is_deterministic() {
        let ui = CadAssemblyUiState {
            selected_instance_id: Some("arm-1".to_string()),
            selected_joint_id: Some("joint.hinge".to_string()),
            last_error: Some(
                "invalid parameter instance.id: unknown instance: missing".to_string(),
            ),
        };

        let compact = ui.to_json().expect("serialize compact ui state");
        let pretty = ui.to_pretty_json().expect("serialize pretty ui state");
        let compact_restored = CadAssemblyUiState::from_json(&compact).expect("parse compact ui");
        let pretty_restored = CadAssemblyUiState::from_json(&pretty).expect("parse pretty ui");

        assert_eq!(compact_restored, ui);
        assert_eq!(pretty_restored, ui);
        assert!(compact.contains("\"selected_instance_id\""));
        assert!(compact.contains("\"selected_joint_id\""));
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
    fn revolute_limits_clamp_state_and_convert_to_radians() {
        let mut joint = CadAssemblyJoint {
            id: "joint.revolute.limit".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Revolute {
                axis: Vec3::new(0.0, 0.0, 1.0),
                limits: Some((-90.0, 90.0)),
            },
            state: 0.0,
        };

        let semantics = joint.set_state_with_limits(120.0);
        assert!(semantics.was_clamped);
        assert_eq!(semantics.effective_state, 90.0);
        assert_eq!(joint.state, 90.0);
        assert_eq!(semantics.dof, 1);
        assert_eq!(semantics.state_unit, "deg");
        assert!((semantics.physics_state - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
    }

    #[test]
    fn slider_limits_clamp_state_and_convert_to_meters() {
        let mut joint = CadAssemblyJoint {
            id: "joint.slider.limit".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Slider {
                axis: Vec3::new(1.0, 0.0, 0.0),
                limits: Some((100.0, -100.0)),
            },
            state: 0.0,
        };

        let semantics = joint.set_state_with_limits(150.0);
        assert!(semantics.was_clamped);
        assert_eq!(semantics.effective_state, 100.0);
        assert_eq!(semantics.limits, Some((-100.0, 100.0)));
        assert_eq!(semantics.state_unit, "mm");
        assert!((semantics.physics_state - 0.1).abs() < 1e-10);
        let back_to_vcad = joint.convert_state_from_physics_units(semantics.physics_state);
        assert!((back_to_vcad - 100.0).abs() < 1e-10);
    }

    #[test]
    fn fixed_joint_state_semantics_force_zero_state() {
        let mut joint = CadAssemblyJoint {
            id: "joint.fixed.state".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Fixed,
            state: 7.0,
        };

        let semantics = joint.set_state_with_limits(42.0);
        assert!(!semantics.was_clamped);
        assert_eq!(semantics.effective_state, 0.0);
        assert_eq!(semantics.physics_state, 0.0);
        assert_eq!(semantics.dof, 0);
        assert_eq!(joint.state, 0.0);
        assert_eq!(semantics.state_unit, "fixed");
    }

    #[test]
    fn cylindrical_and_ball_state_semantics_match_degree_to_radian_conversion() {
        let mut cylindrical = CadAssemblyJoint {
            id: "joint.cyl.state".to_string(),
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
        let cyl = cylindrical.set_state_with_limits(180.0);
        assert_eq!(cyl.dof, 1);
        assert_eq!(cyl.state_unit, "deg");
        assert!((cyl.physics_state - std::f64::consts::PI).abs() < 1e-10);

        let mut ball = CadAssemblyJoint {
            id: "joint.ball.state".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Ball,
            state: 0.0,
        };
        let b = ball.set_state_with_limits(-180.0);
        assert_eq!(b.dof, 3);
        assert_eq!(b.state_unit, "deg");
        assert!((b.physics_state + std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn forward_kinematics_composes_joint_chain_transforms() {
        let schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![
                CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: None,
                    transform: Some(CadTransform3D {
                        translation: Vec3::new(100.0, 0.0, 0.0),
                        rotation: Vec3::new(0.0, 0.0, 0.0),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: None,
                },
                CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: None,
                    transform: Some(CadTransform3D {
                        translation: Vec3::new(10.0, 0.0, 0.0),
                        rotation: Vec3::new(0.0, 0.0, 0.0),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: None,
                },
                CadPartInstance {
                    id: "slider-1".to_string(),
                    part_def_id: "slider".to_string(),
                    name: None,
                    transform: Some(CadTransform3D {
                        translation: Vec3::new(5.0, 0.0, 0.0),
                        rotation: Vec3::new(0.0, 0.0, 0.0),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    }),
                    material: None,
                },
            ],
            joints: vec![
                CadAssemblyJoint {
                    id: "joint.revolute.001".to_string(),
                    name: None,
                    parent_instance_id: Some("base-1".to_string()),
                    child_instance_id: "arm-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(1.0, 0.0, 0.0),
                    kind: CadJointKind::Revolute {
                        axis: Vec3::new(0.0, 0.0, 1.0),
                        limits: None,
                    },
                    state: 90.0,
                },
                CadAssemblyJoint {
                    id: "joint.slider.001".to_string(),
                    name: None,
                    parent_instance_id: Some("arm-1".to_string()),
                    child_instance_id: "slider-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Slider {
                        axis: Vec3::new(1.0, 0.0, 0.0),
                        limits: None,
                    },
                    state: 20.0,
                },
            ],
            ground_instance_id: Some("base-1".to_string()),
        };

        let world = schema.solve_forward_kinematics();
        assert_eq!(world.len(), 3);

        let base = world.get("base-1").expect("base world transform");
        assert_eq!(base.translation, Vec3::new(100.0, 0.0, 0.0));
        assert_eq!(base.rotation, Vec3::new(0.0, 0.0, 0.0));

        let arm = world.get("arm-1").expect("arm world transform");
        assert_eq!(arm.translation, Vec3::new(100.0, 9.0, 0.0));
        assert_eq!(arm.rotation, Vec3::new(0.0, 0.0, 90.0));

        let slider = world.get("slider-1").expect("slider world transform");
        assert_eq!(slider.translation, Vec3::new(100.0, 34.0, 0.0));
        assert_eq!(slider.rotation, Vec3::new(0.0, 0.0, 90.0));
    }

    #[test]
    fn forward_kinematics_uses_identity_for_world_grounded_parent() {
        let schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![CadPartInstance {
                id: "free-1".to_string(),
                part_def_id: "arm".to_string(),
                name: None,
                transform: Some(CadTransform3D {
                    translation: Vec3::new(2.0, 3.0, 4.0),
                    rotation: Vec3::new(0.0, 0.0, 0.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                }),
                material: None,
            }],
            joints: vec![CadAssemblyJoint {
                id: "joint.fixed.world".to_string(),
                name: None,
                parent_instance_id: None,
                child_instance_id: "free-1".to_string(),
                parent_anchor: Vec3::new(10.0, 0.0, 0.0),
                child_anchor: Vec3::new(1.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            }],
            ground_instance_id: None,
        };

        let world = schema.solve_forward_kinematics();
        let free = world.get("free-1").expect("free-1 world transform");
        assert_eq!(free.translation, Vec3::new(11.0, 3.0, 4.0));
        assert_eq!(free.rotation, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn forward_kinematics_cycle_is_stabilized_by_visited_guard() {
        let schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![
                CadPartInstance {
                    id: "a-1".to_string(),
                    part_def_id: "a".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "b-1".to_string(),
                    part_def_id: "b".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
            ],
            joints: vec![
                CadAssemblyJoint {
                    id: "joint.root".to_string(),
                    name: None,
                    parent_instance_id: None,
                    child_instance_id: "a-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Fixed,
                    state: 0.0,
                },
                CadAssemblyJoint {
                    id: "joint.a_to_b".to_string(),
                    name: None,
                    parent_instance_id: Some("a-1".to_string()),
                    child_instance_id: "b-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Slider {
                        axis: Vec3::new(1.0, 0.0, 0.0),
                        limits: None,
                    },
                    state: 10.0,
                },
                CadAssemblyJoint {
                    id: "joint.b_to_a".to_string(),
                    name: None,
                    parent_instance_id: Some("b-1".to_string()),
                    child_instance_id: "a-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Slider {
                        axis: Vec3::new(0.0, 1.0, 0.0),
                        limits: None,
                    },
                    state: 5.0,
                },
            ],
            ground_instance_id: None,
        };

        let world = schema.solve_forward_kinematics();
        assert_eq!(world.len(), 2);
        assert_eq!(
            world.get("a-1").expect("a-1 world transform").translation,
            Vec3::new(0.0, 5.0, 0.0)
        );
        assert_eq!(
            world.get("b-1").expect("b-1 world transform").translation,
            Vec3::new(10.0, 5.0, 0.0)
        );
    }

    #[test]
    fn set_ground_instance_requires_known_instance() {
        let mut schema = CadAssemblySchema::default();
        schema.instances.push(CadPartInstance {
            id: "base-1".to_string(),
            part_def_id: "base".to_string(),
            name: None,
            transform: None,
            material: None,
        });

        schema
            .set_ground_instance("base-1")
            .expect("set known ground instance");
        assert_eq!(schema.ground_instance_id.as_deref(), Some("base-1"));

        let error = schema
            .set_ground_instance("missing")
            .expect_err("unknown ground instance should fail");
        assert!(error.to_string().contains("unknown instance: missing"));
    }

    #[test]
    fn delete_instance_removes_referencing_joints_and_clears_ground() {
        let mut schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![
                CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "tool-1".to_string(),
                    part_def_id: "tool".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
            ],
            joints: vec![
                CadAssemblyJoint {
                    id: "joint.base_arm".to_string(),
                    name: None,
                    parent_instance_id: Some("base-1".to_string()),
                    child_instance_id: "arm-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Fixed,
                    state: 0.0,
                },
                CadAssemblyJoint {
                    id: "joint.arm_tool".to_string(),
                    name: None,
                    parent_instance_id: Some("arm-1".to_string()),
                    child_instance_id: "tool-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Slider {
                        axis: Vec3::new(1.0, 0.0, 0.0),
                        limits: None,
                    },
                    state: 0.0,
                },
            ],
            ground_instance_id: Some("arm-1".to_string()),
        };

        let summary = schema
            .delete_instance("arm-1")
            .expect("delete instance with cleanup");
        assert_eq!(summary.deleted_instance_id, "arm-1");
        assert!(summary.cleared_ground);
        assert_eq!(
            summary.removed_joint_ids,
            vec!["joint.arm_tool".to_string(), "joint.base_arm".to_string()]
        );
        assert_eq!(schema.ground_instance_id, None);
        assert_eq!(schema.instances.len(), 2);
        assert!(
            schema
                .instances
                .iter()
                .all(|instance| instance.id != "arm-1")
        );
        assert!(schema.joints.is_empty());
    }

    #[test]
    fn delete_joint_and_instance_require_known_ids() {
        let mut schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![CadPartInstance {
                id: "base-1".to_string(),
                part_def_id: "base".to_string(),
                name: None,
                transform: None,
                material: None,
            }],
            joints: vec![CadAssemblyJoint {
                id: "joint.1".to_string(),
                name: None,
                parent_instance_id: None,
                child_instance_id: "base-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            }],
            ground_instance_id: None,
        };

        schema.delete_joint("joint.1").expect("delete known joint");
        assert!(schema.joints.is_empty());

        let joint_error = schema
            .delete_joint("joint.missing")
            .expect_err("missing joint should fail");
        assert!(
            joint_error
                .to_string()
                .contains("unknown joint: joint.missing")
        );

        let instance_error = schema
            .delete_instance("missing")
            .expect_err("missing instance should fail");
        assert!(
            instance_error
                .to_string()
                .contains("unknown instance: missing")
        );
    }

    #[test]
    fn assembly_ui_selection_and_editing_follow_selected_entities() {
        let mut schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![
                CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: Some("Base".to_string()),
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: Some("Arm".to_string()),
                    transform: None,
                    material: None,
                },
            ],
            joints: vec![CadAssemblyJoint {
                id: "joint.hinge".to_string(),
                name: Some("Hinge".to_string()),
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Revolute {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    limits: Some((-90.0, 90.0)),
                },
                state: 0.0,
            }],
            ground_instance_id: Some("base-1".to_string()),
        };
        let mut ui = CadAssemblyUiState::default();

        ui.select_instance(&schema, "arm-1")
            .expect("select instance");
        ui.rename_selected_instance(&mut schema, "Arm Segment".to_string())
            .expect("rename selected instance");
        assert_eq!(
            schema
                .instances
                .iter()
                .find(|instance| instance.id == "arm-1")
                .and_then(|instance| instance.name.as_deref()),
            Some("Arm Segment")
        );

        ui.select_joint(&schema, "joint.hinge")
            .expect("select joint");
        let semantics = ui
            .set_selected_joint_state(&mut schema, 120.0)
            .expect("set selected joint state");
        assert!(semantics.was_clamped);
        assert_eq!(semantics.effective_state, 90.0);
    }

    #[test]
    fn assembly_ui_sync_clears_stale_selection_refs_after_deletes() {
        let mut schema = CadAssemblySchema {
            part_defs: BTreeMap::new(),
            instances: vec![
                CadPartInstance {
                    id: "base-1".to_string(),
                    part_def_id: "base".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
                CadPartInstance {
                    id: "arm-1".to_string(),
                    part_def_id: "arm".to_string(),
                    name: None,
                    transform: None,
                    material: None,
                },
            ],
            joints: vec![CadAssemblyJoint {
                id: "joint.hinge".to_string(),
                name: None,
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            }],
            ground_instance_id: None,
        };
        let mut ui = CadAssemblyUiState {
            selected_instance_id: Some("arm-1".to_string()),
            selected_joint_id: Some("joint.hinge".to_string()),
            last_error: None,
        };

        schema
            .delete_instance("arm-1")
            .expect("delete selected instance");
        ui.sync_with_schema(&schema);
        assert_eq!(ui.selected_instance_id, None);
        assert_eq!(ui.selected_joint_id, None);
    }

    #[test]
    fn assembly_serialization_replay_sequence_is_deterministic() {
        fn replay_once() -> (
            CadAssemblySchema,
            CadAssemblyUiState,
            CadJointStateSemantics,
        ) {
            let initial_schema = CadAssemblySchema {
                part_defs: BTreeMap::from([
                    (
                        "base".to_string(),
                        CadPartDef {
                            id: "base".to_string(),
                            name: Some("Base".to_string()),
                            root: 1,
                            default_material: Some("aluminum".to_string()),
                        },
                    ),
                    (
                        "arm".to_string(),
                        CadPartDef {
                            id: "arm".to_string(),
                            name: Some("Arm".to_string()),
                            root: 2,
                            default_material: None,
                        },
                    ),
                ]),
                instances: vec![
                    CadPartInstance {
                        id: "base-1".to_string(),
                        part_def_id: "base".to_string(),
                        name: Some("Base".to_string()),
                        transform: Some(CadTransform3D::identity()),
                        material: None,
                    },
                    CadPartInstance {
                        id: "arm-1".to_string(),
                        part_def_id: "arm".to_string(),
                        name: Some("Arm".to_string()),
                        transform: Some(CadTransform3D {
                            translation: Vec3::new(10.0, 0.0, 0.0),
                            rotation: Vec3::new(0.0, 0.0, 0.0),
                            scale: Vec3::new(1.0, 1.0, 1.0),
                        }),
                        material: None,
                    },
                ],
                joints: vec![CadAssemblyJoint {
                    id: "joint.hinge".to_string(),
                    name: Some("Hinge".to_string()),
                    parent_instance_id: Some("base-1".to_string()),
                    child_instance_id: "arm-1".to_string(),
                    parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                    child_anchor: Vec3::new(0.0, 0.0, 0.0),
                    kind: CadJointKind::Revolute {
                        axis: Vec3::new(0.0, 0.0, 1.0),
                        limits: Some((-90.0, 90.0)),
                    },
                    state: 0.0,
                }],
                ground_instance_id: Some("base-1".to_string()),
            };
            let initial_ui = CadAssemblyUiState::default();

            let mut schema = CadAssemblySchema::from_json(
                &initial_schema.to_json().expect("serialize initial schema"),
            )
            .expect("parse initial schema");
            let mut ui =
                CadAssemblyUiState::from_json(&initial_ui.to_json().expect("serialize initial ui"))
                    .expect("parse initial ui");

            ui.select_instance(&schema, "arm-1")
                .expect("select known instance");
            ui.rename_selected_instance(&mut schema, "Arm Segment".to_string())
                .expect("rename selected instance");
            ui.select_joint(&schema, "joint.hinge")
                .expect("select known joint");
            let semantics = ui
                .set_selected_joint_state(&mut schema, 120.0)
                .expect("set selected joint state");
            schema
                .delete_instance("arm-1")
                .expect("delete selected instance");
            ui.sync_with_schema(&schema);

            (schema, ui, semantics)
        }

        let first = replay_once();
        let second = replay_once();
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
        assert_eq!(first.2, second.2);
        assert!(first.2.was_clamped);
        assert_eq!(first.2.effective_state, 90.0);
        assert!(
            first
                .0
                .instances
                .iter()
                .all(|instance| instance.id != "arm-1")
        );
        assert!(first.0.joints.is_empty());
        assert_eq!(first.1.selected_instance_id, None);
        assert_eq!(first.1.selected_joint_id, None);
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
            parent_anchor: Vec3::new(10.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 5.0, 0.0),
            kind: CadJointKind::Revolute {
                axis: Vec3::new(0.0, 0.0, 0.0),
                limits: None,
            },
            state: 90.0,
        };

        let motion = joint
            .solve_fixed_revolute_slider_motion()
            .expect("revolute motion");
        assert_eq!(
            motion,
            CadJointMotion::Revolute {
                translation_mm: Vec3::new(15.0, 0.0, 0.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
                angle_deg: 90.0,
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

    #[test]
    fn cylindrical_joint_motion_matches_vcad_rotation_path() {
        let joint = CadAssemblyJoint {
            id: "joint.cylindrical.002".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(10.0, 0.0, 0.0),
            child_anchor: Vec3::new(2.0, 0.0, 0.0),
            kind: CadJointKind::Cylindrical {
                axis: Vec3::new(0.0, 2.0, 0.0),
            },
            state: 90.0,
        };

        let motion = joint.solve_motion();
        assert_eq!(
            motion,
            CadJointMotion::Cylindrical {
                translation_mm: Vec3::new(10.0, 0.0, 2.0),
                axis: Vec3::new(0.0, 1.0, 0.0),
                angle_deg: 90.0,
            }
        );
    }

    #[test]
    fn ball_joint_motion_uses_z_axis_rotation_semantics() {
        let joint = CadAssemblyJoint {
            id: "joint.ball.001".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(3.0, 4.0, 0.0),
            child_anchor: Vec3::new(1.0, 0.0, 0.0),
            kind: CadJointKind::Ball,
            state: 90.0,
        };

        let motion = joint.solve_motion();
        assert_eq!(
            motion,
            CadJointMotion::Ball {
                translation_mm: Vec3::new(3.0, 3.0, 0.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
                angle_deg: 90.0,
            }
        );
    }
}
