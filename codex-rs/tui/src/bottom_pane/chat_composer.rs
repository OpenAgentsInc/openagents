use codex_core::protocol::TokenUsageInfo;
use codex_protocol::num_format::format_si_suffix;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::WidgetRef;

use super::chat_composer_history::ChatComposerHistory;
use super::command_popup::CommandItem;
use super::command_popup::CommandPopup;
use super::file_search_popup::FileSearchPopup;
use super::paste_burst::CharDecision;
use super::paste_burst::PasteBurst;
use crate::bottom_pane::paste_burst::FlushResult;
use crate::slash_command::SlashCommand;
use codex_protocol::custom_prompts::CustomPrompt;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use crate::clipboard_paste::normalize_pasted_path;
use crate::clipboard_paste::pasted_image_format;
use crate::key_hint;
use crate::ui_consts::LIVE_PREFIX_COLS;
use codex_file_search::FileMatch;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

/// Result returned when the user interacts with the text area.
#[derive(Debug, PartialEq)]
pub enum InputResult {
    Submitted(String),
    Command(SlashCommand),
    None,
}

#[derive(Clone, Debug, PartialEq)]
struct AttachedImage {
    placeholder: String,
    path: PathBuf,
}

pub(crate) struct ChatComposer {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    ctrl_c_quit_hint: bool,
    esc_backtrack_hint: bool,
    use_shift_enter_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
    pending_pastes: Vec<(String, String)>,
    token_usage_info: Option<TokenUsageInfo>,
    has_focus: bool,
    attached_images: Vec<AttachedImage>,
    placeholder_text: String,
    is_task_running: bool,
    // Non-bracketed paste burst tracker.
    paste_burst: PasteBurst,
    // When true, disables paste-burst logic and inserts characters immediately.
    disable_paste_burst: bool,
    custom_prompts: Vec<CustomPrompt>,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
}

const FOOTER_HINT_HEIGHT: u16 = 1;
const FOOTER_SPACING_HEIGHT: u16 = 1;
const FOOTER_HEIGHT_WITH_HINT: u16 = FOOTER_HINT_HEIGHT + FOOTER_SPACING_HEIGHT;

