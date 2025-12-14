//! Select - sci-fi styled dropdown menu.

use wgpui::{Bounds, Hsla, InputEvent, MouseButton, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// Callback for selection change.
pub type OnChange = Box<dyn FnMut(usize, &str)>;

/// A selectable option.
#[derive(Clone)]
pub struct SelectOption {
    /// Display label.
    pub label: String,
    /// Optional value (defaults to label if not set).
    pub value: Option<String>,
}

impl SelectOption {
    /// Create a new option with just a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: None,
        }
    }

    /// Create an option with both label and value.
    pub fn with_value(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: Some(value.into()),
        }
    }

    /// Get the value (or label if no value set).
    pub fn get_value(&self) -> &str {
        self.value.as_deref().unwrap_or(&self.label)
    }
}

/// Sci-fi styled dropdown select.
///
/// Features:
/// - Animated dropdown expansion
/// - Hover highlights on options
/// - Arrow indicator
///
/// # Example
///
/// ```ignore
/// let mut select = Select::new()
///     .options(vec![
///         SelectOption::new("Option 1"),
///         SelectOption::new("Option 2"),
///         SelectOption::new("Option 3"),
///     ])
///     .on_change(|index, value| println!("Selected: {} = {}", index, value));
///
/// select.animator_mut().enter();
///
/// // In update:
/// select.tick();
///
/// // In paint:
/// select.paint(bounds, &mut scene, &mut text_system);
///
/// // In event handling:
/// select.event(&event, bounds);
/// ```
pub struct Select {
    options: Vec<SelectOption>,
    selected_index: usize,
    placeholder: String,
    animator: HudAnimator,

    // State
    open: bool,
    hovered: bool,
    hovered_option: Option<usize>,

    // Animation
    open_progress: f32,

    // Styling
    font_size: f32,
    padding: (f32, f32),
    option_height: f32,
    border_width: f32,
    max_visible_options: usize,

    // Callbacks
    on_change: Option<OnChange>,
}

