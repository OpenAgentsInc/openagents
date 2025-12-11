//! Connections Story: Demonstrates bezier curve connections between pins

use gpui::{Context, Render, Window, canvas, div, point, prelude::*, px};
use hud::{Connection, ConnectionState, ConnectionStyle};
use theme_oa::bg;

use crate::story::Story;

pub struct ConnectionsStory;

impl Render for ConnectionsStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("Connections"))
            .child(Story::description("Connections are cubic bezier curves that link output pins to input pins."))
            .child(
                Story::section()
                    .child(Story::section_title("Connection States"))
                    .child(Story::description("Connections change color based on data flow state"))
                    .child(
                        div()
                            .h(px(200.0))
                            .w_full()
                            .bg(bg::APP)
                            .rounded(px(8.0))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, (), window, _cx| {
                                        let style = ConnectionStyle::default();

                                        // Inactive connection (top)
                                        let inactive = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(40.0)),
                                            point(bounds.origin.x + px(300.0), bounds.origin.y + px(40.0)),
                                        ).with_state(ConnectionState::Inactive);
                                        inactive.paint(&style, window);

                                        // Active connection (middle)
                                        let active = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(100.0)),
                                            point(bounds.origin.x + px(300.0), bounds.origin.y + px(100.0)),
                                        ).with_state(ConnectionState::Active);
                                        active.paint(&style, window);

                                        // Selected connection (bottom)
                                        let selected = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(160.0)),
                                            point(bounds.origin.x + px(300.0), bounds.origin.y + px(160.0)),
                                        ).with_state(ConnectionState::Selected);
                                        selected.paint(&style, window);
                                    }
                                ).size_full()
                            )
                    )
                    .child(
                        Story::row()
                            .child(Story::label("Inactive (gray) - No data flowing"))
                            .child(Story::label("Active (white) - Data flowing"))
                            .child(Story::label("Selected (cyan) - User selected"))
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Curved Paths"))
                    .child(Story::description("Connections use horizontal-biased bezier curves for clean routing"))
                    .child(
                        div()
                            .h(px(250.0))
                            .w_full()
                            .bg(bg::APP)
                            .rounded(px(8.0))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, (), window, _cx| {
                                        let style = ConnectionStyle::default();

                                        // Horizontal connection
                                        let conn1 = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(50.0)),
                                            point(bounds.origin.x + px(350.0), bounds.origin.y + px(50.0)),
                                        ).with_state(ConnectionState::Active);
                                        conn1.paint(&style, window);

                                        // Diagonal up
                                        let conn2 = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(200.0)),
                                            point(bounds.origin.x + px(350.0), bounds.origin.y + px(100.0)),
                                        ).with_state(ConnectionState::Active);
                                        conn2.paint(&style, window);

                                        // Diagonal down
                                        let conn3 = Connection::new(
                                            point(bounds.origin.x + px(50.0), bounds.origin.y + px(100.0)),
                                            point(bounds.origin.x + px(350.0), bounds.origin.y + px(200.0)),
                                        ).with_state(ConnectionState::Active);
                                        conn3.paint(&style, window);

                                        // Short connection
                                        let conn4 = Connection::new(
                                            point(bounds.origin.x + px(400.0), bounds.origin.y + px(125.0)),
                                            point(bounds.origin.x + px(500.0), bounds.origin.y + px(125.0)),
                                        ).with_state(ConnectionState::Selected);
                                        conn4.paint(&style, window);
                                    }
                                ).size_full()
                            )
                    )
            )
    }
}