impl ChatComposer {
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
        placeholder_text: String,
        disable_paste_burst: bool,
    ) -> Self {
        let use_shift_enter_hint = enhanced_keys_supported;

        let mut this = Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            ctrl_c_quit_hint: false,
            esc_backtrack_hint: false,
            use_shift_enter_hint,
            dismissed_file_popup_token: None,
            current_file_query: None,
            pending_pastes: Vec::new(),
            token_usage_info: None,
            has_focus: has_input_focus,
            attached_images: Vec::new(),
            placeholder_text,
            is_task_running: false,
            paste_burst: PasteBurst::default(),
            disable_paste_burst: false,
            custom_prompts: Vec::new(),
        };
        // Apply configuration via the setter to keep side-effects centralized.
        this.set_disable_paste_burst(disable_paste_burst);
        this
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        // Leave 1 column for the left border and 1 column for left padding
        self.textarea
            .desired_height(width.saturating_sub(LIVE_PREFIX_COLS))
            + match &self.active_popup {
                ActivePopup::None => FOOTER_HEIGHT_WITH_HINT,
                ActivePopup::Command(c) => c.calculate_required_height(width),
                ActivePopup::File(c) => c.calculate_required_height(),
            }
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let popup_constraint = match &self.active_popup {
            ActivePopup::Command(popup) => {
                Constraint::Max(popup.calculate_required_height(area.width))
            }
            ActivePopup::File(popup) => Constraint::Max(popup.calculate_required_height()),
            ActivePopup::None => Constraint::Max(FOOTER_HEIGHT_WITH_HINT),
        };
        let [textarea_rect, _] =
            Layout::vertical([Constraint::Min(1), popup_constraint]).areas(area);
        let mut textarea_rect = textarea_rect;
        // Leave 1 for border and 1 for padding
        textarea_rect.width = textarea_rect.width.saturating_sub(LIVE_PREFIX_COLS);
        textarea_rect.x = textarea_rect.x.saturating_add(LIVE_PREFIX_COLS);
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    /// Returns true if the composer currently contains no user input.
    pub(crate) fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    /// Update the cached *context-left* percentage and refresh the placeholder
    /// text. The UI relies on the placeholder to convey the remaining
    /// context when the composer is empty.
    pub(crate) fn set_token_usage(&mut self, token_info: Option<TokenUsageInfo>) {
        self.token_usage_info = token_info;
    }

    /// Record the history metadata advertised by `SessionConfiguredEvent` so
    /// that the composer can navigate cross-session history.
    pub(crate) fn set_history_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history.set_metadata(log_id, entry_count);
    }

    /// Integrate an asynchronous response to an on-demand history lookup. If
    /// the entry is present and the offset matches the current cursor we
    /// immediately populate the textarea.
    pub(crate) fn on_history_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
    ) -> bool {
        let Some(text) = self.history.on_entry_response(log_id, offset, entry) else {
            return false;
        };
        self.textarea.set_text(&text);
        self.textarea.set_cursor(0);
        true
    }

    pub fn handle_paste(&mut self, pasted: String) -> bool {
        let char_count = pasted.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = format!("[Pasted Content {char_count} chars]");
            self.textarea.insert_element(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
        } else if char_count > 1 && self.handle_paste_image_path(pasted.clone()) {
            self.textarea.insert_str(" ");
        } else {
            self.textarea.insert_str(&pasted);
        }
        // Explicit paste events should not trigger Enter suppression.
        self.paste_burst.clear_after_explicit_paste();
        // Keep popup sync consistent with key handling: prefer slash popup; only
        // sync file popup when slash popup is NOT active.
        self.sync_command_popup();
        if matches!(self.active_popup, ActivePopup::Command(_)) {
            self.dismissed_file_popup_token = None;
        } else {
            self.sync_file_search_popup();
        }
        true
    }

    pub fn handle_paste_image_path(&mut self, pasted: String) -> bool {
        let Some(path_buf) = normalize_pasted_path(&pasted) else {
            return false;
        };

        match image::image_dimensions(&path_buf) {
            Ok((w, h)) => {
                tracing::info!("OK: {pasted}");
                let format_label = pasted_image_format(&path_buf).label();
                self.attach_image(path_buf, w, h, format_label);
                true
            }
            Err(err) => {
                tracing::info!("ERR: {err}");
                false
            }
        }
    }

    pub(crate) fn set_disable_paste_burst(&mut self, disabled: bool) {
        let was_disabled = self.disable_paste_burst;
        self.disable_paste_burst = disabled;
        if disabled && !was_disabled {
            self.paste_burst.clear_window_after_non_char();
        }
    }

    /// Replace the entire composer content with `text` and reset cursor.
    pub(crate) fn set_text_content(&mut self, text: String) {
        // Clear any existing content, placeholders, and attachments first.
        self.textarea.set_text("");
        self.pending_pastes.clear();
        self.attached_images.clear();
        self.textarea.set_text(&text);
        self.textarea.set_cursor(0);
        self.sync_command_popup();
        self.sync_file_search_popup();
    }

    /// Get the current composer text.
    #[cfg(test)]
    pub(crate) fn current_text(&self) -> String {
        self.textarea.text().to_string()
    }

    /// Attempt to start a burst by retro-capturing recent chars before the cursor.
    pub fn attach_image(&mut self, path: PathBuf, width: u32, height: u32, format_label: &str) {
        let placeholder = format!("[image {width}x{height} {format_label}]");
        // Insert as an element to match large paste placeholder behavior:
        // styled distinctly and treated atomically for cursor/mutations.
        self.textarea.insert_element(&placeholder);
        self.attached_images
            .push(AttachedImage { placeholder, path });
    }

    pub fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        let images = std::mem::take(&mut self.attached_images);
        images.into_iter().map(|img| img.path).collect()
    }

    pub(crate) fn flush_paste_burst_if_due(&mut self) -> bool {
        self.handle_paste_burst_flush(Instant::now())
    }

    pub(crate) fn is_in_paste_burst(&self) -> bool {
        self.paste_burst.is_active()
    }

    pub(crate) fn recommended_paste_flush_delay() -> Duration {
        PasteBurst::recommended_flush_delay()
    }

    /// Integrate results from an asynchronous file search.
    pub(crate) fn on_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        // Only apply if user is still editing a token starting with `query`.
        let current_opt = Self::current_at_token(&self.textarea);
        let Some(current_token) = current_opt else {
            return;
        };

        if !current_token.starts_with(&query) {
            return;
        }

        if let ActivePopup::File(popup) = &mut self.active_popup {
            popup.set_matches(&query, matches);
        }
    }

    pub fn set_ctrl_c_quit_hint(&mut self, show: bool, has_focus: bool) {
        self.ctrl_c_quit_hint = show;
        self.set_has_focus(has_focus);
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.textarea.insert_str(text);
        self.sync_command_popup();
        self.sync_file_search_popup();
    }

    /// Handle a key event coming from the main UI.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let result = match &mut self.active_popup {
            ActivePopup::Command(_) => self.handle_key_event_with_slash_popup(key_event),
            ActivePopup::File(_) => self.handle_key_event_with_file_popup(key_event),
            ActivePopup::None => self.handle_key_event_without_popup(key_event),
        };

        // Update (or hide/show) popup after processing the key.
        self.sync_command_popup();
        if matches!(self.active_popup, ActivePopup::Command(_)) {
            self.dismissed_file_popup_token = None;
        } else {
            self.sync_file_search_popup();
        }

        result
    }

    /// Return true if either the slash-command popup or the file-search popup is active.
    pub(crate) fn popup_active(&self) -> bool {
        !matches!(self.active_popup, ActivePopup::None)
    }

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::Command(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Dismiss the slash popup; keep the current input untouched.
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                // Ensure popup filtering/selection reflects the latest composer text
                // before applying completion.
                let first_line = self.textarea.text().lines().next().unwrap_or("");
                popup.on_composer_text_change(first_line.to_string());
                if let Some(sel) = popup.selected_item() {
                    match sel {
                        CommandItem::Builtin(cmd) => {
                            let starts_with_cmd = first_line
                                .trim_start()
                                .starts_with(&format!("/{}", cmd.command()));
                            if !starts_with_cmd {
                                self.textarea.set_text(&format!("/{} ", cmd.command()));
                            }
                        }
                        CommandItem::UserPrompt(idx) => {
                            if let Some(name) = popup.prompt_name(idx) {
                                let starts_with_cmd =
                                    first_line.trim_start().starts_with(&format!("/{name}"));
                                if !starts_with_cmd {
                                    self.textarea.set_text(&format!("/{name} "));
                                }
                            }
                        }
                    }
                    // After completing the command, move cursor to the end.
                    if !self.textarea.text().is_empty() {
                        let end = self.textarea.text().len();
                        self.textarea.set_cursor(end);
                    }
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(sel) = popup.selected_item() {
                    // Clear textarea so no residual text remains.
                    self.textarea.set_text("");
                    // Capture any needed data from popup before clearing it.
                    let prompt_content = match sel {
                        CommandItem::UserPrompt(idx) => {
                            popup.prompt_content(idx).map(str::to_string)
                        }
                        _ => None,
                    };
                    // Hide popup since an action has been dispatched.
                    self.active_popup = ActivePopup::None;

                    match sel {
                        CommandItem::Builtin(cmd) => {
                            return (InputResult::Command(cmd), true);
                        }
                        CommandItem::UserPrompt(_) => {
                            if let Some(contents) = prompt_content {
                                return (InputResult::Submitted(contents), true);
                            }
                            return (InputResult::None, true);
                        }
                    }
                }
                // Fallback to default newline handling if no command selected.
                self.handle_key_event_without_popup(key_event)
            }
            input => self.handle_input_basic(input),
        }
    }
    #[inline]
    fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
        let mut p = pos.min(text.len());
        if p < text.len() && !text.is_char_boundary(p) {
            p = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= p)
                .last()
                .unwrap_or(0);
        }
        p
    }

    #[inline]
    fn handle_non_ascii_char(&mut self, input: KeyEvent) -> (InputResult, bool) {
        if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
            self.handle_paste(pasted);
        }
        self.textarea.input(input);
        let text_after = self.textarea.text();
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));
        (InputResult::None, true)
    }

    /// Handle key events when file search popup is visible.
    fn handle_key_event_with_file_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::File(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Hide popup without modifying text, remember token to avoid immediate reopen.
                if let Some(tok) = Self::current_at_token(&self.textarea) {
                    self.dismissed_file_popup_token = Some(tok);
                }
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let Some(sel) = popup.selected_match() else {
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                };

                let sel_path = sel.to_string();
                // If selected path looks like an image (png/jpeg), attach as image instead of inserting text.
                let is_image = Self::is_image_path(&sel_path);
                if is_image {
                    // Determine dimensions; if that fails fall back to normal path insertion.
                    let path_buf = PathBuf::from(&sel_path);
                    if let Ok((w, h)) = image::image_dimensions(&path_buf) {
                        // Remove the current @token (mirror logic from insert_selected_path without inserting text)
                        // using the flat text and byte-offset cursor API.
                        let cursor_offset = self.textarea.cursor();
                        let text = self.textarea.text();
                        // Clamp to a valid char boundary to avoid panics when slicing.
                        let safe_cursor = Self::clamp_to_char_boundary(text, cursor_offset);
                        let before_cursor = &text[..safe_cursor];
                        let after_cursor = &text[safe_cursor..];

                        // Determine token boundaries in the full text.
                        let start_idx = before_cursor
                            .char_indices()
                            .rfind(|(_, c)| c.is_whitespace())
                            .map(|(idx, c)| idx + c.len_utf8())
                            .unwrap_or(0);
                        let end_rel_idx = after_cursor
                            .char_indices()
                            .find(|(_, c)| c.is_whitespace())
                            .map(|(idx, _)| idx)
                            .unwrap_or(after_cursor.len());
                        let end_idx = safe_cursor + end_rel_idx;

                        self.textarea.replace_range(start_idx..end_idx, "");
                        self.textarea.set_cursor(start_idx);

                        let format_label = match Path::new(&sel_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(str::to_ascii_lowercase)
                        {
                            Some(ext) if ext == "png" => "PNG",
                            Some(ext) if ext == "jpg" || ext == "jpeg" => "JPEG",
                            _ => "IMG",
                        };
                        self.attach_image(path_buf, w, h, format_label);
                        // Add a trailing space to keep typing fluid.
                        self.textarea.insert_str(" ");
                    } else {
                        // Fallback to plain path insertion if metadata read fails.
                        self.insert_selected_path(&sel_path);
                    }
                } else {
                    // Non-image: inserting file path.
                    self.insert_selected_path(&sel_path);
                }
                // No selection: treat Enter as closing the popup/session.
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        }
    }

    fn is_image_path(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
    }

    /// Extract the `@token` that the cursor is currently positioned on, if any.
    ///
    /// The returned string **does not** include the leading `@`.
    ///
    /// Behavior:
    /// - The cursor may be anywhere *inside* the token (including on the
    ///   leading `@`). It does **not** need to be at the end of the line.
    /// - A token is delimited by ASCII whitespace (space, tab, newline).
    /// - If the token under the cursor starts with `@`, that token is
    ///   returned without the leading `@`. This includes the case where the
    ///   token is just "@" (empty query), which is used to trigger a UI hint
    fn current_at_token(textarea: &TextArea) -> Option<String> {
        let cursor_offset = textarea.cursor();
        let text = textarea.text();

        // Adjust the provided byte offset to the nearest valid char boundary at or before it.
        let mut safe_cursor = cursor_offset.min(text.len());
        // If we're not on a char boundary, move back to the start of the current char.
        if safe_cursor < text.len() && !text.is_char_boundary(safe_cursor) {
            // Find the last valid boundary <= cursor_offset.
            safe_cursor = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= cursor_offset)
                .last()
                .unwrap_or(0);
        }

        // Split the line around the (now safe) cursor position.
        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

        // Detect whether we're on whitespace at the cursor boundary.
        let at_whitespace = if safe_cursor < text.len() {
            text[safe_cursor..]
                .chars()
                .next()
                .map(char::is_whitespace)
                .unwrap_or(false)
        } else {
            false
        };

        // Left candidate: token containing the cursor position.
        let start_left = before_cursor
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        let end_left_rel = after_cursor
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(after_cursor.len());
        let end_left = safe_cursor + end_left_rel;
        let token_left = if start_left < end_left {
            Some(&text[start_left..end_left])
        } else {
            None
        };

        // Right candidate: token immediately after any whitespace from the cursor.
        let ws_len_right: usize = after_cursor
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(char::len_utf8)
            .sum();
        let start_right = safe_cursor + ws_len_right;
        let end_right_rel = text[start_right..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(text.len() - start_right);
        let end_right = start_right + end_right_rel;
        let token_right = if start_right < end_right {
            Some(&text[start_right..end_right])
        } else {
            None
        };

        let left_at = token_left
            .filter(|t| t.starts_with('@'))
            .map(|t| t[1..].to_string());
        let right_at = token_right
            .filter(|t| t.starts_with('@'))
            .map(|t| t[1..].to_string());

        if at_whitespace {
            if right_at.is_some() {
                return right_at;
            }
            if token_left.is_some_and(|t| t == "@") {
                return None;
            }
            return left_at;
        }
        if after_cursor.starts_with('@') {
            return right_at.or(left_at);
        }
        left_at.or(right_at)
    }

    /// Replace the active `@token` (the one under the cursor) with `path`.
    ///
    /// The algorithm mirrors `current_at_token` so replacement works no matter
    /// where the cursor is within the token and regardless of how many
    /// `@tokens` exist in the line.
    fn insert_selected_path(&mut self, path: &str) {
        let cursor_offset = self.textarea.cursor();
        let text = self.textarea.text();
        // Clamp to a valid char boundary to avoid panics when slicing.
        let safe_cursor = Self::clamp_to_char_boundary(text, cursor_offset);

        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

        // Determine token boundaries.
        let start_idx = before_cursor
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);

        let end_rel_idx = after_cursor
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(after_cursor.len());
        let end_idx = safe_cursor + end_rel_idx;

        // Replace the slice `[start_idx, end_idx)` with the chosen path and a trailing space.
        let mut new_text =
            String::with_capacity(text.len() - (end_idx - start_idx) + path.len() + 1);
        new_text.push_str(&text[..start_idx]);
        new_text.push_str(path);
        new_text.push(' ');
        new_text.push_str(&text[end_idx..]);

        self.textarea.set_text(&new_text);
        let new_cursor = start_idx.saturating_add(path.len()).saturating_add(1);
        self.textarea.set_cursor(new_cursor);
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } if self.is_empty() => {
                self.app_event_tx.send(AppEvent::ExitRequest);
                (InputResult::None, true)
            }
            // -------------------------------------------------------------
            // History navigation (Up / Down) – only when the composer is not
            // empty or when the cursor is at the correct position, to avoid
            // interfering with normal cursor movement.
            // -------------------------------------------------------------
            KeyEvent {
                code: KeyCode::Up | KeyCode::Down,
                ..
            } => {
                if self
                    .history
                    .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
                {
                    let replace_text = match key_event.code {
                        KeyCode::Up => self.history.navigate_up(&self.app_event_tx),
                        KeyCode::Down => self.history.navigate_down(&self.app_event_tx),
                        _ => unreachable!(),
                    };
                    if let Some(text) = replace_text {
                        self.textarea.set_text(&text);
                        self.textarea.set_cursor(0);
                        return (InputResult::None, true);
                    }
                }
                self.handle_input_basic(key_event)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in a paste-like burst capture, treat Enter as part of the burst
                // and accumulate it rather than submitting or inserting immediately.
                // Do not treat Enter as paste inside a slash-command context.
                let in_slash_context = matches!(self.active_popup, ActivePopup::Command(_))
                    || self
                        .textarea
                        .text()
                        .lines()
                        .next()
                        .unwrap_or("")
                        .starts_with('/');
                if self.paste_burst.is_active() && !in_slash_context {
                    let now = Instant::now();
                    if self.paste_burst.append_newline_if_active(now) {
                        return (InputResult::None, true);
                    }
                }
                // If we have pending placeholder pastes, submit immediately to expand them.
                if !self.pending_pastes.is_empty() {
                    let mut text = self.textarea.text().to_string();
                    self.textarea.set_text("");
                    for (placeholder, actual) in &self.pending_pastes {
                        if text.contains(placeholder) {
                            text = text.replace(placeholder, actual);
                        }
                    }
                    self.pending_pastes.clear();
                    if text.is_empty() {
                        return (InputResult::None, true);
                    }
                    self.history.record_local_submission(&text);
                    return (InputResult::Submitted(text), true);
                }

                // During a paste-like burst, treat Enter as a newline instead of submit.
                let now = Instant::now();
                if self
                    .paste_burst
                    .newline_should_insert_instead_of_submit(now)
                {
                    self.textarea.insert_str("\n");
                    self.paste_burst.extend_window(now);
                    return (InputResult::None, true);
                }
                let mut text = self.textarea.text().to_string();
                self.textarea.set_text("");

                // Replace all pending pastes in the text
                for (placeholder, actual) in &self.pending_pastes {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, actual);
                    }
                }
                self.pending_pastes.clear();

                // If there is neither text nor attachments, suppress submission entirely.
                let has_attachments = !self.attached_images.is_empty();
                text = text.trim().to_string();
                if text.is_empty() && !has_attachments {
                    return (InputResult::None, true);
                }
                if !text.is_empty() {
                    self.history.record_local_submission(&text);
                }
                // Do not clear attached_images here; ChatWidget drains them via take_recent_submission_images().
                (InputResult::Submitted(text), true)
            }
            input => self.handle_input_basic(input),
        }
    }

    fn handle_paste_burst_flush(&mut self, now: Instant) -> bool {
        match self.paste_burst.flush_if_due(now) {
            FlushResult::Paste(pasted) => {
                self.handle_paste(pasted);
                true
            }
            FlushResult::Typed(ch) => {
                // Mirror insert_str() behavior so popups stay in sync when a
                // pending fast char flushes as normal typed input.
                self.textarea.insert_str(ch.to_string().as_str());
                // Keep popup sync consistent with key handling: prefer slash popup; only
                // sync file popup when slash popup is NOT active.
                self.sync_command_popup();
                if matches!(self.active_popup, ActivePopup::Command(_)) {
                    self.dismissed_file_popup_token = None;
                } else {
                    self.sync_file_search_popup();
                }
                true
            }
            FlushResult::None => false,
        }
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: KeyEvent) -> (InputResult, bool) {
        // If we have a buffered non-bracketed paste burst and enough time has
        // elapsed since the last char, flush it before handling a new input.
        let now = Instant::now();
        self.handle_paste_burst_flush(now);

        // If we're capturing a burst and receive Enter, accumulate it instead of inserting.
        if matches!(input.code, KeyCode::Enter)
            && self.paste_burst.is_active()
            && self.paste_burst.append_newline_if_active(now)
        {
            return (InputResult::None, true);
        }

        // Intercept plain Char inputs to optionally accumulate into a burst buffer.
        if let KeyEvent {
            code: KeyCode::Char(ch),
            modifiers,
            ..
        } = input
        {
            let has_ctrl_or_alt =
                modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::ALT);
            if !has_ctrl_or_alt {
                // Non-ASCII characters (e.g., from IMEs) can arrive in quick bursts and be
                // misclassified by paste heuristics. Flush any active burst buffer and insert
                // non-ASCII characters directly.
                if !ch.is_ascii() {
                    return self.handle_non_ascii_char(input);
                }

                match self.paste_burst.on_plain_char(ch, now) {
                    CharDecision::BufferAppend => {
                        self.paste_burst.append_char_to_buffer(ch, now);
                        return (InputResult::None, true);
                    }
                    CharDecision::BeginBuffer { retro_chars } => {
                        let cur = self.textarea.cursor();
                        let txt = self.textarea.text();
                        let safe_cur = Self::clamp_to_char_boundary(txt, cur);
                        let before = &txt[..safe_cur];
                        if let Some(grab) =
                            self.paste_burst
                                .decide_begin_buffer(now, before, retro_chars as usize)
                        {
                            if !grab.grabbed.is_empty() {
                                self.textarea.replace_range(grab.start_byte..safe_cur, "");
                            }
                            self.paste_burst.begin_with_retro_grabbed(grab.grabbed, now);
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return (InputResult::None, true);
                        }
                        // If decide_begin_buffer opted not to start buffering,
                        // fall through to normal insertion below.
                    }
                    CharDecision::BeginBufferFromPending => {
                        // First char was held; now append the current one.
                        self.paste_burst.append_char_to_buffer(ch, now);
                        return (InputResult::None, true);
                    }
                    CharDecision::RetainFirstChar => {
                        // Keep the first fast char pending momentarily.
                        return (InputResult::None, true);
                    }
                }
            }
            if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
                self.handle_paste(pasted);
            }
        }

        // For non-char inputs (or after flushing), handle normally.
        // Special handling for backspace on placeholders
        if let KeyEvent {
            code: KeyCode::Backspace,
            ..
        } = input
            && self.try_remove_any_placeholder_at_cursor()
        {
            return (InputResult::None, true);
        }

        // Normal input handling
        self.textarea.input(input);
        let text_after = self.textarea.text();

        // Update paste-burst heuristic for plain Char (no Ctrl/Alt) events.
        let crossterm::event::KeyEvent {
            code, modifiers, ..
        } = input;
        match code {
            KeyCode::Char(_) => {
                let has_ctrl_or_alt = modifiers.contains(KeyModifiers::CONTROL)
                    || modifiers.contains(KeyModifiers::ALT);
                if has_ctrl_or_alt {
                    self.paste_burst.clear_window_after_non_char();
                }
            }
            KeyCode::Enter => {
                // Keep burst window alive (supports blank lines in paste).
            }
            _ => {
                // Other keys: clear burst window (buffer should have been flushed above if needed).
                self.paste_burst.clear_window_after_non_char();
            }
        }

        // Check if any placeholders were removed and remove their corresponding pending pastes
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));

        // Keep attached images in proportion to how many matching placeholders exist in the text.
        // This handles duplicate placeholders that share the same visible label.
        if !self.attached_images.is_empty() {
            let mut needed: HashMap<String, usize> = HashMap::new();
            for img in &self.attached_images {
                needed
                    .entry(img.placeholder.clone())
                    .or_insert_with(|| text_after.matches(&img.placeholder).count());
            }

            let mut used: HashMap<String, usize> = HashMap::new();
            let mut kept: Vec<AttachedImage> = Vec::with_capacity(self.attached_images.len());
            for img in self.attached_images.drain(..) {
                let total_needed = *needed.get(&img.placeholder).unwrap_or(&0);
                let used_count = used.entry(img.placeholder.clone()).or_insert(0);
                if *used_count < total_needed {
                    kept.push(img);
                    *used_count += 1;
                }
            }
            self.attached_images = kept;
        }

        (InputResult::None, true)
    }

    /// Attempts to remove an image or paste placeholder if the cursor is at the end of one.
    /// Returns true if a placeholder was removed.
    fn try_remove_any_placeholder_at_cursor(&mut self) -> bool {
        // Clamp the cursor to a valid char boundary to avoid panics when slicing.
        let text = self.textarea.text();
        let p = Self::clamp_to_char_boundary(text, self.textarea.cursor());

        // Try image placeholders first
        let mut out: Option<(usize, String)> = None;
        // Detect if the cursor is at the end of any image placeholder.
        // If duplicates exist, remove the specific occurrence's mapping.
        for (i, img) in self.attached_images.iter().enumerate() {
            let ph = &img.placeholder;
            if p < ph.len() {
                continue;
            }
            let start = p - ph.len();
            if text.get(start..p) != Some(ph.as_str()) {
                continue;
            }

            // Count the number of occurrences of `ph` before `start`.
            let mut occ_before = 0usize;
            let mut search_pos = 0usize;
            while search_pos < start {
                let segment = match text.get(search_pos..start) {
                    Some(s) => s,
                    None => break,
                };
                if let Some(found) = segment.find(ph) {
                    occ_before += 1;
                    search_pos += found + ph.len();
                } else {
                    break;
                }
            }

            // Remove the occ_before-th attached image that shares this placeholder label.
            out = if let Some((remove_idx, _)) = self
                .attached_images
                .iter()
                .enumerate()
                .filter(|(_, img2)| img2.placeholder == *ph)
                .nth(occ_before)
            {
                Some((remove_idx, ph.clone()))
            } else {
                Some((i, ph.clone()))
            };
            break;
        }
        if let Some((idx, placeholder)) = out {
            self.textarea.replace_range(p - placeholder.len()..p, "");
            self.attached_images.remove(idx);
            return true;
        }

        // Also handle when the cursor is at the START of an image placeholder.
        // let result = 'out: {
        let out: Option<(usize, String)> = 'out: {
            for (i, img) in self.attached_images.iter().enumerate() {
                let ph = &img.placeholder;
                if p + ph.len() > text.len() {
                    continue;
                }
                if text.get(p..p + ph.len()) != Some(ph.as_str()) {
                    continue;
                }

                // Count occurrences of `ph` before `p`.
                let mut occ_before = 0usize;
                let mut search_pos = 0usize;
                while search_pos < p {
                    let segment = match text.get(search_pos..p) {
                        Some(s) => s,
                        None => break 'out None,
                    };
                    if let Some(found) = segment.find(ph) {
                        occ_before += 1;
                        search_pos += found + ph.len();
                    } else {
                        break 'out None;
                    }
                }

                if let Some((remove_idx, _)) = self
                    .attached_images
                    .iter()
                    .enumerate()
                    .filter(|(_, img2)| img2.placeholder == *ph)
                    .nth(occ_before)
                {
                    break 'out Some((remove_idx, ph.clone()));
                } else {
                    break 'out Some((i, ph.clone()));
                }
            }
            None
        };

        if let Some((idx, placeholder)) = out {
            self.textarea.replace_range(p..p + placeholder.len(), "");
            self.attached_images.remove(idx);
            return true;
        }

        // Then try pasted-content placeholders
        if let Some(placeholder) = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p < ph.len() {
                return None;
            }
            let start = p - ph.len();
            if text.get(start..p) == Some(ph.as_str()) {
                Some(ph.clone())
            } else {
                None
            }
        }) {
            self.textarea.replace_range(p - placeholder.len()..p, "");
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            return true;
        }

        // Also handle when the cursor is at the START of a pasted-content placeholder.
        if let Some(placeholder) = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p + ph.len() > text.len() {
                return None;
            }
            if text.get(p..p + ph.len()) == Some(ph.as_str()) {
                Some(ph.clone())
            } else {
                None
            }
        }) {
            self.textarea.replace_range(p..p + placeholder.len(), "");
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            return true;
        }

        false
    }

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_command_popup(&mut self) {
        let first_line = self.textarea.text().lines().next().unwrap_or("");
        let input_starts_with_slash = first_line.starts_with('/');
        match &mut self.active_popup {
            ActivePopup::Command(popup) => {
                if input_starts_with_slash {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                }
            }
            _ => {
                if input_starts_with_slash {
                    let mut command_popup = CommandPopup::new(self.custom_prompts.clone());
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Command(command_popup);
                }
            }
        }
    }

    pub(crate) fn set_custom_prompts(&mut self, prompts: Vec<CustomPrompt>) {
        self.custom_prompts = prompts.clone();
        if let ActivePopup::Command(popup) = &mut self.active_popup {
            popup.set_prompts(prompts);
        }
    }

    /// Synchronize `self.file_search_popup` with the current text in the textarea.
    /// Note this is only called when self.active_popup is NOT Command.
    fn sync_file_search_popup(&mut self) {
        // Determine if there is an @token underneath the cursor.
        let query = match Self::current_at_token(&self.textarea) {
            Some(token) => token,
            None => {
                self.active_popup = ActivePopup::None;
                self.dismissed_file_popup_token = None;
                return;
            }
        };

        // If user dismissed popup for this exact query, don't reopen until text changes.
        if self.dismissed_file_popup_token.as_ref() == Some(&query) {
            return;
        }

        if !query.is_empty() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(query.clone()));
        }

        match &mut self.active_popup {
            ActivePopup::File(popup) => {
                if query.is_empty() {
                    popup.set_empty_prompt();
                } else {
                    popup.set_query(&query);
                }
            }
            _ => {
                let mut popup = FileSearchPopup::new();
                if query.is_empty() {
                    popup.set_empty_prompt();
                } else {
                    popup.set_query(&query);
                }
                self.active_popup = ActivePopup::File(popup);
            }
        }

        self.current_file_query = Some(query);
        self.dismissed_file_popup_token = None;
    }

    fn set_has_focus(&mut self, has_focus: bool) {
        self.has_focus = has_focus;
    }

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;
    }

    pub(crate) fn set_esc_backtrack_hint(&mut self, show: bool) {
        self.esc_backtrack_hint = show;
    }
}

