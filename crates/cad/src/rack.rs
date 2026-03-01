use std::collections::BTreeMap;

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::keys::feature_params as feature_keys;
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::semantic_refs::CadSemanticRefRegistry;
use crate::{CadError, CadResult};

/// Wall-mount feature controls for the rack template.
#[derive(Clone, Debug, PartialEq)]
pub struct RackWallMountConfig {
    pub enabled: bool,
    pub hole_count: u8,
    pub hole_spacing_mm: f64,
    pub hole_radius_mm: f64,
    pub bracket_thickness_mm: f64,
}

impl Default for RackWallMountConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hole_count: 4,
            hole_spacing_mm: 32.0,
            hole_radius_mm: 2.8,
            bracket_thickness_mm: 6.0,
        }
    }
}

/// Vent-pattern controls for airflow-oriented rack variants.
#[derive(Clone, Debug, PartialEq)]
pub struct RackVentPatternConfig {
    pub enabled: bool,
    pub rows: u8,
    pub cols: u8,
    pub spacing_mm: f64,
    pub hole_radius_mm: f64,
    pub density_scale: f64,
}

impl Default for RackVentPatternConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rows: 3,
            cols: 8,
            spacing_mm: 12.0,
            hole_radius_mm: 2.0,
            density_scale: 1.0,
        }
    }
}

/// Structure optimization hooks consumed by variant objective presets.
#[derive(Clone, Debug, PartialEq)]
pub struct RackStructureOptimizationConfig {
    pub ribs_enabled: bool,
    pub rib_count: u8,
    pub rib_spacing_mm: f64,
    pub rib_thickness_mm: f64,
    pub wall_thickness_scale: f64,
}

impl Default for RackStructureOptimizationConfig {
    fn default() -> Self {
        Self {
            ribs_enabled: true,
            rib_count: 3,
            rib_spacing_mm: 42.0,
            rib_thickness_mm: 3.0,
            wall_thickness_scale: 1.0,
        }
    }
}

/// Deterministic two-bay Mac Studio rack template parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct MacStudioRackTemplateParams {
    pub bay_count: u8,
    pub frame_width_mm: f64,
    pub frame_depth_mm: f64,
    pub frame_height_mm: f64,
    pub bay_pitch_mm: f64,
    pub bay_cut_radius_mm: f64,
    pub wall_thickness_mm: f64,
    pub corner_radius_mm: f64,
    pub wall_mount: RackWallMountConfig,
    pub vent: RackVentPatternConfig,
    pub optimization: RackStructureOptimizationConfig,
}

impl Default for MacStudioRackTemplateParams {
    fn default() -> Self {
        // Baseline dimensions for the scripted demo rack.
        Self {
            bay_count: 2,
            frame_width_mm: 180.0,
            frame_depth_mm: 210.0,
            frame_height_mm: 95.0,
            bay_pitch_mm: 88.0,
            bay_cut_radius_mm: 19.0,
            wall_thickness_mm: 6.0,
            corner_radius_mm: 2.0,
            wall_mount: RackWallMountConfig::default(),
            vent: RackVentPatternConfig::default(),
            optimization: RackStructureOptimizationConfig::default(),
        }
    }
}

