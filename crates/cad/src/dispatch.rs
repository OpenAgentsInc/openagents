use std::collections::BTreeMap;

use crate::intent::{
    AddVentPatternIntent, AdjustParameterIntent, CadAdjustOperation, CadIntent,
    CompareVariantsIntent, CreateRackSpecIntent, ExportIntent, GenerateVariantsIntent,
    SelectIntent, SetMaterialIntent, SetObjectiveIntent, parse_cad_intent_json_cad_result,
};
use crate::{CadError, CadResult};

#[derive(Clone, Debug, PartialEq)]
pub enum CadTypedCommand {
    CreateRackSpec(CreateRackSpecIntent),
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadDispatchReceipt {
    pub state_revision: u64,
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

pub fn dispatch_cad_intent(intent: &CadIntent, state: &mut CadDispatchState) -> CadResult<CadDispatchReceipt> {
    let command = match intent {
        CadIntent::CreateRackSpec(payload) => {
            state.document_created = true;
            state.units = Some(payload.units.clone());
            state.material_id = Some(payload.material.clone());
            state.objective = Some(payload.airflow.clone());
            CadTypedCommand::CreateRackSpec(payload.clone())
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
            state.compared_variants = payload.variant_ids.clone();
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
        CompareVariantsIntent, CreateRackSpecIntent, ExportIntent, GenerateVariantsIntent,
        SelectIntent, SetMaterialIntent, SetObjectiveIntent,
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
                variant_ids: vec!["variant.lightweight".to_string(), "variant.stiffness".to_string()],
            }),
            CadIntent::Export(ExportIntent {
                format: "step".to_string(),
                variant_id: "variant.stiffness".to_string(),
            }),
        ];

        for intent in intents {
            let receipt = dispatch_cad_intent(&intent, &mut state).expect("dispatch should succeed");
            assert!(receipt.state_revision >= 1);
        }
        assert_eq!(state.revision, 9);
        assert_eq!(state.material_id.as_deref(), Some("steel-1018"));
        assert_eq!(state.generated_variant_count, Some(4));
        assert_eq!(state.exported_format.as_deref(), Some("step"));
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
        let receipt = dispatch_cad_payload_json(payload, &mut state).expect("json dispatch should succeed");
        match receipt.command {
            CadTypedCommand::SetMaterial(payload) => {
                assert_eq!(payload.material_id, "al-5052-h32");
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let invalid = dispatch_cad_payload_json("not-json", &mut state);
        assert!(invalid.is_err(), "invalid json should be rejected before dispatch");
    }

    #[test]
    fn free_text_mutation_is_explicitly_rejected() {
        let error = reject_free_text_mutation("set wall thickness to 4mm")
            .expect_err("free-text mutation should fail");
        assert!(error
            .to_string()
            .contains("free-text state mutation is not allowed"));
    }
}
