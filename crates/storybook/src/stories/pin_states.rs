//! Pin States Story: Demonstrates all pin visual states

use gpui::{Context, Render, Window, div, prelude::*, px};
use unit::PinState;
use hud::{PinSnapshot, PinView, PinDirection};

use crate::story::Story;

pub struct PinStatesStory;

impl Render for PinStatesStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Create pin views for each state
        let empty_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "empty".to_string(),
            state: PinState::Empty,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "i32".to_string(),
        }));

        let valid_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "valid".to_string(),
            state: PinState::Valid,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "i32".to_string(),
        }));

        let invalid_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "invalid".to_string(),
            state: PinState::Invalid,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "i32".to_string(),
        }));

        let constant_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "constant".to_string(),
            state: PinState::Valid,
            is_constant: true,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "i32".to_string(),
        }));

        let input_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "input".to_string(),
            state: PinState::Valid,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "String".to_string(),
        }));

        let output_pin = cx.new(|_| PinView::new(PinSnapshot {
            name: "output".to_string(),
            state: PinState::Valid,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Output,
            type_name: "String".to_string(),
        }));

        Story::container()
            .child(Story::title("Pin States"))
            .child(Story::description("Pins are the connection points on units. They have different visual states based on their data status."))
            .child(
                Story::section()
                    .child(Story::section_title("Pin Data States"))
                    .child(Story::description("Pins change color based on whether they contain data"))
                    .child(
                        Story::row()
                            .child(
                                Story::item("Empty (no data)")
                                    .child(div().p(px(8.0)).child(empty_pin))
                            )
                            .child(
                                Story::item("Valid (has data)")
                                    .child(div().p(px(8.0)).child(valid_pin))
                            )
                            .child(
                                Story::item("Invalid (error)")
                                    .child(div().p(px(8.0)).child(invalid_pin))
                            )
                            .child(
                                Story::item("Constant (locked value)")
                                    .child(div().p(px(8.0)).child(constant_pin))
                            )
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Pin Directions"))
                    .child(Story::description("Input pins receive data, output pins emit data"))
                    .child(
                        Story::row()
                            .child(
                                Story::item("Input Pin")
                                    .child(div().p(px(8.0)).child(input_pin))
                            )
                            .child(
                                Story::item("Output Pin")
                                    .child(div().p(px(8.0)).child(output_pin))
                            )
                    )
            )
    }
}
