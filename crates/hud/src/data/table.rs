//! Table - sci-fi styled data table with headers.

use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::theme::hud;

/// Table column definition.
#[derive(Clone)]
pub struct TableColumn {
    /// Column header text.
    pub header: String,
    /// Column width (relative proportion).
    pub width: f32,
}

impl TableColumn {
    /// Create a new column.
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            width: 1.0,
        }
    }

    /// Set the column width proportion.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// A table row with cell values.
pub type TableRow = Vec<String>;

/// Sci-fi styled data table.
///
/// Features:
/// - Animated header reveal
/// - Staggered row animations
/// - Alternating row backgrounds
/// - Header underline
///
/// # Example
///
/// ```ignore
/// let mut table = Table::new()
///     .columns(vec![
///         TableColumn::new("Name").width(2.0),
///         TableColumn::new("Status").width(1.0),
///         TableColumn::new("Value").width(1.0),
///     ])
///     .rows(vec![
///         vec!["Alpha".into(), "Active".into(), "100".into()],
///         vec!["Beta".into(), "Idle".into(), "50".into()],
///     ]);
///
/// table.animator_mut().enter();
///
/// // In update:
/// table.tick();
///
/// // In paint:
/// table.paint(bounds, &mut scene, &mut text_system);
/// ```
pub struct Table {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    animator: HudAnimator,

    // Animation state
    header_progress: f32,
    row_progress: Vec<f32>,

    // Styling
    font_size: f32,
    header_font_size: f32,
    row_height: f32,
    header_height: f32,
    cell_padding: f32,
    stagger_offset: f32,
}

impl Table {
    /// Create a new empty table.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            animator: HudAnimator::new(),
            header_progress: 0.0,
            row_progress: Vec::new(),
            font_size: 12.0,
            header_font_size: 11.0,
            row_height: 28.0,
            header_height: 32.0,
            cell_padding: 10.0,
            stagger_offset: 2.0,
        }
    }

    /// Set the columns.
    pub fn columns(mut self, columns: Vec<TableColumn>) -> Self {
        self.columns = columns;
        self
    }

    /// Set the rows.
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.row_progress = vec![0.0; rows.len()];
        self.rows = rows;
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the row height.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height;
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

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();

        let parent_progress = self.animator.progress();
        let speed = 0.15;

        // Header animates first
        let target_header = parent_progress;
        if self.header_progress < target_header {
            self.header_progress = (self.header_progress + speed).min(target_header);
        } else if self.header_progress > target_header {
            self.header_progress = (self.header_progress - speed).max(target_header);
        }

        // Rows animate with stagger after header reaches ~30%
        for (i, progress) in self.row_progress.iter_mut().enumerate() {
            let stagger_threshold = 0.3 + i as f32 * 0.08; // 30% + 8% per row
            let target = if parent_progress > stagger_threshold {
                ((parent_progress - stagger_threshold) / (1.0 - stagger_threshold)).min(1.0)
            } else {
                0.0
            };

            if *progress < target {
                *progress = (*progress + speed).min(target);
            } else if *progress > target {
                *progress = (*progress - speed).max(target);
            }
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self) -> Size {
        let height = self.header_height + self.rows.len() as f32 * self.row_height;
        Size::new(300.0, height)
    }

    /// Calculate column widths based on proportions.
    fn calculate_column_widths(&self, total_width: f32) -> Vec<f32> {
        let total_proportion: f32 = self.columns.iter().map(|c| c.width).sum();
        self.columns
            .iter()
            .map(|c| (c.width / total_proportion) * total_width)
            .collect()
    }

    /// Paint the table.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let parent_progress = self.animator.progress();
        if parent_progress <= 0.0 {
            return;
        }

        let column_widths = self.calculate_column_widths(bounds.size.width);
        let header_eased = easing::ease_out_expo(self.header_progress);

        // Draw header background
        if header_eased > 0.0 {
            let header_bounds = Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                self.header_height,
            );

            scene.draw_quad(
                wgpui::Quad::new(header_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 1.0, 0.03 * header_eased))
            );

            // Draw header underline
            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    bounds.origin.x,
                    bounds.origin.y + self.header_height - 1.0,
                    bounds.size.width * header_eased,
                    1.0,
                ))
                .with_background(Hsla::new(
                    hud::FRAME_DIM.h,
                    hud::FRAME_DIM.s,
                    hud::FRAME_DIM.l,
                    hud::FRAME_DIM.a * header_eased,
                ))
            );

            // Draw header text
            let mut x = bounds.origin.x + self.cell_padding;
            for (i, col) in self.columns.iter().enumerate() {
                let text_y = bounds.origin.y + (self.header_height - self.header_font_size) / 2.0;
                let text_color = Hsla::new(
                    hud::TEXT_MUTED.h,
                    hud::TEXT_MUTED.s,
                    hud::TEXT_MUTED.l,
                    hud::TEXT_MUTED.a * header_eased,
                );

                let text_run = text_system.layout(
                    &col.header.to_uppercase(),
                    Point::new(x, text_y),
                    self.header_font_size,
                    text_color,
                );
                scene.draw_text(text_run);

                x += column_widths[i];
            }
        }

        // Draw rows
        for (row_idx, row) in self.rows.iter().enumerate() {
            let row_progress = self.row_progress.get(row_idx).copied().unwrap_or(0.0);
            if row_progress <= 0.0 {
                continue;
            }

            let eased = easing::ease_out_expo(row_progress);
            let row_y = bounds.origin.y + self.header_height + row_idx as f32 * self.row_height;

            // Alternating background
            if row_idx % 2 == 0 {
                scene.draw_quad(
                    wgpui::Quad::new(Bounds::new(
                        bounds.origin.x,
                        row_y,
                        bounds.size.width,
                        self.row_height,
                    ))
                    .with_background(Hsla::new(0.0, 0.0, 1.0, 0.02 * eased))
                );
            }

            // Draw cells
            let mut x = bounds.origin.x + self.cell_padding;
            let slide_offset = 15.0 * (1.0 - eased);

            for (col_idx, cell) in row.iter().enumerate() {
                if col_idx >= column_widths.len() {
                    break;
                }

                let text_y = row_y + (self.row_height - self.font_size) / 2.0;
                let text_color = Hsla::new(
                    hud::TEXT.h,
                    hud::TEXT.s,
                    hud::TEXT.l,
                    hud::TEXT.a * eased,
                );

                let text_run = text_system.layout(
                    cell,
                    Point::new(x + slide_offset, text_y),
                    self.font_size,
                    text_color,
                );
                scene.draw_text(text_run);

                x += column_widths[col_idx];
            }
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}
