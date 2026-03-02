use super::{
    CadDimensionConstraintKind, CadSketchConstraint, CadSketchEntity, CadSketchModel,
    CadSketchPlane, CadSketchPlanePreset, CadSketchSolveSeverity,
};
use crate::kernel_primitives::{make_cube, make_cylinder};
use crate::kernel_topology::{FaceId, Orientation};

fn primary_plane() -> CadSketchPlane {
    CadSketchPlane {
        id: "plane.front".to_string(),
        name: "Front".to_string(),
        origin_mm: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        y_axis: [0.0, 1.0, 0.0],
    }
}

#[test]
fn sketch_plane_presets_match_vcad_axes() {
    let xy = CadSketchPlane::from_preset(CadSketchPlanePreset::Xy);
    assert_eq!(xy.id, "plane.xy");
    assert_eq!(xy.name, "XY");
    assert_eq!(xy.origin_mm, [0.0, 0.0, 0.0]);
    assert_eq!(xy.normal, [0.0, 0.0, 1.0]);
    assert_eq!(xy.x_axis, [1.0, 0.0, 0.0]);
    assert_eq!(xy.y_axis, [0.0, 1.0, 0.0]);

    let xz = CadSketchPlane::xz();
    assert_eq!(xz.id, "plane.xz");
    assert_eq!(xz.name, "XZ");
    assert_eq!(xz.origin_mm, [0.0, 0.0, 0.0]);
    assert_eq!(xz.normal, [0.0, -1.0, 0.0]);
    assert_eq!(xz.x_axis, [1.0, 0.0, 0.0]);
    assert_eq!(xz.y_axis, [0.0, 0.0, 1.0]);

    let yz = CadSketchPlane::yz();
    assert_eq!(yz.id, "plane.yz");
    assert_eq!(yz.name, "YZ");
    assert_eq!(yz.origin_mm, [0.0, 0.0, 0.0]);
    assert_eq!(yz.normal, [1.0, 0.0, 0.0]);
    assert_eq!(yz.x_axis, [0.0, 1.0, 0.0]);
    assert_eq!(yz.y_axis, [0.0, 0.0, 1.0]);
}

#[test]
fn sketch_plane_from_planar_face_extracts_plane_basis() {
    let cube = make_cube(40.0, 20.0, 10.0).expect("cube should build");
    let face_plane =
        CadSketchPlane::from_planar_face(&cube, "face.1").expect("face.1 should be planar");
    assert_eq!(face_plane.id, "plane.face.1");
    assert_eq!(face_plane.name, "Face 1");
    assert_eq!(face_plane.origin_mm, [0.0, 0.0, 0.0]);
    assert_eq!(face_plane.normal, [0.0, 0.0, -1.0]);
    assert_eq!(face_plane.x_axis, [0.0, 1.0, 0.0]);
    assert_eq!(face_plane.y_axis, [1.0, 0.0, 0.0]);
}

#[test]
fn sketch_plane_from_planar_face_reversed_orientation_flips_normal() {
    let mut cube = make_cube(40.0, 20.0, 10.0).expect("cube should build");
    cube.topology
        .faces
        .get_mut(&FaceId(1))
        .expect("cube should contain face.1")
        .orientation = Orientation::Reversed;

    let face_plane =
        CadSketchPlane::from_planar_face(&cube, "face.1").expect("face.1 should be planar");
    assert_eq!(face_plane.normal, [0.0, 0.0, 1.0]);
    assert_eq!(face_plane.x_axis, [0.0, 1.0, 0.0]);
    assert_eq!(face_plane.y_axis, [-1.0, 0.0, 0.0]);
}

#[test]
fn sketch_plane_from_planar_face_rejects_non_planar_face() {
    let cylinder = make_cylinder(8.0, 12.0, 24).expect("cylinder should build");
    let error = CadSketchPlane::from_planar_face(&cylinder, "face.1")
        .expect_err("cylinder lateral face should not be planar");
    let message = error.to_string();
    assert!(
        message.contains("must reference a planar face"),
        "error should identify non-planar face: {message}"
    );
}

