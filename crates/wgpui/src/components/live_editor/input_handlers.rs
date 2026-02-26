impl LiveEditor {
    pub(super) fn handle_component_event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        match event {
            InputEvent::MouseDown {
                button,
                x,
                y,
                modifiers,
            } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    self.focused = true;
                    self.cursor_blink_start = Instant::now();

                    let new_cursor = self.cursor_position_from_point(*x, *y, &bounds);

                    // Shift+click extends selection from current cursor to click position
                    if modifiers.shift {
                        let anchor = self
                            .selection
                            .as_ref()
                            .map(|s| s.anchor)
                            .unwrap_or(self.cursor);
                        self.selection = Some(Selection::new(anchor, new_cursor));
                        self.cursor = new_cursor;
                        self.is_dragging = true;
                        self.drag_start_pos = Some(anchor);
                        if let Some(id) = self.id {
                            cx.set_focus(id);
                        }
                        return EventResult::Handled;
                    }

                    // Detect double/triple click
                    let now = Instant::now();
                    let time_since_last = now.duration_since(self.last_click_time).as_millis();
                    let distance = ((x - self.last_click_pos.0).powi(2)
                        + (y - self.last_click_pos.1).powi(2))
                    .sqrt();

                    if time_since_last < 400 && distance < 5.0 {
                        self.click_count += 1;
                    } else {
                        self.click_count = 1;
                    }
                    self.last_click_time = now;
                    self.last_click_pos = (*x, *y);

                    match self.click_count {
                        1 => {
                            // Single click - position cursor and start drag
                            self.cursor = new_cursor;
                            self.clear_selection();
                            self.is_dragging = true;
                            self.drag_start_pos = Some(new_cursor);
                        }
                        2 => {
                            // Double click - select word
                            self.cursor = new_cursor;
                            self.select_word_at_cursor();
                            self.is_dragging = false;
                        }
                        _ => {
                            // Triple+ click - select line
                            self.cursor = new_cursor;
                            self.select_line_at_cursor();
                            self.is_dragging = false;
                            self.click_count = 3; // Cap at 3
                        }
                    }

                    if let Some(id) = self.id {
                        cx.set_focus(id);
                    }
                    return EventResult::Handled;
                } else if self.focused {
                    self.blur();
                    self.is_dragging = false;
                    cx.clear_focus();
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseMove { x, y } => {
                if self.is_dragging && self.focused {
                    if let Some(start) = self.drag_start_pos {
                        let new_cursor = self.cursor_position_from_point(*x, *y, &bounds);
                        self.cursor = new_cursor;
                        self.selection = Some(Selection::new(start, new_cursor));
                    }
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left {
                    self.is_dragging = false;
                    return EventResult::Handled;
                }
            }

            InputEvent::Scroll { dy, .. } => {
                if self.focused {
                    let line_height = self.style.font_size * self.style.line_height;
                    let status_bar_height = 24.0;
                    let visible_height =
                        bounds.size.height - self.style.padding * 2.0 - status_bar_height;

                    // Calculate total visual rows accounting for wrapped lines
                    let max_content_width = self.scaled_max_content_width();
                    let content_width = bounds.size.width.min(max_content_width);
                    let available_text_width = content_width - self.style.padding * 2.0;
                    let max_chars = if self.style.wrap_text {
                        self.max_chars_per_line(available_text_width, self.mono_char_width)
                    } else {
                        usize::MAX
                    };

                    let mut total_visual_rows = 0;
                    for (line_idx, line) in self.lines.iter().enumerate() {
                        if line_idx == 1 {
                            total_visual_rows += 1; // Title margin
                        }
                        let segments = if self.style.wrap_text {
                            self.wrap_line(line, max_chars)
                        } else {
                            vec![(0, line.clone())]
                        };
                        total_visual_rows += segments.len();
                    }

                    let total_content_height = total_visual_rows as f32 * line_height;
                    let max_scroll = (total_content_height - visible_height).max(0.0);
                    self.scroll_offset =
                        (self.scroll_offset - dy * line_height * 3.0).clamp(0.0, max_scroll);
                    return EventResult::Handled;
                }
            }

            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                match key {
                    Key::Character(c) => {
                        if modifiers.ctrl || modifiers.meta {
                            match c.as_str() {
                                "a" | "A" => self.select_all(),
                                "c" | "C" => {
                                    if let Some(text) = self.get_selected_text() {
                                        cx.write_clipboard(&text);
                                    }
                                }
                                "x" | "X" => {
                                    if let Some(text) = self.get_selected_text() {
                                        cx.write_clipboard(&text);
                                        self.delete_selection();
                                    }
                                }
                                "v" | "V" => {
                                    if let Some(text) = cx.read_clipboard() {
                                        self.delete_selection();
                                        self.insert_str(&text);
                                    }
                                }
                                "s" | "S" => {
                                    self.notify_save();
                                }
                                "z" => {
                                    // Ctrl+Z = undo
                                    self.undo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                "Z" => {
                                    // Ctrl+Shift+Z = redo
                                    self.redo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                "y" | "Y" => {
                                    // Ctrl+Y = redo (alternative)
                                    self.redo();
                                    self.cursor_blink_start = Instant::now();
                                }
                                _ => {}
                            }
                        } else {
                            self.delete_selection();
                            self.insert_str(c);
                        }
                        self.ensure_cursor_visible(&bounds);
                        return EventResult::Handled;
                    }

                    Key::Named(named) => {
                        let shift = modifiers.shift;

                        match named {
                            NamedKey::Space => {
                                self.delete_selection();
                                self.insert_char(' ');
                            }
                            NamedKey::Enter => {
                                self.insert_newline();
                            }
                            NamedKey::Backspace => {
                                self.delete_backward();
                            }
                            NamedKey::Delete => {
                                self.delete_forward();
                            }
                            NamedKey::ArrowLeft => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_line_start();
                                    } else {
                                        self.move_cursor_left();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_line_start();
                                    } else {
                                        self.move_cursor_left();
                                    }
                                }
                            }
                            NamedKey::ArrowRight => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_line_end();
                                    } else {
                                        self.move_cursor_right();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_line_end();
                                    } else {
                                        self.move_cursor_right();
                                    }
                                }
                            }
                            NamedKey::ArrowUp => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_document_start();
                                    } else {
                                        self.move_cursor_up();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_document_start();
                                    } else {
                                        self.move_cursor_up();
                                    }
                                }
                            }
                            NamedKey::ArrowDown => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    if modifiers.meta {
                                        self.move_cursor_to_document_end();
                                    } else {
                                        self.move_cursor_down();
                                    }
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    if modifiers.meta {
                                        self.move_cursor_to_document_end();
                                    } else {
                                        self.move_cursor_down();
                                    }
                                }
                            }
                            NamedKey::Home => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_to_line_start();
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_to_line_start();
                                }
                            }
                            NamedKey::End => {
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_to_line_end();
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_to_line_end();
                                }
                            }
                            NamedKey::PageUp => {
                                let line_height = self.style.font_size * self.style.line_height;
                                let visible_height = bounds.size.height - self.style.padding * 2.0;
                                let visible_lines = (visible_height / line_height) as usize;
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_page_up(visible_lines);
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_page_up(visible_lines);
                                }
                                self.cursor_blink_start = Instant::now();
                            }
                            NamedKey::PageDown => {
                                let line_height = self.style.font_size * self.style.line_height;
                                let visible_height = bounds.size.height - self.style.padding * 2.0;
                                let visible_lines = (visible_height / line_height) as usize;
                                if shift {
                                    if self.selection.is_none() {
                                        self.start_selection();
                                    }
                                    self.move_cursor_page_down(visible_lines);
                                    self.extend_selection();
                                } else {
                                    self.clear_selection();
                                    self.move_cursor_page_down(visible_lines);
                                }
                                self.cursor_blink_start = Instant::now();
                            }
                            NamedKey::Tab => {
                                self.delete_selection();
                                self.insert_str("    "); // 4 spaces
                            }
                            NamedKey::Escape => {
                                self.blur();
                                cx.clear_focus();
                            }
                            _ => {}
                        }
                        // Reset blink timer so cursor shows immediately after movement
                        self.cursor_blink_start = Instant::now();
                        self.ensure_cursor_visible(&bounds);
                        return EventResult::Handled;
                    }
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }
}
