use std::collections::BTreeMap;

use crate::intent::{
    AddVentPatternIntent, AdjustParameterIntent, CadAdjustOperation, CadIntent,
    CompareVariantsIntent, CreateParallelJawGripperSpecIntent, CreateRackSpecIntent, ExportIntent,
    GenerateVariantsIntent, SelectIntent, SetMaterialIntent, SetObjectiveIntent,
    parse_cad_intent_json_cad_result,
};
use crate::{CadError, CadResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadDesignProfile {
    Rack,
    ParallelJawGripper,
    ParallelJawGripperUnderactuated,
    ThreeFingerThumb,
}

impl Default for CadDesignProfile {
    fn default() -> Self {
        Self::Rack
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CadTypedCommand {
    CreateRackSpec(CreateRackSpecIntent),
    CreateParallelJawGripperSpec(CreateParallelJawGripperSpecIntent),
    GenerateVariants(GenerateVariantsIntent),
    SetObjective(SetObjectiveIntent),
    AdjustParameter(AdjustParameterIntent),
    SetMaterial(SetMaterialIntent),
    AddVentPattern(AddVentPatternIntent),
    Select(SelectIntent),
    CompareVariants(CompareVariantsIntent),
    Export(ExportIntent),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CadDispatchState {
    pub revision: u64,
    pub document_created: bool,
    pub design_profile: CadDesignProfile,
    pub units: Option<String>,
    pub material_id: Option<String>,
    pub objective: Option<String>,
    pub parameter_values: BTreeMap<String, f64>,
    pub selected_selector: Option<String>,
    pub compared_variants: Vec<String>,
    pub exported_variant: Option<String>,
    pub exported_format: Option<String>,
    pub generated_variant_count: Option<u8>,
    pub generated_objective_set: Option<String>,
    pub underactuated_mode: bool,
    pub compliant_joint_count: Option<u8>,
    pub flexure_thickness_mm: Option<f64>,
    pub single_servo_drive: bool,
    pub finger_count: Option<u8>,
    pub opposable_thumb: bool,
    pub thumb_base_angle_deg: Option<f64>,
    pub tendon_channel_diameter_mm: Option<f64>,
    pub joint_min_deg: Option<f64>,
    pub joint_max_deg: Option<f64>,
    pub tendon_route_clearance_mm: Option<f64>,
    pub tendon_bend_radius_mm: Option<f64>,
    pub servo_integration_enabled: bool,
    pub compact_servo_layout: bool,
    pub servo_envelope_length_mm: Option<f64>,
    pub servo_envelope_width_mm: Option<f64>,
    pub servo_envelope_height_mm: Option<f64>,
    pub servo_shaft_axis_offset_mm: Option<f64>,
    pub servo_mount_pattern_pitch_mm: Option<f64>,
    pub servo_bracket_thickness_mm: Option<f64>,
    pub servo_housing_wall_mm: Option<f64>,
    pub servo_standoff_diameter_mm: Option<f64>,
    pub pose_preset: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadDispatchReceipt {
    pub state_revision: u64,
    pub design_profile: CadDesignProfile,
    pub command: CadTypedCommand,
    pub summary: String,
}

pub fn dispatch_cad_payload_json(
    payload: &str,
    state: &mut CadDispatchState,
) -> CadResult<CadDispatchReceipt> {
    let intent = parse_cad_intent_json_cad_result(payload)?;
    dispatch_cad_intent(&intent, state)
}

pub fn dispatch_cad_intent(
    intent: &CadIntent,
    state: &mut CadDispatchState,
) -> CadResult<CadDispatchReceipt> {
    let command = match intent {
        CadIntent::CreateRackSpec(payload) => {
            state.document_created = true;
            state.design_profile = CadDesignProfile::Rack;
            state.units = Some(payload.units.clone());
            state.material_id = Some(payload.material.clone());
            state.objective = Some(payload.airflow.clone());
            CadTypedCommand::CreateRackSpec(payload.clone())
        }
        CadIntent::CreateParallelJawGripperSpec(payload) => {
            state.document_created = true;
            let is_three_finger_thumb = payload.finger_count >= 3 && payload.opposable_thumb;
            let is_servo_integrated_hand = is_three_finger_thumb && payload.servo_integration_enabled;
            state.design_profile = if is_three_finger_thumb {
                CadDesignProfile::ThreeFingerThumb
            } else if payload.underactuated_mode {
                CadDesignProfile::ParallelJawGripperUnderactuated
            } else {
                CadDesignProfile::ParallelJawGripper
            };
            state.objective = Some(if is_servo_integrated_hand && payload.compact_servo_layout {
                "three-finger-thumb-servo-compact".to_string()
            } else if is_servo_integrated_hand {
                "three-finger-thumb-servo-integration".to_string()
            } else if is_three_finger_thumb {
                "three-finger-thumb-hand".to_string()
            } else if payload.underactuated_mode {
                "parallel-jaw-gripper-underactuated".to_string()
            } else {
                "parallel-jaw-gripper".to_string()
            });
            state.underactuated_mode = payload.underactuated_mode;
            state.single_servo_drive = payload.single_servo_drive;
            state.compliant_joint_count = Some(payload.compliant_joint_count);
            state.flexure_thickness_mm = Some(payload.flexure_thickness_mm);
            state.finger_count = Some(payload.finger_count);
            state.opposable_thumb = payload.opposable_thumb;
            state.thumb_base_angle_deg = Some(payload.thumb_base_angle_deg);
            state.tendon_channel_diameter_mm = Some(payload.tendon_channel_diameter_mm);
            state.joint_min_deg = Some(payload.joint_min_deg);
            state.joint_max_deg = Some(payload.joint_max_deg);
            state.tendon_route_clearance_mm = Some(payload.tendon_route_clearance_mm);
            state.tendon_bend_radius_mm = Some(payload.tendon_bend_radius_mm);
            state.servo_integration_enabled = payload.servo_integration_enabled;
            state.compact_servo_layout = payload.compact_servo_layout;
            state.servo_envelope_length_mm = Some(payload.servo_envelope_length_mm);
            state.servo_envelope_width_mm = Some(payload.servo_envelope_width_mm);
            state.servo_envelope_height_mm = Some(payload.servo_envelope_height_mm);
            state.servo_shaft_axis_offset_mm = Some(payload.servo_shaft_axis_offset_mm);
            state.servo_mount_pattern_pitch_mm = Some(payload.servo_mount_pattern_pitch_mm);
            state.servo_bracket_thickness_mm = Some(payload.servo_bracket_thickness_mm);
            state.servo_housing_wall_mm = Some(payload.servo_housing_wall_mm);
            state.servo_standoff_diameter_mm = Some(payload.servo_standoff_diameter_mm);
            state.pose_preset = Some(payload.pose_preset.clone());
            state
                .parameter_values
                .insert("jaw_open_mm".to_string(), payload.jaw_open_mm);
            state
                .parameter_values
                .insert("finger_length_mm".to_string(), payload.finger_length_mm);
            state.parameter_values.insert(
                "finger_thickness_mm".to_string(),
                payload.finger_thickness_mm,
            );
            state
                .parameter_values
                .insert("base_width_mm".to_string(), payload.base_width_mm);
            state
                .parameter_values
                .insert("base_depth_mm".to_string(), payload.base_depth_mm);
            state
                .parameter_values
                .insert("base_thickness_mm".to_string(), payload.base_thickness_mm);
            state.parameter_values.insert(
                "servo_mount_hole_diameter_mm".to_string(),
                payload.servo_mount_hole_diameter_mm,
            );
            state
                .parameter_values
                .insert("print_fit_mm".to_string(), payload.print_fit_mm);
            state
                .parameter_values
                .insert("print_clearance_mm".to_string(), payload.print_clearance_mm);
            state.parameter_values.insert(
                "underactuated_mode".to_string(),
                if payload.underactuated_mode { 1.0 } else { 0.0 },
            );
            state.parameter_values.insert(
                "single_servo_drive".to_string(),
                if payload.single_servo_drive { 1.0 } else { 0.0 },
            );
            state.parameter_values.insert(
                "compliant_joint_count".to_string(),
                payload.compliant_joint_count as f64,
            );
            state.parameter_values.insert(
                "flexure_thickness_mm".to_string(),
                payload.flexure_thickness_mm,
            );
            state
                .parameter_values
                .insert("finger_count".to_string(), payload.finger_count as f64);
            state.parameter_values.insert(
                "opposable_thumb".to_string(),
                if payload.opposable_thumb { 1.0 } else { 0.0 },
            );
            state.parameter_values.insert(
                "thumb_base_angle_deg".to_string(),
                payload.thumb_base_angle_deg,
            );
            state.parameter_values.insert(
                "tendon_channel_diameter_mm".to_string(),
                payload.tendon_channel_diameter_mm,
            );
            state
                .parameter_values
                .insert("joint_min_deg".to_string(), payload.joint_min_deg);
            state
                .parameter_values
                .insert("joint_max_deg".to_string(), payload.joint_max_deg);
            state.parameter_values.insert(
                "tendon_route_clearance_mm".to_string(),
                payload.tendon_route_clearance_mm,
            );
            state.parameter_values.insert(
                "tendon_bend_radius_mm".to_string(),
                payload.tendon_bend_radius_mm,
            );
            state.parameter_values.insert(
                "servo_integration_enabled".to_string(),
                if payload.servo_integration_enabled {
                    1.0
                } else {
                    0.0
                },
            );
            state.parameter_values.insert(
                "compact_servo_layout".to_string(),
                if payload.compact_servo_layout { 1.0 } else { 0.0 },
            );
            state.parameter_values.insert(
                "servo_envelope_length_mm".to_string(),
                payload.servo_envelope_length_mm,
            );
            state.parameter_values.insert(
                "servo_envelope_width_mm".to_string(),
                payload.servo_envelope_width_mm,
            );
            state.parameter_values.insert(
                "servo_envelope_height_mm".to_string(),
                payload.servo_envelope_height_mm,
            );
            state.parameter_values.insert(
                "servo_shaft_axis_offset_mm".to_string(),
                payload.servo_shaft_axis_offset_mm,
            );
            state.parameter_values.insert(
                "servo_mount_pattern_pitch_mm".to_string(),
                payload.servo_mount_pattern_pitch_mm,
            );
            state.parameter_values.insert(
                "servo_bracket_thickness_mm".to_string(),
                payload.servo_bracket_thickness_mm,
            );
            state.parameter_values.insert(
                "servo_housing_wall_mm".to_string(),
                payload.servo_housing_wall_mm,
            );
            state.parameter_values.insert(
                "servo_standoff_diameter_mm".to_string(),
                payload.servo_standoff_diameter_mm,
            );
            CadTypedCommand::CreateParallelJawGripperSpec(payload.clone())
        }
        CadIntent::GenerateVariants(payload) => {
            state.generated_variant_count = Some(payload.count);
            state.generated_objective_set = Some(payload.objective_set.clone());
            CadTypedCommand::GenerateVariants(payload.clone())
        }
        CadIntent::SetObjective(payload) => {
            state.objective = Some(payload.objective.clone());
            CadTypedCommand::SetObjective(payload.clone())
        }
        CadIntent::AdjustParameter(payload) => {
            apply_parameter_adjustment(state, payload)?;
            CadTypedCommand::AdjustParameter(payload.clone())
        }
        CadIntent::SetMaterial(payload) => {
            state.material_id = Some(payload.material_id.clone());
            CadTypedCommand::SetMaterial(payload.clone())
        }
        CadIntent::AddVentPattern(payload) => {
            state
                .parameter_values
                .insert("vent.pattern.size_mm".to_string(), payload.size_mm);
            state
                .parameter_values
                .insert("vent.pattern.density".to_string(), payload.density);
            CadTypedCommand::AddVentPattern(payload.clone())
        }
        CadIntent::Select(payload) => {
            state.selected_selector = Some(payload.selector.clone());
            CadTypedCommand::Select(payload.clone())
        }
        CadIntent::CompareVariants(payload) => {
            state.compared_variants.clone_from(&payload.variant_ids);
            CadTypedCommand::CompareVariants(payload.clone())
        }
        CadIntent::Export(payload) => {
            state.exported_variant = Some(payload.variant_id.clone());
            state.exported_format = Some(payload.format.clone());
            CadTypedCommand::Export(payload.clone())
        }
    };

    state.revision = state.revision.saturating_add(1);
    let summary = format!(
        "{} dispatched at state revision {}",
        intent.intent_name(),
        state.revision
    );
    Ok(CadDispatchReceipt {
        state_revision: state.revision,
        design_profile: state.design_profile,
        command,
        summary,
    })
}

pub fn reject_free_text_mutation(request: &str) -> CadResult<()> {
    if request.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: "free-text mutation request is empty; expected structured CadIntent JSON"
                .to_string(),
        });
    }
    Err(CadError::ParseFailed {
        reason: "free-text state mutation is not allowed; use schema-validated CadIntent"
            .to_string(),
    })
}

fn apply_parameter_adjustment(
    state: &mut CadDispatchState,
    payload: &AdjustParameterIntent,
) -> CadResult<()> {
    let entry = state
        .parameter_values
        .entry(payload.parameter.clone())
        .or_insert(0.0);
    match payload.operation {
        CadAdjustOperation::Set => {
            *entry = payload.value;
        }
        CadAdjustOperation::Increase => {
            *entry += payload.value;
        }
        CadAdjustOperation::Decrease => {
            *entry -= payload.value;
        }
    }
    if !entry.is_finite() {
        return Err(CadError::InvalidParameter {
            name: payload.parameter.clone(),
            reason: "parameter adjustment produced non-finite result".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CadDispatchState, CadTypedCommand, dispatch_cad_intent, dispatch_cad_payload_json,
        reject_free_text_mutation,
    };
    use crate::intent::{
        AddVentPatternIntent, AdjustParameterIntent, CadAdjustOperation, CadIntent,
        CompareVariantsIntent, CreateParallelJawGripperSpecIntent, CreateRackSpecIntent,
        ExportIntent, GenerateVariantsIntent, SelectIntent, SetMaterialIntent, SetObjectiveIntent,
    };

    #[test]
    fn dispatch_covers_all_intent_types() {
        let mut state = CadDispatchState::default();
        let intents = vec![
            CadIntent::CreateRackSpec(CreateRackSpecIntent {
                units: "mm".to_string(),
                material: "al-6061-t6".to_string(),
                airflow: "balanced".to_string(),
                mount_type: "wall".to_string(),
            }),
            CadIntent::CreateParallelJawGripperSpec(CreateParallelJawGripperSpecIntent {
                jaw_open_mm: 42.0,
                finger_length_mm: 65.0,
                finger_thickness_mm: 8.0,
                base_width_mm: 78.0,
                base_depth_mm: 52.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
                print_fit_mm: 0.15,
                print_clearance_mm: 0.35,
                underactuated_mode: false,
                compliant_joint_count: 0,
                flexure_thickness_mm: 1.4,
                single_servo_drive: true,
                finger_count: 2,
                opposable_thumb: false,
                thumb_base_angle_deg: 42.0,
                tendon_channel_diameter_mm: 1.8,
                joint_min_deg: 12.0,
                joint_max_deg: 82.0,
                tendon_route_clearance_mm: 1.4,
                tendon_bend_radius_mm: 3.2,
                servo_integration_enabled: false,
                compact_servo_layout: false,
                servo_envelope_length_mm: 23.0,
                servo_envelope_width_mm: 12.0,
                servo_envelope_height_mm: 24.0,
                servo_shaft_axis_offset_mm: 5.0,
                servo_mount_pattern_pitch_mm: 16.0,
                servo_bracket_thickness_mm: 2.6,
                servo_housing_wall_mm: 2.0,
                servo_standoff_diameter_mm: 4.2,
                pose_preset: "open".to_string(),
            }),
            CadIntent::GenerateVariants(GenerateVariantsIntent {
                count: 4,
                objective_set: "rack.demo.v1".to_string(),
            }),
            CadIntent::SetObjective(SetObjectiveIntent {
                objective: "stiffness".to_string(),
            }),
            CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: "vent_spacing_mm".to_string(),
                operation: CadAdjustOperation::Set,
                value: 14.0,
            }),
            CadIntent::SetMaterial(SetMaterialIntent {
                material_id: "steel-1018".to_string(),
            }),
            CadIntent::AddVentPattern(AddVentPatternIntent {
                pattern: "hex".to_string(),
                size_mm: 4.0,
                density: 1.3,
            }),
            CadIntent::Select(SelectIntent {
                selector: "semantic:vent_face_set".to_string(),
            }),
            CadIntent::CompareVariants(CompareVariantsIntent {
                variant_ids: vec![
                    "variant.lightweight".to_string(),
                    "variant.stiffness".to_string(),
                ],
            }),
            CadIntent::Export(ExportIntent {
                format: "step".to_string(),
                variant_id: "variant.stiffness".to_string(),
            }),
        ];

        for intent in intents {
            let receipt =
                dispatch_cad_intent(&intent, &mut state).expect("dispatch should succeed");
            assert!(receipt.state_revision >= 1);
            assert_eq!(receipt.design_profile, state.design_profile);
        }
        assert_eq!(state.revision, 10);
        assert_eq!(state.material_id.as_deref(), Some("steel-1018"));
        assert_eq!(state.generated_variant_count, Some(4));
        assert_eq!(state.exported_format.as_deref(), Some("step"));
    }

    #[test]
    fn parallel_jaw_spec_sets_gripper_design_profile_and_parameter_surface() {
        let mut state = CadDispatchState::default();
        let receipt = dispatch_cad_intent(
            &CadIntent::CreateParallelJawGripperSpec(CreateParallelJawGripperSpecIntent {
                jaw_open_mm: 42.0,
                finger_length_mm: 65.0,
                finger_thickness_mm: 8.0,
                base_width_mm: 78.0,
                base_depth_mm: 52.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
                print_fit_mm: 0.15,
                print_clearance_mm: 0.35,
                underactuated_mode: false,
                compliant_joint_count: 0,
                flexure_thickness_mm: 1.4,
                single_servo_drive: true,
                finger_count: 2,
                opposable_thumb: false,
                thumb_base_angle_deg: 42.0,
                tendon_channel_diameter_mm: 1.8,
                joint_min_deg: 12.0,
                joint_max_deg: 82.0,
                tendon_route_clearance_mm: 1.4,
                tendon_bend_radius_mm: 3.2,
                servo_integration_enabled: false,
                compact_servo_layout: false,
                servo_envelope_length_mm: 23.0,
                servo_envelope_width_mm: 12.0,
                servo_envelope_height_mm: 24.0,
                servo_shaft_axis_offset_mm: 5.0,
                servo_mount_pattern_pitch_mm: 16.0,
                servo_bracket_thickness_mm: 2.6,
                servo_housing_wall_mm: 2.0,
                servo_standoff_diameter_mm: 4.2,
                pose_preset: "open".to_string(),
            }),
            &mut state,
        )
        .expect("gripper spec should dispatch");
        assert_eq!(
            receipt.design_profile,
            super::CadDesignProfile::ParallelJawGripper
        );
        assert_eq!(
            state.design_profile,
            super::CadDesignProfile::ParallelJawGripper
        );
        assert_eq!(
            state.parameter_values.get("jaw_open_mm").copied(),
            Some(42.0)
        );
        assert_eq!(
            state.parameter_values.get("print_fit_mm").copied(),
            Some(0.15)
        );
        assert_eq!(state.underactuated_mode, false);
        assert_eq!(state.single_servo_drive, true);
        assert_eq!(state.compliant_joint_count, Some(0));
        assert_eq!(state.flexure_thickness_mm, Some(1.4));
    }

    #[test]
    fn underactuated_parallel_jaw_spec_sets_underactuated_profile_state() {
        let mut state = CadDispatchState::default();
        let receipt = dispatch_cad_intent(
            &CadIntent::CreateParallelJawGripperSpec(CreateParallelJawGripperSpecIntent {
                jaw_open_mm: 36.0,
                finger_length_mm: 66.0,
                finger_thickness_mm: 7.5,
                base_width_mm: 82.0,
                base_depth_mm: 54.0,
                base_thickness_mm: 8.5,
                servo_mount_hole_diameter_mm: 2.9,
                print_fit_mm: 0.15,
                print_clearance_mm: 0.35,
                underactuated_mode: true,
                compliant_joint_count: 3,
                flexure_thickness_mm: 1.2,
                single_servo_drive: true,
                finger_count: 2,
                opposable_thumb: false,
                thumb_base_angle_deg: 42.0,
                tendon_channel_diameter_mm: 1.8,
                joint_min_deg: 12.0,
                joint_max_deg: 82.0,
                tendon_route_clearance_mm: 1.4,
                tendon_bend_radius_mm: 3.2,
                servo_integration_enabled: false,
                compact_servo_layout: false,
                servo_envelope_length_mm: 23.0,
                servo_envelope_width_mm: 12.0,
                servo_envelope_height_mm: 24.0,
                servo_shaft_axis_offset_mm: 5.0,
                servo_mount_pattern_pitch_mm: 16.0,
                servo_bracket_thickness_mm: 2.6,
                servo_housing_wall_mm: 2.0,
                servo_standoff_diameter_mm: 4.2,
                pose_preset: "open".to_string(),
            }),
            &mut state,
        )
        .expect("underactuated gripper spec should dispatch");
        assert_eq!(
            receipt.design_profile,
            super::CadDesignProfile::ParallelJawGripperUnderactuated
        );
        assert_eq!(
            state.design_profile,
            super::CadDesignProfile::ParallelJawGripperUnderactuated
        );
        assert_eq!(state.underactuated_mode, true);
        assert_eq!(state.single_servo_drive, true);
        assert_eq!(state.compliant_joint_count, Some(3));
        assert_eq!(state.flexure_thickness_mm, Some(1.2));
        assert_eq!(
            state.parameter_values.get("underactuated_mode").copied(),
            Some(1.0)
        );
    }

    #[test]
    fn three_finger_thumb_spec_sets_three_finger_profile_state() {
        let mut state = CadDispatchState::default();
        let receipt = dispatch_cad_intent(
            &CadIntent::CreateParallelJawGripperSpec(CreateParallelJawGripperSpecIntent {
                jaw_open_mm: 34.0,
                finger_length_mm: 68.0,
                finger_thickness_mm: 7.0,
                base_width_mm: 90.0,
                base_depth_mm: 58.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
                print_fit_mm: 0.15,
                print_clearance_mm: 0.35,
                underactuated_mode: true,
                compliant_joint_count: 3,
                flexure_thickness_mm: 1.2,
                single_servo_drive: true,
                finger_count: 3,
                opposable_thumb: true,
                thumb_base_angle_deg: 48.0,
                tendon_channel_diameter_mm: 1.6,
                joint_min_deg: 15.0,
                joint_max_deg: 88.0,
                tendon_route_clearance_mm: 1.6,
                tendon_bend_radius_mm: 3.8,
                servo_integration_enabled: true,
                compact_servo_layout: true,
                servo_envelope_length_mm: 23.0,
                servo_envelope_width_mm: 12.0,
                servo_envelope_height_mm: 24.0,
                servo_shaft_axis_offset_mm: 5.0,
                servo_mount_pattern_pitch_mm: 16.0,
                servo_bracket_thickness_mm: 2.6,
                servo_housing_wall_mm: 2.0,
                servo_standoff_diameter_mm: 4.2,
                pose_preset: "tripod".to_string(),
            }),
            &mut state,
        )
        .expect("three-finger hand spec should dispatch");
        assert_eq!(
            receipt.design_profile,
            super::CadDesignProfile::ThreeFingerThumb
        );
        assert_eq!(
            state.design_profile,
            super::CadDesignProfile::ThreeFingerThumb
        );
        assert_eq!(state.finger_count, Some(3));
        assert!(state.opposable_thumb);
        assert_eq!(state.thumb_base_angle_deg, Some(48.0));
        assert_eq!(state.tendon_channel_diameter_mm, Some(1.6));
        assert_eq!(state.joint_min_deg, Some(15.0));
        assert_eq!(state.joint_max_deg, Some(88.0));
        assert_eq!(state.tendon_route_clearance_mm, Some(1.6));
        assert_eq!(state.tendon_bend_radius_mm, Some(3.8));
        assert!(state.servo_integration_enabled);
        assert!(state.compact_servo_layout);
        assert_eq!(
            state.objective.as_deref(),
            Some("three-finger-thumb-servo-compact")
        );
        assert_eq!(state.pose_preset.as_deref(), Some("tripod"));
        assert_eq!(
            state.parameter_values.get("finger_count").copied(),
            Some(3.0)
        );
    }

    #[test]
    fn adjust_parameter_operations_apply_deterministically() {
        let mut state = CadDispatchState::default();
        dispatch_cad_intent(
            &CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: "wall_thickness_mm".to_string(),
                operation: CadAdjustOperation::Set,
                value: 6.0,
            }),
            &mut state,
        )
        .expect("set should succeed");
        dispatch_cad_intent(
            &CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: "wall_thickness_mm".to_string(),
                operation: CadAdjustOperation::Increase,
                value: 0.5,
            }),
            &mut state,
        )
        .expect("increase should succeed");
        dispatch_cad_intent(
            &CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: "wall_thickness_mm".to_string(),
                operation: CadAdjustOperation::Decrease,
                value: 1.0,
            }),
            &mut state,
        )
        .expect("decrease should succeed");

        assert_eq!(
            state.parameter_values.get("wall_thickness_mm").copied(),
            Some(5.5)
        );
        assert_eq!(state.revision, 3);
    }

    #[test]
    fn json_dispatch_enforces_intent_schema() {
        let mut state = CadDispatchState::default();
        let payload = r#"{"intent":"SetMaterial","material_id":"al-5052-h32"}"#;
        let receipt =
            dispatch_cad_payload_json(payload, &mut state).expect("json dispatch should succeed");
        match receipt.command {
            CadTypedCommand::SetMaterial(payload) => {
                assert_eq!(payload.material_id, "al-5052-h32");
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let invalid = dispatch_cad_payload_json("not-json", &mut state);
        assert!(
            invalid.is_err(),
            "invalid json should be rejected before dispatch"
        );
    }

    #[test]
    fn free_text_mutation_is_explicitly_rejected() {
        let error = reject_free_text_mutation("set wall thickness to 4mm")
            .expect_err("free-text mutation should fail");
        assert!(
            error
                .to_string()
                .contains("free-text state mutation is not allowed")
        );
    }
}
