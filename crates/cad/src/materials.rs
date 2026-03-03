use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hash::stable_hex_digest;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadMaterialPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub density_kg_m3: f64,
    pub youngs_modulus_gpa: f64,
    pub cnc_cost_usd_per_kg: f64,
    pub cnc_setup_usd: f64,
}

pub const DEFAULT_CAD_MATERIAL_ID: &str = "al-6061-t6";
pub const CAD_COST_HEURISTIC_MODEL_ID: &str = "cad.cost.wave1.v1";

const MATERIAL_PRESETS: [CadMaterialPreset; 4] = [
    CadMaterialPreset {
        id: "al-6061-t6",
        label: "Aluminum 6061-T6",
        density_kg_m3: 2_700.0,
        youngs_modulus_gpa: 69.0,
        cnc_cost_usd_per_kg: 18.0,
        cnc_setup_usd: 42.0,
    },
    CadMaterialPreset {
        id: "al-5052-h32",
        label: "Aluminum 5052-H32",
        density_kg_m3: 2_680.0,
        youngs_modulus_gpa: 70.0,
        cnc_cost_usd_per_kg: 16.0,
        cnc_setup_usd: 38.0,
    },
    CadMaterialPreset {
        id: "steel-1018",
        label: "Steel 1018",
        density_kg_m3: 7_870.0,
        youngs_modulus_gpa: 200.0,
        cnc_cost_usd_per_kg: 9.0,
        cnc_setup_usd: 56.0,
    },
    CadMaterialPreset {
        id: "ti-6al-4v",
        label: "Titanium Ti-6Al-4V",
        density_kg_m3: 4_430.0,
        youngs_modulus_gpa: 114.0,
        cnc_cost_usd_per_kg: 95.0,
        cnc_setup_usd: 84.0,
    },
];

pub fn material_presets() -> &'static [CadMaterialPreset] {
    &MATERIAL_PRESETS
}

pub fn material_preset_by_id(id: &str) -> Option<CadMaterialPreset> {
    material_presets()
        .iter()
        .copied()
        .find(|preset| preset.id.eq_ignore_ascii_case(id))
}

pub fn next_material_preset_id(current_id: &str) -> &'static str {
    let presets = material_presets();
    if presets.is_empty() {
        return DEFAULT_CAD_MATERIAL_ID;
    }
    let current_index = presets
        .iter()
        .position(|preset| preset.id.eq_ignore_ascii_case(current_id))
        .unwrap_or(0);
    let next_index = (current_index + 1) % presets.len();
    presets[next_index].id
}

