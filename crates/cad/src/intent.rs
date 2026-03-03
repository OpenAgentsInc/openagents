use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{CadError, CadResult};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadIntentValidationError {
    pub code: String,
    pub intent: Option<String>,
    pub field: Option<String>,
    pub message: String,
}

impl CadIntentValidationError {
    fn new(
        code: impl Into<String>,
        intent: Option<String>,
        field: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            intent,
            field,
            message: message.into(),
        }
    }

    pub fn to_cad_error(&self) -> CadError {
        CadError::ParseFailed {
            reason: format!("{}: {}", self.code, self.message),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CadAdjustOperation {
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "increase")]
    Increase,
    #[serde(rename = "decrease")]
    Decrease,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRackSpecIntent {
    pub units: String,
    pub material: String,
    pub airflow: String,
    pub mount_type: String,
}

pub const PARALLEL_JAW_GRIPPER_DEFAULT_JAW_OPEN_MM: f64 = 42.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_LENGTH_MM: f64 = 65.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_THICKNESS_MM: f64 = 8.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_BASE_WIDTH_MM: f64 = 78.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_BASE_DEPTH_MM: f64 = 52.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_BASE_THICKNESS_MM: f64 = 8.0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_HOLE_DIAMETER_MM: f64 = 2.9;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_FIT_MM: f64 = 0.15;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_CLEARANCE_MM: f64 = 0.35;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_UNDERACTUATED_MODE: bool = false;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_COMPLIANT_JOINT_COUNT: u8 = 0;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM: f64 = 1.4;
pub const PARALLEL_JAW_GRIPPER_DEFAULT_SINGLE_SERVO_DRIVE: bool = true;
pub const PARALLEL_JAW_GRIPPER_MIN_JAW_OPEN_MM: f64 = 8.0;
pub const PARALLEL_JAW_GRIPPER_MAX_JAW_OPEN_MM: f64 = 140.0;
pub const PARALLEL_JAW_GRIPPER_MIN_FINGER_LENGTH_MM: f64 = 25.0;
pub const PARALLEL_JAW_GRIPPER_MAX_FINGER_LENGTH_MM: f64 = 180.0;
pub const PARALLEL_JAW_GRIPPER_MIN_FINGER_THICKNESS_MM: f64 = 2.0;
pub const PARALLEL_JAW_GRIPPER_MAX_FINGER_THICKNESS_MM: f64 = 24.0;
pub const PARALLEL_JAW_GRIPPER_MIN_BASE_WIDTH_MM: f64 = 30.0;
pub const PARALLEL_JAW_GRIPPER_MAX_BASE_WIDTH_MM: f64 = 240.0;
pub const PARALLEL_JAW_GRIPPER_MIN_BASE_DEPTH_MM: f64 = 20.0;
pub const PARALLEL_JAW_GRIPPER_MAX_BASE_DEPTH_MM: f64 = 180.0;
pub const PARALLEL_JAW_GRIPPER_MIN_BASE_THICKNESS_MM: f64 = 2.0;
pub const PARALLEL_JAW_GRIPPER_MAX_BASE_THICKNESS_MM: f64 = 40.0;
pub const PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_HOLE_DIAMETER_MM: f64 = 1.2;
pub const PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_HOLE_DIAMETER_MM: f64 = 8.0;
pub const PARALLEL_JAW_GRIPPER_MIN_PRINT_FIT_MM: f64 = 0.05;
pub const PARALLEL_JAW_GRIPPER_MAX_PRINT_FIT_MM: f64 = 0.4;
pub const PARALLEL_JAW_GRIPPER_MIN_PRINT_CLEARANCE_MM: f64 = 0.1;
pub const PARALLEL_JAW_GRIPPER_MAX_PRINT_CLEARANCE_MM: f64 = 0.8;
pub const PARALLEL_JAW_GRIPPER_MIN_COMPLIANT_JOINT_COUNT: u8 = 0;
pub const PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT: u8 = 6;
pub const PARALLEL_JAW_GRIPPER_MIN_FLEXURE_THICKNESS_MM: f64 = 0.8;
pub const PARALLEL_JAW_GRIPPER_MAX_FLEXURE_THICKNESS_MM: f64 = 4.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateParallelJawGripperSpecIntent {
    pub jaw_open_mm: f64,
    pub finger_length_mm: f64,
    pub finger_thickness_mm: f64,
    pub base_width_mm: f64,
    pub base_depth_mm: f64,
    pub base_thickness_mm: f64,
    pub servo_mount_hole_diameter_mm: f64,
    pub print_fit_mm: f64,
    pub print_clearance_mm: f64,
    #[serde(default = "default_underactuated_mode")]
    pub underactuated_mode: bool,
    #[serde(default = "default_compliant_joint_count")]
    pub compliant_joint_count: u8,
    #[serde(default = "default_flexure_thickness_mm")]
    pub flexure_thickness_mm: f64,
    #[serde(default = "default_single_servo_drive")]
    pub single_servo_drive: bool,
}

impl Default for CreateParallelJawGripperSpecIntent {
    fn default() -> Self {
        Self {
            jaw_open_mm: PARALLEL_JAW_GRIPPER_DEFAULT_JAW_OPEN_MM,
            finger_length_mm: PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_LENGTH_MM,
            finger_thickness_mm: PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_THICKNESS_MM,
            base_width_mm: PARALLEL_JAW_GRIPPER_DEFAULT_BASE_WIDTH_MM,
            base_depth_mm: PARALLEL_JAW_GRIPPER_DEFAULT_BASE_DEPTH_MM,
            base_thickness_mm: PARALLEL_JAW_GRIPPER_DEFAULT_BASE_THICKNESS_MM,
            servo_mount_hole_diameter_mm: PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_HOLE_DIAMETER_MM,
            print_fit_mm: PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_FIT_MM,
            print_clearance_mm: PARALLEL_JAW_GRIPPER_DEFAULT_PRINT_CLEARANCE_MM,
            underactuated_mode: PARALLEL_JAW_GRIPPER_DEFAULT_UNDERACTUATED_MODE,
            compliant_joint_count: PARALLEL_JAW_GRIPPER_DEFAULT_COMPLIANT_JOINT_COUNT,
            flexure_thickness_mm: PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM,
            single_servo_drive: PARALLEL_JAW_GRIPPER_DEFAULT_SINGLE_SERVO_DRIVE,
        }
    }
}

const fn default_underactuated_mode() -> bool {
    PARALLEL_JAW_GRIPPER_DEFAULT_UNDERACTUATED_MODE
}

const fn default_compliant_joint_count() -> u8 {
    PARALLEL_JAW_GRIPPER_DEFAULT_COMPLIANT_JOINT_COUNT
}

const fn default_flexure_thickness_mm() -> f64 {
    PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM
}

const fn default_single_servo_drive() -> bool {
    PARALLEL_JAW_GRIPPER_DEFAULT_SINGLE_SERVO_DRIVE
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateVariantsIntent {
    pub count: u8,
    pub objective_set: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetObjectiveIntent {
    pub objective: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdjustParameterIntent {
    pub parameter: String,
    pub operation: CadAdjustOperation,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetMaterialIntent {
    pub material_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddVentPatternIntent {
    pub pattern: String,
    pub size_mm: f64,
    pub density: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectIntent {
    pub selector: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompareVariantsIntent {
    pub variant_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportIntent {
    pub format: String,
    pub variant_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CadIntent {
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

impl CadIntent {
    pub fn intent_name(&self) -> &'static str {
        match self {
            Self::CreateRackSpec(_) => "CreateRackSpec",
            Self::CreateParallelJawGripperSpec(_) => "CreateParallelJawGripperSpec",
            Self::GenerateVariants(_) => "GenerateVariants",
            Self::SetObjective(_) => "SetObjective",
            Self::AdjustParameter(_) => "AdjustParameter",
            Self::SetMaterial(_) => "SetMaterial",
            Self::AddVentPattern(_) => "AddVentPattern",
            Self::Select(_) => "Select",
            Self::CompareVariants(_) => "CompareVariants",
            Self::Export(_) => "Export",
        }
    }
}

pub fn allowed_intent_names() -> [&'static str; 10] {
    [
        "CreateRackSpec",
        "CreateParallelJawGripperSpec",
        "GenerateVariants",
        "SetObjective",
        "AdjustParameter",
        "SetMaterial",
        "AddVentPattern",
        "Select",
        "CompareVariants",
        "Export",
    ]
}

pub fn cad_intent_json_schema() -> Value {
    json!({
      "schema_version": 1,
      "type": "object",
      "required": ["intent"],
      "allowed_intents": allowed_intent_names(),
      "intents": {
        "CreateRackSpec": {"required": ["units", "material", "airflow", "mount_type"]},
        "CreateParallelJawGripperSpec": {
          "required": [
            "jaw_open_mm",
            "finger_length_mm",
            "finger_thickness_mm",
            "base_width_mm",
            "base_depth_mm",
            "base_thickness_mm",
            "servo_mount_hole_diameter_mm",
            "print_fit_mm",
            "print_clearance_mm"
          ],
          "optional": [
            "underactuated_mode",
            "compliant_joint_count",
            "flexure_thickness_mm",
            "single_servo_drive"
          ]
        },
        "GenerateVariants": {"required": ["count", "objective_set"]},
        "SetObjective": {"required": ["objective"]},
        "AdjustParameter": {"required": ["parameter", "operation", "value"]},
        "SetMaterial": {"required": ["material_id"]},
        "AddVentPattern": {"required": ["pattern", "size_mm", "density"]},
        "Select": {"required": ["selector"]},
        "CompareVariants": {"required": ["variant_ids"]},
        "Export": {"required": ["format", "variant_id"]}
      }
    })
}

pub fn parse_cad_intent_json(payload: &str) -> Result<CadIntent, CadIntentValidationError> {
    let root: Value = serde_json::from_str(payload).map_err(|error| {
        CadIntentValidationError::new(
            "CAD-INTENT-INVALID-JSON",
            None,
            None,
            format!("invalid json payload: {error}"),
        )
    })?;
    let Some(object) = root.as_object() else {
        return Err(CadIntentValidationError::new(
            "CAD-INTENT-INVALID-SHAPE",
            None,
            None,
            "payload must be a JSON object",
        ));
    };
    let Some(intent_name) = object.get("intent").and_then(Value::as_str) else {
        return Err(CadIntentValidationError::new(
            "CAD-INTENT-MISSING-INTENT",
            None,
            Some("intent".to_string()),
            "payload requires string field 'intent'",
        ));
    };

    let content = strip_intent_field(object);
    let parsed = match intent_name {
        "CreateRackSpec" => parse_payload::<CreateRackSpecIntent>(intent_name, content)
            .map(CadIntent::CreateRackSpec),
        "CreateParallelJawGripperSpec" => {
            parse_payload::<CreateParallelJawGripperSpecIntent>(intent_name, content)
                .map(CadIntent::CreateParallelJawGripperSpec)
        }
        "GenerateVariants" => parse_payload::<GenerateVariantsIntent>(intent_name, content)
            .map(CadIntent::GenerateVariants),
        "SetObjective" => {
            parse_payload::<SetObjectiveIntent>(intent_name, content).map(CadIntent::SetObjective)
        }
        "AdjustParameter" => parse_payload::<AdjustParameterIntent>(intent_name, content)
            .map(CadIntent::AdjustParameter),
        "SetMaterial" => {
            parse_payload::<SetMaterialIntent>(intent_name, content).map(CadIntent::SetMaterial)
        }
        "AddVentPattern" => parse_payload::<AddVentPatternIntent>(intent_name, content)
            .map(CadIntent::AddVentPattern),
        "Select" => parse_payload::<SelectIntent>(intent_name, content).map(CadIntent::Select),
        "CompareVariants" => parse_payload::<CompareVariantsIntent>(intent_name, content)
            .map(CadIntent::CompareVariants),
        "Export" => parse_payload::<ExportIntent>(intent_name, content).map(CadIntent::Export),
        _ => Err(CadIntentValidationError::new(
            "CAD-INTENT-UNKNOWN-OP",
            Some(intent_name.to_string()),
            Some("intent".to_string()),
            format!(
                "unsupported intent '{}'; allowed intents: {}",
                intent_name,
                allowed_intent_names().join(", ")
            ),
        )),
    }?;
    validate_cad_intent(&parsed)?;
    Ok(parsed)
}

pub fn parse_cad_intent_json_cad_result(payload: &str) -> CadResult<CadIntent> {
    parse_cad_intent_json(payload).map_err(|error| error.to_cad_error())
}

pub fn validate_cad_intent(intent: &CadIntent) -> Result<(), CadIntentValidationError> {
    match intent {
        CadIntent::CreateRackSpec(payload) => {
            validate_non_empty("CreateRackSpec", "units", &payload.units)?;
            validate_non_empty("CreateRackSpec", "material", &payload.material)?;
            validate_non_empty("CreateRackSpec", "airflow", &payload.airflow)?;
            validate_non_empty("CreateRackSpec", "mount_type", &payload.mount_type)?;
        }
        CadIntent::CreateParallelJawGripperSpec(payload) => {
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "jaw_open_mm",
                payload.jaw_open_mm,
                PARALLEL_JAW_GRIPPER_MIN_JAW_OPEN_MM,
                PARALLEL_JAW_GRIPPER_MAX_JAW_OPEN_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "finger_length_mm",
                payload.finger_length_mm,
                PARALLEL_JAW_GRIPPER_MIN_FINGER_LENGTH_MM,
                PARALLEL_JAW_GRIPPER_MAX_FINGER_LENGTH_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "finger_thickness_mm",
                payload.finger_thickness_mm,
                PARALLEL_JAW_GRIPPER_MIN_FINGER_THICKNESS_MM,
                PARALLEL_JAW_GRIPPER_MAX_FINGER_THICKNESS_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "base_width_mm",
                payload.base_width_mm,
                PARALLEL_JAW_GRIPPER_MIN_BASE_WIDTH_MM,
                PARALLEL_JAW_GRIPPER_MAX_BASE_WIDTH_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "base_depth_mm",
                payload.base_depth_mm,
                PARALLEL_JAW_GRIPPER_MIN_BASE_DEPTH_MM,
                PARALLEL_JAW_GRIPPER_MAX_BASE_DEPTH_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "base_thickness_mm",
                payload.base_thickness_mm,
                PARALLEL_JAW_GRIPPER_MIN_BASE_THICKNESS_MM,
                PARALLEL_JAW_GRIPPER_MAX_BASE_THICKNESS_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "servo_mount_hole_diameter_mm",
                payload.servo_mount_hole_diameter_mm,
                PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_HOLE_DIAMETER_MM,
                PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_HOLE_DIAMETER_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "print_fit_mm",
                payload.print_fit_mm,
                PARALLEL_JAW_GRIPPER_MIN_PRINT_FIT_MM,
                PARALLEL_JAW_GRIPPER_MAX_PRINT_FIT_MM,
            )?;
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "print_clearance_mm",
                payload.print_clearance_mm,
                PARALLEL_JAW_GRIPPER_MIN_PRINT_CLEARANCE_MM,
                PARALLEL_JAW_GRIPPER_MAX_PRINT_CLEARANCE_MM,
            )?;
            if payload.compliant_joint_count < PARALLEL_JAW_GRIPPER_MIN_COMPLIANT_JOINT_COUNT
                || payload.compliant_joint_count > PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT
            {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("CreateParallelJawGripperSpec".to_string()),
                    Some("compliant_joint_count".to_string()),
                    format!(
                        "compliant_joint_count must be in range [{}, {}]",
                        PARALLEL_JAW_GRIPPER_MIN_COMPLIANT_JOINT_COUNT,
                        PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT
                    ),
                ));
            }
            validate_finite_range(
                "CreateParallelJawGripperSpec",
                "flexure_thickness_mm",
                payload.flexure_thickness_mm,
                PARALLEL_JAW_GRIPPER_MIN_FLEXURE_THICKNESS_MM,
                PARALLEL_JAW_GRIPPER_MAX_FLEXURE_THICKNESS_MM,
            )?;
            if payload.print_clearance_mm <= payload.print_fit_mm {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("CreateParallelJawGripperSpec".to_string()),
                    Some("print_clearance_mm".to_string()),
                    "print_clearance_mm must be greater than print_fit_mm",
                ));
            }
            if payload.underactuated_mode {
                if payload.compliant_joint_count == 0 {
                    return Err(CadIntentValidationError::new(
                        "CAD-INTENT-INVALID-RANGE",
                        Some("CreateParallelJawGripperSpec".to_string()),
                        Some("compliant_joint_count".to_string()),
                        "underactuated_mode requires compliant_joint_count >= 1",
                    ));
                }
                if !payload.single_servo_drive {
                    return Err(CadIntentValidationError::new(
                        "CAD-INTENT-INVALID-FIELD",
                        Some("CreateParallelJawGripperSpec".to_string()),
                        Some("single_servo_drive".to_string()),
                        "underactuated_mode requires single_servo_drive=true",
                    ));
                }
            }
        }
        CadIntent::GenerateVariants(payload) => {
            if payload.count == 0 || payload.count > 4 {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("GenerateVariants".to_string()),
                    Some("count".to_string()),
                    "count must be in range [1, 4]",
                ));
            }
            validate_non_empty("GenerateVariants", "objective_set", &payload.objective_set)?;
        }
        CadIntent::SetObjective(payload) => {
            validate_non_empty("SetObjective", "objective", &payload.objective)?;
        }
        CadIntent::AdjustParameter(payload) => {
            validate_non_empty("AdjustParameter", "parameter", &payload.parameter)?;
            if !payload.value.is_finite() {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-NUMBER",
                    Some("AdjustParameter".to_string()),
                    Some("value".to_string()),
                    "value must be a finite number",
                ));
            }
        }
        CadIntent::SetMaterial(payload) => {
            validate_non_empty("SetMaterial", "material_id", &payload.material_id)?;
        }
        CadIntent::AddVentPattern(payload) => {
            validate_non_empty("AddVentPattern", "pattern", &payload.pattern)?;
            if !payload.size_mm.is_finite() || payload.size_mm <= 0.0 {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("AddVentPattern".to_string()),
                    Some("size_mm".to_string()),
                    "size_mm must be finite and > 0",
                ));
            }
            if !payload.density.is_finite() || payload.density <= 0.0 {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("AddVentPattern".to_string()),
                    Some("density".to_string()),
                    "density must be finite and > 0",
                ));
            }
        }
        CadIntent::Select(payload) => {
            validate_non_empty("Select", "selector", &payload.selector)?;
        }
        CadIntent::CompareVariants(payload) => {
            if payload.variant_ids.is_empty() {
                return Err(CadIntentValidationError::new(
                    "CAD-INTENT-INVALID-RANGE",
                    Some("CompareVariants".to_string()),
                    Some("variant_ids".to_string()),
                    "variant_ids must include at least one ID",
                ));
            }
            for variant in &payload.variant_ids {
                if variant.trim().is_empty() {
                    return Err(CadIntentValidationError::new(
                        "CAD-INTENT-INVALID-FIELD",
                        Some("CompareVariants".to_string()),
                        Some("variant_ids".to_string()),
                        "variant IDs must not be empty",
                    ));
                }
            }
        }
        CadIntent::Export(payload) => {
            validate_non_empty("Export", "format", &payload.format)?;
            validate_non_empty("Export", "variant_id", &payload.variant_id)?;
        }
    }
    Ok(())
}

