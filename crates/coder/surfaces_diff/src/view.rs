//! Diff viewer widget.

use crate::diff::{ChangeKind, DiffResult, Hunk};
use coder_ui_runtime::Signal;
use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{EventResult, Widget};
use wgpui::{Bounds, InputEvent, Point, Quad};

/// Diff view mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiffViewMode {
    /// Unified diff (single column with +/- markers).
    #[default]
    Unified,
    /// Side-by-side diff (two columns).
    SideBySide,
    /// Inline diff with word-level highlighting.
    Inline,
}

/// Colors for diff rendering.
pub mod colors {
    use wgpui::Hsla;

    /// Addition background.
    pub const ADDED_BG: Hsla = Hsla::new(120.0 / 360.0, 0.4, 0.15, 1.0);
    /// Addition foreground.
    pub const ADDED_FG: Hsla = Hsla::new(120.0 / 360.0, 0.6, 0.7, 1.0);
    /// Addition gutter.
    pub const ADDED_GUTTER: Hsla = Hsla::new(120.0 / 360.0, 0.5, 0.25, 1.0);

    /// Removal background.
    pub const REMOVED_BG: Hsla = Hsla::new(0.0, 0.4, 0.15, 1.0);
    /// Removal foreground.
    pub const REMOVED_FG: Hsla = Hsla::new(0.0, 0.6, 0.7, 1.0);
    /// Removal gutter.
    pub const REMOVED_GUTTER: Hsla = Hsla::new(0.0, 0.5, 0.25, 1.0);

    /// Context/unchanged background.
    pub const CONTEXT_BG: Hsla = Hsla::new(0.0, 0.0, 0.1, 1.0);
    /// Context foreground.
    pub const CONTEXT_FG: Hsla = Hsla::new(0.0, 0.0, 0.6, 1.0);

    /// Hunk header background.
    pub const HUNK_HEADER_BG: Hsla = Hsla::new(220.0 / 360.0, 0.3, 0.2, 1.0);
    /// Hunk header foreground.
    pub const HUNK_HEADER_FG: Hsla = Hsla::new(220.0 / 360.0, 0.5, 0.7, 1.0);

    /// Line number color.
    pub const LINE_NUMBER: Hsla = Hsla::new(0.0, 0.0, 0.4, 1.0);

    /// Word-level addition highlight.
    pub const WORD_ADDED: Hsla = Hsla::new(120.0 / 360.0, 0.5, 0.3, 1.0);
    /// Word-level removal highlight.
    pub const WORD_REMOVED: Hsla = Hsla::new(0.0, 0.5, 0.3, 1.0);
}

/// Diff viewer widget.
pub struct DiffView {
    /// The diff to display.
    diff: DiffResult,
    /// View mode.
    mode: Signal<DiffViewMode>,
    /// Scroll offset (lines from top).
    scroll_offset: Signal<f32>,
    /// Whether to show line numbers.
    show_line_numbers: bool,
    /// Font size.
    font_size: f32,
    /// Line height.
    line_height: f32,
    /// Gutter width.
    gutter_width: f32,
    /// Whether view has focus.
    focused: bool,
    /// Expanded hunks (all by default).
    expanded_hunks: Vec<bool>,
}

impl DiffView {
    /// Create a new diff view.
    pub fn new(diff: DiffResult) -> Self {
        let hunk_count = diff.hunks.len();
        Self {
            diff,
            mode: Signal::new(DiffViewMode::default()),
            scroll_offset: Signal::new(0.0),
            show_line_numbers: true,
            font_size: 13.0,
            line_height: 20.0,
            gutter_width: 50.0,
            focused: false,
            expanded_hunks: vec![true; hunk_count],
        }
    }

    /// Set the diff to display.
    pub fn set_diff(&mut self, diff: DiffResult) {
        let hunk_count = diff.hunks.len();
        self.diff = diff;
        self.expanded_hunks = vec![true; hunk_count];
        self.scroll_offset.set(0.0);
    }

    /// Get the current diff.
    pub fn diff(&self) -> &DiffResult {
        &self.diff
    }

