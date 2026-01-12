use wgpui::{Bounds, Hsla, Quad, Scene};

const CONNECTOR_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.3,
    a: 1.0,
};
const CONNECTOR_WIDTH: f32 = 2.0;
const ARROW_SIZE: f32 = 6.0;

pub struct Connector;

impl Connector {
    /// Draw a vertical connector line with arrow from y_start to y_end at x_center
    pub fn paint(y_start: f32, y_end: f32, x_center: f32, scene: &mut Scene) {
        let line_height = y_end - y_start - ARROW_SIZE;

        if line_height > 0.0 {
            // Vertical line
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - CONNECTOR_WIDTH / 2.0,
                    y_start,
                    CONNECTOR_WIDTH,
                    line_height,
                ))
                .with_background(CONNECTOR_COLOR),
            );

            // Arrow head (simple triangle approximated with small quads)
            let arrow_y = y_end - ARROW_SIZE;

            // Left part of arrow
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - ARROW_SIZE / 2.0,
                    arrow_y,
                    ARROW_SIZE / 2.0,
                    CONNECTOR_WIDTH,
                ))
                .with_background(CONNECTOR_COLOR),
            );

            // Right part of arrow
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center,
                    arrow_y,
                    ARROW_SIZE / 2.0,
                    CONNECTOR_WIDTH,
                ))
                .with_background(CONNECTOR_COLOR),
            );

            // Center vertical tip
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - CONNECTOR_WIDTH / 2.0,
                    arrow_y,
                    CONNECTOR_WIDTH,
                    ARROW_SIZE,
                ))
                .with_background(CONNECTOR_COLOR),
            );
        }
    }
}
