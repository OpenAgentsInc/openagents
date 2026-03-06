use crate::intent::{
    AdjustParameterIntent, CadAdjustOperation, CadIntent, CadIntentValidationError,
    CompareVariantsIntent, CreateParallelJawGripperSpecIntent, CreateRackSpecIntent, ExportIntent,
    PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_COUNT, PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_POSE_PRESET,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_BRACKET_THICKNESS_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_HEIGHT_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_LENGTH_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_WIDTH_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_HOUSING_WALL_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_PATTERN_PITCH_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_SHAFT_AXIS_OFFSET_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_STANDOFF_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_BEND_RADIUS_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_CHANNEL_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_ROUTE_CLEARANCE_MM,
    PARALLEL_JAW_GRIPPER_DEFAULT_THUMB_BASE_ANGLE_DEG,
    PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT, PARALLEL_JAW_GRIPPER_MAX_FLEXURE_THICKNESS_MM,
    PARALLEL_JAW_GRIPPER_MAX_JOINT_MAX_DEG, PARALLEL_JAW_GRIPPER_MAX_JOINT_MIN_DEG,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_BRACKET_THICKNESS_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_HEIGHT_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_LENGTH_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_WIDTH_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_HOUSING_WALL_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_PATTERN_PITCH_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_SHAFT_AXIS_OFFSET_MM,
    PARALLEL_JAW_GRIPPER_MAX_SERVO_STANDOFF_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_MAX_TENDON_BEND_RADIUS_MM,
    PARALLEL_JAW_GRIPPER_MAX_TENDON_CHANNEL_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_MAX_TENDON_ROUTE_CLEARANCE_MM,
    PARALLEL_JAW_GRIPPER_MIN_FLEXURE_THICKNESS_MM, PARALLEL_JAW_GRIPPER_MIN_JOINT_MAX_DEG,
    PARALLEL_JAW_GRIPPER_MIN_JOINT_MIN_DEG, PARALLEL_JAW_GRIPPER_MIN_SERVO_BRACKET_THICKNESS_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_HEIGHT_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_LENGTH_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_WIDTH_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_HOUSING_WALL_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_PATTERN_PITCH_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_SHAFT_AXIS_OFFSET_MM,
    PARALLEL_JAW_GRIPPER_MIN_SERVO_STANDOFF_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_MIN_TENDON_BEND_RADIUS_MM,
    PARALLEL_JAW_GRIPPER_MIN_TENDON_CHANNEL_DIAMETER_MM,
    PARALLEL_JAW_GRIPPER_MIN_TENDON_ROUTE_CLEARANCE_MM, SelectIntent, SetMaterialIntent,
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

    if looks_like_gripper_prompt(input) {
        return parse_failure(
            "CAD-CHAT-GRIPPER-AMBIGUOUS",
            "gripper prompt is missing deterministic week-1 details",
            "For week-1 use: `Create a basic 2-jaw robotic gripper with a base plate, two parallel fingers, and mounting holes for a servo motor. Make it 3D-printable and parametric for easy scaling.` or send explicit intent_json.",
        );
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

    if lower.contains("vent")
        && lower.contains("hole")
        && lower.contains('%')
        && let Some(percent) = extract_percent_value(&lower)
    {
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

    if let Some(intent) = translate_parallel_jaw_gripper_prompt(&lower) {
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

fn translate_parallel_jaw_gripper_prompt(lower: &str) -> Option<CadIntent> {
    let has_design_verb = [
        "design",
        "build",
        "create",
        "model",
        "draft",
        "generate",
        "modify",
        "evolve",
        "add",
        "integrate",
        "incorporate",
    ]
    .iter()
    .any(|verb| lower.contains(verb));
    let has_gripper_target = [
        "gripper",
        "parallel-jaw",
        "parallel jaw",
        "robot hand",
        "robotic hand",
        "humanoid hand",
        "5-finger",
        "5 finger",
        "five-finger",
        "five finger",
        "finger joint",
        "opposable thumb",
        "fingertip",
        "fingertips",
        "sensor",
        "control board",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if !has_design_verb || !has_gripper_target {
        return None;
    }

    let mut spec = CreateParallelJawGripperSpecIntent::default();

    if lower.contains("wide") || lower.contains("large jaw") {
        spec.jaw_open_mm = 64.0;
        spec.base_width_mm = 94.0;
    }
    if lower.contains("long reach") || lower.contains("extended reach") {
        spec.finger_length_mm = 85.0;
    }
    if lower.contains("stiff") || lower.contains("stronger finger") {
        spec.finger_thickness_mm = 10.0;
    }
    if lower.contains("m2.5") {
        spec.servo_mount_hole_diameter_mm = 2.9;
    } else if lower.contains("m2") {
        spec.servo_mount_hole_diameter_mm = 2.2;
    } else if lower.contains("m3") {
        spec.servo_mount_hole_diameter_mm = 3.2;
    }
    if lower.contains("tight fit") {
        spec.print_fit_mm = 0.12;
        spec.print_clearance_mm = 0.28;
    } else if lower.contains("loose fit") {
        spec.print_fit_mm = 0.2;
        spec.print_clearance_mm = 0.4;
    }
    let requests_underactuated = [
        "underactuated",
        "compliant",
        "openhand",
        "adaptive grasp",
        "single servo",
        "single-drive",
        "single drive",
        "flexure",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if requests_underactuated {
        spec.underactuated_mode = true;
        spec.compliant_joint_count = 3;
        spec.flexure_thickness_mm = PARALLEL_JAW_GRIPPER_DEFAULT_FLEXURE_THICKNESS_MM;
        spec.single_servo_drive = true;
    }
    if lower.contains("dual servo")
        || lower.contains("multi servo")
        || lower.contains("independent servos")
    {
        spec.single_servo_drive = false;
    }
    if let Some(count) = extract_compliant_joint_count(lower) {
        spec.underactuated_mode = true;
        spec.compliant_joint_count = count.min(PARALLEL_JAW_GRIPPER_MAX_COMPLIANT_JOINT_COUNT);
    }
    if let Some(flexure_mm) = extract_flexure_thickness_mm(lower) {
        spec.underactuated_mode = true;
        spec.flexure_thickness_mm = flexure_mm.clamp(
            PARALLEL_JAW_GRIPPER_MIN_FLEXURE_THICKNESS_MM,
            PARALLEL_JAW_GRIPPER_MAX_FLEXURE_THICKNESS_MM,
        );
    }
    let requests_three_finger_thumb = [
        "3-finger",
        "3 finger",
        "three-finger",
        "three finger",
        "opposable thumb",
        "thumb",
        "tripod",
        "pinch",
        "tendon-driven",
        "tendon driven",
        "tendon channel",
        "cable routing",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if requests_three_finger_thumb {
        spec.finger_count = PARALLEL_JAW_GRIPPER_DEFAULT_FINGER_COUNT.max(3);
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(3);
        spec.thumb_base_angle_deg = PARALLEL_JAW_GRIPPER_DEFAULT_THUMB_BASE_ANGLE_DEG;
        spec.tendon_channel_diameter_mm = PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_CHANNEL_DIAMETER_MM;
        spec.tendon_route_clearance_mm = PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_ROUTE_CLEARANCE_MM;
        spec.tendon_bend_radius_mm = PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_BEND_RADIUS_MM;
        spec.pose_preset = if lower.contains("tripod") {
            "tripod".to_string()
        } else if lower.contains("pinch") {
            "pinch".to_string()
        } else {
            PARALLEL_JAW_GRIPPER_DEFAULT_POSE_PRESET.to_string()
        };
    }
    if let Some(count) = extract_finger_count(lower) {
        spec.finger_count = count.max(3);
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(3);
    }
    if let Some(angle_deg) = extract_thumb_base_angle_deg(lower) {
        spec.thumb_base_angle_deg = angle_deg;
        spec.opposable_thumb = true;
    }
    if let Some(channel_mm) = extract_tendon_channel_diameter_mm(lower) {
        spec.tendon_channel_diameter_mm = channel_mm.clamp(
            PARALLEL_JAW_GRIPPER_MIN_TENDON_CHANNEL_DIAMETER_MM,
            PARALLEL_JAW_GRIPPER_MAX_TENDON_CHANNEL_DIAMETER_MM,
        );
    }
    if let Some((joint_min_deg, joint_max_deg)) = extract_joint_range_deg(lower) {
        spec.joint_min_deg = joint_min_deg;
        spec.joint_max_deg = joint_max_deg.max(joint_min_deg + 5.0);
    }
    if let Some(clearance_mm) = extract_tendon_route_clearance_mm(lower) {
        spec.tendon_route_clearance_mm = clearance_mm.clamp(
            PARALLEL_JAW_GRIPPER_MIN_TENDON_ROUTE_CLEARANCE_MM,
            PARALLEL_JAW_GRIPPER_MAX_TENDON_ROUTE_CLEARANCE_MM,
        );
    }
    if let Some(radius_mm) = extract_tendon_bend_radius_mm(lower) {
        spec.tendon_bend_radius_mm = radius_mm.clamp(
            PARALLEL_JAW_GRIPPER_MIN_TENDON_BEND_RADIUS_MM,
            PARALLEL_JAW_GRIPPER_MAX_TENDON_BEND_RADIUS_MM,
        );
    }
    if lower.contains("tripod") {
        spec.pose_preset = "tripod".to_string();
    } else if lower.contains("pinch") {
        spec.pose_preset = "pinch".to_string();
    }
    let requests_motor_integration = [
        "servo",
        "motor",
        "actuation system",
        "gearbox housing",
        "motor mount",
        "wiring path",
        "compact layout",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if requests_motor_integration {
        spec.servo_integration_enabled = true;
        spec.finger_count = spec.finger_count.max(3);
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(3);
        spec.servo_envelope_length_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_LENGTH_MM;
        spec.servo_envelope_width_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_WIDTH_MM;
        spec.servo_envelope_height_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_ENVELOPE_HEIGHT_MM;
        spec.servo_shaft_axis_offset_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_SHAFT_AXIS_OFFSET_MM;
        spec.servo_mount_pattern_pitch_mm =
            PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_MOUNT_PATTERN_PITCH_MM;
        spec.servo_bracket_thickness_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_BRACKET_THICKNESS_MM;
        spec.servo_housing_wall_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_HOUSING_WALL_MM;
        spec.servo_standoff_diameter_mm = PARALLEL_JAW_GRIPPER_DEFAULT_SERVO_STANDOFF_DIAMETER_MM;
        spec.compact_servo_layout =
            lower.contains("compact") || lower.contains("low-cost") || lower.contains("low cost");
    }
    let requests_sensor_electronics = [
        "force sensor",
        "fingertip sensor",
        "proximity sensor",
        "control board",
        "electronics mount",
        "sensor feedback",
        "modular",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if requests_sensor_electronics {
        spec.servo_integration_enabled = true;
        spec.finger_count = spec.finger_count.max(3);
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(3);
        spec.tendon_channel_diameter_mm = spec
            .tendon_channel_diameter_mm
            .max(PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_CHANNEL_DIAMETER_MM);
        spec.tendon_route_clearance_mm = spec
            .tendon_route_clearance_mm
            .max(PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_ROUTE_CLEARANCE_MM);
        spec.tendon_bend_radius_mm = spec
            .tendon_bend_radius_mm
            .max(PARALLEL_JAW_GRIPPER_DEFAULT_TENDON_BEND_RADIUS_MM);
    }
    if let Some(value) = extract_servo_envelope_length_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_envelope_length_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_LENGTH_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_LENGTH_MM,
        );
    }
    if let Some(value) = extract_servo_envelope_width_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_envelope_width_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_WIDTH_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_WIDTH_MM,
        );
    }
    if let Some(value) = extract_servo_envelope_height_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_envelope_height_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_ENVELOPE_HEIGHT_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_ENVELOPE_HEIGHT_MM,
        );
    }
    if let Some(value) = extract_servo_shaft_axis_offset_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_shaft_axis_offset_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_SHAFT_AXIS_OFFSET_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_SHAFT_AXIS_OFFSET_MM,
        );
    }
    if let Some(value) = extract_servo_mount_pattern_pitch_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_mount_pattern_pitch_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_MOUNT_PATTERN_PITCH_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_MOUNT_PATTERN_PITCH_MM,
        );
    }
    if let Some(value) = extract_servo_bracket_thickness_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_bracket_thickness_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_BRACKET_THICKNESS_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_BRACKET_THICKNESS_MM,
        );
    }
    if let Some(value) = extract_servo_housing_wall_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_housing_wall_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_HOUSING_WALL_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_HOUSING_WALL_MM,
        );
    }
    if let Some(value) = extract_servo_standoff_diameter_mm(lower) {
        spec.servo_integration_enabled = true;
        spec.servo_standoff_diameter_mm = value.clamp(
            PARALLEL_JAW_GRIPPER_MIN_SERVO_STANDOFF_DIAMETER_MM,
            PARALLEL_JAW_GRIPPER_MAX_SERVO_STANDOFF_DIAMETER_MM,
        );
    }
    if spec.servo_integration_enabled {
        spec.finger_count = spec.finger_count.max(3);
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(3);
    }
    let requests_humanoid_full_hand = [
        "humanoid hand",
        "5-finger",
        "5 finger",
        "five-finger",
        "five finger",
        "full hand",
        "fully functioning",
        "complete hand assembly",
        "arm interface",
        "mounting arm",
    ]
    .iter()
    .any(|token| lower.contains(token));
    if requests_humanoid_full_hand {
        spec.finger_count = 5;
        spec.opposable_thumb = true;
        spec.underactuated_mode = true;
        spec.compliant_joint_count = spec.compliant_joint_count.max(4);
        spec.single_servo_drive = false;
        spec.servo_integration_enabled = true;
        spec.compact_servo_layout =
            spec.compact_servo_layout || lower.contains("compact") || lower.contains("low-cost");
        spec.pose_preset = if lower.contains("tripod") {
            "tripod".to_string()
        } else if lower.contains("pinch") {
            "pinch".to_string()
        } else if lower.contains("open") {
            "open".to_string()
        } else {
            "precision".to_string()
        };
    }

    Some(CadIntent::CreateParallelJawGripperSpec(spec))
}

