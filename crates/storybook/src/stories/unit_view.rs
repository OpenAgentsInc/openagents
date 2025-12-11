//! Unit View Story: Demonstrates unit boxes with different states

use gpui_oa::{Context, Render, Window, div, point, prelude::*, px};
use unit::{Lifecycle, PinState};
use hud::{PinSnapshot, PinDirection, UnitSnapshot, UnitView};

use crate::story::Story;

pub struct UnitViewStory;

impl Render for UnitViewStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Playing unit with pins
        let playing_unit = cx.new(|cx| UnitView::new(UnitSnapshot {
            id: "add".to_string(),
            lifecycle: Lifecycle::Playing,
            inputs: vec![
                PinSnapshot {
                    name: "a".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "i32".to_string(),
                },
                PinSnapshot {
                    name: "b".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "i32".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "sum".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "i32".to_string(),
                },
            ],
            error: None,
            position: point(px(0.0), px(0.0)),
        }, cx));

        // Paused unit
        let paused_unit = cx.new(|cx| UnitView::new(UnitSnapshot {
            id: "multiply".to_string(),
            lifecycle: Lifecycle::Paused,
            inputs: vec![
                PinSnapshot {
                    name: "x".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
                PinSnapshot {
                    name: "y".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "product".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "f64".to_string(),
                },
            ],
            error: None,
            position: point(px(0.0), px(0.0)),
        }, cx));

        // Error unit
        let error_unit = cx.new(|cx| UnitView::new(UnitSnapshot {
            id: "divide".to_string(),
            lifecycle: Lifecycle::Playing,
            inputs: vec![
                PinSnapshot {
                    name: "numerator".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
                PinSnapshot {
                    name: "denominator".to_string(),
                    state: PinState::Invalid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "quotient".to_string(),
                    state: PinState::Invalid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "f64".to_string(),
                },
            ],
            error: Some("Division by zero".to_string()),
            position: point(px(0.0), px(0.0)),
        }, cx));

        // Unit with many pins
        let many_pins_unit = cx.new(|cx| UnitView::new(UnitSnapshot {
            id: "transform".to_string(),
            lifecycle: Lifecycle::Playing,
            inputs: vec![
                PinSnapshot {
                    name: "x".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
                PinSnapshot {
                    name: "y".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
                PinSnapshot {
                    name: "z".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
                PinSnapshot {
                    name: "scale".to_string(),
                    state: PinState::Valid,
                    is_constant: true,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "f64".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "result".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "Vec3".to_string(),
                },
                PinSnapshot {
                    name: "magnitude".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "f64".to_string(),
                },
            ],
            error: None,
            position: point(px(0.0), px(0.0)),
        }, cx));

        Story::container()
            .child(Story::title("Unit View"))
            .child(Story::description("Units are the building blocks of dataflow graphs. They process inputs and produce outputs."))
            .child(
                Story::section()
                    .child(Story::section_title("Lifecycle States"))
                    .child(Story::description("Units can be playing (active) or paused"))
                    .child(
                        Story::row()
                            .child(
                                Story::item("Playing")
                                    .child(div().p(px(16.0)).child(playing_unit))
                            )
                            .child(
                                Story::item("Paused")
                                    .child(div().p(px(16.0)).child(paused_unit))
                            )
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Error State"))
                    .child(Story::description("Units with errors show a red background"))
                    .child(
                        div().p(px(16.0)).child(error_unit)
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Multiple Pins"))
                    .child(Story::description("Units can have many input and output pins"))
                    .child(
                        div().p(px(16.0)).child(many_pins_unit)
                    )
            )
    }
}
