//! Diff view component for trajectory replay
//!
//! Displays file changes from tool calls in side-by-side, unified, or inline format.

use std::cell::RefCell;
use std::rc::Rc;

use wgpui::{Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Quad, Text, theme};
use wgpui::components::{Button, ButtonVariant};

use crate::views::fit_text;

/// Diff display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    #[default]
    Unified,
    SideBySide,
    Inline,
}

impl DiffMode {
    pub fn label(&self) -> &'static str {
        match self {
            DiffMode::Unified => "Unified",
            DiffMode::SideBySide => "Side-by-Side",
            DiffMode::Inline => "Inline",
        }
    }
}

/// A line in a diff
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Context line (unchanged)
    Context { line_num: usize, content: String },
    /// Added line
    Added { line_num: usize, content: String },
    /// Removed line
    Removed { line_num: usize, content: String },
    /// Modified line (for inline mode)
    Modified {
        line_num: usize,
        old_content: String,
        new_content: String,
    },
    /// Hunk header (@@ -X,Y +A,B @@)
    Header { text: String },
}

impl DiffLine {
    pub fn color(&self) -> Hsla {
        match self {
            DiffLine::Context { .. } => diff_colors::context(),
            DiffLine::Added { .. } => diff_colors::added(),
            DiffLine::Removed { .. } => diff_colors::removed(),
            DiffLine::Modified { .. } => diff_colors::modified(),
            DiffLine::Header { .. } => diff_colors::header(),
        }
    }

    pub fn background(&self) -> Option<Hsla> {
        match self {
            DiffLine::Added { .. } => Some(diff_colors::added_bg()),
            DiffLine::Removed { .. } => Some(diff_colors::removed_bg()),
            DiffLine::Modified { .. } => Some(diff_colors::modified_bg()),
            _ => None,
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            DiffLine::Context { .. } => " ",
            DiffLine::Added { .. } => "+",
            DiffLine::Removed { .. } => "-",
            DiffLine::Modified { .. } => "~",
            DiffLine::Header { .. } => "@",
        }
    }
}

/// Diff colors
mod diff_colors {
    use wgpui::Hsla;

    fn rgb(r: u8, g: u8, b: u8) -> Hsla {
        Hsla::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    fn rgba(r: u8, g: u8, b: u8, a: f32) -> Hsla {
        Hsla::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0).with_alpha(a)
    }

    pub fn context() -> Hsla { rgb(156, 163, 175) }    // gray-400
    pub fn added() -> Hsla { rgb(34, 197, 94) }        // green-500
    pub fn removed() -> Hsla { rgb(239, 68, 68) }      // red-500
    pub fn modified() -> Hsla { rgb(245, 158, 11) }    // amber-500
    pub fn header() -> Hsla { rgb(99, 102, 241) }      // indigo-500

    pub fn added_bg() -> Hsla { rgba(34, 197, 94, 0.15) }
    pub fn removed_bg() -> Hsla { rgba(239, 68, 68, 0.15) }
    pub fn modified_bg() -> Hsla { rgba(245, 158, 11, 0.15) }
}

/// A file diff with metadata
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub file_path: String,
    pub old_path: Option<String>,
    pub lines: Vec<DiffLine>,
    pub additions: usize,
    pub deletions: usize,
}