fn looks_like_gripper_prompt(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    ["gripper", "robot hand", "robotic hand", "parallel jaw"]
        .iter()
        .any(|token| lower.contains(token))
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

fn extract_compliant_joint_count(lower: &str) -> Option<u8> {
    for token in lower.split_whitespace() {
        if let Ok(value) = token.parse::<u8>() && value > 0 {
            return Some(value);
        }
    }
    None
}

fn extract_finger_count(lower: &str) -> Option<u8> {
    if lower.contains("3-finger")
        || lower.contains("3 finger")
        || lower.contains("three-finger")
        || lower.contains("three finger")
    {
        return Some(3);
    }
    for token in lower.split_whitespace() {
        let trimmed = token.trim_matches(|ch: char| !ch.is_ascii_digit());
        if let Ok(value) = trimmed.parse::<u8>()
            && (3..=5).contains(&value)
            && lower.contains("finger")
        {
            return Some(value);
        }
    }
    None
}

fn extract_flexure_thickness_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("flexure thickness") {
        let tail = &lower[index + "flexure thickness".len()..];
        if let Some(value) = extract_first_numeric_token(tail) {
            return Some(value);
        }
    }
    let flexure_index = lower.find("flexure")?;
    let tail = &lower[flexure_index..];
    for raw in tail.split_whitespace() {
        if !raw.contains("mm") {
            continue;
        }
        let cleaned = raw.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
        if cleaned.is_empty() {
            continue;
        }
        if let Ok(value) = cleaned.parse::<f64>() {
            return Some(value);
        }
    }
    None
}

