use crate::intent::{
    AdjustParameterIntent, CadAdjustOperation, CadIntent, CadIntentValidationError,
    CompareVariantsIntent, CreateRackSpecIntent, ExportIntent, SelectIntent, SetMaterialIntent,
    SetObjectiveIntent, parse_cad_intent_json,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadIntentTranslationError {
    pub code: String,
    pub message: String,
    pub recovery_prompt: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CadIntentTranslationOutcome {
    Intent(CadIntent),
    ParseFailure(CadIntentTranslationError),
}

pub fn translate_chat_to_cad_intent(message: &str) -> CadIntentTranslationOutcome {
    let input = message.trim();
    if input.is_empty() {
        return parse_failure(
            "CAD-CHAT-EMPTY",
            "chat message is empty",
            "Send a CAD command, for example: `Set material al-6061-t6` or provide `CadIntent` JSON.",
        );
    }

    if let Some(json_candidate) = extract_json_candidate(input) {
        match parse_cad_intent_json(json_candidate) {
            Ok(intent) => return CadIntentTranslationOutcome::Intent(intent),
            Err(error) => {
                return parse_failure_with_intent_error(
                    error,
                    "JSON was detected but did not match CadIntent schema. Send valid intent JSON or a supported command phrase.",
                );
            }
        }
    }

    if let Some(intent) = translate_phrase(input) {
        return CadIntentTranslationOutcome::Intent(intent);
    }

    parse_failure(
        "CAD-CHAT-AMBIGUOUS",
        "could not map message to a supported CadIntent",
        "Try one of: `Set material <id>`, `Set objective <name>`, `Select <selector>`, `Compare <variantA,variantB>`, `Export <format> <variant>`, or provide CadIntent JSON.",
    )
}

fn translate_phrase(input: &str) -> Option<CadIntent> {
    let lower = input.to_ascii_lowercase();

    if let Some(material_id) = lower.strip_prefix("set material ") {
        return Some(CadIntent::SetMaterial(SetMaterialIntent {
            material_id: material_id.trim().to_string(),
        }));
    }

    if let Some(objective) = lower.strip_prefix("set objective ") {
        return Some(CadIntent::SetObjective(SetObjectiveIntent {
            objective: objective.trim().to_string(),
        }));
    }

    if let Some(selector) = lower.strip_prefix("select ") {
        return Some(CadIntent::Select(SelectIntent {
            selector: selector.trim().to_string(),
        }));
    }

    if let Some(target) = lower.strip_prefix("compare ") {
        let variants = target
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !variants.is_empty() {
            return Some(CadIntent::CompareVariants(CompareVariantsIntent {
                variant_ids: variants,
            }));
        }
    }

    if let Some(rest) = lower.strip_prefix("export ") {
        let mut parts = rest.split_whitespace();
        let format = parts.next()?;
        let variant_id = parts.next().unwrap_or("variant.baseline");
        return Some(CadIntent::Export(ExportIntent {
            format: format.to_string(),
            variant_id: variant_id.to_string(),
        }));
    }

    if lower.contains("vent") && lower.contains("hole") && lower.contains('%') {
        if let Some(percent) = extract_percent_value(&lower) {
            return Some(CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: "vent_hole_radius_mm".to_string(),
                operation: if lower.contains("reduce") || lower.contains("smaller") {
                    CadAdjustOperation::Decrease
                } else {
                    CadAdjustOperation::Increase
                },
                value: percent,
            }));
        }
    }

    if lower.starts_with("set ") && lower.contains('=') {
        let trimmed = lower.trim_start_matches("set ");
        let mut parts = trimmed.splitn(2, '=');
        let parameter = parts.next()?.trim();
        let value_raw = parts.next()?.trim();
        if let Ok(value) = value_raw.parse::<f64>() {
            return Some(CadIntent::AdjustParameter(AdjustParameterIntent {
                parameter: parameter.to_string(),
                operation: CadAdjustOperation::Set,
                value,
            }));
        }
    }

    if let Some(intent) = translate_rack_design_prompt(&lower) {
        return Some(intent);
    }

    None
}

fn translate_rack_design_prompt(lower: &str) -> Option<CadIntent> {
    let has_design_verb = ["design", "build", "create", "model", "draft"]
        .iter()
        .any(|verb| lower.contains(verb));
    if !has_design_verb || !lower.contains("rack") {
        return None;
    }

    let units = infer_units(lower);
    let material = infer_material(lower);
    let airflow = infer_airflow(lower);
    let mount_type = infer_mount_type(lower);

    Some(CadIntent::CreateRackSpec(CreateRackSpecIntent {
        units,
        material,
        airflow,
        mount_type,
    }))
}

