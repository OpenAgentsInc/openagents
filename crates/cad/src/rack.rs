use std::collections::BTreeMap;

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::semantic_refs::CadSemanticRefRegistry;
use crate::{CadError, CadResult};

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
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(CadError::InvalidParameter {
                    name: name.to_string(),
                    reason: "value must be finite and > 0".to_string(),
                });
            }
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
        let error = generate_mac_studio_rack_template(&params)
            .expect_err("bay count other than 2 should fail");
        assert!(error.to_string().contains("requires exactly 2 bays"));
    }

    #[test]
    fn rack_template_emits_semantic_refs() {
        let params = MacStudioRackTemplateParams::default();
        let rack = generate_mac_studio_rack_template(&params).expect("template should generate");
        assert!(rack.semantic_refs.resolve("rack_outer_face").is_some());
        assert!(rack.semantic_refs.resolve("rack_bay_pattern").is_some());
        assert!(rack.semantic_refs.resolve("rack_corner_break").is_some());
    }

    fn openagents_unit_mm() -> crate::params::ScalarUnit {
        crate::params::ScalarUnit::Millimeter
    }
}