#[test]
fn sketch_model_serialization_is_deterministic_across_insertion_order() {
    let line = CadSketchEntity::Line {
        id: "entity.line.001".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [120.0, 0.0],
        anchor_ids: ["anchor.l.start".to_string(), "anchor.l.end".to_string()],
        construction: false,
    };
    let arc = CadSketchEntity::Arc {
        id: "entity.arc.001".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [60.0, 40.0],
        radius_mm: 20.0,
        start_deg: 0.0,
        end_deg: 180.0,
        anchor_ids: [
            "anchor.a.center".to_string(),
            "anchor.a.start".to_string(),
            "anchor.a.end".to_string(),
        ],
        construction: false,
    };

    let mut left = CadSketchModel::default();
    left.insert_plane(primary_plane())
        .expect("left plane insert should succeed");
    left.insert_entity(line.clone())
        .expect("left line insert should succeed");
    left.insert_entity(arc.clone())
        .expect("left arc insert should succeed");

    let mut right = CadSketchModel::default();
    right
        .insert_plane(primary_plane())
        .expect("right plane insert should succeed");
    right
        .insert_entity(arc)
        .expect("right arc insert should succeed");
    right
        .insert_entity(line)
        .expect("right line insert should succeed");

    let left_json =
        serde_json::to_string(&left).expect("left model should serialize deterministically");
    let right_json =
        serde_json::to_string(&right).expect("right model should serialize deterministically");
    assert_eq!(left_json, right_json);
}

#[test]
fn sketch_model_rejects_entities_that_reference_unknown_planes() {
    let mut model = CadSketchModel::default();
    let entity = CadSketchEntity::Point {
        id: "entity.point.001".to_string(),
        plane_id: "plane.missing".to_string(),
        position_mm: [5.0, 8.0],
        anchor_id: "anchor.p.001".to_string(),
        construction: true,
    };
    let result = model.insert_entity(entity);
    assert!(result.is_err(), "unknown plane reference must fail");
}

#[test]
fn sketch_entity_validation_rejects_duplicate_anchor_ids() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane insert should succeed");
    let result = model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.dup".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [50.0, 0.0],
        anchor_ids: ["anchor.dup".to_string(), "anchor.dup".to_string()],
        construction: false,
    });
    assert!(result.is_err(), "duplicate anchors must fail validation");
}