fn extract_thumb_base_angle_deg(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("thumb angle") {
        return extract_first_numeric_token(&lower[index + "thumb angle".len()..]);
    }
    if let Some(index) = lower.find("thumb base angle") {
        return extract_first_numeric_token(&lower[index + "thumb base angle".len()..]);
    }
    None
}

fn extract_tendon_channel_diameter_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("tendon channel") {
        return extract_first_numeric_token(&lower[index + "tendon channel".len()..]);
    }
    if let Some(index) = lower.find("channel diameter") {
        return extract_first_numeric_token(&lower[index + "channel diameter".len()..]);
    }
    None
}

fn extract_joint_range_deg(lower: &str) -> Option<(f64, f64)> {
    if let Some(index) = lower.find("joint range") {
        let tail = &lower[index + "joint range".len()..];
        let numbers = extract_numeric_tokens(tail);
        if numbers.len() >= 2 {
            let min_value = numbers[0].clamp(
                PARALLEL_JAW_GRIPPER_MIN_JOINT_MIN_DEG,
                PARALLEL_JAW_GRIPPER_MAX_JOINT_MIN_DEG,
            );
            let max_value = numbers[1].clamp(
                PARALLEL_JAW_GRIPPER_MIN_JOINT_MAX_DEG,
                PARALLEL_JAW_GRIPPER_MAX_JOINT_MAX_DEG,
            );
            return Some((min_value, max_value));
        }
    }
    if let Some(index) = lower.find("travel range") {
        let tail = &lower[index + "travel range".len()..];
        let numbers = extract_numeric_tokens(tail);
        if numbers.len() >= 2 {
            let min_value = numbers[0].clamp(
                PARALLEL_JAW_GRIPPER_MIN_JOINT_MIN_DEG,
                PARALLEL_JAW_GRIPPER_MAX_JOINT_MIN_DEG,
            );
            let max_value = numbers[1].clamp(
                PARALLEL_JAW_GRIPPER_MIN_JOINT_MAX_DEG,
                PARALLEL_JAW_GRIPPER_MAX_JOINT_MAX_DEG,
            );
            return Some((min_value, max_value));
        }
    }
    None
}

