#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use openagents_cad::rack::{
    MacStudioRackTemplateParams, RackObjectivePreset, generate_mac_studio_rack_template,
    generate_objective_variants,
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
    let disabled = generate_mac_studio_rack_template(&disabled_params)
        .expect("disabled template should generate");

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
    assert!(
        enabled
            .semantic_refs
            .resolve("mount_hole_pattern")
            .is_some()
    );
    assert!(
        disabled
            .semantic_refs
            .resolve("mount_hole_pattern")
            .is_some()
    );
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
            .get_required_with_unit("vent_rows", openagents_cad::params::ScalarUnit::Unitless)
            .unwrap_or_default(),
        4.0
    );
    assert_eq!(
        rack.params
            .get_required_with_unit("vent_cols", openagents_cad::params::ScalarUnit::Unitless)
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
            .get_required_with_unit(
                "opt_rib_count",
                openagents_cad::params::ScalarUnit::Unitless
            )
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
            .get_required_with_unit(
                "opt_ribs_enabled",
                openagents_cad::params::ScalarUnit::Unitless
            )
            .unwrap_or_default(),
        0.0
    );
    assert_eq!(
        rack.params
            .get_required_with_unit(
                "opt_wall_thickness_scale",
                openagents_cad::params::ScalarUnit::Unitless
            )
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
    let right = generate_objective_variants(&baseline, 42).expect("right variants should build");
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
    let variants = generate_objective_variants(&baseline, 777).expect("variants should generate");
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

fn openagents_unit_mm() -> openagents_cad::params::ScalarUnit {
    openagents_cad::params::ScalarUnit::Millimeter
}