fn validate_non_empty(
    intent: &str,
    field: &str,
    value: &str,
) -> Result<(), CadIntentValidationError> {
    if value.trim().is_empty() {
        return Err(CadIntentValidationError::new(
            "CAD-INTENT-INVALID-FIELD",
            Some(intent.to_string()),
            Some(field.to_string()),
            format!("{} must not be empty", field),
        ));
    }
    Ok(())
}

fn validate_finite_range(
    intent: &str,
    field: &str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), CadIntentValidationError> {
    if !value.is_finite() || value < min || value > max {
        return Err(CadIntentValidationError::new(
            "CAD-INTENT-INVALID-RANGE",
            Some(intent.to_string()),
            Some(field.to_string()),
            format!("{field} must be finite and in range [{min}, {max}]"),
        ));
    }
    Ok(())
}

fn strip_intent_field(object: &Map<String, Value>) -> Value {
    let mut content = object.clone();
    let _ = content.remove("intent");
    Value::Object(content)
}

fn parse_payload<T>(intent_name: &str, payload: Value) -> Result<T, CadIntentValidationError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value::<T>(payload).map_err(|error| {
        CadIntentValidationError::new(
            "CAD-INTENT-INVALID-PAYLOAD",
            Some(intent_name.to_string()),
            None,
            format!("intent payload failed schema validation: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CadIntent, CadIntentValidationError, allowed_intent_names, cad_intent_json_schema,
        parse_cad_intent_json, parse_cad_intent_json_cad_result,
    };

    #[test]
    fn schema_lists_allowed_intents() {
        let schema = cad_intent_json_schema();
        let names = schema
            .get("allowed_intents")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(names.len(), allowed_intent_names().len());
        assert!(names.iter().any(|value| value == "CreateRackSpec"));
        assert!(
            names
                .iter()
                .any(|value| value == "CreateParallelJawGripperSpec")
        );
    }

    #[test]
    fn parse_rejects_unknown_intent() {
        let payload = r#"{"intent":"InventOperation","foo":1}"#;
        let error = parse_cad_intent_json(payload).expect_err("unknown intent should fail");
        assert_eq!(error.code, "CAD-INTENT-UNKNOWN-OP");
        assert_eq!(error.field.as_deref(), Some("intent"));
    }

    #[test]
    fn parse_rejects_unknown_fields_via_strict_schema() {
        let payload = r#"{"intent":"SetMaterial","material_id":"al-6061-t6","x":1}"#;
        let error =
            parse_cad_intent_json(payload).expect_err("unknown field in strict schema should fail");
        assert_eq!(error.code, "CAD-INTENT-INVALID-PAYLOAD");
    }

    #[test]
    fn parse_and_validate_happy_path() {
        let payload = r#"{"intent":"AdjustParameter","parameter":"vent_spacing_mm","operation":"set","value":14.5}"#;
        let intent = parse_cad_intent_json(payload).expect("payload should parse");
        match intent {
            CadIntent::AdjustParameter(value) => {
                assert_eq!(value.parameter, "vent_spacing_mm");
                assert!((value.value - 14.5).abs() < f64::EPSILON);
            }
            other => panic!("unexpected intent variant: {other:?}"),
        }
    }

    #[test]
    fn parse_and_validate_parallel_jaw_gripper_spec() {
        let payload = r#"{
            "intent":"CreateParallelJawGripperSpec",
            "jaw_open_mm":42.0,
            "finger_length_mm":65.0,
            "finger_thickness_mm":8.0,
            "base_width_mm":78.0,
            "base_depth_mm":52.0,
            "base_thickness_mm":8.0,
            "servo_mount_hole_diameter_mm":2.9,
            "print_fit_mm":0.15,
            "print_clearance_mm":0.35
        }"#;
        let parsed = parse_cad_intent_json(payload).expect("parallel jaw payload should parse");
        match parsed {
            CadIntent::CreateParallelJawGripperSpec(spec) => {
                assert!((spec.jaw_open_mm - 42.0).abs() < f64::EPSILON);
                assert!((spec.print_fit_mm - 0.15).abs() < f64::EPSILON);
                assert!(!spec.underactuated_mode);
                assert_eq!(spec.compliant_joint_count, 0);
                assert!((spec.flexure_thickness_mm - 1.4).abs() < f64::EPSILON);
                assert!(spec.single_servo_drive);
            }
            other => panic!("unexpected intent variant: {other:?}"),
        }
    }

    #[test]
    fn parse_and_validate_underactuated_parallel_jaw_gripper_spec() {
        let payload = r#"{
            "intent":"CreateParallelJawGripperSpec",
            "jaw_open_mm":36.0,
            "finger_length_mm":66.0,
            "finger_thickness_mm":7.0,
            "base_width_mm":78.0,
            "base_depth_mm":52.0,
            "base_thickness_mm":8.0,
            "servo_mount_hole_diameter_mm":2.9,
            "print_fit_mm":0.15,
            "print_clearance_mm":0.35,
            "underactuated_mode":true,
            "compliant_joint_count":3,
            "flexure_thickness_mm":1.2,
            "single_servo_drive":true
        }"#;
        let parsed = parse_cad_intent_json(payload).expect("underactuated payload should parse");
        match parsed {
            CadIntent::CreateParallelJawGripperSpec(spec) => {
                assert!(spec.underactuated_mode);
                assert_eq!(spec.compliant_joint_count, 3);
                assert!((spec.flexure_thickness_mm - 1.2).abs() < f64::EPSILON);
                assert!(spec.single_servo_drive);
            }
            other => panic!("unexpected intent variant: {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_invalid_parallel_jaw_clearance_relationship() {
        let payload = r#"{
            "intent":"CreateParallelJawGripperSpec",
            "jaw_open_mm":42.0,
            "finger_length_mm":65.0,
            "finger_thickness_mm":8.0,
            "base_width_mm":78.0,
            "base_depth_mm":52.0,
            "base_thickness_mm":8.0,
            "servo_mount_hole_diameter_mm":2.9,
            "print_fit_mm":0.35,
            "print_clearance_mm":0.15
        }"#;
        let error = parse_cad_intent_json(payload)
            .expect_err("clearance <= fit should fail validation");
        assert_eq!(error.code, "CAD-INTENT-INVALID-RANGE");
        assert_eq!(
            error.intent.as_deref(),
            Some("CreateParallelJawGripperSpec")
        );
        assert_eq!(error.field.as_deref(), Some("print_clearance_mm"));
    }

    #[test]
    fn parse_rejects_invalid_underactuated_single_servo_contract() {
        let payload = r#"{
            "intent":"CreateParallelJawGripperSpec",
            "jaw_open_mm":42.0,
            "finger_length_mm":65.0,
            "finger_thickness_mm":8.0,
            "base_width_mm":78.0,
            "base_depth_mm":52.0,
            "base_thickness_mm":8.0,
            "servo_mount_hole_diameter_mm":2.9,
            "print_fit_mm":0.15,
            "print_clearance_mm":0.35,
            "underactuated_mode":true,
            "compliant_joint_count":2,
            "flexure_thickness_mm":1.2,
            "single_servo_drive":false
        }"#;
        let error = parse_cad_intent_json(payload)
            .expect_err("underactuated mode without single-servo assumption should fail");
        assert_eq!(error.code, "CAD-INTENT-INVALID-FIELD");
        assert_eq!(error.field.as_deref(), Some("single_servo_drive"));
    }

    #[test]
    fn parse_reports_machine_readable_range_errors() {
        let payload = r#"{"intent":"GenerateVariants","count":0,"objective_set":"rack-demo"}"#;
        let error = parse_cad_intent_json(payload).expect_err("count 0 should fail");
        assert_eq!(error.code, "CAD-INTENT-INVALID-RANGE");
        assert_eq!(error.intent.as_deref(), Some("GenerateVariants"));
        assert_eq!(error.field.as_deref(), Some("count"));
    }

    #[test]
    fn cad_result_bridge_maps_to_parse_failed() {
        let payload = r#"{"intent":"SetMaterial","material_id":""}"#;
        let error = parse_cad_intent_json_cad_result(payload)
            .expect_err("invalid intent should map to cad error");
        assert!(error.to_string().contains("parse failed"));
    }

    #[test]
    fn validation_error_payload_is_serde_stable() {
        let error = CadIntentValidationError {
            code: "CAD-INTENT-INVALID-FIELD".to_string(),
            intent: Some("SetObjective".to_string()),
            field: Some("objective".to_string()),
            message: "objective must not be empty".to_string(),
        };
        let encoded = serde_json::to_string(&error).expect("error should serialize");
        assert!(encoded.contains("CAD-INTENT-INVALID-FIELD"));
        assert!(encoded.contains("SetObjective"));
    }
}
