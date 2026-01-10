    pub(crate) fn chat_selection_point_at(
        &mut self,
        layout: &ChatLayout,
        x: f32,
        y: f32,
    ) -> Option<ChatSelectionPoint> {
        if y < layout.viewport_top || y > layout.viewport_bottom {
            return None;
        }
        let mut lines = layout
            .message_layouts
            .iter()
            .flat_map(|layout| layout.lines.iter());

        let first_line = lines.next()?;
        let mut closest = first_line;
        if y < first_line.y {
            return Some(ChatSelectionPoint {
                message_index: first_line.message_index,
                offset: first_line.display_range.start,
            });
        }

        if y >= first_line.y && y <= first_line.y + first_line.line_height {
            return Some(self.chat_point_for_line(first_line, x));
        }

        for line in lines {
            if y >= line.y && y <= line.y + line.line_height {
                return Some(self.chat_point_for_line(line, x));
            }
            closest = line;
        }

        if y > closest.y + closest.line_height {
            return Some(ChatSelectionPoint {
                message_index: closest.message_index,
                offset: closest.display_range.end,
            });
        }

        Some(self.chat_point_for_line(closest, x))
    }

    fn chat_point_for_line(&mut self, line: &ChatLineLayout, x: f32) -> ChatSelectionPoint {
        let char_width = self
            .text_system
            .measure_styled_mono("M", line.font_size, wgpui::text::FontStyle::default())
            .max(1.0);
        let char_count = line.text.chars().count();
        let rel_x = (x - line.x).max(0.0);
        let mut char_index = (rel_x / char_width).floor() as usize;
        if char_index > char_count {
            char_index = char_count;
        }
        let byte_offset = byte_offset_for_char_index(&line.text, char_index);
        ChatSelectionPoint {
            message_index: line.message_index,
            offset: line.display_range.start + byte_offset,
        }
    }

    pub(crate) fn chat_selection_contains(&self, point: ChatSelectionPoint) -> bool {
        let Some(selection) = self.chat.chat_selection else {
            return false;
        };
        let (start, end) = selection.normalized();
        crate::app::selection_point_cmp(&point, &start).is_ge()
            && crate::app::selection_point_cmp(&point, &end).is_le()
    }

    fn chat_selection_text(&self, layout: &ChatLayout) -> Option<String> {
        let selection = self.chat.chat_selection?;
        if selection.is_empty() {
            return None;
        }
        let (start, end) = selection.normalized();
        let mut out = String::new();
        for idx in start.message_index..=end.message_index {
            let Some(message) = layout.message_layouts.get(idx) else {
                continue;
            };
            let text = &message.display_text;
            let start_offset = if idx == start.message_index {
                start.offset.min(text.len())
            } else {
                0
            };
            let end_offset = if idx == end.message_index {
                end.offset.min(text.len())
            } else {
                text.len()
            };
            if start_offset <= end_offset {
                if let Some(slice) = text.get(start_offset..end_offset) {
                    out.push_str(slice);
                }
            }
            if idx != end.message_index {
                out.push('\n');
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    fn select_all_chat(&mut self, layout: &ChatLayout) {
        if layout.message_layouts.is_empty() {
            return;
        }
        let last_idx = layout.message_layouts.len() - 1;
        let end_offset = layout.message_layouts[last_idx].display_text.len();
        self.chat.chat_selection = Some(ChatSelection {
            anchor: ChatSelectionPoint {
                message_index: 0,
                offset: 0,
            },
            focus: ChatSelectionPoint {
                message_index: last_idx,
                offset: end_offset,
            },
        });
    }

    pub(crate) fn open_chat_context_menu(
        &mut self,
        position: Point,
        target_message: Option<usize>,
        copy_enabled: bool,
    ) {
        let mod_key = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        let copy_item = wgpui::MenuItem::new("copy", "Copy")
            .shortcut(format!("{}+C", mod_key))
            .disabled(!copy_enabled);
        let items = vec![
            copy_item,
            wgpui::MenuItem::separator(),
            wgpui::MenuItem::new("select_all", "Select All").shortcut(format!("{}+A", mod_key)),
        ];
        self.chat.chat_context_menu = wgpui::ContextMenu::new().items(items);
        self.chat.chat_context_menu_target = target_message;
        self.chat.chat_context_menu.open(position);
    }

    pub(crate) fn handle_chat_menu_action(&mut self, action: &str, layout: &ChatLayout) {
        match action {
            "copy" => {
                if let Some(text) = self.chat_selection_text(layout) {
                    self.write_chat_clipboard(&text);
                } else if let Some(target) = self.chat.chat_context_menu_target {
                    if let Some(message) = layout.message_layouts.get(target) {
                        self.write_chat_clipboard(&message.display_text);
                    }
                }
            }
            "select_all" => {
                self.select_all_chat(layout);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_chat_shortcut(
        &mut self,
        key: &winit::keyboard::Key<winit::keyboard::SmolStr>,
        modifiers: wgpui::input::Modifiers,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> bool {
        if self.input.is_focused() {
            return false;
        }
        let ctrl_or_meta = modifiers.ctrl || modifiers.meta;
        if !ctrl_or_meta {
            return false;
        }
        match key {
            winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                if self
                    .chat
                    .chat_selection
                    .as_ref()
                    .is_some_and(|sel| !sel.is_empty())
                {
                    let chat_layout = self.build_chat_layout(sidebar_layout, logical_height);
                    if let Some(text) = self.chat_selection_text(&chat_layout) {
                        self.write_chat_clipboard(&text);
                        return true;
                    }
                }
            }
            winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                let chat_layout = self.build_chat_layout(sidebar_layout, logical_height);
                self.select_all_chat(&chat_layout);
                return true;
            }
            _ => {}
        }
        false
    }

    fn write_chat_clipboard(&mut self, text: &str) {
        // Always use system clipboard command (wl-copy on Wayland) for reliability
        let _ = copy_to_clipboard(text);
    }
}
