//! Topology Panel - visualization for execution venue topology

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{ExecutionVenue, FmVizState};

/// Draw the venue topology panel
pub fn draw_topology_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg())
        .with_corner_radius(4.0),
    );

    // Header
    let header_height = 24.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, header_height),
        })
        .with_background(Hsla::new(0.0, 0.0, 0.12, 1.0))
        .with_corner_radius(4.0),
    );

    // Title
    let title = "EXECUTION TOPOLOGY";
    let title_run = text.layout(title, Point::new(x + 8.0, y + 6.0), 10.0, text_dim());
    scene.draw_text(title_run);

    // Graph area
    let graph_y = y + header_height + 4.0;
    let graph_height = height - header_height - 50.0; // Leave room for legend

    if graph_height > 50.0 {
        let bounds = Bounds {
            origin: Point::new(x + 4.0, graph_y),
            size: Size::new(width - 8.0, graph_height),
        };

        // Paint the graph
        let scale_factor = 1.0;
        let mut paint_cx = PaintContext::new(scene, text, scale_factor);
        state.venue_topology.graph.paint(bounds, &mut paint_cx);
    }

    // Legend at bottom
    let legend_y = y + height - 40.0;
    draw_venue_legend(scene, text, x + 8.0, legend_y, width - 16.0);
}

/// Draw venue legend
fn draw_venue_legend(
    scene: &mut Scene,
    text: &mut TextSystem,
    x: f32,
    y: f32,
    _width: f32,
) {
    let venues = [
        (ExecutionVenue::Local, "Local (FM)"),
        (ExecutionVenue::Swarm, "Swarm (NIP-90)"),
        (ExecutionVenue::Datacenter, "Datacenter"),
    ];

    let mut lx = x;
    for (venue, label) in venues.iter() {
        let color = venue.color();

        // Draw dot
        let dot_size = 8.0;
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(lx, y + 4.0),
                size: Size::new(dot_size, dot_size),
            })
            .with_background(color)
            .with_corner_radius(dot_size / 2.0),
        );

        // Draw label
        let label_run = text.layout(label, Point::new(lx + 12.0, y + 2.0), 9.0, text_dim());
        scene.draw_text(label_run);

        // Measure label width for spacing
        let label_width = text.measure(label, 9.0);
        lx += 12.0 + label_width + 16.0;
    }
}

/// Draw compact topology indicator (for use in other panels)
#[allow(dead_code)]
pub fn draw_venue_indicator(
    scene: &mut Scene,
    text: &mut TextSystem,
    venue: ExecutionVenue,
    x: f32,
    y: f32,
) {
    let color = venue.color();
    let label = venue.label();

    // Draw dot
    let dot_size = 6.0;
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y + 2.0),
            size: Size::new(dot_size, dot_size),
        })
        .with_background(color)
        .with_corner_radius(dot_size / 2.0),
    );

    // Draw label
    let label_run = text.layout(label, Point::new(x + 10.0, y), 9.0, text_dim());
    scene.draw_text(label_run);
}

// Color helpers
fn panel_bg() -> Hsla {
    Hsla::new(220.0 / 360.0, 0.15, 0.08, 1.0)
}

fn text_dim() -> Hsla {
    Hsla::new(0.0, 0.0, 0.5, 1.0)
}