impl MacStudioRackTemplateParams {
    pub fn validate(&self) -> CadResult<()> {
        if self.bay_count != 2 {
            return Err(CadError::InvalidParameter {
                name: "bay_count".to_string(),
                reason: "Mac Studio rack template requires exactly 2 bays".to_string(),
            });
        }
        for (name, value) in [
            ("frame_width_mm", self.frame_width_mm),
            ("frame_depth_mm", self.frame_depth_mm),
            ("frame_height_mm", self.frame_height_mm),
            ("bay_pitch_mm", self.bay_pitch_mm),
            ("bay_cut_radius_mm", self.bay_cut_radius_mm),
            ("wall_thickness_mm", self.wall_thickness_mm),
            ("corner_radius_mm", self.corner_radius_mm),
            (
                "wall_mount.hole_spacing_mm",
                self.wall_mount.hole_spacing_mm,
            ),
            ("wall_mount.hole_radius_mm", self.wall_mount.hole_radius_mm),
            (
                "wall_mount.bracket_thickness_mm",
                self.wall_mount.bracket_thickness_mm,
            ),
            ("vent.spacing_mm", self.vent.spacing_mm),
            ("vent.hole_radius_mm", self.vent.hole_radius_mm),
            ("vent.density_scale", self.vent.density_scale),
            (
                "optimization.rib_spacing_mm",
                self.optimization.rib_spacing_mm,
            ),
            (
                "optimization.rib_thickness_mm",
                self.optimization.rib_thickness_mm,
            ),
            (
                "optimization.wall_thickness_scale",
                self.optimization.wall_thickness_scale,
            ),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(CadError::InvalidParameter {
                    name: name.to_string(),
                    reason: "value must be finite and > 0".to_string(),
                });
            }
        }
        if self.wall_mount.hole_count < 2 {
            return Err(CadError::InvalidParameter {
                name: "wall_mount.hole_count".to_string(),
                reason: "wall mount hole count must be >= 2".to_string(),
            });
        }
        if self.vent.rows < 1 {
            return Err(CadError::InvalidParameter {
                name: "vent.rows".to_string(),
                reason: "vent rows must be >= 1".to_string(),
            });
        }
        if self.vent.cols < 1 {
            return Err(CadError::InvalidParameter {
                name: "vent.cols".to_string(),
                reason: "vent cols must be >= 1".to_string(),
            });
        }
        if self.optimization.rib_count < 1 {
            return Err(CadError::InvalidParameter {
                name: "optimization.rib_count".to_string(),
                reason: "rib count must be >= 1".to_string(),
            });
        }
        Ok(())
    }

    pub fn to_parameter_store(&self) -> CadResult<ParameterStore> {
        self.validate()?;
        let mut params = ParameterStore::default();
        params.set(
            "frame_width_mm",
            ScalarValue {
                value: self.frame_width_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "frame_depth_mm",
            ScalarValue {
                value: self.frame_depth_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "frame_height_mm",
            ScalarValue {
                value: self.frame_height_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "bay_pitch_mm",
            ScalarValue {
                value: self.bay_pitch_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "bay_cut_radius_mm",
            ScalarValue {
                value: self.bay_cut_radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "wall_thickness_mm",
            ScalarValue {
                value: self.wall_thickness_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "effective_wall_thickness_mm",
            ScalarValue {
                value: self.wall_thickness_mm * self.optimization.wall_thickness_scale,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "corner_radius_mm",
            ScalarValue {
                value: self.corner_radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "bay_count",
            ScalarValue {
                value: f64::from(self.bay_count),
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "wall_mount_enabled",
            ScalarValue {
                value: if self.wall_mount.enabled { 1.0 } else { 0.0 },
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "wall_mount_hole_count",
            ScalarValue {
                value: f64::from(self.wall_mount.hole_count),
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "wall_mount_hole_spacing_mm",
            ScalarValue {
                value: self.wall_mount.hole_spacing_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "wall_mount_hole_radius_mm",
            ScalarValue {
                value: self.wall_mount.hole_radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "wall_mount_bracket_thickness_mm",
            ScalarValue {
                value: self.wall_mount.bracket_thickness_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "vent_enabled",
            ScalarValue {
                value: if self.vent.enabled { 1.0 } else { 0.0 },
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "vent_rows",
            ScalarValue {
                value: f64::from(self.vent.rows),
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "vent_cols",
            ScalarValue {
                value: f64::from(self.vent.cols),
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "vent_spacing_mm",
            ScalarValue {
                value: self.vent.spacing_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "vent_hole_radius_mm",
            ScalarValue {
                value: self.vent.hole_radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "vent_density_scale",
            ScalarValue {
                value: self.vent.density_scale,
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "opt_ribs_enabled",
            ScalarValue {
                value: if self.optimization.ribs_enabled { 1.0 } else { 0.0 },
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "opt_rib_count",
            ScalarValue {
                value: f64::from(self.optimization.rib_count),
                unit: ScalarUnit::Unitless,
            },
        )?;
        params.set(
            "opt_rib_spacing_mm",
            ScalarValue {
                value: self.optimization.rib_spacing_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "opt_rib_thickness_mm",
            ScalarValue {
                value: self.optimization.rib_thickness_mm,
                unit: ScalarUnit::Millimeter,
            },
        )?;
        params.set(
            "opt_wall_thickness_scale",
            ScalarValue {
                value: self.optimization.wall_thickness_scale,
                unit: ScalarUnit::Unitless,
            },
        )?;
        Ok(params)
    }
}

/// Deterministic rack template payload consumed by reducers/panes/tests.
#[derive(Clone, Debug, PartialEq)]
pub struct MacStudioRackTemplate {
    pub feature_graph: FeatureGraph,
    pub params: ParameterStore,
    pub semantic_refs: CadSemanticRefRegistry,
    pub metadata: BTreeMap<String, String>,
}

/// Deterministic objective presets for rack variant generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RackObjectivePreset {
    LowestWeight,
    LowestCost,
    HighestStiffness,
    AirflowBiased,
}

impl RackObjectivePreset {
    pub fn variant_id(self) -> &'static str {
        match self {
            Self::LowestWeight => "variant.lightweight",
            Self::LowestCost => "variant.low-cost",
            Self::HighestStiffness => "variant.stiffness",
            Self::AirflowBiased => "variant.airflow",
        }
    }

    fn all() -> [Self; 4] {
        [
            Self::LowestWeight,
            Self::LowestCost,
            Self::HighestStiffness,
            Self::AirflowBiased,
        ]
    }
}

/// Deterministic objective variant payload with score outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct RackObjectiveVariant {
    pub variant_id: String,
    pub objective: RackObjectivePreset,
    pub seed: u64,
    pub params: MacStudioRackTemplateParams,
    pub template: MacStudioRackTemplate,
    pub objective_scores: BTreeMap<String, f64>,
}

/// Generate four deterministic objective variants from a baseline template seed.
pub fn generate_objective_variants(
    base: &MacStudioRackTemplateParams,
    seed: u64,
) -> CadResult<Vec<RackObjectiveVariant>> {
    let mut variants = Vec::with_capacity(4);
    for objective in RackObjectivePreset::all() {
        let mut params = base.clone();
        apply_objective_preset(&mut params, objective);
        let mut template = generate_mac_studio_rack_template(&params)?;
        template.metadata.insert(
            "variant.objective".to_string(),
            objective.variant_id().to_string(),
        );
        template
            .metadata
            .insert("variant.seed".to_string(), seed.to_string());
        let objective_scores = objective_scores_for(&params, objective, seed);
        variants.push(RackObjectiveVariant {
            variant_id: objective.variant_id().to_string(),
            objective,
            seed,
            params,
            template,
            objective_scores,
        });
    }
    Ok(variants)
}

fn apply_objective_preset(params: &mut MacStudioRackTemplateParams, objective: RackObjectivePreset) {
    match objective {
        RackObjectivePreset::LowestWeight => {
            params.optimization.wall_thickness_scale = 0.82;
            params.optimization.rib_count = 2;
            params.optimization.rib_thickness_mm = 2.2;
            params.vent.rows = 4;
            params.vent.cols = 10;
            params.vent.density_scale = 1.25;
        }
        RackObjectivePreset::LowestCost => {
            params.optimization.wall_thickness_scale = 0.9;
            params.optimization.rib_count = 2;
            params.optimization.rib_spacing_mm = 55.0;
            params.vent.rows = 2;
            params.vent.cols = 6;
            params.vent.density_scale = 0.9;
            params.wall_mount.hole_count = 2;
        }
        RackObjectivePreset::HighestStiffness => {
            params.optimization.wall_thickness_scale = 1.28;
            params.optimization.rib_count = 6;
            params.optimization.rib_spacing_mm = 24.0;
            params.optimization.rib_thickness_mm = 3.8;
            params.vent.rows = 2;
            params.vent.cols = 5;
            params.vent.density_scale = 0.7;
        }
        RackObjectivePreset::AirflowBiased => {
            params.optimization.wall_thickness_scale = 0.95;
            params.optimization.rib_count = 3;
            params.optimization.rib_spacing_mm = 48.0;
            params.vent.rows = 5;
            params.vent.cols = 12;
            params.vent.spacing_mm = 9.5;
            params.vent.hole_radius_mm = 2.4;
            params.vent.density_scale = 1.5;
        }
    }
}

fn objective_scores_for(
    params: &MacStudioRackTemplateParams,
    objective: RackObjectivePreset,
    seed: u64,
) -> BTreeMap<String, f64> {
    let thickness = params.wall_thickness_mm * params.optimization.wall_thickness_scale;
    let rib_factor = f64::from(params.optimization.rib_count) * params.optimization.rib_thickness_mm;
    let vent_factor = f64::from(params.vent.rows) * f64::from(params.vent.cols) * params.vent.density_scale;
    let mount_factor = f64::from(params.wall_mount.hole_count) * params.wall_mount.hole_radius_mm;

    let weight_score = (1.0 / (thickness * 0.6 + rib_factor * 0.3 + mount_factor * 0.1)).min(1.0);
    let cost_score = (1.0 / (thickness * 0.55 + rib_factor * 0.35 + vent_factor * 0.1)).min(1.0);
    let stiffness_score = ((thickness * 0.55 + rib_factor * 0.45) / 20.0).min(1.0);
    let airflow_score = ((vent_factor * 0.8 + params.vent.hole_radius_mm * 2.0) / 80.0).min(1.0);
    let seed_bias = seeded_bias(seed, objective);

    BTreeMap::from([
        ("weight".to_string(), round4((weight_score + seed_bias).clamp(0.0, 1.0))),
        ("cost".to_string(), round4((cost_score + seed_bias).clamp(0.0, 1.0))),
        ("stiffness".to_string(), round4((stiffness_score + seed_bias).clamp(0.0, 1.0))),
        ("airflow".to_string(), round4((airflow_score + seed_bias).clamp(0.0, 1.0))),
    ])
}

fn seeded_bias(seed: u64, objective: RackObjectivePreset) -> f64 {
    let objective_id = match objective {
        RackObjectivePreset::LowestWeight => 11_u64,
        RackObjectivePreset::LowestCost => 23_u64,
        RackObjectivePreset::HighestStiffness => 37_u64,
        RackObjectivePreset::AirflowBiased => 49_u64,
    };
    let mixed = seed
        .wrapping_mul(0x9e3779b97f4a7c15)
        .wrapping_add(objective_id);
    let normalized = (mixed % 1000) as f64 / 1000.0;
    (normalized - 0.5) * 0.01
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// Build deterministic two-bay rack template graph + parameter schema + semantic refs.
pub fn generate_mac_studio_rack_template(
    params: &MacStudioRackTemplateParams,
) -> CadResult<MacStudioRackTemplate> {
    let params_store = params.to_parameter_store()?;

    let feature_graph = FeatureGraph {
        nodes: vec![
            FeatureNode {
                id: "feature.rack.base".to_string(),
                name: "rack_base".to_string(),
                operation_key: "primitive.box.v1".to_string(),
                depends_on: Vec::new(),
                params: BTreeMap::from([
                    ("width_param".to_string(), "frame_width_mm".to_string()),
                    ("depth_param".to_string(), "frame_depth_mm".to_string()),
                    ("height_param".to_string(), "frame_height_mm".to_string()),
                    (
                        "thickness_param".to_string(),
                        "effective_wall_thickness_mm".to_string(),
                    ),
                ]),
            },
            FeatureNode {
                id: "feature.rack.bay_cut".to_string(),
                name: "bay_cut".to_string(),
                operation_key: "cut.hole.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    (feature_keys::RADIUS_PARAM.owned(), "bay_cut_radius_mm".to_string()),
                    ("depth_param".to_string(), "frame_depth_mm".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.bay_pattern".to_string(),
                name: "bay_pattern".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.rack.bay_cut".to_string()],
                params: BTreeMap::from([
                    ("count_param".to_string(), "bay_count".to_string()),
                    ("spacing_param".to_string(), "bay_pitch_mm".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.vent_hole".to_string(),
                name: "vent_hole".to_string(),
                operation_key: "cut.hole.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    (
                        feature_keys::RADIUS_PARAM.owned(),
                        "vent_hole_radius_mm".to_string(),
                    ),
                    ("depth_param".to_string(), "wall_thickness_mm".to_string()),
                    ("enabled_param".to_string(), "vent_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.vent_pattern_x".to_string(),
                name: "vent_pattern_x".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.rack.vent_hole".to_string()],
                params: BTreeMap::from([
                    ("count_param".to_string(), "vent_cols".to_string()),
                    ("spacing_param".to_string(), "vent_spacing_mm".to_string()),
                    ("enabled_param".to_string(), "vent_enabled".to_string()),
                    ("density_param".to_string(), "vent_density_scale".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.vent_face_set".to_string(),
                name: "vent_face_set".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.rack.vent_pattern_x".to_string()],
                params: BTreeMap::from([
                    ("count_param".to_string(), "vent_rows".to_string()),
                    ("spacing_param".to_string(), "vent_spacing_mm".to_string()),
                    ("enabled_param".to_string(), "vent_enabled".to_string()),
                    ("density_param".to_string(), "vent_density_scale".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.rib_seed".to_string(),
                name: "rib_seed".to_string(),
                operation_key: "transform.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    ("thickness_param".to_string(), "opt_rib_thickness_mm".to_string()),
                    ("enabled_param".to_string(), "opt_ribs_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.rib_pattern".to_string(),
                name: "rib_pattern".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.rack.rib_seed".to_string()],
                params: BTreeMap::from([
                    ("count_param".to_string(), "opt_rib_count".to_string()),
                    ("spacing_param".to_string(), "opt_rib_spacing_mm".to_string()),
                    ("enabled_param".to_string(), "opt_ribs_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.wall_mount_bracket".to_string(),
                name: "wall_mount_bracket".to_string(),
                operation_key: "transform.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    (
                        "offset_param".to_string(),
                        "wall_mount_bracket_thickness_mm".to_string(),
                    ),
                    ("enabled_param".to_string(), "wall_mount_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.wall_mount_hole".to_string(),
                name: "wall_mount_hole".to_string(),
                operation_key: "cut.hole.v1".to_string(),
                depends_on: vec!["feature.rack.wall_mount_bracket".to_string()],
                params: BTreeMap::from([
                    (
                        feature_keys::RADIUS_PARAM.owned(),
                        "wall_mount_hole_radius_mm".to_string(),
                    ),
                    (
                        "depth_param".to_string(),
                        "wall_mount_bracket_thickness_mm".to_string(),
                    ),
                    ("enabled_param".to_string(), "wall_mount_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.mount_hole_pattern".to_string(),
                name: "mount_hole_pattern".to_string(),
                operation_key: "linear.pattern.v1".to_string(),
                depends_on: vec!["feature.rack.wall_mount_hole".to_string()],
                params: BTreeMap::from([
                    (
                        "count_param".to_string(),
                        "wall_mount_hole_count".to_string(),
                    ),
                    (
                        "spacing_param".to_string(),
                        "wall_mount_hole_spacing_mm".to_string(),
                    ),
                    ("enabled_param".to_string(), "wall_mount_enabled".to_string()),
                ]),
            },
            FeatureNode {
                id: "feature.rack.corner_break".to_string(),
                name: "corner_break".to_string(),
                operation_key: "fillet.placeholder.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    (feature_keys::RADIUS_PARAM.owned(), "corner_radius_mm".to_string()),
                    (feature_keys::KIND.owned(), "fillet".to_string()),
                ]),
            },
        ],
    };
    feature_graph.validate()?;

    let mut semantic_refs = CadSemanticRefRegistry::default();
    semantic_refs.register("rack_outer_face", "feature.rack.base", "feature.rack.base")?;
    semantic_refs.register(
        "rack_bay_pattern",
        "feature.rack.bay_pattern",
        "feature.rack.bay_pattern",
    )?;
    semantic_refs.register(
        "vent_face_set",
        "feature.rack.vent_face_set",
        "feature.rack.vent_face_set",
    )?;
    semantic_refs.register(
        "rack_rib_set",
        "feature.rack.rib_pattern",
        "feature.rack.rib_pattern",
    )?;
    semantic_refs.register(
        "wall_mount_bracket",
        "feature.rack.wall_mount_bracket",
        "feature.rack.wall_mount_bracket",
    )?;
    semantic_refs.register(
        "mount_hole_pattern",
        "feature.rack.mount_hole_pattern",
        "feature.rack.mount_hole_pattern",
    )?;
    semantic_refs.register(
        "rack_corner_break",
        "feature.rack.corner_break",
        "feature.rack.corner_break",
    )?;

    let metadata = BTreeMap::from([
        (
            "template.id".to_string(),
            "mac-studio-rack.two-bay.v1".to_string(),
        ),
        (
            "template.description".to_string(),
            "Deterministic two-bay Mac Studio rack demo template".to_string(),
        ),
    ]);

    Ok(MacStudioRackTemplate {
        feature_graph,
        params: params_store,
        semantic_refs,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        RackObjectivePreset, generate_mac_studio_rack_template, generate_objective_variants,
        MacStudioRackTemplateParams,
    };

    #[test]
    fn rack_template_generator_is_deterministic() {
        let params = MacStudioRackTemplateParams::default();
        let left = generate_mac_studio_rack_template(&params).expect("left build should succeed");
        let right = generate_mac_studio_rack_template(&params).expect("right build should succeed");

        assert_eq!(left.feature_graph, right.feature_graph);
        assert_eq!(left.params, right.params);
        assert_eq!(
            left.semantic_refs.to_stable_ids(),
            right.semantic_refs.to_stable_ids()
        );
    }

    #[test]
    fn rack_template_schema_contains_demo_dimensions() {
        let params = MacStudioRackTemplateParams::default();
        let rack = generate_mac_studio_rack_template(&params).expect("template should generate");
        assert_eq!(
            rack.params
                .get_required_with_unit("frame_width_mm", openagents_unit_mm())
                .unwrap_or_default(),
            180.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("frame_depth_mm", openagents_unit_mm())
                .unwrap_or_default(),
            210.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("frame_height_mm", openagents_unit_mm())
                .unwrap_or_default(),
            95.0
        );
        assert_eq!(rack.feature_graph.nodes[0].id, "feature.rack.base");
        assert_eq!(rack.feature_graph.nodes[1].id, "feature.rack.bay_cut");
        assert_eq!(rack.feature_graph.nodes[2].id, "feature.rack.bay_pattern");
    }

    #[test]
    fn rack_template_rejects_invalid_bay_count() {
        let mut params = MacStudioRackTemplateParams::default();
        params.bay_count = 3;
        let error =
            generate_mac_studio_rack_template(&params).expect_err("bay count other than 2 should fail");
        assert!(error.to_string().contains("requires exactly 2 bays"));
    }

    #[test]
    fn rack_template_emits_semantic_refs() {
        let params = MacStudioRackTemplateParams::default();
        let rack = generate_mac_studio_rack_template(&params).expect("template should generate");
        assert!(rack.semantic_refs.resolve("rack_outer_face").is_some());
        assert!(rack.semantic_refs.resolve("rack_bay_pattern").is_some());
        assert!(rack.semantic_refs.resolve("vent_face_set").is_some());
        assert!(rack.semantic_refs.resolve("rack_rib_set").is_some());
        assert!(rack.semantic_refs.resolve("mount_hole_pattern").is_some());
        assert!(rack.semantic_refs.resolve("wall_mount_bracket").is_some());
        assert!(rack.semantic_refs.resolve("rack_corner_break").is_some());
    }

    #[test]
    fn wall_mount_toggle_keeps_feature_ids_stable() {
        let enabled = generate_mac_studio_rack_template(&MacStudioRackTemplateParams::default())
            .expect("enabled template should generate");
        let mut disabled_params = MacStudioRackTemplateParams::default();
        disabled_params.wall_mount.enabled = false;
        let disabled =
            generate_mac_studio_rack_template(&disabled_params).expect("disabled template should generate");

        let enabled_ids = enabled
            .feature_graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        let disabled_ids = disabled
            .feature_graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(enabled_ids, disabled_ids);
        assert!(enabled.semantic_refs.resolve("mount_hole_pattern").is_some());
        assert!(disabled.semantic_refs.resolve("mount_hole_pattern").is_some());
    }

    #[test]
    fn vent_pattern_parameters_are_projected_to_store() {
        let mut params = MacStudioRackTemplateParams::default();
        params.vent.rows = 4;
        params.vent.cols = 10;
        params.vent.spacing_mm = 10.5;
        params.vent.hole_radius_mm = 1.9;
        params.vent.density_scale = 1.2;
        let rack = generate_mac_studio_rack_template(&params).expect("template should generate");
        assert_eq!(
            rack.params
                .get_required_with_unit("vent_rows", crate::params::ScalarUnit::Unitless)
                .unwrap_or_default(),
            4.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("vent_cols", crate::params::ScalarUnit::Unitless)
                .unwrap_or_default(),
            10.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("vent_spacing_mm", openagents_unit_mm())
                .unwrap_or_default(),
            10.5
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("vent_hole_radius_mm", openagents_unit_mm())
                .unwrap_or_default(),
            1.9
        );
    }

    #[test]
    fn optimization_hooks_are_projected_to_store() {
        let mut params = MacStudioRackTemplateParams::default();
        params.optimization.rib_count = 5;
        params.optimization.rib_spacing_mm = 31.0;
        params.optimization.rib_thickness_mm = 2.4;
        params.optimization.wall_thickness_scale = 0.85;
        params.optimization.ribs_enabled = false;

        let rack = generate_mac_studio_rack_template(&params).expect("template should generate");
        assert_eq!(
            rack.params
                .get_required_with_unit("opt_rib_count", crate::params::ScalarUnit::Unitless)
                .unwrap_or_default(),
            5.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("opt_rib_spacing_mm", openagents_unit_mm())
                .unwrap_or_default(),
            31.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("opt_rib_thickness_mm", openagents_unit_mm())
                .unwrap_or_default(),
            2.4
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("opt_ribs_enabled", crate::params::ScalarUnit::Unitless)
                .unwrap_or_default(),
            0.0
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("opt_wall_thickness_scale", crate::params::ScalarUnit::Unitless)
                .unwrap_or_default(),
            0.85
        );
        assert_eq!(
            rack.params
                .get_required_with_unit("effective_wall_thickness_mm", openagents_unit_mm())
                .unwrap_or_default(),
            5.1
        );
    }

    #[test]
    fn objective_engine_generates_four_seeded_variants_deterministically() {
        let baseline = MacStudioRackTemplateParams::default();
        let left = generate_objective_variants(&baseline, 42).expect("left variants should build");
        let right =
            generate_objective_variants(&baseline, 42).expect("right variants should build");
        assert_eq!(left, right);
        assert_eq!(left.len(), 4);
        assert_eq!(left[0].objective, RackObjectivePreset::LowestWeight);
        assert_eq!(left[1].objective, RackObjectivePreset::LowestCost);
        assert_eq!(left[2].objective, RackObjectivePreset::HighestStiffness);
        assert_eq!(left[3].objective, RackObjectivePreset::AirflowBiased);
    }

    #[test]
    fn objective_engine_emits_expected_variant_ids_and_scores() {
        let baseline = MacStudioRackTemplateParams::default();
        let variants =
            generate_objective_variants(&baseline, 777).expect("variants should generate");
        let ids = variants
            .iter()
            .map(|variant| variant.variant_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "variant.lightweight",
                "variant.low-cost",
                "variant.stiffness",
                "variant.airflow"
            ]
        );
        for variant in variants {
            assert!(variant.objective_scores.contains_key("weight"));
            assert!(variant.objective_scores.contains_key("cost"));
            assert!(variant.objective_scores.contains_key("stiffness"));
            assert!(variant.objective_scores.contains_key("airflow"));
        }
    }

    fn openagents_unit_mm() -> crate::params::ScalarUnit {
        crate::params::ScalarUnit::Millimeter
    }
}