fn extract_tendon_route_clearance_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("routing clearance") {
        return extract_first_numeric_token(&lower[index + "routing clearance".len()..]);
    }
    if let Some(index) = lower.find("route clearance") {
        return extract_first_numeric_token(&lower[index + "route clearance".len()..]);
    }
    None
}

fn extract_tendon_bend_radius_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("bend radius") {
        return extract_first_numeric_token(&lower[index + "bend radius".len()..]);
    }
    None
}

fn extract_servo_envelope_length_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("servo length") {
        return extract_first_numeric_token(&lower[index + "servo length".len()..]);
    }
    if let Some(index) = lower.find("motor length") {
        return extract_first_numeric_token(&lower[index + "motor length".len()..]);
    }
    None
}

fn extract_servo_envelope_width_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("servo width") {
        return extract_first_numeric_token(&lower[index + "servo width".len()..]);
    }
    if let Some(index) = lower.find("motor width") {
        return extract_first_numeric_token(&lower[index + "motor width".len()..]);
    }
    None
}

fn extract_servo_envelope_height_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("servo height") {
        return extract_first_numeric_token(&lower[index + "servo height".len()..]);
    }
    if let Some(index) = lower.find("motor height") {
        return extract_first_numeric_token(&lower[index + "motor height".len()..]);
    }
    None
}