fn infer_units(lower: &str) -> String {
    if lower.contains(" inch") || lower.contains(" inches") || lower.contains('"') {
        return "in".to_string();
    }
    for token in lower.split_whitespace() {
        if token.len() > 2 && token.ends_with("in") {
            let value = &token[..token.len().saturating_sub(2)];
            if value
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-')
            {
                return "in".to_string();
            }
        }
    }
    "mm".to_string()
}

fn infer_material(lower: &str) -> String {
    if lower.contains("al-5052") || lower.contains("5052") || lower.contains("sheet metal") {
        return "al-5052-h32".to_string();
    }
    if lower.contains("al-6061")
        || lower.contains("6061")
        || lower.contains("aluminum")
        || lower.contains("aluminium")
    {
        return "al-6061-t6".to_string();
    }
    if lower.contains("steel") {
        return "steel-1018".to_string();
    }
    "al-6061-t6".to_string()
}

fn infer_airflow(lower: &str) -> String {
    if lower.contains("high airflow")
        || lower.contains("airflow")
        || lower.contains("vent")
        || lower.contains("cooling")
    {
        return "high".to_string();
    }
    if lower.contains("quiet") || lower.contains("acoustic") {
        return "low".to_string();
    }
    "balanced".to_string()
}

fn infer_mount_type(lower: &str) -> String {
    if lower.contains("wall") {
        return "wall".to_string();
    }
    if lower.contains("desktop") || lower.contains("desk") || lower.contains("table") {
        return "desktop".to_string();
    }
    "wall".to_string()
}

fn extract_percent_value(input: &str) -> Option<f64> {
    let mut digits = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            digits.push(ch);
            continue;
        }
        if ch == '%' && !digits.is_empty() {
            return digits.parse::<f64>().ok();
        }
        if !digits.is_empty() {
            digits.clear();
        }
    }
    None
}

fn parse_failure(code: &str, message: &str, recovery_prompt: &str) -> CadIntentTranslationOutcome {
    CadIntentTranslationOutcome::ParseFailure(CadIntentTranslationError {
        code: code.to_string(),
        message: message.to_string(),
        recovery_prompt: recovery_prompt.to_string(),
    })
}

fn parse_failure_with_intent_error(
    error: CadIntentValidationError,
    recovery_prompt: &str,
) -> CadIntentTranslationOutcome {
    CadIntentTranslationOutcome::ParseFailure(CadIntentTranslationError {
        code: error.code,
        message: error.message,
        recovery_prompt: recovery_prompt.to_string(),
    })
}

fn extract_json_candidate(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }

    let first = trimmed.find('{')?;
    let last = trimmed.rfind('}')?;
    if first < last {
        return Some(&trimmed[first..=last]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{CadIntentTranslationOutcome, translate_chat_to_cad_intent};
    use crate::intent::{CadAdjustOperation, CadIntent};

    #[test]
    fn adapter_accepts_valid_intent_json() {
        let message = r#"{"intent":"SetMaterial","material_id":"al-6061-t6"}"#;
        let outcome = translate_chat_to_cad_intent(message);
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::SetMaterial(payload)) => {
                assert_eq!(payload.material_id, "al-6061-t6");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_simple_phrase_commands() {
        let outcome = translate_chat_to_cad_intent("Set objective stiffness");
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::SetObjective(payload)) => {
                assert_eq!(payload.objective, "stiffness");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_vent_percent_adjustments() {
        let outcome = translate_chat_to_cad_intent("Make vent holes 20% larger");
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::AdjustParameter(payload)) => {
                assert_eq!(payload.parameter, "vent_hole_radius_mm");
                assert_eq!(payload.operation, CadAdjustOperation::Increase);
                assert_eq!(payload.value, 20.0);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_returns_recovery_prompt_for_malformed_json() {
        let outcome = translate_chat_to_cad_intent(
            "{", /* intentionally malformed payload to trigger JSON parse path */
        );
        match outcome {
            CadIntentTranslationOutcome::ParseFailure(error) => {
                assert!(error.code.starts_with("CAD-"));
                assert!(error.recovery_prompt.contains("CadIntent"));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_returns_recovery_prompt_for_ambiguous_message() {
        let outcome = translate_chat_to_cad_intent("can you just make it better");
        match outcome {
            CadIntentTranslationOutcome::ParseFailure(error) => {
                assert_eq!(error.code, "CAD-CHAT-AMBIGUOUS");
                assert!(error.recovery_prompt.contains("Set material"));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_rack_design_prompt_into_create_rack_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Build a wall-mount rack for 2 Mac Studio units with high airflow in sheet metal",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateRackSpec(payload)) => {
                assert_eq!(payload.units, "mm");
                assert_eq!(payload.material, "al-5052-h32");
                assert_eq!(payload.airflow, "high");
                assert_eq!(payload.mount_type, "wall");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}
