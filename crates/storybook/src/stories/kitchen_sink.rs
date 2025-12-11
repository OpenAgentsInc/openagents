//! Kitchen Sink Story: All components in one view

use gpui_oa::{Context, Entity, Render, Window, canvas, div, point, prelude::*, px};
use unit::{Lifecycle, PinState};
use hud::{Connection, ConnectionState, ConnectionStyle, GraphView, PinSnapshot, PinDirection, UnitSnapshot, UnitView, PinView};
use theme_oa::bg;

use crate::story::Story;

pub struct KitchenSinkStory {
    graph_view: Entity<GraphView>,
}

impl KitchenSinkStory {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let graph_view = cx.new(|cx| {
            let mut view = GraphView::new(cx);
            view.add_demo_node_with_cx("source", 100.0, 150.0, cx);
            view.add_demo_node_with_cx("transform", 300.0, 100.0, cx);
            view.add_demo_node_with_cx("sink", 500.0, 150.0, cx);
            view
        });

        Self { graph_view }
    }
}

impl Render for KitchenSinkStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Create pin views
        let pins: Vec<Entity<PinView>> = vec![
            cx.new(|_| PinView::new(PinSnapshot {
                name: "empty".to_string(),
                state: PinState::Empty,
                is_constant: false,
                is_ignored: false,
                direction: PinDirection::Input,
                type_name: "i32".to_string(),
            })),
            cx.new(|_| PinView::new(PinSnapshot {
                name: "valid".to_string(),
                state: PinState::Valid,
                is_constant: false,
                is_ignored: false,
                direction: PinDirection::Input,
                type_name: "i32".to_string(),
            })),
            cx.new(|_| PinView::new(PinSnapshot {
                name: "invalid".to_string(),
                state: PinState::Invalid,
                is_constant: false,
                is_ignored: false,
                direction: PinDirection::Input,
                type_name: "i32".to_string(),
            })),
            cx.new(|_| PinView::new(PinSnapshot {
                name: "constant".to_string(),
                state: PinState::Valid,
                is_constant: true,
                is_ignored: false,
                direction: PinDirection::Input,
                type_name: "i32".to_string(),
            })),
        ];

        // Create unit views
        let unit_playing = cx.new(|cx| UnitView::new(UnitSnapshot {
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

        let unit_paused = cx.new(|cx| UnitView::new(UnitSnapshot {
            id: "filter".to_string(),
            lifecycle: Lifecycle::Paused,
            inputs: vec![
                PinSnapshot {
                    name: "input".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "T".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "output".to_string(),
                    state: PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "T".to_string(),
                },
            ],
            error: None,
            position: point(px(0.0), px(0.0)),
        }, cx));

        Story::container()
            .child(Story::title("Kitchen Sink - All Components"))
            .child(Story::description("Overview of all hud components for Unit dataflow graphs"))

            // Pins section
            .child(
                Story::section()
                    .child(Story::section_title("Pins"))
                    .child(Story::description("Connection points on units with state-based coloring"))
                    .child(
                        Story::row()
                            .child(Story::item("Empty").child(div().p(px(8.0)).child(pins[0].clone())))
                            .child(Story::item("Valid").child(div().p(px(8.0)).child(pins[1].clone())))
                            .child(Story::item("Invalid").child(div().p(px(8.0)).child(pins[2].clone())))
                            .child(Story::item("Constant").child(div().p(px(8.0)).child(pins[3].clone())))
                    )
            )

            // Units section
            .child(
                Story::section()
                    .child(Story::section_title("Units"))
                    .child(Story::description("Processing nodes with inputs and outputs"))
                    .child(
                        Story::row()
                            .child(Story::item("Playing").child(div().p(px(16.0)).child(unit_playing)))
                            .child(Story::item("Paused").child(div().p(px(16.0)).child(unit_paused)))
                    )
            )

            // Connections section
            .child(
                Story::section()
                    .child(Story::section_title("Connections"))
                    .child(Story::description("Bezier curves linking pins"))
                    .child(
                        div()
                            .h(px(120.0))
                            .w_full()
                            .bg(bg::APP)
                            .rounded(px(8.0))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, (), window, _cx| {
                                        let style = ConnectionStyle::default();

                                        let inactive = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(30.0)),
                                            point(bounds.origin.x + px(250.0), bounds.origin.y + px(30.0)),
                                        ).with_state(ConnectionState::Inactive);
                                        inactive.paint(&style, window);

                                        let active = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(60.0)),
                                            point(bounds.origin.x + px(250.0), bounds.origin.y + px(60.0)),
                                        ).with_state(ConnectionState::Active);
                                        active.paint(&style, window);

                                        let selected = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(90.0)),
                                            point(bounds.origin.x + px(250.0), bounds.origin.y + px(90.0)),
                                        ).with_state(ConnectionState::Selected);
                                        selected.paint(&style, window);
                                    }
                                ).size_full()
                            )
                    )
            )

            // Graph View section
            .child(
                Story::section()
                    .child(Story::section_title("Graph View"))
                    .child(Story::description("Interactive canvas with physics-based layout"))
                    .child(
                        div()
                            .h(px(300.0))
                            .w_full()
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .child(self.graph_view.clone())
                    )
            )
    }
}