#[test]
fn constraint_solver_solves_common_mvp_constraints_deterministically() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");

    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.edit".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [20.0, 4.0],
            anchor_ids: [
                "anchor.edit.start".to_string(),
                "anchor.edit.end".to_string(),
            ],
            construction: false,
        })
        .expect("editable line should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.vertical".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [30.0, 0.0],
            end_mm: [35.0, 20.0],
            anchor_ids: [
                "anchor.vert.start".to_string(),
                "anchor.vert.end".to_string(),
            ],
            construction: false,
        })
        .expect("vertical line should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.tangent".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 10.0],
            end_mm: [20.0, 10.0],
            anchor_ids: ["anchor.tan.start".to_string(), "anchor.tan.end".to_string()],
            construction: false,
        })
        .expect("tangent line should insert");
    model
        .insert_entity(CadSketchEntity::Point {
            id: "entity.point.coincident".to_string(),
            plane_id: "plane.front".to_string(),
            position_mm: [9.0, 7.0],
            anchor_id: "anchor.point.coincident".to_string(),
            construction: false,
        })
        .expect("coincident point should insert");
    model
        .insert_entity(CadSketchEntity::Arc {
            id: "entity.arc.dimension".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [50.0, 20.0],
            radius_mm: 8.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.dim.center".to_string(),
                "anchor.dim.start".to_string(),
                "anchor.dim.end".to_string(),
            ],
            construction: false,
        })
        .expect("dimension arc should insert");
    model
        .insert_entity(CadSketchEntity::Arc {
            id: "entity.arc.tangent".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [10.0, 0.0],
            radius_mm: 10.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.tan.center".to_string(),
                "anchor.tan.arc_start".to_string(),
                "anchor.tan.arc_end".to_string(),
            ],
            construction: false,
        })
        .expect("tangent arc should insert");

    model
        .insert_constraint(CadSketchConstraint::Horizontal {
            id: "constraint.horizontal.001".to_string(),
            line_entity_id: "entity.line.edit".to_string(),
        })
        .expect("horizontal constraint should insert");
    model
        .insert_constraint(CadSketchConstraint::Vertical {
            id: "constraint.vertical.001".to_string(),
            line_entity_id: "entity.line.vertical".to_string(),
        })
        .expect("vertical constraint should insert");
    model
        .insert_constraint(CadSketchConstraint::Coincident {
            id: "constraint.zz.coincident.001".to_string(),
            first_anchor_id: "anchor.edit.end".to_string(),
            second_anchor_id: "anchor.point.coincident".to_string(),
            tolerance_mm: Some(0.001),
        })
        .expect("coincident constraint should insert");
    model
        .insert_constraint(CadSketchConstraint::Dimension {
            id: "constraint.dimension.length.001".to_string(),
            entity_id: "entity.line.edit".to_string(),
            dimension_kind: CadDimensionConstraintKind::Length,
            target_mm: 30.0,
            tolerance_mm: Some(0.001),
        })
        .expect("length dimension should insert");
    model
        .insert_constraint(CadSketchConstraint::Dimension {
            id: "constraint.dimension.radius.001".to_string(),
            entity_id: "entity.arc.dimension".to_string(),
            dimension_kind: CadDimensionConstraintKind::Radius,
            target_mm: 12.0,
            tolerance_mm: Some(0.001),
        })
        .expect("radius dimension should insert");
    model
        .insert_constraint(CadSketchConstraint::Tangent {
            id: "constraint.tangent.001".to_string(),
            line_entity_id: "entity.line.tangent".to_string(),
            arc_entity_id: "entity.arc.tangent".to_string(),
            at_anchor_id: None,
            tolerance_mm: Some(0.001),
        })
        .expect("tangent constraint should insert");

    let replay_seed = model.clone();
    let report_first = model
        .solve_constraints_deterministic()
        .expect("solver should run");
    assert!(report_first.passed, "common scenario should solve");
    assert!(
        report_first.iteration_count > 1,
        "iterative LM parity should perform multi-iteration solve for coupled constraints"
    );
    assert_eq!(report_first.unsolved_constraints, 0);
    assert_eq!(report_first.solved_constraints, 6);
    assert!(report_first.diagnostics.is_empty());

    let report_json_first =
        serde_json::to_string(&report_first).expect("solver report should serialize");
    let mut replay_model = replay_seed;
    let report_second = replay_model
        .solve_constraints_deterministic()
        .expect("solver should stay deterministic across repeated runs");
    let report_json_second =
        serde_json::to_string(&report_second).expect("solver report should serialize");
    assert_eq!(
        report_json_first, report_json_second,
        "solver report must remain deterministic for same inputs"
    );

    match model
        .entities
        .get("entity.line.edit")
        .expect("line.edit should exist")
    {
        CadSketchEntity::Line {
            start_mm, end_mm, ..
        } => {
            assert!((start_mm[1] - end_mm[1]).abs() <= 0.001);
            let length =
                ((end_mm[0] - start_mm[0]).powi(2) + (end_mm[1] - start_mm[1]).powi(2)).sqrt();
            assert!((length - 30.0).abs() <= 0.001);
        }
        _ => panic!("line.edit should remain a line"),
    }
    match model
        .entities
        .get("entity.line.vertical")
        .expect("line.vertical should exist")
    {
        CadSketchEntity::Line {
            start_mm, end_mm, ..
        } => {
            assert!((start_mm[0] - end_mm[0]).abs() <= 0.001);
        }
        _ => panic!("line.vertical should remain a line"),
    }
    match model
        .entities
        .get("entity.arc.dimension")
        .expect("arc.dimension should exist")
    {
        CadSketchEntity::Arc { radius_mm, .. } => {
            assert!((*radius_mm - 12.0).abs() <= 0.001);
        }
        _ => panic!("arc.dimension should remain an arc"),
    }
    let anchors = model
        .collect_anchor_bindings()
        .expect("anchors should remain resolvable");
    let line_end = anchors
        .get("anchor.edit.end")
        .expect("line end anchor should exist");
    let point = anchors
        .get("anchor.point.coincident")
        .expect("point anchor should exist");
    let delta_x = (line_end.position_mm[0] - point.position_mm[0]).abs();
    let delta_y = (line_end.position_mm[1] - point.position_mm[1]).abs();
    assert!(delta_x <= 0.001 && delta_y <= 0.001);
}

