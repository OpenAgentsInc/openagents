//! Graph View Story: Demonstrates the full graph canvas with physics

use gpui_oa::{Context, Entity, Render, Window, div, prelude::*, px};
use hud::GraphView;

use crate::story::Story;

pub struct GraphViewStory {
    graph_view: Entity<GraphView>,
}

impl GraphViewStory {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Create a graph view with some demo nodes
        let graph_view = cx.new(|cx| {
            let mut view = GraphView::new(cx);

            // Add some demo nodes at different positions with proper context
            view.add_demo_node_with_cx("input", 100.0, 200.0, cx);
            view.add_demo_node_with_cx("process", 300.0, 150.0, cx);
            view.add_demo_node_with_cx("output", 500.0, 200.0, cx);
            view.add_demo_node_with_cx("filter", 300.0, 300.0, cx);

            view
        });

        Self { graph_view }
    }
}

impl Render for GraphViewStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("Graph View"))
            .child(Story::description("Interactive graph canvas with force-directed physics layout."))
            .child(
                Story::section()
                    .child(Story::section_title("Interactive Canvas"))
                    .child(Story::description("Pan: Click and drag on background | Zoom: Scroll wheel | Select: Click node | Multi-select: Cmd+Click"))
                    .child(
                        div()
                            .h(px(500.0))
                            .w_full()
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .child(self.graph_view.clone())
                    )
            )
            .child(
                Story::section()
                    .child(Story::section_title("Physics Simulation"))
                    .child(Story::description("Nodes repel each other and are attracted to the center. The simulation runs automatically and cools down over time."))
                    .child(
                        Story::column()
                            .child(Story::label("Forces applied:"))
                            .child(Story::label("  - Repulsion: Nodes push away from each other"))
                            .child(Story::label("  - Center gravity: Nodes are pulled toward the center"))
                            .child(Story::label("  - Velocity damping: Motion slows over time"))
                    )
            )
    }
}
