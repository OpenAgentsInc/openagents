use arc_core::ArcGrid;
use arc_solvers::{
    ArcDslTier, ArcGridBinding, ArcGridExpr, ArcInterpreter, ArcInterpreterError,
    ArcObjectSelector, ArcObjectTransform, ArcProgram, ArcProgramMetadata, ArcSymbol,
};

fn symbol(name: &str) -> ArcSymbol {
    ArcSymbol::new(name).expect("symbol should validate")
}

fn grid(width: u8, height: u8, rows: &[&[u8]]) -> ArcGrid {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    ArcGrid::new(width, height, cells).expect("grid should validate")
}

#[test]
fn crop_and_recolor_pipeline_round_trips_through_json() {
    let program = ArcProgram {
        input_symbol: symbol("input"),
        bindings: Vec::new(),
        body: ArcGridExpr::Recolor {
            source: Box::new(ArcGridExpr::CropToSelector {
                source: Box::new(ArcGridExpr::Input),
                selector: ArcObjectSelector::ByColor { color: 2 },
            }),
            from: 2,
            to: 3,
        },
        metadata: ArcProgramMetadata {
            label: Some("crop-red-then-recolor".to_owned()),
            tier: ArcDslTier::TierA,
        },
    };

    let encoded = serde_json::to_string_pretty(&program).expect("program should serialize");
    let decoded: ArcProgram =
        serde_json::from_str(&encoded).expect("program should deserialize cleanly");
    assert_eq!(decoded, program);

    let input = grid(
        5,
        5,
        &[
            &[0, 0, 0, 0, 0],
            &[0, 2, 2, 0, 0],
            &[0, 2, 0, 0, 0],
            &[0, 0, 0, 4, 0],
            &[0, 0, 0, 0, 0],
        ],
    );
    let output = ArcInterpreter::execute(&program, &input).expect("program should execute");
    assert_eq!(output, grid(2, 2, &[&[3, 3], &[3, 0]]),);
}

#[test]
fn let_binding_and_if_any_objects_choose_the_expected_branch() {
    let program = ArcProgram {
        input_symbol: symbol("input"),
        bindings: vec![ArcGridBinding {
            name: symbol("focus"),
            value: ArcGridExpr::CropToSelector {
                source: Box::new(ArcGridExpr::Input),
                selector: ArcObjectSelector::ByColor { color: 6 },
            },
        }],
        body: ArcGridExpr::IfAnyObjects {
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::ByColor { color: 9 },
            then_branch: Box::new(ArcGridExpr::Var {
                name: symbol("focus"),
            }),
            else_branch: Box::new(ArcGridExpr::ReflectHorizontal {
                source: Box::new(ArcGridExpr::Var {
                    name: symbol("focus"),
                }),
            }),
        },
        metadata: ArcProgramMetadata::default(),
    };

    let input = grid(
        5,
        4,
        &[
            &[0, 0, 0, 0, 0],
            &[0, 6, 0, 0, 0],
            &[0, 6, 6, 0, 0],
            &[0, 0, 0, 0, 0],
        ],
    );
    let output = ArcInterpreter::execute(&program, &input).expect("program should execute");
    assert_eq!(output, grid(2, 2, &[&[0, 6], &[6, 6]]),);
}

#[test]
fn paint_selector_translates_and_recolors_objects_onto_a_new_canvas() {
    let program = ArcProgram::new(
        symbol("input"),
        ArcGridExpr::PaintSelector {
            base: Box::new(ArcGridExpr::Empty {
                width: 5,
                height: 4,
                fill: 0,
            }),
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::Largest,
            recolor: Some(8),
            transform: ArcObjectTransform::Translate { dx: 2, dy: 1 },
        },
    );

    let input = grid(
        5,
        4,
        &[
            &[5, 5, 0, 0, 0],
            &[5, 0, 0, 0, 0],
            &[0, 0, 4, 0, 0],
            &[0, 0, 0, 0, 0],
        ],
    );
    let output = ArcInterpreter::execute(&program, &input).expect("program should execute");
    assert_eq!(
        output,
        grid(
            5,
            4,
            &[
                &[0, 0, 0, 0, 0],
                &[0, 0, 8, 8, 0],
                &[0, 0, 8, 0, 0],
                &[0, 0, 0, 0, 0],
            ],
        ),
    );
}

#[test]
fn invalid_quarter_turns_are_rejected_explicitly() {
    let program = ArcProgram::new(
        symbol("input"),
        ArcGridExpr::RotateQuarterTurns {
            source: Box::new(ArcGridExpr::Input),
            quarter_turns: 4,
        },
    );

    let input = grid(2, 2, &[&[1, 0], &[0, 1]]);
    let error = ArcInterpreter::execute(&program, &input).expect_err("turn count should refuse");
    assert_eq!(error, ArcInterpreterError::InvalidQuarterTurns(4));
}

#[test]
fn translated_paint_refuses_when_cells_leave_the_canvas() {
    let program = ArcProgram::new(
        symbol("input"),
        ArcGridExpr::PaintSelector {
            base: Box::new(ArcGridExpr::Empty {
                width: 3,
                height: 3,
                fill: 0,
            }),
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::Largest,
            recolor: None,
            transform: ArcObjectTransform::Translate { dx: 2, dy: 2 },
        },
    );

    let input = grid(2, 2, &[&[7, 7], &[7, 0]]);
    let error = ArcInterpreter::execute(&program, &input).expect_err("paint should refuse");
    assert_eq!(error, ArcInterpreterError::PaintOutOfBounds { x: 3, y: 2 });
}