#[test]
fn tangent_constraint_reports_diagnostic_when_unsolved() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.diag".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [10.0, 7.0],
            anchor_ids: [
                "anchor.diag.start".to_string(),
                "anchor.diag.end".to_string(),
            ],
            construction: false,
        })
        .expect("line should insert");
    model
        .insert_entity(CadSketchEntity::Arc {
            id: "entity.arc.unsolved".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [20.0, 20.0],
            radius_mm: 5.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.unsolved.center".to_string(),
                "anchor.unsolved.start".to_string(),
                "anchor.unsolved.end".to_string(),
            ],
            construction: false,
        })
        .expect("arc should insert");
    model
        .insert_constraint(CadSketchConstraint::Tangent {
            id: "constraint.tangent.unsolved".to_string(),
            line_entity_id: "entity.line.diag".to_string(),
            arc_entity_id: "entity.arc.unsolved".to_string(),
            at_anchor_id: None,
            tolerance_mm: Some(0.001),
        })
        .expect("constraint should insert");

    let report = model
        .solve_constraints_deterministic()
        .expect("solver should run");
    assert!(!report.passed, "unsatisfied tangent should fail");
    assert_eq!(
        report.constraint_status.get("constraint.tangent.unsolved"),
        Some(&"unsolved".to_string())
    );
    assert!(report.diagnostics.iter().any(|entry| entry.code
        == "SKETCH_CONSTRAINT_TANGENT_UNSATISFIED"
        && entry.severity == CadSketchSolveSeverity::Error));
}

#[test]
fn constraint_validation_rejects_unknown_entity_references() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.known".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [10.0, 0.0],
            anchor_ids: [
                "anchor.known.start".to_string(),
                "anchor.known.end".to_string(),
            ],
            construction: false,
        })
        .expect("known line should insert");

    let result = model.insert_constraint(CadSketchConstraint::Horizontal {
        id: "constraint.horizontal.bad".to_string(),
        line_entity_id: "entity.line.missing".to_string(),
    });
    assert!(
        result.is_err(),
        "constraint must fail when referencing unknown entity"
    );
}

#[test]
fn sketch_model_supports_rectangle_circle_and_spline_entities() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");

    model
        .insert_entity(CadSketchEntity::Rectangle {
            id: "entity.rect.001".to_string(),
            plane_id: "plane.front".to_string(),
            min_mm: [0.0, 0.0],
            max_mm: [40.0, 20.0],
            anchor_ids: [
                "anchor.rect.00".to_string(),
                "anchor.rect.10".to_string(),
                "anchor.rect.11".to_string(),
                "anchor.rect.01".to_string(),
            ],
            construction: false,
        })
        .expect("rectangle should insert");
    model
        .insert_entity(CadSketchEntity::Circle {
            id: "entity.circle.001".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [60.0, 30.0],
            radius_mm: 12.0,
            anchor_ids: [
                "anchor.circle.center".to_string(),
                "anchor.circle.radius".to_string(),
            ],
            construction: false,
        })
        .expect("circle should insert");
    model
        .insert_entity(CadSketchEntity::Spline {
            id: "entity.spline.001".to_string(),
            plane_id: "plane.front".to_string(),
            control_points_mm: vec![[80.0, 0.0], [90.0, 10.0], [100.0, 0.0], [110.0, 8.0]],
            anchor_ids: vec![
                "anchor.spline.0".to_string(),
                "anchor.spline.1".to_string(),
                "anchor.spline.2".to_string(),
                "anchor.spline.3".to_string(),
            ],
            closed: false,
            construction: false,
        })
        .expect("spline should insert");

    let roundtrip_json = serde_json::to_string(&model).expect("model should serialize");
    let parsed: CadSketchModel =
        serde_json::from_str(&roundtrip_json).expect("model should deserialize");
    assert_eq!(model, parsed, "entity roundtrip must stay deterministic");
}

#[test]
fn spline_validation_rejects_anchor_point_count_mismatch() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");

    let result = model.insert_entity(CadSketchEntity::Spline {
        id: "entity.spline.bad".to_string(),
        plane_id: "plane.front".to_string(),
        control_points_mm: vec![[0.0, 0.0], [10.0, 10.0], [20.0, 0.0]],
        anchor_ids: vec!["anchor.bad.0".to_string(), "anchor.bad.1".to_string()],
        closed: false,
        construction: true,
    });
    assert!(
        result.is_err(),
        "spline must reject mismatched control point and anchor counts"
    );
}