fn extract_servo_shaft_axis_offset_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("shaft offset") {
        return extract_first_numeric_token(&lower[index + "shaft offset".len()..]);
    }
    if let Some(index) = lower.find("shaft axis offset") {
        return extract_first_numeric_token(&lower[index + "shaft axis offset".len()..]);
    }
    None
}

fn extract_servo_mount_pattern_pitch_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("mount pattern") {
        return extract_first_numeric_token(&lower[index + "mount pattern".len()..]);
    }
    if let Some(index) = lower.find("pattern pitch") {
        return extract_first_numeric_token(&lower[index + "pattern pitch".len()..]);
    }
    None
}

fn extract_servo_bracket_thickness_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("bracket thickness") {
        return extract_first_numeric_token(&lower[index + "bracket thickness".len()..]);
    }
    None
}

fn extract_servo_housing_wall_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("housing wall") {
        return extract_first_numeric_token(&lower[index + "housing wall".len()..]);
    }
    None
}

fn extract_servo_standoff_diameter_mm(lower: &str) -> Option<f64> {
    if let Some(index) = lower.find("standoff diameter") {
        return extract_first_numeric_token(&lower[index + "standoff diameter".len()..]);
    }
    None
}

fn extract_numeric_tokens(input: &str) -> Vec<f64> {
    input
        .split_whitespace()
        .filter_map(|raw| {
            let cleaned = raw.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
            if cleaned.is_empty() {
                return None;
            }
            cleaned.parse::<f64>().ok()
        })
        .collect()
}

