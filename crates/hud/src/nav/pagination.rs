//! Pagination component for page navigation.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Pagination navigation component.
pub struct Pagination {
    current_page: usize,
    total_pages: usize,
    animator: HudAnimator,
    hovered: Option<PaginationButton>,

    // Styling
    color: Hsla,
    text_color: Hsla,
    button_size: f32,
    button_gap: f32,
    max_visible: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PaginationButton {
    Prev,
    Next,
    Page(usize),
    Ellipsis,
}

impl Pagination {
    /// Create pagination with current page and total pages.
    pub fn new(current: usize, total: usize) -> Self {
        Self {
            current_page: current.max(1).min(total),
            total_pages: total.max(1),
            animator: HudAnimator::new().enter_duration(20),
            hovered: None,
            color: colors::FRAME_NORMAL,
            text_color: colors::TEXT,
            button_size: 32.0,
            button_gap: 4.0,
            max_visible: 5,
        }
    }

    /// Set current page.
    pub fn current(mut self, page: usize) -> Self {
        self.current_page = page.max(1).min(self.total_pages);
        self
    }

    /// Set total pages.
    pub fn total(mut self, total: usize) -> Self {
        self.total_pages = total.max(1);
        self.current_page = self.current_page.min(self.total_pages);
        self
    }

    /// Set frame color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// Set maximum visible page buttons.
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max.max(3);
        self
    }

    /// Start enter animation.
    pub fn enter(&mut self) {
        self.animator.enter();
    }

    /// Start exit animation.
    pub fn exit(&mut self) {
        self.animator.exit();
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Get current page number.
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Handle mouse move.
    pub fn on_mouse_move(&mut self, bounds: Bounds, x: f32, y: f32) {
        if !bounds.contains(Point::new(x, y)) {
            self.hovered = None;
            return;
        }

        let buttons = self.get_visible_buttons();
        let mut current_x = bounds.x();

        // Check prev button
        if x >= current_x && x < current_x + self.button_size && self.current_page > 1 {
            self.hovered = Some(PaginationButton::Prev);
            return;
        }
        current_x += self.button_size + self.button_gap;

        // Check page buttons
        for button in &buttons {
            if x >= current_x && x < current_x + self.button_size {
                if let PaginationButton::Page(p) = button {
                    if *p != self.current_page {
                        self.hovered = Some(*button);
                        return;
                    }
                }
            }
            current_x += self.button_size + self.button_gap;
        }

        // Check next button
        if x >= current_x && x < current_x + self.button_size && self.current_page < self.total_pages {
            self.hovered = Some(PaginationButton::Next);
            return;
        }

        self.hovered = None;
    }

    /// Handle click, returns new page if changed.
    pub fn on_click(&mut self, bounds: Bounds, x: f32, y: f32) -> Option<usize> {
        self.on_mouse_move(bounds, x, y);

        match self.hovered {
            Some(PaginationButton::Prev) if self.current_page > 1 => {
                self.current_page -= 1;
                Some(self.current_page)
            }
            Some(PaginationButton::Next) if self.current_page < self.total_pages => {
                self.current_page += 1;
                Some(self.current_page)
            }
            Some(PaginationButton::Page(p)) if p != self.current_page => {
                self.current_page = p;
                Some(self.current_page)
            }
            _ => None,
        }
    }

    fn get_visible_buttons(&self) -> Vec<PaginationButton> {
        let mut buttons = Vec::new();

        if self.total_pages <= self.max_visible {
            // Show all pages
            for i in 1..=self.total_pages {
                buttons.push(PaginationButton::Page(i));
            }
        } else {
            // Show first, last, current, and neighbors with ellipsis
            let half = self.max_visible / 2;
            let start = if self.current_page <= half + 1 {
                1
            } else if self.current_page >= self.total_pages - half {
                self.total_pages - self.max_visible + 1
            } else {
                self.current_page - half
            };

            let end = (start + self.max_visible - 1).min(self.total_pages);

            if start > 1 {
                buttons.push(PaginationButton::Page(1));
                if start > 2 {
                    buttons.push(PaginationButton::Ellipsis);
                }
            }

            for i in start..=end {
                if i != 1 && i != self.total_pages {
                    buttons.push(PaginationButton::Page(i));
                }
            }

            if end < self.total_pages {
                if end < self.total_pages - 1 {
                    buttons.push(PaginationButton::Ellipsis);
                }
                buttons.push(PaginationButton::Page(self.total_pages));
            }
        }

        buttons
    }

    /// Paint the pagination.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let base_alpha = self.color.a * progress;
        let text_alpha = self.text_color.a * progress;
        let mut current_x = bounds.x();
        let button_y = bounds.y() + (bounds.height() - self.button_size) / 2.0;

        // Draw prev button
        self.draw_nav_button(
            scene,
            text_system,
            current_x,
            button_y,
            "<",
            self.current_page > 1,
            self.hovered == Some(PaginationButton::Prev),
            base_alpha,
            text_alpha,
        );
        current_x += self.button_size + self.button_gap;

        // Draw page buttons
        let buttons = self.get_visible_buttons();
        for button in &buttons {
            match button {
                PaginationButton::Page(p) => {
                    let is_current = *p == self.current_page;
                    let is_hovered = self.hovered == Some(*button);
                    self.draw_page_button(
                        scene,
                        text_system,
                        current_x,
                        button_y,
                        *p,
                        is_current,
                        is_hovered,
                        base_alpha,
                        text_alpha,
                    );
                }
                PaginationButton::Ellipsis => {
                    self.draw_ellipsis(scene, text_system, current_x, button_y, text_alpha);
                }
                _ => {}
            }
            current_x += self.button_size + self.button_gap;
        }

        // Draw next button
        self.draw_nav_button(
            scene,
            text_system,
            current_x,
            button_y,
            ">",
            self.current_page < self.total_pages,
            self.hovered == Some(PaginationButton::Next),
            base_alpha,
            text_alpha,
        );
    }