impl WidgetRef for ChatComposer {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let (popup_constraint, hint_spacing) = match &self.active_popup {
            ActivePopup::Command(popup) => (
                Constraint::Max(popup.calculate_required_height(area.width)),
                0,
            ),
            ActivePopup::File(popup) => (Constraint::Max(popup.calculate_required_height()), 0),
            ActivePopup::None => (
                Constraint::Length(FOOTER_HEIGHT_WITH_HINT),
                FOOTER_SPACING_HEIGHT,
            ),
        };
        let [textarea_rect, popup_rect] =
            Layout::vertical([Constraint::Min(1), popup_constraint]).areas(area);
        match &self.active_popup {
            ActivePopup::Command(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::File(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::None => {
                let hint_rect = if hint_spacing > 0 {
                    let [_, hint_rect] = Layout::vertical([
                        Constraint::Length(hint_spacing),
                        Constraint::Length(FOOTER_HINT_HEIGHT),
                    ])
                    .areas(popup_rect);
                    hint_rect
                } else {
                    popup_rect
                };
                let mut hint: Vec<Span<'static>> = if self.ctrl_c_quit_hint {
                    let ctrl_c_followup = if self.is_task_running {
                        " to interrupt"
                    } else {
                        " to quit"
                    };
                    vec![
                        " ".into(),
                        key_hint::ctrl('C'),
                        " again".into(),
                        ctrl_c_followup.into(),
                    ]
                } else {
                    let newline_hint_key = if self.use_shift_enter_hint {
                        key_hint::shift('⏎')
                    } else {
                        key_hint::ctrl('J')
                    };
                    vec![
                        key_hint::plain('⏎'),
                        " send   ".into(),
                        newline_hint_key,
                        " newline   ".into(),
                        key_hint::ctrl('T'),
                        " transcript   ".into(),
                        key_hint::ctrl('C'),
                        " quit".into(),
                    ]
                };

                if !self.ctrl_c_quit_hint && self.esc_backtrack_hint {
                    hint.push("   ".into());
                    hint.push(key_hint::plain("Esc"));
                    hint.push(" edit prev".into());
                }

                // Append token/context usage info to the footer hints when available.
                if let Some(token_usage_info) = &self.token_usage_info {
                    let token_usage = &token_usage_info.total_token_usage;
                    hint.push("   ".into());
                    hint.push(
                        Span::from(format!(
                            "{} tokens used",
                            format_si_suffix(token_usage.blended_total())
                        ))
                        .style(Style::default().add_modifier(Modifier::DIM)),
                    );
                    let last_token_usage = &token_usage_info.last_token_usage;
                    if let Some(context_window) = token_usage_info.model_context_window {
                        let percent_remaining: u8 = if context_window > 0 {
                            last_token_usage.percent_of_context_window_remaining(context_window)
                        } else {
                            100
                        };
                        let context_style = if percent_remaining < 20 {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().add_modifier(Modifier::DIM)
                        };
                        hint.push("   ".into());
                        hint.push(Span::styled(
                            format!("{percent_remaining}% context left"),
                            context_style,
                        ));
                    }
                }

                Line::from(hint)
                    .style(Style::default().dim())
                    .render_ref(hint_rect, buf);
            }
        }
        let border_style = if self.has_focus {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        Block::default()
            .borders(Borders::LEFT)
            .border_type(BorderType::QuadrantOutside)
            .border_style(border_style)
            .render_ref(
                Rect::new(textarea_rect.x, textarea_rect.y, 1, textarea_rect.height),
                buf,
            );
        let mut textarea_rect = textarea_rect;
        // Leave 1 for border and 1 for padding
        textarea_rect.width = textarea_rect.width.saturating_sub(LIVE_PREFIX_COLS);
        textarea_rect.x = textarea_rect.x.saturating_add(LIVE_PREFIX_COLS);

        let mut state = self.textarea_state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        if self.textarea.text().is_empty() {
            Line::from(self.placeholder_text.as_str())
                .style(Style::default().dim())
                .render_ref(textarea_rect.inner(Margin::new(0, 0)), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;
    use image::Rgba;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::tempdir;

    use crate::app_event::AppEvent;
    use crate::bottom_pane::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::bottom_pane::chat_composer::AttachedImage;
    use crate::bottom_pane::chat_composer::LARGE_PASTE_CHAR_THRESHOLD;
    use crate::bottom_pane::textarea::TextArea;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn footer_hint_row_is_separated_from_composer() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        composer.render_ref(area, &mut buf);

        let row_to_string = |y: u16| {
            let mut row = String::new();
            for x in 0..area.width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            row
        };

        let mut hint_row: Option<(u16, String)> = None;
        for y in 0..area.height {
            let row = row_to_string(y);
            if row.contains(" send") {
                hint_row = Some((y, row));
                break;
            }
        }

        let (hint_row_idx, hint_row_contents) =
            hint_row.expect("expected footer hint row to be rendered");
        assert_eq!(
            hint_row_idx,
            area.height - 1,
            "hint row should occupy the bottom line: {hint_row_contents:?}",
        );

        assert!(
            hint_row_idx > 0,
            "expected a spacing row above the footer hints",
        );

        let spacing_row = row_to_string(hint_row_idx - 1);
        assert_eq!(
            spacing_row.trim(),
            "",
            "expected blank spacing row above hints but saw: {spacing_row:?}",
        );
    }

    #[test]
    fn test_current_at_token_basic_cases() {
        let test_cases = vec![
            // Valid @ tokens
            ("@hello", 3, Some("hello".to_string()), "Basic ASCII token"),
            (
                "@file.txt",
                4,
                Some("file.txt".to_string()),
                "ASCII with extension",
            ),
            (
                "hello @world test",
                8,
                Some("world".to_string()),
                "ASCII token in middle",
            ),
            (
                "@test123",
                5,
                Some("test123".to_string()),
                "ASCII with numbers",
            ),
            // Unicode examples
            ("@İstanbul", 3, Some("İstanbul".to_string()), "Turkish text"),
            (
                "@testЙЦУ.rs",
                8,
                Some("testЙЦУ.rs".to_string()),
                "Mixed ASCII and Cyrillic",
            ),
            ("@诶", 2, Some("诶".to_string()), "Chinese character"),
            ("@👍", 2, Some("👍".to_string()), "Emoji token"),
            // Invalid cases (should return None)
            ("hello", 2, None, "No @ symbol"),
            (
                "@",
                1,
                Some("".to_string()),
                "Only @ symbol triggers empty query",
            ),
            ("@ hello", 2, None, "@ followed by space"),
            ("test @ world", 6, None, "@ with spaces around"),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for case: {description} - input: '{input}', cursor: {cursor_pos}"
            );
        }
    }

    #[test]
    fn test_current_at_token_cursor_positions() {
        let test_cases = vec![
            // Different cursor positions within a token
            ("@test", 0, Some("test".to_string()), "Cursor at @"),
            ("@test", 1, Some("test".to_string()), "Cursor after @"),
            ("@test", 5, Some("test".to_string()), "Cursor at end"),
            // Multiple tokens - cursor determines which token
            ("@file1 @file2", 0, Some("file1".to_string()), "First token"),
            (
                "@file1 @file2",
                8,
                Some("file2".to_string()),
                "Second token",
            ),
            // Edge cases
            ("@", 0, Some("".to_string()), "Only @ symbol"),
            ("@a", 2, Some("a".to_string()), "Single character after @"),
            ("", 0, None, "Empty input"),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for cursor position case: {description} - input: '{input}', cursor: {cursor_pos}",
            );
        }
    }

    #[test]
    fn test_current_at_token_whitespace_boundaries() {
        let test_cases = vec![
            // Space boundaries
            (
                "aaa@aaa",
                4,
                None,
                "Connected @ token - no completion by design",
            ),
            (
                "aaa @aaa",
                5,
                Some("aaa".to_string()),
                "@ token after space",
            ),
            (
                "test @file.txt",
                7,
                Some("file.txt".to_string()),
                "@ token after space",
            ),
            // Full-width space boundaries
            (
                "test　@İstanbul",
                8,
                Some("İstanbul".to_string()),
                "@ token after full-width space",
            ),
            (
                "@ЙЦУ　@诶",
                10,
                Some("诶".to_string()),
                "Full-width space between Unicode tokens",
            ),
            // Tab and newline boundaries
            (
                "test\t@file",
                6,
                Some("file".to_string()),
                "@ token after tab",
            ),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for whitespace boundary case: {description} - input: '{input}', cursor: {cursor_pos}",
            );
        }
    }