fn extract_first_numeric_token(input: &str) -> Option<f64> {
    for raw in input.split_whitespace() {
        let cleaned = raw.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
        if cleaned.is_empty() {
            continue;
        }
        if let Ok(value) = cleaned.parse::<f64>() {
            return Some(value);
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
    use crate::intent::{
        CadAdjustOperation, CadIntent, PARALLEL_JAW_GRIPPER_MIN_TENDON_CHANNEL_DIAMETER_MM,
    };

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

    #[test]
    fn adapter_translates_gripper_design_prompt_into_parallel_jaw_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Create a basic 2-jaw robotic gripper with a base plate, two parallel fingers, and mounting holes for a servo motor. Make it 3D-printable and parametric for easy scaling.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert!((spec.jaw_open_mm - 42.0).abs() < f64::EPSILON);
                assert!((spec.print_fit_mm - 0.15).abs() < f64::EPSILON);
                assert!((spec.print_clearance_mm - 0.35).abs() < f64::EPSILON);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_underactuated_compliance_prompt_into_typed_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Modify the gripper to be underactuated with compliant flexure joints and a single servo drive; use 3 compliant joints and 1.3mm flexure thickness.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert!(spec.underactuated_mode);
                assert_eq!(spec.compliant_joint_count, 3);
                assert!(spec.single_servo_drive);
                assert!((spec.flexure_thickness_mm - 1.3).abs() < f64::EPSILON);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_three_finger_thumb_prompt_into_hand_profile_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Evolve the gripper into a 3-finger hand with an opposable thumb, tendon-driven for dexterity. Add cable routing channels and tripod grasp pose.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert_eq!(spec.finger_count, 3);
                assert!(spec.opposable_thumb);
                assert!(spec.underactuated_mode);
                assert!(spec.compliant_joint_count >= 3);
                assert_eq!(spec.pose_preset, "tripod".to_string());
                assert!(
                    spec.tendon_channel_diameter_mm
                        >= PARALLEL_JAW_GRIPPER_MIN_TENDON_CHANNEL_DIAMETER_MM
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_motor_integration_prompt_into_servo_schema_fields() {
        let outcome = translate_chat_to_cad_intent(
            "Add servo motors to each finger joint, including wiring paths and gearbox housings. Optimize for compact layout and low-cost 3D printing.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert!(spec.servo_integration_enabled);
                assert!(spec.compact_servo_layout);
                assert!(spec.servo_envelope_length_mm > 0.0);
                assert!(spec.servo_envelope_width_mm > 0.0);
                assert!(spec.servo_envelope_height_mm > 0.0);
                assert!(spec.servo_mount_pattern_pitch_mm > 0.0);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_sensor_electronics_prompt_into_hand_profile_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Incorporate force sensors on fingertips, proximity sensors, and a control board mount. Ensure the design is modular for easy upgrades.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert!(spec.servo_integration_enabled);
                assert!(spec.underactuated_mode);
                assert!(spec.opposable_thumb);
                assert!(spec.finger_count >= 3);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_translates_full_humanoid_hand_prompt_into_profile_spec() {
        let outcome = translate_chat_to_cad_intent(
            "Generate a complete 5-finger humanoid robotic hand with all motors, tendons, sensors, electronics, and mounting arm interface.",
        );
        match outcome {
            CadIntentTranslationOutcome::Intent(CadIntent::CreateParallelJawGripperSpec(spec)) => {
                assert_eq!(spec.finger_count, 5);
                assert!(spec.opposable_thumb);
                assert!(spec.underactuated_mode);
                assert!(spec.servo_integration_enabled);
                assert_eq!(spec.pose_preset, "precision".to_string());
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn adapter_returns_clarification_for_ambiguous_gripper_prompt() {
        let outcome = translate_chat_to_cad_intent("Can you make a robot hand gripper?");
        match outcome {
            CadIntentTranslationOutcome::ParseFailure(error) => {
                assert_eq!(error.code, "CAD-CHAT-GRIPPER-AMBIGUOUS");
                assert!(error.recovery_prompt.contains("2-jaw robotic gripper"));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}
