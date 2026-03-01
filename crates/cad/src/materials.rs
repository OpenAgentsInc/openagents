use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadMaterialPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub density_kg_m3: f64,
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
        cnc_cost_usd_per_kg: 18.0,
        cnc_setup_usd: 42.0,
    },
    CadMaterialPreset {
        id: "al-5052-h32",
        label: "Aluminum 5052-H32",
        density_kg_m3: 2_680.0,
        cnc_cost_usd_per_kg: 16.0,
        cnc_setup_usd: 38.0,
    },
    CadMaterialPreset {
        id: "steel-1018",
        label: "Steel 1018",
        density_kg_m3: 7_870.0,
        cnc_cost_usd_per_kg: 9.0,
        cnc_setup_usd: 56.0,
    },
    CadMaterialPreset {
        id: "ti-6al-4v",
        label: "Titanium Ti-6Al-4V",
        density_kg_m3: 4_430.0,
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
        || !preset.cnc_cost_usd_per_kg.is_finite()
        || !preset.cnc_setup_usd.is_finite()
    {
        return None;
    }
    Some((mass_kg * preset.cnc_cost_usd_per_kg) + preset.cnc_setup_usd)
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
        DEFAULT_CAD_MATERIAL_ID, estimate_cnc_cost_heuristic_usd, estimate_material_cost_usd,
        material_preset_by_id, material_presets, next_material_preset_id,
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
}
