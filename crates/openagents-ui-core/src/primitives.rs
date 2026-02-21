use wgpui::{Bounds, Quad, Scene, Size};

use crate::tokens::{palette, spacing};

#[derive(Debug, Clone, Copy)]
pub struct ShellCardSpec {
    pub max_width: f32,
    pub height: f32,
    pub edge_margin: f32,
}

impl Default for ShellCardSpec {
    fn default() -> Self {
        Self {
            max_width: spacing::CARD_MAX_WIDTH,
            height: spacing::CARD_HEIGHT,
            edge_margin: spacing::EDGE_MARGIN,
        }
    }
}

pub fn draw_shell_backdrop(scene: &mut Scene, size: Size) {
    let background = Quad::new(Bounds::new(0.0, 0.0, size.width, size.height))
        .with_background(palette::canvas_bg());
    scene.draw_quad(background);
}

pub fn draw_shell_card(scene: &mut Scene, size: Size, spec: ShellCardSpec) -> Bounds {
    let width = (size.width * 0.72).min(spec.max_width);
    let card_x = ((size.width - width) * 0.5).max(spec.edge_margin);
    let card_y = ((size.height - spec.height) * 0.5).max(spec.edge_margin);
    let bounds = Bounds::new(card_x, card_y, width, spec.height);

    let card = Quad::new(bounds)
        .with_background(palette::surface_card())
        .with_border(palette::border_subtle(), 1.0);
    scene.draw_quad(card);

    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_card_is_centered_with_minimum_margins() {
        let mut scene = Scene::new();
        let size = Size::new(1200.0, 800.0);
        let bounds = draw_shell_card(&mut scene, size, ShellCardSpec::default());

        assert!(bounds.origin.x >= spacing::EDGE_MARGIN);
        assert!(bounds.origin.y >= spacing::EDGE_MARGIN);
        assert!(bounds.size.width <= spacing::CARD_MAX_WIDTH);
        assert_eq!(bounds.size.height, spacing::CARD_HEIGHT);
        assert_eq!(scene.quads.len(), 1);
    }
}