    /// Set view mode.
    pub fn set_mode(&mut self, mode: DiffViewMode) {
        self.mode.set(mode);
    }

    /// Get view mode.
    pub fn mode(&self) -> DiffViewMode {
        self.mode.get_untracked()
    }

    /// Set font size.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        self.line_height = size * 1.5;
    }

    /// Toggle line numbers.
    pub fn toggle_line_numbers(&mut self) {
        self.show_line_numbers = !self.show_line_numbers;
    }

    /// Toggle a hunk's expanded state.
    pub fn toggle_hunk(&mut self, index: usize) {
        if let Some(expanded) = self.expanded_hunks.get_mut(index) {
            *expanded = !*expanded;
        }
    }

    /// Expand all hunks.
    pub fn expand_all(&mut self) {
        self.expanded_hunks.fill(true);
    }

    /// Collapse all hunks.
    pub fn collapse_all(&mut self) {
        self.expanded_hunks.fill(false);
    }

    /// Calculate total height for all content.
    fn content_height(&self) -> f32 {
        let mode = self.mode.get_untracked();
        match mode {
            DiffViewMode::Unified | DiffViewMode::Inline => {
                let mut lines = 0;
                for (i, hunk) in self.diff.hunks.iter().enumerate() {
                    lines += 1; // Hunk header
                    if self.expanded_hunks.get(i).copied().unwrap_or(true) {
                        lines += hunk.changes.len();
                    }
                }
                lines as f32 * self.line_height
            }
            DiffViewMode::SideBySide => {
                let mut lines = 0;
                for (i, hunk) in self.diff.hunks.iter().enumerate() {
                    lines += 1;
                    if self.expanded_hunks.get(i).copied().unwrap_or(true) {
                        // Each side shows all lines
                        lines += hunk.changes.len();
                    }
                }
                lines as f32 * self.line_height
            }
        }
    }

    /// Scroll by given delta.
    pub fn scroll(&mut self, delta: f32) {
        let max_scroll = (self.content_height() - 200.0).max(0.0);
        self.scroll_offset.update(|offset| {
            *offset = (*offset - delta).clamp(0.0, max_scroll);
        });
    }
}

impl Default for DiffView {
    fn default() -> Self {
        Self::new(DiffResult {
            changes: Vec::new(),
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
        })
    }
}