#[test]
fn sketch_model_validates_full_constraint_enum_set() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");

    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.a".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [20.0, 0.0],
            anchor_ids: [
                "anchor.line.a.start".to_string(),
                "anchor.line.a.end".to_string(),
            ],
            construction: false,
        })
        .expect("line a should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.b".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 10.0],
            end_mm: [20.0, 10.0],
            anchor_ids: [
                "anchor.line.b.start".to_string(),
                "anchor.line.b.end".to_string(),
            ],
            construction: false,
        })
        .expect("line b should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.c".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [10.0, -10.0],
            end_mm: [10.0, 20.0],
            anchor_ids: [
                "anchor.line.c.start".to_string(),
                "anchor.line.c.end".to_string(),
            ],
            construction: false,
        })
        .expect("line c should insert");
    model
        .insert_entity(CadSketchEntity::Arc {
            id: "entity.arc.a".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [40.0, 0.0],
            radius_mm: 5.0,
            start_deg: 0.0,
            end_deg: 180.0,
            anchor_ids: [
                "anchor.arc.a.center".to_string(),
                "anchor.arc.a.start".to_string(),
                "anchor.arc.a.end".to_string(),
            ],
            construction: false,
        })
        .expect("arc a should insert");
    model
        .insert_entity(CadSketchEntity::Circle {
            id: "entity.circle.a".to_string(),
            plane_id: "plane.front".to_string(),
            center_mm: [40.0, 12.0],
            radius_mm: 5.0,
            anchor_ids: [
                "anchor.circle.a.center".to_string(),
                "anchor.circle.a.radius".to_string(),
            ],
            construction: false,
        })
        .expect("circle a should insert");
    model
        .insert_entity(CadSketchEntity::Point {
            id: "entity.point.a".to_string(),
            plane_id: "plane.front".to_string(),
            position_mm: [5.0, 5.0],
            anchor_id: "anchor.point.a".to_string(),
            construction: false,
        })
        .expect("point a should insert");
    model
        .insert_entity(CadSketchEntity::Point {
            id: "entity.point.b".to_string(),
            plane_id: "plane.front".to_string(),
            position_mm: [15.0, 5.0],
            anchor_id: "anchor.point.b".to_string(),
            construction: false,
        })
        .expect("point b should insert");

    model
        .insert_constraint(CadSketchConstraint::Coincident {
            id: "constraint.enum.coincident".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.line.a.start".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("coincident should insert");
    model
        .insert_constraint(CadSketchConstraint::PointOnLine {
            id: "constraint.enum.point_on_line".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            line_entity_id: "entity.line.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("point_on_line should insert");
    model
        .insert_constraint(CadSketchConstraint::Parallel {
            id: "constraint.enum.parallel".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("parallel should insert");
    model
        .insert_constraint(CadSketchConstraint::Perpendicular {
            id: "constraint.enum.perpendicular".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.c".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("perpendicular should insert");
    model
        .insert_constraint(CadSketchConstraint::Horizontal {
            id: "constraint.enum.horizontal".to_string(),
            line_entity_id: "entity.line.a".to_string(),
        })
        .expect("horizontal should insert");
    model
        .insert_constraint(CadSketchConstraint::Vertical {
            id: "constraint.enum.vertical".to_string(),
            line_entity_id: "entity.line.c".to_string(),
        })
        .expect("vertical should insert");
    model
        .insert_constraint(CadSketchConstraint::Tangent {
            id: "constraint.enum.tangent".to_string(),
            line_entity_id: "entity.line.b".to_string(),
            arc_entity_id: "entity.arc.a".to_string(),
            at_anchor_id: Some("anchor.arc.a.start".to_string()),
            tolerance_mm: Some(0.01),
        })
        .expect("tangent should insert");
    model
        .insert_constraint(CadSketchConstraint::EqualLength {
            id: "constraint.enum.equal_length".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("equal_length should insert");
    model
        .insert_constraint(CadSketchConstraint::EqualRadius {
            id: "constraint.enum.equal_radius".to_string(),
            first_curve_entity_id: "entity.arc.a".to_string(),
            second_curve_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("equal_radius should insert");
    model
        .insert_constraint(CadSketchConstraint::Concentric {
            id: "constraint.enum.concentric".to_string(),
            first_curve_entity_id: "entity.arc.a".to_string(),
            second_curve_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("concentric should insert");
    model
        .insert_constraint(CadSketchConstraint::Fixed {
            id: "constraint.enum.fixed".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: [5.0, 5.0],
            tolerance_mm: Some(0.01),
        })
        .expect("fixed should insert");
    model
        .insert_constraint(CadSketchConstraint::PointOnCircle {
            id: "constraint.enum.point_on_circle".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("point_on_circle should insert");
    model
        .insert_constraint(CadSketchConstraint::LineThroughCenter {
            id: "constraint.enum.line_through_center".to_string(),
            line_entity_id: "entity.line.c".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("line_through_center should insert");
    model
        .insert_constraint(CadSketchConstraint::Midpoint {
            id: "constraint.enum.midpoint".to_string(),
            midpoint_anchor_id: "anchor.point.a".to_string(),
            line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("midpoint should insert");
    model
        .insert_constraint(CadSketchConstraint::Symmetric {
            id: "constraint.enum.symmetric".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.point.b".to_string(),
            axis_line_entity_id: "entity.line.c".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("symmetric should insert");
    model
        .insert_constraint(CadSketchConstraint::Distance {
            id: "constraint.enum.distance".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.point.b".to_string(),
            target_mm: 10.0,
            tolerance_mm: Some(0.01),
        })
        .expect("distance should insert");
    model
        .insert_constraint(CadSketchConstraint::PointLineDistance {
            id: "constraint.enum.point_line_distance".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            line_entity_id: "entity.line.c".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        })
        .expect("point_line_distance should insert");
    model
        .insert_constraint(CadSketchConstraint::Angle {
            id: "constraint.enum.angle".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.c".to_string(),
            target_deg: 90.0,
            tolerance_deg: Some(0.01),
        })
        .expect("angle should insert");
    model
        .insert_constraint(CadSketchConstraint::Radius {
            id: "constraint.enum.radius".to_string(),
            curve_entity_id: "entity.circle.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        })
        .expect("radius should insert");
    model
        .insert_constraint(CadSketchConstraint::Length {
            id: "constraint.enum.length".to_string(),
            line_entity_id: "entity.line.a".to_string(),
            target_mm: 20.0,
            tolerance_mm: Some(0.01),
        })
        .expect("length should insert");
    model
        .insert_constraint(CadSketchConstraint::HorizontalDistance {
            id: "constraint.enum.horizontal_distance".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        })
        .expect("horizontal_distance should insert");
    model
        .insert_constraint(CadSketchConstraint::VerticalDistance {
            id: "constraint.enum.vertical_distance".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        })
        .expect("vertical_distance should insert");
    model
        .insert_constraint(CadSketchConstraint::Diameter {
            id: "constraint.enum.diameter".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            target_mm: 10.0,
            tolerance_mm: Some(0.01),
        })
        .expect("diameter should insert");
    model
        .insert_constraint(CadSketchConstraint::Dimension {
            id: "constraint.enum.dimension.legacy".to_string(),
            entity_id: "entity.line.b".to_string(),
            dimension_kind: CadDimensionConstraintKind::Length,
            target_mm: 20.0,
            tolerance_mm: Some(0.01),
        })
        .expect("legacy dimension should insert");

    model
        .validate()
        .expect("full constraint enum should validate");
}

#[test]
fn unsupported_constraint_kinds_emit_deterministic_warning_diagnostic() {
    let mut model = CadSketchModel::default();
    model
        .insert_plane(primary_plane())
        .expect("plane should insert");
    model
        .insert_entity(CadSketchEntity::Line {
            id: "entity.line.a".to_string(),
            plane_id: "plane.front".to_string(),
            start_mm: [0.0, 0.0],
            end_mm: [20.0, 0.0],
            anchor_ids: [
                "anchor.line.a.start".to_string(),
                "anchor.line.a.end".to_string(),
            ],
            construction: false,
        })
        .expect("line should insert");
    model
        .insert_entity(CadSketchEntity::Point {
            id: "entity.point.a".to_string(),
            plane_id: "plane.front".to_string(),
            position_mm: [5.0, 5.0],
            anchor_id: "anchor.point.a".to_string(),
            construction: false,
        })
        .expect("point should insert");
    model
        .insert_constraint(CadSketchConstraint::PointOnLine {
            id: "constraint.point_on_line".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            line_entity_id: "entity.line.a".to_string(),
            tolerance_mm: Some(0.01),
        })
        .expect("point_on_line should insert");

    let report = model
        .solve_constraints_deterministic()
        .expect("solver should run");
    assert!(
        !report.passed,
        "unsupported constraints should keep solve incomplete"
    );
    assert_eq!(report.solved_constraints, 0);
    assert_eq!(report.unsolved_constraints, 1);
    assert_eq!(
        report.constraint_status.get("constraint.point_on_line"),
        Some(&"unsolved".to_string())
    );
    assert!(report.diagnostics.iter().any(|entry| {
        entry.code == "SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED"
            && entry.constraint_id == "constraint.point_on_line"
            && entry.severity == CadSketchSolveSeverity::Warning
            && entry.message.contains("point_on_line")
    }));
}