impl Select {
    /// Create a new select.
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            selected_index: 0,
            placeholder: String::from("Select..."),
            animator: HudAnimator::new(),
            open: false,
            hovered: false,
            hovered_option: None,
            open_progress: 0.0,
            font_size: 14.0,
            padding: (12.0, 8.0),
            option_height: 32.0,
            border_width: 1.0,
            max_visible_options: 5,
            on_change: None,
        }
    }

    /// Set the options.
    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    /// Set the initially selected index.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the padding (horizontal, vertical).
    pub fn padding(mut self, h: f32, v: f32) -> Self {
        self.padding = (h, v);
        self
    }

    /// Set the max visible options before scrolling.
    pub fn max_visible_options(mut self, count: usize) -> Self {
        self.max_visible_options = count;
        self
    }

    /// Set the on_change callback.
    pub fn on_change<F: FnMut(usize, &str) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get the selected option, if any.
    pub fn selected_option(&self) -> Option<&SelectOption> {
        self.options.get(self.selected_index)
    }

    /// Set the selected index programmatically.
    pub fn set_selected(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected_index = index;
        }
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Check if the dropdown is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();

        // Animate open progress
        let target = if self.open { 1.0 } else { 0.0 };
        let speed = 0.15;
        if self.open_progress < target {
            self.open_progress = (self.open_progress + speed).min(target);
        } else if self.open_progress > target {
            self.open_progress = (self.open_progress - speed).max(target);
        }
    }

    /// Calculate preferred size for the closed select.
    pub fn preferred_size(&self) -> Size {
        let height = self.font_size + self.padding.1 * 2.0;
        Size::new(200.0, height) // Default width
    }

    /// Calculate the dropdown bounds when open.
    fn dropdown_bounds(&self, header_bounds: Bounds) -> Bounds {
        let visible_count = self.options.len().min(self.max_visible_options);
        let dropdown_height = visible_count as f32 * self.option_height;

        Bounds::new(
            header_bounds.origin.x,
            header_bounds.origin.y + header_bounds.size.height,
            header_bounds.size.width,
            dropdown_height,
        )
    }

    /// Paint the select.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Draw header background
        let header_bg_alpha = if self.open {
            0.08
        } else if self.hovered {
            0.04
        } else {
            0.02
        };
        scene.draw_quad(
            wgpui::Quad::new(bounds)
                .with_background(Hsla::new(0.0, 0.0, 1.0, header_bg_alpha * progress))
        );

        // Draw header border
        let header_border_color = if self.open || self.hovered {
            hud::FRAME_BRIGHT
        } else {
            hud::FRAME_DIM
        };
        scene.draw_quad(
            wgpui::Quad::new(bounds)
                .with_border(Hsla::new(
                    header_border_color.h,
                    header_border_color.s,
                    header_border_color.l,
                    header_border_color.a * progress,
                ), self.border_width)
        );

        // Draw selected text or placeholder
        let display_text = if self.options.is_empty() {
            &self.placeholder
        } else {
            &self.options[self.selected_index].label
        };

        let text_color = if self.options.is_empty() {
            hud::TEXT_MUTED
        } else {
            hud::TEXT
        };

        let text_x = bounds.origin.x + self.padding.0;
        let text_y = bounds.origin.y + (bounds.size.height - self.font_size) / 2.0;
        let text_run = text_system.layout(
            display_text,
            Point::new(text_x, text_y),
            self.font_size,
            Hsla::new(text_color.h, text_color.s, text_color.l, text_color.a * progress),
        );
        scene.draw_text(text_run);

        // Draw arrow indicator
        let arrow_size = 8.0;
        let arrow_x = bounds.origin.x + bounds.size.width - self.padding.0 - arrow_size;
        let arrow_y = bounds.origin.y + (bounds.size.height - arrow_size) / 2.0;

        // Simple down arrow using two lines
        let arrow_color = Hsla::new(
            hud::TEXT_MUTED.h,
            hud::TEXT_MUTED.s,
            hud::TEXT_MUTED.l,
            hud::TEXT_MUTED.a * progress,
        );

        // Left leg of arrow
        scene.draw_quad(
            wgpui::Quad::new(Bounds::new(
                arrow_x, arrow_y,
                arrow_size / 2.0 + 1.0, 2.0,
            ))
            .with_background(arrow_color)
        );
        // Right leg of arrow
        scene.draw_quad(
            wgpui::Quad::new(Bounds::new(
                arrow_x + arrow_size / 2.0, arrow_y,
                arrow_size / 2.0 + 1.0, 2.0,
            ))
            .with_background(arrow_color)
        );

        // Draw dropdown if open
        if self.open_progress > 0.0 {
            let eased = easing::ease_out_expo(self.open_progress);
            let dropdown_bounds = self.dropdown_bounds(bounds);

            // Clip height by progress
            let visible_height = dropdown_bounds.size.height * eased;
            let clipped_bounds = Bounds::new(
                dropdown_bounds.origin.x,
                dropdown_bounds.origin.y,
                dropdown_bounds.size.width,
                visible_height,
            );

            // Draw dropdown background
            scene.draw_quad(
                wgpui::Quad::new(clipped_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 0.0, 0.95 * progress))
            );

            // Draw dropdown border
            scene.draw_quad(
                wgpui::Quad::new(clipped_bounds)
                    .with_border(Hsla::new(
                        hud::FRAME_DIM.h,
                        hud::FRAME_DIM.s,
                        hud::FRAME_DIM.l,
                        hud::FRAME_DIM.a * progress * eased,
                    ), self.border_width)
            );

            // Draw options
            let visible_count = self.options.len().min(self.max_visible_options);
            for i in 0..visible_count {
                let option_y = dropdown_bounds.origin.y + i as f32 * self.option_height;

                // Only draw if within visible area
                if option_y + self.option_height > dropdown_bounds.origin.y + visible_height {
                    break;
                }

                let option_bounds = Bounds::new(
                    dropdown_bounds.origin.x, option_y,
                    dropdown_bounds.size.width, self.option_height,
                );

                // Draw hover highlight
                let is_hovered = self.hovered_option == Some(i);
                let is_selected = i == self.selected_index;

                if is_hovered {
                    scene.draw_quad(
                        wgpui::Quad::new(option_bounds)
                            .with_background(Hsla::new(0.0, 0.0, 1.0, 0.1 * progress * eased))
                    );
                }

                // Draw option text
                let option_text_color = if is_selected {
                    hud::FRAME_BRIGHT
                } else if is_hovered {
                    hud::TEXT
                } else {
                    hud::TEXT_MUTED
                };

                let option_text_x = option_bounds.origin.x + self.padding.0;
                let option_text_y = option_bounds.origin.y + (self.option_height - self.font_size) / 2.0;

                let option_text_run = text_system.layout(
                    &self.options[i].label,
                    Point::new(option_text_x, option_text_y),
                    self.font_size,
                    Hsla::new(
                        option_text_color.h,
                        option_text_color.s,
                        option_text_color.l,
                        option_text_color.a * progress * eased,
                    ),
                );
                scene.draw_text(option_text_run);
            }
        }
    }

    /// Handle an input event.
    ///
    /// Returns `true` if the event was handled.
    pub fn event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let dropdown_bounds = self.dropdown_bounds(bounds);

        match event {
            InputEvent::MouseMove { position, .. } => {
                let was_hovered = self.hovered;
                let was_hovered_option = self.hovered_option;

                self.hovered = bounds.contains(*position);

                // Check option hover when open
                if self.open && dropdown_bounds.contains(*position) {
                    let relative_y = position.y - dropdown_bounds.origin.y;
                    let option_index = (relative_y / self.option_height) as usize;
                    if option_index < self.options.len() && option_index < self.max_visible_options {
                        self.hovered_option = Some(option_index);
                    } else {
                        self.hovered_option = None;
                    }
                } else {
                    self.hovered_option = None;
                }

                was_hovered != self.hovered || was_hovered_option != self.hovered_option
            }

            InputEvent::MouseDown { position, button, .. } => {
                if *button == MouseButton::Left {
                    // Check if clicking on header
                    if bounds.contains(*position) {
                        self.open = !self.open;
                        return true;
                    }

                    // Check if clicking on an option
                    if self.open && dropdown_bounds.contains(*position) {
                        let relative_y = position.y - dropdown_bounds.origin.y;
                        let option_index = (relative_y / self.option_height) as usize;
                        if option_index < self.options.len() && option_index < self.max_visible_options {
                            self.selected_index = option_index;
                            self.open = false;
                            if let Some(on_change) = &mut self.on_change {
                                let value = self.options[option_index].get_value().to_string();
                                on_change(option_index, &value);
                            }
                            return true;
                        }
                    }

                    // Clicking outside closes dropdown
                    if self.open {
                        self.open = false;
                        return true;
                    }
                }
                false
            }

            _ => false,
        }
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}