    #[test]
    fn handle_paste_small_inserts_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let needs_redraw = composer.handle_paste("hello".to_string());
        assert!(needs_redraw);
        assert_eq!(composer.textarea.text(), "hello");
        assert!(composer.pending_pastes.is_empty());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "hello"),
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn empty_enter_returns_none() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Ensure composer is empty and press Enter.
        assert!(composer.textarea.text().is_empty());
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match result {
            InputResult::None => {}
            other => panic!("expected None for empty enter, got: {other:?}"),
        }
    }

    #[test]
    fn handle_paste_large_uses_placeholder_and_replaces_on_submit() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let large = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 10);
        let needs_redraw = composer.handle_paste(large.clone());
        assert!(needs_redraw);
        let placeholder = format!("[Pasted Content {} chars]", large.chars().count());
        assert_eq!(composer.textarea.text(), placeholder);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, placeholder);
        assert_eq!(composer.pending_pastes[0].1, large);

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, large),
            _ => panic!("expected Submitted"),
        }
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn edit_clears_pending_paste() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let large = "y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.handle_paste(large);
        assert_eq!(composer.pending_pastes.len(), 1);

        // Any edit that removes the placeholder should clear pending_paste
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn ui_snapshots() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut terminal = match Terminal::new(TestBackend::new(100, 10)) {
            Ok(t) => t,
            Err(e) => panic!("Failed to create terminal: {e}"),
        };

        let test_cases = vec![
            ("empty", None),
            ("small", Some("short".to_string())),
            ("large", Some("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5))),
            ("multiple_pastes", None),
            ("backspace_after_pastes", None),
        ];

        for (name, input) in test_cases {
            // Create a fresh composer for each test case
            let mut composer = ChatComposer::new(
                true,
                sender.clone(),
                false,
                "Ask Codex to do anything".to_string(),
                false,
            );

            if let Some(text) = input {
                composer.handle_paste(text);
            } else if name == "multiple_pastes" {
                // First large paste
                composer.handle_paste("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3));
                // Second large paste
                composer.handle_paste("y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 7));
                // Small paste
                composer.handle_paste(" another short paste".to_string());
            } else if name == "backspace_after_pastes" {
                // Three large pastes
                composer.handle_paste("a".repeat(LARGE_PASTE_CHAR_THRESHOLD + 2));
                composer.handle_paste("b".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4));
                composer.handle_paste("c".repeat(LARGE_PASTE_CHAR_THRESHOLD + 6));
                // Move cursor to end and press backspace
                composer.textarea.set_cursor(composer.textarea.text().len());
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }

            terminal
                .draw(|f| f.render_widget_ref(composer, f.area()))
                .unwrap_or_else(|e| panic!("Failed to draw {name} composer: {e}"));

            insta::assert_snapshot!(name, terminal.backend());
        }
    }

    #[test]
    fn slash_popup_model_first_for_mo_ui() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);

        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Type "/mo" humanlike so paste-burst doesn’t interfere.
        type_chars_humanlike(&mut composer, &['/', 'm', 'o']);

        let mut terminal = match Terminal::new(TestBackend::new(60, 4)) {
            Ok(t) => t,
            Err(e) => panic!("Failed to create terminal: {e}"),
        };
        terminal
            .draw(|f| f.render_widget_ref(composer, f.area()))
            .unwrap_or_else(|e| panic!("Failed to draw composer: {e}"));

        // Visual snapshot should show the slash popup with /model as the first entry.
        insta::assert_snapshot!("slash_popup_mo", terminal.backend());
    }

    #[test]
    fn slash_popup_model_first_for_mo_logic() {
        use super::super::command_popup::CommandItem;
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        type_chars_humanlike(&mut composer, &['/', 'm', 'o']);

        match &composer.active_popup {
            ActivePopup::Command(popup) => match popup.selected_item() {
                Some(CommandItem::Builtin(cmd)) => {
                    assert_eq!(cmd.command(), "model")
                }
                Some(CommandItem::UserPrompt(_)) => {
                    panic!("unexpected prompt selected for '/mo'")
                }
                None => panic!("no selected command for '/mo'"),
            },
            _ => panic!("slash popup not active after typing '/mo'"),
        }
    }

    // Test helper: simulate human typing with a brief delay and flush the paste-burst buffer
    fn type_chars_humanlike(composer: &mut ChatComposer, chars: &[char]) {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        for &ch in chars {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            std::thread::sleep(ChatComposer::recommended_paste_flush_delay());
            let _ = composer.flush_paste_burst_if_due();
        }
    }

    #[test]
    fn slash_init_dispatches_command_and_does_not_submit_literal_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Type the slash command.
        type_chars_humanlike(&mut composer, &['/', 'i', 'n', 'i', 't']);

        // Press Enter to dispatch the selected command.
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // When a slash command is dispatched, the composer should return a
        // Command result (not submit literal text) and clear its textarea.
        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "init");
            }
            InputResult::Submitted(text) => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::None => panic!("expected Command result for '/init'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
    }

    #[test]
    fn slash_tab_completion_moves_cursor_to_end() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['/', 'c']);

        let (_result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(composer.textarea.text(), "/compact ");
        assert_eq!(composer.textarea.cursor(), composer.textarea.text().len());
    }

    #[test]
    fn slash_mention_dispatches_command_and_inserts_at() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['/', 'm', 'e', 'n', 't', 'i', 'o', 'n']);

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "mention");
            }
            InputResult::Submitted(text) => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::None => panic!("expected Command result for '/mention'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
        composer.insert_str("@");
        assert_eq!(composer.textarea.text(), "@");
    }

    #[test]
    fn test_multiple_pastes_submission() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Define test cases: (paste content, is_large)
        let test_cases = [
            ("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3), true),
            (" and ".to_string(), false),
            ("y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 7), true),
        ];

        // Expected states after each paste
        let mut expected_text = String::new();
        let mut expected_pending_count = 0;

        // Apply all pastes and build expected state
        let states: Vec<_> = test_cases
            .iter()
            .map(|(content, is_large)| {
                composer.handle_paste(content.clone());
                if *is_large {
                    let placeholder = format!("[Pasted Content {} chars]", content.chars().count());
                    expected_text.push_str(&placeholder);
                    expected_pending_count += 1;
                } else {
                    expected_text.push_str(content);
                }
                (expected_text.clone(), expected_pending_count)
            })
            .collect();

        // Verify all intermediate states were correct
        assert_eq!(
            states,
            vec![
                (
                    format!("[Pasted Content {} chars]", test_cases[0].0.chars().count()),
                    1
                ),
                (
                    format!(
                        "[Pasted Content {} chars] and ",
                        test_cases[0].0.chars().count()
                    ),
                    1
                ),
                (
                    format!(
                        "[Pasted Content {} chars] and [Pasted Content {} chars]",
                        test_cases[0].0.chars().count(),
                        test_cases[2].0.chars().count()
                    ),
                    2
                ),
            ]
        );

        // Submit and verify final expansion
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        if let InputResult::Submitted(text) = result {
            assert_eq!(text, format!("{} and {}", test_cases[0].0, test_cases[2].0));
        } else {
            panic!("expected Submitted");
        }
    }

    #[test]
    fn test_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Define test cases: (content, is_large)
        let test_cases = [
            ("a".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5), true),
            (" and ".to_string(), false),
            ("b".repeat(LARGE_PASTE_CHAR_THRESHOLD + 6), true),
        ];

        // Apply all pastes
        let mut current_pos = 0;
        let states: Vec<_> = test_cases
            .iter()
            .map(|(content, is_large)| {
                composer.handle_paste(content.clone());
                if *is_large {
                    let placeholder = format!("[Pasted Content {} chars]", content.chars().count());
                    current_pos += placeholder.len();
                } else {
                    current_pos += content.len();
                }
                (
                    composer.textarea.text().to_string(),
                    composer.pending_pastes.len(),
                    current_pos,
                )
            })
            .collect();

        // Delete placeholders one by one and collect states
        let mut deletion_states = vec![];

        // First deletion
        composer.textarea.set_cursor(states[0].2);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.text().to_string(),
            composer.pending_pastes.len(),
        ));

        // Second deletion
        composer.textarea.set_cursor(composer.textarea.text().len());
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.text().to_string(),
            composer.pending_pastes.len(),
        ));

        // Verify all states
        assert_eq!(
            deletion_states,
            vec![
                (" and [Pasted Content 1006 chars]".to_string(), 1),
                (" and ".to_string(), 0),
            ]
        );
    }

    #[test]
    fn test_partial_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Define test cases: (cursor_position_from_end, expected_pending_count)
        let test_cases = [
            5, // Delete from middle - should clear tracking
            0, // Delete from end - should clear tracking
        ];

        let paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4);
        let placeholder = format!("[Pasted Content {} chars]", paste.chars().count());

        let states: Vec<_> = test_cases
            .into_iter()
            .map(|pos_from_end| {
                composer.handle_paste(paste.clone());
                composer
                    .textarea
                    .set_cursor(placeholder.len() - pos_from_end);
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                let result = (
                    composer.textarea.text().contains(&placeholder),
                    composer.pending_pastes.len(),
                );
                composer.textarea.set_text("");
                result
            })
            .collect();

        assert_eq!(
            states,
            vec![
                (false, 0), // After deleting from middle
                (false, 0), // After deleting from end
            ]
        );
    }

    // --- Image attachment tests ---
    #[test]
    fn attach_image_and_submit_includes_image_paths() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        let path = PathBuf::from("/tmp/image1.png");
        composer.attach_image(path.clone(), 32, 16, "PNG");
        composer.handle_paste(" hi".into());
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "[image 32x16 PNG] hi"),
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn attach_image_without_text_submits_empty_text_and_images() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        let path = PathBuf::from("/tmp/image2.png");
        composer.attach_image(path.clone(), 10, 5, "PNG");
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "[image 10x5 PNG]"),
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0], path);
        assert!(composer.attached_images.is_empty());
    }

    #[test]
    fn image_placeholder_backspace_behaves_like_text_placeholder() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        let path = PathBuf::from("/tmp/image3.png");
        composer.attach_image(path.clone(), 20, 10, "PNG");
        let placeholder = composer.attached_images[0].placeholder.clone();

        // Case 1: backspace at end
        composer.textarea.move_cursor_to_end_of_line(false);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!composer.textarea.text().contains(&placeholder));
        assert!(composer.attached_images.is_empty());

        // Re-add and test backspace in middle: should break the placeholder string
        // and drop the image mapping (same as text placeholder behavior).
        composer.attach_image(path, 20, 10, "PNG");
        let placeholder2 = composer.attached_images[0].placeholder.clone();
        // Move cursor to roughly middle of placeholder
        if let Some(start_pos) = composer.textarea.text().find(&placeholder2) {
            let mid_pos = start_pos + (placeholder2.len() / 2);
            composer.textarea.set_cursor(mid_pos);
            composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            assert!(!composer.textarea.text().contains(&placeholder2));
            assert!(composer.attached_images.is_empty());
        } else {
            panic!("Placeholder not found in textarea");
        }
    }

    #[test]
    fn backspace_with_multibyte_text_before_placeholder_does_not_panic() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Insert an image placeholder at the start
        let path = PathBuf::from("/tmp/image_multibyte.png");
        composer.attach_image(path, 10, 5, "PNG");
        // Add multibyte text after the placeholder
        composer.textarea.insert_str("日本語");

        // Cursor is at end; pressing backspace should delete the last character
        // without panicking and leave the placeholder intact.
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(composer.attached_images.len(), 1);
        assert!(composer.textarea.text().starts_with("[image 10x5 PNG]"));
    }

    #[test]
    fn deleting_one_of_duplicate_image_placeholders_removes_matching_entry() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let path1 = PathBuf::from("/tmp/image_dup1.png");
        let path2 = PathBuf::from("/tmp/image_dup2.png");

        composer.attach_image(path1, 10, 5, "PNG");
        // separate placeholders with a space for clarity
        composer.handle_paste(" ".into());
        composer.attach_image(path2.clone(), 10, 5, "PNG");

        let ph = composer.attached_images[0].placeholder.clone();
        let text = composer.textarea.text().to_string();
        let start1 = text.find(&ph).expect("first placeholder present");
        let end1 = start1 + ph.len();
        composer.textarea.set_cursor(end1);

        // Backspace should delete the first placeholder and its mapping.
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        let new_text = composer.textarea.text().to_string();
        assert_eq!(1, new_text.matches(&ph).count(), "one placeholder remains");
        assert_eq!(
            vec![AttachedImage {
                path: path2,
                placeholder: "[image 10x5 PNG]".to_string()
            }],
            composer.attached_images,
            "one image mapping remains"
        );
    }

    #[test]
    fn pasting_filepath_attaches_image() {
        let tmp = tempdir().expect("create TempDir");
        let tmp_path: PathBuf = tmp.path().join("codex_tui_test_paste_image.png");
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_fn(3, 2, |_x, _y| Rgba([1, 2, 3, 255]));
        img.save(&tmp_path).expect("failed to write temp png");

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let needs_redraw = composer.handle_paste(tmp_path.to_string_lossy().to_string());
        assert!(needs_redraw);
        assert!(composer.textarea.text().starts_with("[image 3x2 PNG] "));

        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs, vec![tmp_path]);
    }

    #[test]
    fn selecting_custom_prompt_submits_file_contents() {
        let prompt_text = "Hello from saved prompt";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Inject prompts as if received via event.
        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: prompt_text.to_string(),
        }]);

        type_chars_humanlike(
            &mut composer,
            &['/', 'm', 'y', '-', 'p', 'r', 'o', 'm', 'p', 't'],
        );

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(InputResult::Submitted(prompt_text.to_string()), result);
    }

    #[test]
    fn burst_paste_fast_small_buffers_and_flushes_on_stop() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = 32;
        for _ in 0..count {
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
            assert!(
                composer.is_in_paste_burst(),
                "expected active paste burst during fast typing"
            );
            assert!(
                composer.textarea.text().is_empty(),
                "text should not appear during burst"
            );
        }

        assert!(
            composer.textarea.text().is_empty(),
            "text should remain empty until flush"
        );
        std::thread::sleep(ChatComposer::recommended_paste_flush_delay());
        let flushed = composer.flush_paste_burst_if_due();
        assert!(flushed, "expected buffered text to flush after stop");
        assert_eq!(composer.textarea.text(), "a".repeat(count));
        assert!(
            composer.pending_pastes.is_empty(),
            "no placeholder for small burst"
        );
    }

    #[test]
    fn burst_paste_fast_large_inserts_placeholder_on_flush() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = LARGE_PASTE_CHAR_THRESHOLD + 1; // > threshold to trigger placeholder
        for _ in 0..count {
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        }

        // Nothing should appear until we stop and flush
        assert!(composer.textarea.text().is_empty());
        std::thread::sleep(ChatComposer::recommended_paste_flush_delay());
        let flushed = composer.flush_paste_burst_if_due();
        assert!(flushed, "expected flush after stopping fast input");

        let expected_placeholder = format!("[Pasted Content {count} chars]");
        assert_eq!(composer.textarea.text(), expected_placeholder);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, expected_placeholder);
        assert_eq!(composer.pending_pastes[0].1.len(), count);
        assert!(composer.pending_pastes[0].1.chars().all(|c| c == 'x'));
    }

    #[test]
    fn humanlike_typing_1000_chars_appears_live_no_placeholder() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = LARGE_PASTE_CHAR_THRESHOLD; // 1000 in current config
        let chars: Vec<char> = vec!['z'; count];
        type_chars_humanlike(&mut composer, &chars);

        assert_eq!(composer.textarea.text(), "z".repeat(count));
        assert!(composer.pending_pastes.is_empty());
    }
}
