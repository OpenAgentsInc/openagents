//! MovingLinesBackground - animated moving line pattern.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;

/// Direction for moving lines.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LineDirection {
    /// Lines move from left to right.
    #[default]
    Right,
    /// Lines move from right to left.
    Left,
    /// Lines move from top to bottom.
    Down,
    /// Lines move from bottom to top.
    Up,
}

/// Animated moving lines background.
///
/// Creates lines that continuously move across the screen,
/// wrapping around when they reach the edge. Creates a
/// dynamic, flowing effect.
///
/// # Example
///
/// ```ignore
/// let mut bg = MovingLinesBackground::new()
///     .spacing(80.0)
///     .line_width(1.0)
///     .speed(2.0)
///     .direction(LineDirection::Right);
///
/// bg.animator_mut().enter();
///
/// // In update:
/// bg.tick();
///
/// // In paint:
/// bg.paint(screen_bounds, &mut scene);
/// ```
pub struct MovingLinesBackground {
    animator: HudAnimator,
    /// Spacing between lines.
    spacing: f32,
    /// Line width.
    line_width: f32,
    /// Line color.
    color: Hsla,
    /// Movement speed in pixels per tick.
    speed: f32,
    /// Movement direction.
    direction: LineDirection,
    /// Current offset for animation.
    offset: f32,
}

impl Default for MovingLinesBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl MovingLinesBackground {
    /// Create a new moving lines background with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            spacing: 80.0,
            line_width: 1.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.08), // Very subtle white
            speed: 1.0,
            direction: LineDirection::Right,
            offset: 0.0,
        }
    }

    /// Set line spacing.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(10.0);
        self
    }

    /// Set line width.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Set line color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set movement speed (pixels per tick).
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Set movement direction.
    pub fn direction(mut self, direction: LineDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Tick the animation.
    ///
    /// Updates both the enter/exit animation and the movement offset.
    pub fn tick(&mut self) -> bool {
        let animating = self.animator.tick();

        // Update offset for continuous movement when visible
        if self.animator.state().is_visible() {
            self.offset += self.speed;
            // Wrap around
            if self.offset >= self.spacing {
                self.offset -= self.spacing;
            } else if self.offset < 0.0 {
                self.offset += self.spacing;
            }
        }

        animating || self.animator.state().is_entered()
    }

    /// Paint the moving lines.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            self.color.a * progress,
        );
        let w = self.line_width;

        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let width = bounds.size.width;
        let height = bounds.size.height;

        match self.direction {
            LineDirection::Right | LineDirection::Left => {
                // Vertical lines moving horizontally
                let start_offset = match self.direction {
                    LineDirection::Right => self.offset,
                    LineDirection::Left => self.spacing - self.offset,
                    _ => 0.0,
                };

                let mut line_x = x - self.spacing + start_offset;
                while line_x <= x + width + self.spacing {
                    if line_x >= x - w && line_x <= x + width + w {
                        scene.draw_quad(
                            Quad::new(Bounds::new(line_x - w / 2.0, y, w, height))
                                .with_background(color),
                        );
                    }
                    line_x += self.spacing;
                }
            }
            LineDirection::Down | LineDirection::Up => {
                // Horizontal lines moving vertically
                let start_offset = match self.direction {
                    LineDirection::Down => self.offset,
                    LineDirection::Up => self.spacing - self.offset,
                    _ => 0.0,
                };

                let mut line_y = y - self.spacing + start_offset;
                while line_y <= y + height + self.spacing {
                    if line_y >= y - w && line_y <= y + height + w {
                        scene.draw_quad(
                            Quad::new(Bounds::new(x, line_y - w / 2.0, width, w))
                                .with_background(color),
                        );
                    }
                    line_y += self.spacing;
                }
            }
        }
    }
}