    fn draw_nav_button(
        &self,
        scene: &mut Scene,
        text_system: &mut TextSystem,
        x: f32,
        y: f32,
        label: &str,
        enabled: bool,
        hovered: bool,
        base_alpha: f32,
        text_alpha: f32,
    ) {
        let button_bounds = Bounds::from_origin_size(
            Point::new(x, y),
            Size::new(self.button_size, self.button_size),
        );

        let border_alpha = if !enabled {
            base_alpha * 0.2
        } else if hovered {
            base_alpha
        } else {
            base_alpha * 0.5
        };

        // Draw border
        scene.draw_quad(
            wgpui::Quad::new(button_bounds)
                .with_border(
                    Hsla::new(self.color.h, self.color.s, self.color.l, border_alpha),
                    1.0,
                ),
        );

        // Draw label
        let label_alpha = if !enabled {
            text_alpha * 0.2
        } else if hovered {
            text_alpha
        } else {
            text_alpha * 0.6
        };

        let text_run = text_system.layout(
            label,
            Point::new(x + self.button_size / 2.0 - 4.0, y + self.button_size / 2.0 + 4.0),
            14.0,
            Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, label_alpha),
        );
        scene.draw_text(text_run);
    }

    fn draw_page_button(
        &self,
        scene: &mut Scene,
        text_system: &mut TextSystem,
        x: f32,
        y: f32,
        page: usize,
        is_current: bool,
        is_hovered: bool,
        base_alpha: f32,
        text_alpha: f32,
    ) {
        let button_bounds = Bounds::from_origin_size(
            Point::new(x, y),
            Size::new(self.button_size, self.button_size),
        );

        if is_current {
            // Filled background for current page
            scene.draw_quad(
                wgpui::Quad::new(button_bounds)
                    .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, base_alpha * 0.3))
                    .with_border(
                        Hsla::new(self.color.h, self.color.s, self.color.l, base_alpha),
                        1.0,
                    ),
            );
        } else {
            let border_alpha = if is_hovered { base_alpha } else { base_alpha * 0.3 };
            scene.draw_quad(
                wgpui::Quad::new(button_bounds)
                    .with_border(
                        Hsla::new(self.color.h, self.color.s, self.color.l, border_alpha),
                        1.0,
                    ),
            );
        }

        // Draw page number
        let label = page.to_string();
        let label_alpha = if is_current || is_hovered {
            text_alpha
        } else {
            text_alpha * 0.5
        };

        let offset = if page >= 10 { 8.0 } else { 4.0 };
        let text_run = text_system.layout(
            &label,
            Point::new(x + self.button_size / 2.0 - offset, y + self.button_size / 2.0 + 4.0),
            14.0,
            Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, label_alpha),
        );
        scene.draw_text(text_run);
    }

    fn draw_ellipsis(&self, scene: &mut Scene, text_system: &mut TextSystem, x: f32, y: f32, text_alpha: f32) {
        let text_run = text_system.layout(
            "...",
            Point::new(x + self.button_size / 2.0 - 8.0, y + self.button_size / 2.0 + 4.0),
            14.0,
            Hsla::new(self.text_color.h, self.text_color.s, self.text_color.l, text_alpha * 0.5),
        );
        scene.draw_text(text_run);
    }
}
