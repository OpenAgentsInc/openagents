use std::collections::BTreeMap;

use crate::feature_graph::{FeatureGraph, FeatureNode};
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
                        "wall_thickness_mm".to_string(),
                    ),
                ]),
            },
            FeatureNode {
                id: "feature.rack.bay_cut".to_string(),
                name: "bay_cut".to_string(),
                operation_key: "cut.hole.v1".to_string(),
                depends_on: vec!["feature.rack.base".to_string()],
                params: BTreeMap::from([
                    ("radius_param".to_string(), "bay_cut_radius_mm".to_string()),
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
                    ("radius_param".to_string(), "vent_hole_radius_mm".to_string()),
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
                        "radius_param".to_string(),
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
                    ("radius_param".to_string(), "corner_radius_mm".to_string()),
                    ("kind".to_string(), "fillet".to_string()),
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
    use super::{generate_mac_studio_rack_template, MacStudioRackTemplateParams};

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

    fn openagents_unit_mm() -> crate::params::ScalarUnit {
        crate::params::ScalarUnit::Millimeter
    }
}