impl FileDiff {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            old_path: None,
            lines: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }

    pub fn add_line(&mut self, line: DiffLine) {
        match &line {
            DiffLine::Added { .. } => self.additions += 1,
            DiffLine::Removed { .. } => self.deletions += 1,
            _ => {}
        }
        self.lines.push(line);
    }

    /// Parse a unified diff string into FileDiff
    pub fn parse_unified(diff_text: &str) -> Option<Self> {
        let mut lines = diff_text.lines().peekable();
        let mut file_path = String::new();
        let mut old_path = None;

        // Parse header
        while let Some(line) = lines.peek() {
            if line.starts_with("---") {
                old_path = Some(line.trim_start_matches("--- ").trim_start_matches("a/").to_string());
                lines.next();
            } else if line.starts_with("+++") {
                file_path = line.trim_start_matches("+++ ").trim_start_matches("b/").to_string();
                lines.next();
                break;
            } else if line.starts_with("diff ") {
                lines.next();
            } else {
                break;
            }
        }

        if file_path.is_empty() {
            return None;
        }

        let mut diff = FileDiff::new(file_path);
        diff.old_path = old_path;

        let mut old_line_num = 1usize;
        let mut new_line_num = 1usize;

        for line in lines {
            if line.starts_with("@@") {
                // Parse hunk header
                diff.add_line(DiffLine::Header { text: line.to_string() });

                // Extract line numbers from @@ -X,Y +A,B @@
                if let Some(nums) = parse_hunk_header(line) {
                    old_line_num = nums.0;
                    new_line_num = nums.2;
                }
            } else if let Some(content) = line.strip_prefix('+') {
                diff.add_line(DiffLine::Added {
                    line_num: new_line_num,
                    content: content.to_string(),
                });
                new_line_num += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                diff.add_line(DiffLine::Removed {
                    line_num: old_line_num,
                    content: content.to_string(),
                });
                old_line_num += 1;
            } else if let Some(content) = line.strip_prefix(' ') {
                diff.add_line(DiffLine::Context {
                    line_num: new_line_num,
                    content: content.to_string(),
                });
                old_line_num += 1;
                new_line_num += 1;
            } else if !line.starts_with('\\') {
                // Regular context line without prefix
                diff.add_line(DiffLine::Context {
                    line_num: new_line_num,
                    content: line.to_string(),
                });
                old_line_num += 1;
                new_line_num += 1;
            }
        }

        Some(diff)
    }
}

/// Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
fn parse_hunk_header(header: &str) -> Option<(usize, usize, usize, usize)> {
    let header = header.trim_start_matches("@@ ").trim_end_matches(" @@");
    let parts: Vec<&str> = header.split(' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let old_part = parts[0].trim_start_matches('-');
    let new_part = parts[1].trim_start_matches('+');

    let (old_start, old_count) = parse_range(old_part)?;
    let (new_start, new_count) = parse_range(new_part)?;

    Some((old_start, old_count, new_start, new_count))
}

fn parse_range(range: &str) -> Option<(usize, usize)> {
    if let Some((start, count)) = range.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        Some((range.parse().ok()?, 1))
    }
}

/// State for the diff view
pub struct DiffState {
    pub diff: Option<FileDiff>,
    pub mode: DiffMode,
    pub scroll_offset: f32,
}

impl DiffState {
    pub fn new() -> Self {
        Self {
            diff: None,
            mode: DiffMode::Unified,
            scroll_offset: 0.0,
        }
    }

    pub fn set_diff(&mut self, diff: FileDiff) {
        self.diff = Some(diff);
        self.scroll_offset = 0.0;
    }

    pub fn set_diff_from_text(&mut self, diff_text: &str) {
        if let Some(diff) = FileDiff::parse_unified(diff_text) {
            self.set_diff(diff);
        }
    }

    pub fn clear(&mut self) {
        self.diff = None;
        self.scroll_offset = 0.0;
    }

    pub fn set_mode(&mut self, mode: DiffMode) {
        self.mode = mode;
    }
}

impl Default for DiffState {
    fn default() -> Self {
        Self::new()
    }
}

/// Diff view component
pub struct DiffView {
    state: Rc<RefCell<DiffState>>,
    mode_unified: Button,
    mode_side_by_side: Button,
    mode_inline: Button,
}

impl DiffView {
    pub fn new(state: Rc<RefCell<DiffState>>) -> Self {
        let mode_unified = Button::new("Unified")
            .variant(ButtonVariant::Primary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_mode(DiffMode::Unified);
                }
            });

        let mode_side_by_side = Button::new("Side-by-Side")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_mode(DiffMode::SideBySide);
                }
            });

        let mode_inline = Button::new("Inline")
            .variant(ButtonVariant::Secondary)
            .on_click({
                let state = state.clone();
                move || {
                    state.borrow_mut().set_mode(DiffMode::Inline);
                }
            });

        Self {
            state,
            mode_unified,
            mode_side_by_side,
            mode_inline,
        }
    }
}