pub fn estimate_material_cost_usd(mass_kg: f64, preset: CadMaterialPreset) -> Option<f64> {
    if !mass_kg.is_finite() || mass_kg < 0.0 {
        return None;
    }
    if !preset.density_kg_m3.is_finite()
        || !preset.youngs_modulus_gpa.is_finite()
        || !preset.cnc_cost_usd_per_kg.is_finite()
        || !preset.cnc_setup_usd.is_finite()
    {
        return None;
    }
    Some((mass_kg * preset.cnc_cost_usd_per_kg) + preset.cnc_setup_usd)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadMaterialAssignmentScope {
    Feature,
    Part,
    Default,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMaterialAssignmentReceipt {
    pub part_id: String,
    pub feature_id: String,
    pub material_id: String,
    pub scope: CadMaterialAssignmentScope,
    pub density_kg_m3: f64,
    pub assignment_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadMaterialAssignmentState {
    pub default_material_id: String,
    pub part_materials: BTreeMap<String, String>,
    pub feature_materials: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadMaterialAssignmentErrorCode {
    InvalidEntityId,
    UnknownMaterialPreset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadMaterialAssignmentError {
    pub code: CadMaterialAssignmentErrorCode,
    pub message: String,
}

impl CadMaterialAssignmentError {
    fn new(code: CadMaterialAssignmentErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Default for CadMaterialAssignmentState {
    fn default() -> Self {
        Self {
            default_material_id: DEFAULT_CAD_MATERIAL_ID.to_string(),
            part_materials: BTreeMap::new(),
            feature_materials: BTreeMap::new(),
        }
    }
}

impl CadMaterialAssignmentState {
    pub fn with_default_material(
        default_material_id: &str,
    ) -> Result<Self, CadMaterialAssignmentError> {
        let canonical_default = canonical_material_id(default_material_id)?;
        Ok(Self {
            default_material_id: canonical_default,
            part_materials: BTreeMap::new(),
            feature_materials: BTreeMap::new(),
        })
    }

    pub fn set_part_material(
        &mut self,
        part_id: &str,
        material_id: &str,
    ) -> Result<(), CadMaterialAssignmentError> {
        validate_entity_id(part_id, "part_id")?;
        let canonical_material = canonical_material_id(material_id)?;
        self.part_materials
            .insert(part_id.to_string(), canonical_material);
        Ok(())
    }

    pub fn set_feature_material(
        &mut self,
        feature_id: &str,
        material_id: &str,
    ) -> Result<(), CadMaterialAssignmentError> {
        validate_entity_id(feature_id, "feature_id")?;
        let canonical_material = canonical_material_id(material_id)?;
        self.feature_materials
            .insert(feature_id.to_string(), canonical_material);
        Ok(())
    }

    pub fn clear_part_material(&mut self, part_id: &str) {
        self.part_materials.remove(part_id);
    }

    pub fn clear_feature_material(&mut self, feature_id: &str) {
        self.feature_materials.remove(feature_id);
    }

    pub fn resolve_assignment(
        &self,
        part_id: &str,
        feature_id: &str,
    ) -> Result<CadMaterialAssignmentReceipt, CadMaterialAssignmentError> {
        validate_entity_id(part_id, "part_id")?;
        validate_entity_id(feature_id, "feature_id")?;

        let (scope, material_id) = if let Some(material) = self.feature_materials.get(feature_id) {
            (CadMaterialAssignmentScope::Feature, material.clone())
        } else if let Some(material) = self.part_materials.get(part_id) {
            (CadMaterialAssignmentScope::Part, material.clone())
        } else {
            (
                CadMaterialAssignmentScope::Default,
                self.default_material_id.clone(),
            )
        };

        let preset = material_preset_by_id(&material_id).ok_or_else(|| {
            CadMaterialAssignmentError::new(
                CadMaterialAssignmentErrorCode::UnknownMaterialPreset,
                format!("unknown material preset: {material_id}"),
            )
        })?;

        let assignment_hash = stable_hex_digest(
            format!(
                "material_assignment|part={}|feature={}|scope={:?}|material={}",
                part_id, feature_id, scope, material_id
            )
            .as_bytes(),
        )[..16]
            .to_string();

        Ok(CadMaterialAssignmentReceipt {
            part_id: part_id.to_string(),
            feature_id: feature_id.to_string(),
            material_id,
            scope,
            density_kg_m3: preset.density_kg_m3,
            assignment_hash,
        })
    }
}

fn validate_entity_id(entity_id: &str, label: &str) -> Result<(), CadMaterialAssignmentError> {
    if entity_id.trim().is_empty() {
        return Err(CadMaterialAssignmentError::new(
            CadMaterialAssignmentErrorCode::InvalidEntityId,
            format!("{label} must not be empty"),
        ));
    }
    Ok(())
}

fn canonical_material_id(material_id: &str) -> Result<String, CadMaterialAssignmentError> {
    let preset = material_preset_by_id(material_id).ok_or_else(|| {
        CadMaterialAssignmentError::new(
            CadMaterialAssignmentErrorCode::UnknownMaterialPreset,
            format!("unknown material preset: {material_id}"),
        )
    })?;
    Ok(preset.id.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadCostHeuristicInput {
    pub mass_kg: f64,
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub triangle_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadCostHeuristicErrorCode {
    InvalidMass,
    InvalidVolume,
    InvalidSurfaceArea,
    InvalidMaterialPreset,
}

impl CadCostHeuristicErrorCode {
    pub fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidMass => "CAD-COST-INVALID-MASS",
            Self::InvalidVolume => "CAD-COST-INVALID-VOLUME",
            Self::InvalidSurfaceArea => "CAD-COST-INVALID-SURFACE-AREA",
            Self::InvalidMaterialPreset => "CAD-COST-INVALID-MATERIAL",
        }
    }

    pub fn remediation_hint(self) -> &'static str {
        match self {
            Self::InvalidMass => {
                "Ensure body mass is finite and non-negative before cost estimation."
            }
            Self::InvalidVolume => "Rebuild geometry so volume is finite and non-negative.",
            Self::InvalidSurfaceArea => {
                "Rebuild tessellation so surface area is finite and non-negative."
            }
            Self::InvalidMaterialPreset => {
                "Use a material preset with finite non-negative CNC rates."
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadCostHeuristicError {
    pub code: CadCostHeuristicErrorCode,
    pub message: String,
}

impl CadCostHeuristicError {
    fn new(code: CadCostHeuristicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn remediation_hint(&self) -> &'static str {
        self.code.remediation_hint()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadCostHeuristicEstimate {
    pub total_cost_usd: f64,
    pub material_cost_usd: f64,
    pub setup_cost_usd: f64,
    pub machining_cost_usd: f64,
    pub machining_minutes: f64,
    pub complexity_factor: f64,
    pub metadata: BTreeMap<String, String>,
}

pub fn estimate_cnc_cost_heuristic_usd(
    input: CadCostHeuristicInput,
    preset: CadMaterialPreset,
) -> Result<CadCostHeuristicEstimate, CadCostHeuristicError> {
    if !input.mass_kg.is_finite() || input.mass_kg < 0.0 {
        return Err(CadCostHeuristicError::new(
            CadCostHeuristicErrorCode::InvalidMass,
            format!(
                "mass_kg must be finite and non-negative, got {}",
                input.mass_kg
            ),
        ));
    }
    if !input.volume_mm3.is_finite() || input.volume_mm3 < 0.0 {
        return Err(CadCostHeuristicError::new(
            CadCostHeuristicErrorCode::InvalidVolume,
            format!(
                "volume_mm3 must be finite and non-negative, got {}",
                input.volume_mm3
            ),
        ));
    }
    if !input.surface_area_mm2.is_finite() || input.surface_area_mm2 < 0.0 {
        return Err(CadCostHeuristicError::new(
            CadCostHeuristicErrorCode::InvalidSurfaceArea,
            format!(
                "surface_area_mm2 must be finite and non-negative, got {}",
                input.surface_area_mm2
            ),
        ));
    }
    if !preset.cnc_cost_usd_per_kg.is_finite()
        || !preset.cnc_setup_usd.is_finite()
        || preset.cnc_cost_usd_per_kg < 0.0
        || preset.cnc_setup_usd < 0.0
    {
        return Err(CadCostHeuristicError::new(
            CadCostHeuristicErrorCode::InvalidMaterialPreset,
            format!(
                "material preset has invalid CNC rates (kg={}, setup={})",
                preset.cnc_cost_usd_per_kg, preset.cnc_setup_usd
            ),
        ));
    }

    const BASE_MACHINE_RATE_USD_PER_MIN: f64 = 1.35;
    const BASE_PROGRAMMING_MINUTES: f64 = 4.0;
    const SURFACE_MINUTES_PER_CM2: f64 = 0.045;
    const TRIANGLE_COMPLEXITY_WEIGHT: f64 = 0.07;
    const THIN_WALL_RATIO_WEIGHT: f64 = 0.22;

    let surface_area_cm2 = input.surface_area_mm2 * 0.01;
    let volume_cm3 = input.volume_mm3 * 0.001;
    let surface_to_volume_ratio = if volume_cm3 > 1e-9 {
        surface_area_cm2 / volume_cm3
    } else {
        0.0
    };
    let thin_wall_factor = 1.0 + (surface_to_volume_ratio * THIN_WALL_RATIO_WEIGHT).clamp(0.0, 2.5);
    let triangle_complexity_factor = 1.0
        + (((input.triangle_count as f64) + 1.0).ln() * TRIANGLE_COMPLEXITY_WEIGHT).clamp(0.0, 1.0);
    let complexity_factor = thin_wall_factor * triangle_complexity_factor;

    let base_minutes = BASE_PROGRAMMING_MINUTES + (surface_area_cm2 * SURFACE_MINUTES_PER_CM2);
    let machining_minutes = base_minutes * complexity_factor;
    let machining_cost_usd = machining_minutes * BASE_MACHINE_RATE_USD_PER_MIN;
    let material_cost_usd = input.mass_kg * preset.cnc_cost_usd_per_kg;
    let setup_cost_usd = preset.cnc_setup_usd;
    let total_cost_usd = material_cost_usd + setup_cost_usd + machining_cost_usd;

    let metadata = BTreeMap::from([
        (
            "model_id".to_string(),
            CAD_COST_HEURISTIC_MODEL_ID.to_string(),
        ),
        ("material_id".to_string(), preset.id.to_string()),
        (
            "assumption.machine_rate_usd_per_min".to_string(),
            fmt6(BASE_MACHINE_RATE_USD_PER_MIN),
        ),
        (
            "assumption.base_programming_minutes".to_string(),
            fmt6(BASE_PROGRAMMING_MINUTES),
        ),
        (
            "assumption.surface_minutes_per_cm2".to_string(),
            fmt6(SURFACE_MINUTES_PER_CM2),
        ),
        (
            "assumption.triangle_complexity_weight".to_string(),
            fmt6(TRIANGLE_COMPLEXITY_WEIGHT),
        ),
        (
            "assumption.thin_wall_ratio_weight".to_string(),
            fmt6(THIN_WALL_RATIO_WEIGHT),
        ),
        ("input.mass_kg".to_string(), fmt6(input.mass_kg)),
        ("input.volume_mm3".to_string(), fmt6(input.volume_mm3)),
        (
            "input.surface_area_mm2".to_string(),
            fmt6(input.surface_area_mm2),
        ),
        (
            "input.triangle_count".to_string(),
            input.triangle_count.to_string(),
        ),
        (
            "derived.surface_area_cm2".to_string(),
            fmt6(surface_area_cm2),
        ),
        ("derived.volume_cm3".to_string(), fmt6(volume_cm3)),
        (
            "derived.surface_to_volume_ratio".to_string(),
            fmt6(surface_to_volume_ratio),
        ),
        (
            "derived.thin_wall_factor".to_string(),
            fmt6(thin_wall_factor),
        ),
        (
            "derived.triangle_complexity_factor".to_string(),
            fmt6(triangle_complexity_factor),
        ),
        (
            "derived.complexity_factor".to_string(),
            fmt6(complexity_factor),
        ),
        (
            "derived.machining_minutes".to_string(),
            fmt6(machining_minutes),
        ),
        (
            "component.material_cost_usd".to_string(),
            fmt6(material_cost_usd),
        ),
        ("component.setup_cost_usd".to_string(), fmt6(setup_cost_usd)),
        (
            "component.machining_cost_usd".to_string(),
            fmt6(machining_cost_usd),
        ),
        ("result.total_cost_usd".to_string(), fmt6(total_cost_usd)),
    ]);

    Ok(CadCostHeuristicEstimate {
        total_cost_usd,
        material_cost_usd,
        setup_cost_usd,
        machining_cost_usd,
        machining_minutes,
        complexity_factor,
        metadata,
    })
}

fn fmt6(value: f64) -> String {
    format!("{value:.6}")
}

#[cfg(test)]
mod tests {
    use super::{
        CAD_COST_HEURISTIC_MODEL_ID, CadCostHeuristicErrorCode, CadCostHeuristicInput,
        CadMaterialAssignmentScope, CadMaterialAssignmentState, DEFAULT_CAD_MATERIAL_ID,
        estimate_cnc_cost_heuristic_usd, estimate_material_cost_usd, material_preset_by_id,
        material_presets, next_material_preset_id,
    };

    #[test]
    fn preset_table_is_stable_and_contains_default() {
        let presets = material_presets();
        assert!(presets.len() >= 3);
        assert!(
            presets
                .iter()
                .any(|preset| preset.id == DEFAULT_CAD_MATERIAL_ID)
        );
        assert!(presets.iter().all(|preset| preset.density_kg_m3 > 0.0));
        assert!(presets.iter().all(|preset| preset.youngs_modulus_gpa > 0.0));
    }

    #[test]
    fn lookup_and_cycle_are_deterministic() {
        let al = material_preset_by_id("al-6061-t6").expect("default material should resolve");
        assert_eq!(al.label, "Aluminum 6061-T6");
        assert_eq!(next_material_preset_id("al-6061-t6"), "al-5052-h32");
        assert_eq!(next_material_preset_id("ti-6al-4v"), "al-6061-t6");
        // Unknown ids should always map to first-next deterministic entry.
        assert_eq!(next_material_preset_id("unknown-material"), "al-5052-h32");
    }

    #[test]
    fn material_cost_estimate_is_stable() {
        let steel = material_preset_by_id("steel-1018").expect("steel preset should exist");
        let first = estimate_material_cost_usd(3.2, steel).expect("cost estimate should exist");
        let second =
            estimate_material_cost_usd(3.2, steel).expect("cost estimate should be stable");
        assert_eq!(first, second);
        assert!((first - 84.8).abs() < 1e-12);
        assert!(estimate_material_cost_usd(-1.0, steel).is_none());
    }

    #[test]
    fn cnc_cost_heuristic_is_deterministic_and_metadata_rich() {
        let material = material_preset_by_id("al-6061-t6").expect("material should exist");
        let input = CadCostHeuristicInput {
            mass_kg: 2.7,
            volume_mm3: 1_000_000.0,
            surface_area_mm2: 425_000.0,
            triangle_count: 128,
        };
        let first = estimate_cnc_cost_heuristic_usd(input, material)
            .expect("heuristic estimate should work");
        let second = estimate_cnc_cost_heuristic_usd(input, material)
            .expect("heuristic estimate should remain deterministic");
        assert_eq!(first, second);
        assert!(first.total_cost_usd > first.setup_cost_usd);
        assert_eq!(
            first.metadata.get("model_id").map(String::as_str),
            Some(CAD_COST_HEURISTIC_MODEL_ID)
        );
        assert!(
            first
                .metadata
                .contains_key("assumption.machine_rate_usd_per_min")
        );
        assert!(first.metadata.contains_key("derived.complexity_factor"));
    }

    #[test]
    fn cnc_cost_heuristic_classifies_invalid_inputs() {
        let material = material_preset_by_id("al-6061-t6").expect("material should exist");
        let error = estimate_cnc_cost_heuristic_usd(
            CadCostHeuristicInput {
                mass_kg: -1.0,
                volume_mm3: 1.0,
                surface_area_mm2: 1.0,
                triangle_count: 1,
            },
            material,
        )
        .expect_err("negative mass must fail");
        assert_eq!(error.code, CadCostHeuristicErrorCode::InvalidMass);
        assert_eq!(error.code.stable_code(), "CAD-COST-INVALID-MASS");
        assert!(!error.remediation_hint().is_empty());
    }

    #[test]
    fn material_assignment_resolves_feature_over_part_over_default() {
        let mut assignments =
            CadMaterialAssignmentState::with_default_material("al-6061-t6").expect("default");
        assignments
            .set_part_material("part.housing", "steel-1018")
            .expect("part assignment");
        assignments
            .set_feature_material("feature.fillet.1", "ti-6al-4v")
            .expect("feature assignment");

        let feature_receipt = assignments
            .resolve_assignment("part.housing", "feature.fillet.1")
            .expect("feature receipt");
        assert_eq!(feature_receipt.scope, CadMaterialAssignmentScope::Feature);
        assert_eq!(feature_receipt.material_id, "ti-6al-4v");

        let part_receipt = assignments
            .resolve_assignment("part.housing", "feature.chamfer.1")
            .expect("part receipt");
        assert_eq!(part_receipt.scope, CadMaterialAssignmentScope::Part);
        assert_eq!(part_receipt.material_id, "steel-1018");

        let default_receipt = assignments
            .resolve_assignment("part.frame", "feature.base")
            .expect("default receipt");
        assert_eq!(default_receipt.scope, CadMaterialAssignmentScope::Default);
        assert_eq!(default_receipt.material_id, "al-6061-t6");
    }

    #[test]
    fn material_assignment_rejects_unknown_materials() {
        let mut assignments = CadMaterialAssignmentState::default();
        let error = assignments
            .set_part_material("part.housing", "unknown-material")
            .expect_err("unknown material should fail");
        assert!(error.message.contains("unknown material preset"));
    }

    #[test]
    fn material_assignment_receipts_are_deterministic() {
        let mut assignments = CadMaterialAssignmentState::default();
        assignments
            .set_part_material("part.housing", "steel-1018")
            .expect("part assignment");
        let first = assignments
            .resolve_assignment("part.housing", "feature.base")
            .expect("first receipt");
        let second = assignments
            .resolve_assignment("part.housing", "feature.base")
            .expect("second receipt");
        assert_eq!(first, second);
        assert_eq!(first.assignment_hash.len(), 16);
    }
}
