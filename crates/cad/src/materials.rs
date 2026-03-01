#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadMaterialPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub density_kg_m3: f64,
    pub cnc_cost_usd_per_kg: f64,
    pub cnc_setup_usd: f64,
}

pub const DEFAULT_CAD_MATERIAL_ID: &str = "al-6061-t6";

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

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_CAD_MATERIAL_ID, estimate_material_cost_usd, material_preset_by_id,
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
        let second = estimate_material_cost_usd(3.2, steel).expect("cost estimate should be stable");
        assert_eq!(first, second);
        assert!((first - 84.8).abs() < 1e-12);
        assert!(estimate_material_cost_usd(-1.0, steel).is_none());
    }
}