impl Component for DiffView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let button_height = 28.0;
        let button_spacing = 4.0;
        let mut y = bounds.origin.y + padding;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(theme::bg::SURFACE),
        );

        // File header
        if let Some(ref diff) = state.diff {
            let header_text = format!("{} (+{} -{}) ", diff.file_path, diff.additions, diff.deletions);
            let header_text = fit_text(cx, &header_text, theme::font_size::SM, available_width * 0.6);

            let mut header = Text::new(&header_text)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            header.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width * 0.6, line_height),
                cx,
            );
        } else {
            let mut no_diff = Text::new("No diff to display")
                .font_size(theme::font_size::SM)
                .color(theme::text::MUTED);
            no_diff.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        }

        // Mode buttons (right side)
        let button_x_start = bounds.origin.x + bounds.size.width - padding - (80.0 * 3.0 + button_spacing * 2.0);
        let current_mode = state.mode;
        drop(state);

        // Update button variants based on current mode
        let modes = [
            (DiffMode::Unified, &mut self.mode_unified, "Unified"),
            (DiffMode::SideBySide, &mut self.mode_side_by_side, "Side-by-Side"),
            (DiffMode::Inline, &mut self.mode_inline, "Inline"),
        ];

        let mut button_x = button_x_start;
        for (mode, button, label) in modes {
            let variant = if current_mode == mode {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            };

            *button = Button::new(label)
                .variant(variant)
                .on_click({
                    let state = self.state.clone();
                    move || {
                        state.borrow_mut().set_mode(mode);
                    }
                });

            let btn_bounds = Bounds::new(button_x, y, 80.0, button_height);
            button.paint(btn_bounds, cx);
            button_x += 80.0 + button_spacing;
        }

        y += line_height + theme::spacing::SM;

        // Diff content
        let state = self.state.borrow();
        if let Some(ref diff) = state.diff {
            let content_y = y;
            let content_height = bounds.origin.y + bounds.size.height - y - padding;
            let line_num_width = 40.0;
            let code_font_size = theme::font_size::XS;
            let code_line_height = code_font_size * 1.4;

            match state.mode {
                DiffMode::Unified | DiffMode::Inline => {
                    // Unified/Inline diff display
                    let mut line_y = content_y;

                    for diff_line in &diff.lines {
                        if line_y > bounds.origin.y + bounds.size.height - padding {
                            break; // Stop if we've gone past the visible area
                        }

                        // Background for changed lines
                        if let Some(bg) = diff_line.background() {
                            cx.scene.draw_quad(
                                Quad::new(Bounds::new(
                                    bounds.origin.x + padding,
                                    line_y,
                                    available_width,
                                    code_line_height,
                                ))
                                .with_background(bg),
                            );
                        }

                        // Line prefix
                        let mut prefix = Text::new(diff_line.prefix())
                            .font_size(code_font_size)
                            .color(diff_line.color());
                        prefix.paint(
                            Bounds::new(bounds.origin.x + padding, line_y, 16.0, code_line_height),
                            cx,
                        );

                        // Line number
                        let line_num = match diff_line {
                            DiffLine::Context { line_num, .. }
                            | DiffLine::Added { line_num, .. }
                            | DiffLine::Removed { line_num, .. }
                            | DiffLine::Modified { line_num, .. } => Some(*line_num),
                            DiffLine::Header { .. } => None,
                        };

                        if let Some(num) = line_num {
                            let mut num_text = Text::new(&num.to_string())
                                .font_size(code_font_size)
                                .color(theme::text::MUTED);
                            num_text.paint(
                                Bounds::new(
                                    bounds.origin.x + padding + 16.0,
                                    line_y,
                                    line_num_width,
                                    code_line_height,
                                ),
                                cx,
                            );
                        }

                        // Line content
                        let content = match diff_line {
                            DiffLine::Context { content, .. }
                            | DiffLine::Added { content, .. }
                            | DiffLine::Removed { content, .. } => content.as_str(),
                            DiffLine::Modified { new_content, .. } => new_content.as_str(),
                            DiffLine::Header { text } => text.as_str(),
                        };

                        let content_x = bounds.origin.x + padding + 16.0 + line_num_width + 8.0;
                        let content_width = available_width - 16.0 - line_num_width - 8.0;
                        let content = fit_text(cx, content, code_font_size, content_width);

                        let mut content_text = Text::new(&content)
                            .font_size(code_font_size)
                            .color(diff_line.color());
                        content_text.paint(
                            Bounds::new(content_x, line_y, content_width, code_line_height),
                            cx,
                        );

                        line_y += code_line_height;
                    }
                }

                DiffMode::SideBySide => {
                    // Side-by-side diff display
                    let half_width = (available_width - theme::spacing::SM) / 2.0;
                    let left_x = bounds.origin.x + padding;
                    let right_x = left_x + half_width + theme::spacing::SM;

                    // Headers
                    let mut old_header = Text::new("Old")
                        .font_size(code_font_size)
                        .color(diff_colors::removed());
                    old_header.paint(
                        Bounds::new(left_x, content_y, half_width, code_line_height),
                        cx,
                    );

                    let mut new_header = Text::new("New")
                        .font_size(code_font_size)
                        .color(diff_colors::added());
                    new_header.paint(
                        Bounds::new(right_x, content_y, half_width, code_line_height),
                        cx,
                    );

                    let mut line_y = content_y + code_line_height;

                    // Group lines by removed/added pairs
                    let mut i = 0;
                    while i < diff.lines.len() {
                        if line_y > bounds.origin.y + bounds.size.height - padding {
                            break;
                        }

                        let line = &diff.lines[i];

                        match line {
                            DiffLine::Context { content, line_num } => {
                                // Show on both sides
                                let content = fit_text(cx, content, code_font_size, half_width - line_num_width - 8.0);

                                let mut num = Text::new(&line_num.to_string())
                                    .font_size(code_font_size)
                                    .color(theme::text::MUTED);
                                num.paint(
                                    Bounds::new(left_x, line_y, line_num_width, code_line_height),
                                    cx,
                                );
                                num.paint(
                                    Bounds::new(right_x, line_y, line_num_width, code_line_height),
                                    cx,
                                );

                                let mut text = Text::new(&content)
                                    .font_size(code_font_size)
                                    .color(diff_colors::context());
                                text.paint(
                                    Bounds::new(left_x + line_num_width + 8.0, line_y, half_width - line_num_width - 8.0, code_line_height),
                                    cx,
                                );
                                text.paint(
                                    Bounds::new(right_x + line_num_width + 8.0, line_y, half_width - line_num_width - 8.0, code_line_height),
                                    cx,
                                );

                                line_y += code_line_height;
                            }
                            DiffLine::Removed { content, line_num } => {
                                // Show on left side with background
                                cx.scene.draw_quad(
                                    Quad::new(Bounds::new(left_x, line_y, half_width, code_line_height))
                                        .with_background(diff_colors::removed_bg()),
                                );

                                let mut num = Text::new(&line_num.to_string())
                                    .font_size(code_font_size)
                                    .color(theme::text::MUTED);
                                num.paint(
                                    Bounds::new(left_x, line_y, line_num_width, code_line_height),
                                    cx,
                                );

                                let content = fit_text(cx, content, code_font_size, half_width - line_num_width - 8.0);
                                let mut text = Text::new(&content)
                                    .font_size(code_font_size)
                                    .color(diff_colors::removed());
                                text.paint(
                                    Bounds::new(left_x + line_num_width + 8.0, line_y, half_width - line_num_width - 8.0, code_line_height),
                                    cx,
                                );

                                line_y += code_line_height;
                            }
                            DiffLine::Added { content, line_num } => {
                                // Show on right side with background
                                cx.scene.draw_quad(
                                    Quad::new(Bounds::new(right_x, line_y, half_width, code_line_height))
                                        .with_background(diff_colors::added_bg()),
                                );

                                let mut num = Text::new(&line_num.to_string())
                                    .font_size(code_font_size)
                                    .color(theme::text::MUTED);
                                num.paint(
                                    Bounds::new(right_x, line_y, line_num_width, code_line_height),
                                    cx,
                                );

                                let content = fit_text(cx, content, code_font_size, half_width - line_num_width - 8.0);
                                let mut text = Text::new(&content)
                                    .font_size(code_font_size)
                                    .color(diff_colors::added());
                                text.paint(
                                    Bounds::new(right_x + line_num_width + 8.0, line_y, half_width - line_num_width - 8.0, code_line_height),
                                    cx,
                                );

                                line_y += code_line_height;
                            }
                            DiffLine::Header { text } => {
                                let mut header = Text::new(text)
                                    .font_size(code_font_size)
                                    .color(diff_colors::header());
                                header.paint(
                                    Bounds::new(left_x, line_y, available_width, code_line_height),
                                    cx,
                                );
                                line_y += code_line_height;
                            }
                            _ => {
                                line_y += code_line_height;
                            }
                        }

                        i += 1;
                    }
                }
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let button_height = 28.0;
        let button_spacing = 4.0;
        let y = bounds.origin.y + padding;

        // Mode button events
        let button_x_start = bounds.origin.x + bounds.size.width - padding - (80.0 * 3.0 + button_spacing * 2.0);
        let mut button_x = button_x_start;

        for button in [&mut self.mode_unified, &mut self.mode_side_by_side, &mut self.mode_inline] {
            let btn_bounds = Bounds::new(button_x, y, 80.0, button_height);
            if button.event(event, btn_bounds, cx).is_handled() {
                return EventResult::Handled;
            }
            button_x += 80.0 + button_spacing;
        }

        // Scroll handling
        match event {
            InputEvent::Scroll { dy, .. } => {
                let mut state = self.state.borrow_mut();
                state.scroll_offset = (state.scroll_offset - dy).max(0.0);
                return EventResult::Handled;
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Flexible size
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_mode_label() {
        assert_eq!(DiffMode::Unified.label(), "Unified");
        assert_eq!(DiffMode::SideBySide.label(), "Side-by-Side");
        assert_eq!(DiffMode::Inline.label(), "Inline");
    }

    #[test]
    fn test_diff_line_prefix() {
        assert_eq!(DiffLine::Context { line_num: 1, content: String::new() }.prefix(), " ");
        assert_eq!(DiffLine::Added { line_num: 1, content: String::new() }.prefix(), "+");
        assert_eq!(DiffLine::Removed { line_num: 1, content: String::new() }.prefix(), "-");
    }

    #[test]
    fn test_file_diff_new() {
        let diff = FileDiff::new("src/main.rs".to_string());
        assert_eq!(diff.file_path, "src/main.rs");
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 0);
        assert!(diff.lines.is_empty());
    }

    #[test]
    fn test_file_diff_add_line() {
        let mut diff = FileDiff::new("src/main.rs".to_string());

        diff.add_line(DiffLine::Added { line_num: 1, content: "new line".to_string() });
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);

        diff.add_line(DiffLine::Removed { line_num: 2, content: "old line".to_string() });
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn test_parse_hunk_header() {
        let result = parse_hunk_header("@@ -1,5 +1,7 @@");
        assert_eq!(result, Some((1, 5, 1, 7)));

        let result = parse_hunk_header("@@ -10 +10,3 @@");
        assert_eq!(result, Some((10, 1, 10, 3)));
    }

    #[test]
    fn test_file_diff_parse_unified() {
        let diff_text = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello");
     run();
 }
"#;

        let diff = FileDiff::parse_unified(diff_text).unwrap();
        assert_eq!(diff.file_path, "src/main.rs");
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn test_diff_state_new() {
        let state = DiffState::new();
        assert!(state.diff.is_none());
        assert_eq!(state.mode, DiffMode::Unified);
        assert_eq!(state.scroll_offset, 0.0);
    }

    #[test]
    fn test_diff_state_set_mode() {
        let mut state = DiffState::new();
        state.set_mode(DiffMode::SideBySide);
        assert_eq!(state.mode, DiffMode::SideBySide);
    }
}