impl Widget for DiffView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw background
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(wgpui::theme::bg::SURFACE));

        let mode = self.mode.get_untracked();
        let scroll = self.scroll_offset.get_untracked();

        match mode {
            DiffViewMode::Unified => self.paint_unified(bounds, scroll, cx),
            DiffViewMode::SideBySide => self.paint_side_by_side(bounds, scroll, cx),
            DiffViewMode::Inline => self.paint_inline(bounds, scroll, cx),
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown { position, .. } => {
                if bounds.contains(*position) {
                    self.focused = true;
                    return EventResult::Handled;
                } else {
                    self.focused = false;
                }
            }
            InputEvent::Wheel { delta, .. } => {
                self.scroll(delta.y * 40.0);
                return EventResult::Handled;
            }
            InputEvent::KeyDown { key, .. } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                match key {
                    wgpui::Key::Character(c) if c == "u" => {
                        self.set_mode(DiffViewMode::Unified);
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "s" => {
                        self.set_mode(DiffViewMode::SideBySide);
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "i" => {
                        self.set_mode(DiffViewMode::Inline);
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "e" => {
                        self.expand_all();
                        return EventResult::Handled;
                    }
                    wgpui::Key::Character(c) if c == "c" => {
                        self.collapse_all();
                        return EventResult::Handled;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }
}

impl DiffView {
    /// Paint unified diff view.
    fn paint_unified(&self, bounds: Bounds, scroll: f32, cx: &mut PaintContext) {
        let mut y = bounds.origin.y - scroll;
        let gutter_width = if self.show_line_numbers {
            self.gutter_width * 2.0 // Two columns for old/new line numbers
        } else {
            0.0
        };

        for (hunk_idx, hunk) in self.diff.hunks.iter().enumerate() {
            // Paint hunk header
            if y + self.line_height > bounds.origin.y && y < bounds.origin.y + bounds.size.height {
                self.paint_hunk_header(bounds, y, hunk, cx);
            }
            y += self.line_height;

            // Paint changes if expanded
            if !self.expanded_hunks.get(hunk_idx).copied().unwrap_or(true) {
                continue;
            }

            for change in &hunk.changes {
                if y + self.line_height > bounds.origin.y
                    && y < bounds.origin.y + bounds.size.height
                {
                    let (bg, fg, marker) = match change.kind {
                        ChangeKind::Added => (colors::ADDED_BG, colors::ADDED_FG, "+"),
                        ChangeKind::Removed => (colors::REMOVED_BG, colors::REMOVED_FG, "-"),
                        ChangeKind::Equal => (colors::CONTEXT_BG, colors::CONTEXT_FG, " "),
                    };

                    // Draw line background
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(
                            bounds.origin.x,
                            y,
                            bounds.size.width,
                            self.line_height,
                        ))
                        .with_background(bg),
                    );

                    // Draw gutter
                    if self.show_line_numbers {
                        // Old line number
                        if let Some(ln) = change.old_line {
                            let ln_text = format!("{:>4}", ln);
                            let run = cx.text.layout(
                                &ln_text,
                                Point::new(bounds.origin.x + 4.0, y),
                                self.font_size,
                                colors::LINE_NUMBER,
                            );
                            cx.scene.draw_text(run);
                        }

                        // New line number
                        if let Some(ln) = change.new_line {
                            let ln_text = format!("{:>4}", ln);
                            let run = cx.text.layout(
                                &ln_text,
                                Point::new(bounds.origin.x + self.gutter_width + 4.0, y),
                                self.font_size,
                                colors::LINE_NUMBER,
                            );
                            cx.scene.draw_text(run);
                        }
                    }

                    // Draw marker
                    let marker_x = bounds.origin.x + gutter_width;
                    let run = cx
                        .text
                        .layout(marker, Point::new(marker_x, y), self.font_size, fg);
                    cx.scene.draw_text(run);

                    // Draw content
                    let content_x = marker_x + 20.0;
                    let run = cx.text.layout(
                        &change.content,
                        Point::new(content_x, y),
                        self.font_size,
                        fg,
                    );
                    cx.scene.draw_text(run);
                }
                y += self.line_height;
            }
        }
    }

    /// Paint side-by-side diff view.
    fn paint_side_by_side(&self, bounds: Bounds, scroll: f32, cx: &mut PaintContext) {
        let half_width = bounds.size.width / 2.0;
        let mut y = bounds.origin.y - scroll;

        for (hunk_idx, hunk) in self.diff.hunks.iter().enumerate() {
            // Paint hunk header spanning both sides
            if y + self.line_height > bounds.origin.y && y < bounds.origin.y + bounds.size.height {
                self.paint_hunk_header(bounds, y, hunk, cx);
            }
            y += self.line_height;

            if !self.expanded_hunks.get(hunk_idx).copied().unwrap_or(true) {
                continue;
            }

            // Separate changes into left (old) and right (new)
            let mut left_changes: Vec<_> = hunk
                .changes
                .iter()
                .filter(|c| c.kind == ChangeKind::Removed || c.kind == ChangeKind::Equal)
                .collect();
            let mut right_changes: Vec<_> = hunk
                .changes
                .iter()
                .filter(|c| c.kind == ChangeKind::Added || c.kind == ChangeKind::Equal)
                .collect();

            let max_len = left_changes.len().max(right_changes.len());

            for i in 0..max_len {
                if y + self.line_height > bounds.origin.y
                    && y < bounds.origin.y + bounds.size.height
                {
                    // Left side (old)
                    if let Some(change) = left_changes.get(i) {
                        let (bg, fg) = match change.kind {
                            ChangeKind::Removed => (colors::REMOVED_BG, colors::REMOVED_FG),
                            _ => (colors::CONTEXT_BG, colors::CONTEXT_FG),
                        };

                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(
                                bounds.origin.x,
                                y,
                                half_width - 2.0,
                                self.line_height,
                            ))
                            .with_background(bg),
                        );

                        if self.show_line_numbers {
                            if let Some(ln) = change.old_line {
                                let run = cx.text.layout(
                                    &format!("{:>4}", ln),
                                    Point::new(bounds.origin.x + 4.0, y),
                                    self.font_size,
                                    colors::LINE_NUMBER,
                                );
                                cx.scene.draw_text(run);
                            }
                        }

                        let run = cx.text.layout(
                            &change.content,
                            Point::new(bounds.origin.x + self.gutter_width, y),
                            self.font_size,
                            fg,
                        );
                        cx.scene.draw_text(run);
                    }

                    // Right side (new)
                    let right_x = bounds.origin.x + half_width + 2.0;
                    if let Some(change) = right_changes.get(i) {
                        let (bg, fg) = match change.kind {
                            ChangeKind::Added => (colors::ADDED_BG, colors::ADDED_FG),
                            _ => (colors::CONTEXT_BG, colors::CONTEXT_FG),
                        };

                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(right_x, y, half_width - 2.0, self.line_height))
                                .with_background(bg),
                        );

                        if self.show_line_numbers {
                            if let Some(ln) = change.new_line {
                                let run = cx.text.layout(
                                    &format!("{:>4}", ln),
                                    Point::new(right_x + 4.0, y),
                                    self.font_size,
                                    colors::LINE_NUMBER,
                                );
                                cx.scene.draw_text(run);
                            }
                        }

                        let run = cx.text.layout(
                            &change.content,
                            Point::new(right_x + self.gutter_width, y),
                            self.font_size,
                            fg,
                        );
                        cx.scene.draw_text(run);
                    }
                }
                y += self.line_height;
            }
        }
    }

    /// Paint inline diff view with word-level highlighting.
    fn paint_inline(&self, bounds: Bounds, scroll: f32, cx: &mut PaintContext) {
        // For now, same as unified - word-level highlighting would require
        // computing word diffs for each changed line pair
        self.paint_unified(bounds, scroll, cx);
    }

    /// Paint a hunk header.
    fn paint_hunk_header(&self, bounds: Bounds, y: f32, hunk: &Hunk, cx: &mut PaintContext) {
        // Draw header background
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                y,
                bounds.size.width,
                self.line_height,
            ))
            .with_background(colors::HUNK_HEADER_BG),
        );

        // Draw header text
        let header = hunk.header();
        let run = cx.text.layout(
            &header,
            Point::new(bounds.origin.x + 8.0, y),
            self.font_size,
            colors::HUNK_HEADER_FG,
        );
        cx.scene.draw_text(run);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::compute_diff;

    #[test]
    fn test_diff_view_creation() {
        let diff = compute_diff("old\n", "new\n");
        let view = DiffView::new(diff);

        assert!(view.diff().has_changes());
    }

    #[test]
    fn test_view_mode_switching() {
        let mut view = DiffView::default();

        view.set_mode(DiffViewMode::SideBySide);
        assert_eq!(view.mode(), DiffViewMode::SideBySide);

        view.set_mode(DiffViewMode::Unified);
        assert_eq!(view.mode(), DiffViewMode::Unified);
    }

    #[test]
    fn test_hunk_expansion() {
        let diff = compute_diff("a\nb\nc\n", "a\nX\nc\n");
        let mut view = DiffView::new(diff);

        assert!(!view.expanded_hunks.is_empty());

        view.collapse_all();
        assert!(view.expanded_hunks.iter().all(|&e| !e));

        view.expand_all();
        assert!(view.expanded_hunks.iter().all(|&e| e));
    }

    #[test]
    fn test_scrolling() {
        let mut view = DiffView::default();

        view.scroll(-100.0);
        assert!(view.scroll_offset.get_untracked() >= 0.0);
    }

    #[test]
    fn test_toggle_line_numbers() {
        let mut view = DiffView::default();

        assert!(view.show_line_numbers);
        view.toggle_line_numbers();
        assert!(!view.show_line_numbers);
    }
}
