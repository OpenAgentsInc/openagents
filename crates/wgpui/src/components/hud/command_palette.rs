use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult, TextInput};
use crate::input::{Key, NamedKey};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Clone, Debug)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub keybinding: Option<String>,
    pub category: Option<String>,
}

impl Command {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            keybinding: None,
            category: None,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn keybinding(mut self, keys: impl Into<String>) -> Self {
        self.keybinding = Some(keys.into());
        self
    }

    pub fn category(mut self, cat: impl Into<String>) -> Self {
        self.category = Some(cat.into());
        self
    }
}

pub struct CommandPalette {
    id: Option<ComponentId>,
    commands: Vec<Command>,
    filtered_commands: Vec<usize>,
    search_input: TextInput,
    selected_index: usize,
    is_open: bool,
    max_visible_items: usize,
    scroll_offset: usize,
    item_height: f32,
    mono: bool,
    on_select: Option<Box<dyn FnMut(&Command)>>,
    on_close: Option<Box<dyn FnMut()>>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            id: None,
            commands: Vec::new(),
            filtered_commands: Vec::new(),
            search_input: TextInput::new()
                .placeholder("Type a command...")
                .background(theme::bg::SURFACE),
            selected_index: 0,
            is_open: false,
            max_visible_items: 8,
            scroll_offset: 0,
            item_height: 40.0,
            mono: false,
            on_select: None,
            on_close: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn commands(mut self, commands: Vec<Command>) -> Self {
        self.commands = commands;
        self.update_filtered();
        self
    }

    pub fn max_visible_items(mut self, count: usize) -> Self {
        self.max_visible_items = count;
        self
    }

    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self.search_input.set_mono(mono);
        self
    }

    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: FnMut(&Command) + 'static,
    {
        self.on_select = Some(Box::new(f));
        self
    }

    pub fn on_close<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_close = Some(Box::new(f));
        self
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.search_input.set_value("");
        self.search_input.focus();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.update_filtered();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.search_input.blur();
        if let Some(callback) = &mut self.on_close {
            callback();
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get a reference to the search input
    pub fn search_input(&self) -> &TextInput {
        &self.search_input
    }

    /// Get a mutable reference to the search input
    pub fn search_input_mut(&mut self) -> &mut TextInput {
        &mut self.search_input
    }

    pub fn add_command(&mut self, command: Command) {
        self.commands.push(command);
        self.update_filtered();
    }

    pub fn set_commands(&mut self, commands: Vec<Command>) {
        self.commands = commands;
        self.update_filtered();
    }

    fn update_filtered(&mut self) {
        let query = self.search_input.get_value().to_lowercase();

        if query.is_empty() {
            self.filtered_commands = (0..self.commands.len()).collect();
        } else {
            self.filtered_commands = self
                .commands
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    cmd.label.to_lowercase().contains(&query)
                        || cmd
                            .description
                            .as_ref()
                            .is_some_and(|d| d.to_lowercase().contains(&query))
                        || cmd
                            .category
                            .as_ref()
                            .is_some_and(|c| c.to_lowercase().contains(&query))
                })
                .map(|(i, _)| i)
                .collect();
        }

        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn select_current(&mut self) {
        if let Some(&cmd_index) = self.filtered_commands.get(self.selected_index) {
            if let Some(callback) = &mut self.on_select {
                callback(&self.commands[cmd_index]);
            }
            self.close();
        }
    }

    pub fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if self.selected_index + 1 < self.filtered_commands.len() {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + self.max_visible_items {
                self.scroll_offset = self.selected_index - self.max_visible_items + 1;
            }
        }
    }

    fn item_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let input_height = 48.0;
        let padding = theme::spacing::XS;
        let y = bounds.origin.y
            + input_height
            + padding
            + (index - self.scroll_offset) as f32 * self.item_height;

        Bounds::new(
            bounds.origin.x + padding,
            y,
            bounds.size.width - padding * 2.0,
            self.item_height,
        )
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandPalette {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.is_open {
            return;
        }

        // Render on layer 1 to be on top of all layer 0 content
        cx.scene.set_layer(1);

        // Semi-transparent backdrop
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7)));

        let palette_width = 500.0_f32.min(bounds.size.width - 40.0);
        let input_height = 48.0;
        let padding = theme::spacing::XS;
        let visible_items = self.filtered_commands.len().min(self.max_visible_items);
        let list_height = visible_items as f32 * self.item_height;
        let palette_height = input_height + padding + list_height + padding;

        let palette_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - palette_width) / 2.0,
            bounds.origin.y + 100.0,
            palette_width,
            palette_height,
        );

        cx.scene.draw_quad(
            Quad::new(palette_bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let input_bounds = Bounds::new(
            palette_bounds.origin.x + padding,
            palette_bounds.origin.y + padding,
            palette_bounds.size.width - padding * 2.0,
            input_height - padding * 2.0,
        );
        self.search_input.paint(input_bounds, cx);

        let visible_end =
            (self.scroll_offset + self.max_visible_items).min(self.filtered_commands.len());

        for vis_index in self.scroll_offset..visible_end {
            if let Some(&cmd_index) = self.filtered_commands.get(vis_index) {
                let item_bounds = self.item_bounds(&palette_bounds, vis_index);
                let command = &self.commands[cmd_index];
                let is_selected = vis_index == self.selected_index;

                let bg = if is_selected {
                    theme::accent::PRIMARY.with_alpha(0.2)
                } else {
                    Hsla::transparent()
                };
                cx.scene
                    .draw_quad(Quad::new(item_bounds).with_background(bg));

                let label_origin = Point::new(
                    item_bounds.origin.x + theme::spacing::SM,
                    item_bounds.origin.y + theme::spacing::XS,
                );
                let label_run = if self.mono {
                    cx.text.layout_styled_mono(
                        &command.label,
                        label_origin,
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                        FontStyle::default(),
                    )
                } else {
                    cx.text.layout_mono(
                        &command.label,
                        label_origin,
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    )
                };
                cx.scene.draw_text(label_run);

                if let Some(desc) = &command.description {
                    let desc_origin = Point::new(
                        item_bounds.origin.x + theme::spacing::SM,
                        item_bounds.origin.y + theme::spacing::XS + theme::font_size::SM + 2.0,
                    );
                    let desc_run = if self.mono {
                        cx.text.layout_styled_mono(
                            desc,
                            desc_origin,
                            theme::font_size::XS,
                            theme::text::MUTED,
                            FontStyle::default(),
                        )
                    } else {
                        cx.text.layout_mono(
                            desc,
                            desc_origin,
                            theme::font_size::XS,
                            theme::text::MUTED,
                        )
                    };
                    cx.scene.draw_text(desc_run);
                }

                if let Some(keys) = &command.keybinding {
                    let key_width = if self.mono {
                        cx.text.measure_styled_mono(
                            keys,
                            theme::font_size::XS,
                            FontStyle::default(),
                        )
                    } else {
                        cx.text
                            .measure_styled(keys, theme::font_size::XS, FontStyle::default())
                    };
                    let key_origin = Point::new(
                        item_bounds.origin.x + item_bounds.size.width
                            - key_width
                            - theme::spacing::SM,
                        item_bounds.origin.y
                            + (item_bounds.size.height - theme::font_size::XS) / 2.0,
                    );
                    let key_run = if self.mono {
                        cx.text.layout_styled_mono(
                            keys,
                            key_origin,
                            theme::font_size::XS,
                            theme::text::DISABLED,
                            FontStyle::default(),
                        )
                    } else {
                        cx.text.layout_mono(
                            keys,
                            key_origin,
                            theme::font_size::XS,
                            theme::text::DISABLED,
                        )
                    };
                    cx.scene.draw_text(key_run);
                }
            }
        }

        if self.filtered_commands.is_empty() {
            let empty_text = "No matching commands";
            let empty_width = if self.mono {
                cx.text
                    .measure_styled_mono(empty_text, theme::font_size::SM, FontStyle::default())
            } else {
                cx.text
                    .measure_styled(empty_text, theme::font_size::SM, FontStyle::default())
            };
            let empty_origin = Point::new(
                palette_bounds.origin.x + (palette_bounds.size.width - empty_width) / 2.0,
                palette_bounds.origin.y + input_height + padding + theme::spacing::MD,
            );
            let empty_run = if self.mono {
                cx.text.layout_styled_mono(
                    empty_text,
                    empty_origin,
                    theme::font_size::SM,
                    theme::text::MUTED,
                    FontStyle::default(),
                )
            } else {
                cx.text.layout_mono(
                    empty_text,
                    empty_origin,
                    theme::font_size::SM,
                    theme::text::MUTED,
                )
            };
            cx.scene.draw_text(empty_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if !self.is_open {
            return EventResult::Ignored;
        }

        match event {
            InputEvent::KeyDown { key, .. } => match key {
                Key::Named(NamedKey::Escape) => {
                    self.close();
                    return EventResult::Handled;
                }
                Key::Named(NamedKey::Enter) => {
                    self.select_current();
                    return EventResult::Handled;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.move_selection_up();
                    return EventResult::Handled;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.move_selection_down();
                    return EventResult::Handled;
                }
                _ => {}
            },
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);

                let palette_width = 500.0_f32.min(bounds.size.width - 40.0);
                let input_height = 48.0;
                let padding = theme::spacing::XS;
                let visible_items = self.filtered_commands.len().min(self.max_visible_items);
                let list_height = visible_items as f32 * self.item_height;
                let palette_height = input_height + padding + list_height + padding;

                let palette_bounds = Bounds::new(
                    bounds.origin.x + (bounds.size.width - palette_width) / 2.0,
                    bounds.origin.y + 100.0,
                    palette_width,
                    palette_height,
                );

                if !palette_bounds.contains(point) {
                    self.close();
                    return EventResult::Handled;
                }

                let visible_end =
                    (self.scroll_offset + self.max_visible_items).min(self.filtered_commands.len());
                for vis_index in self.scroll_offset..visible_end {
                    let item_bounds = self.item_bounds(&palette_bounds, vis_index);
                    if item_bounds.contains(point) {
                        self.selected_index = vis_index;
                        self.select_current();
                        return EventResult::Handled;
                    }
                }
            }
            _ => {}
        }

        let palette_width = 500.0_f32.min(bounds.size.width - 40.0);
        let padding = theme::spacing::XS;
        let palette_x = bounds.origin.x + (bounds.size.width - palette_width) / 2.0;

        let input_bounds = Bounds::new(
            palette_x + padding,
            bounds.origin.y + 100.0 + padding,
            palette_width - padding * 2.0,
            48.0 - padding * 2.0,
        );

        let old_value = self.search_input.get_value().to_string();
        let result = self.search_input.event(event, input_bounds, cx);

        if self.search_input.get_value() != old_value {
            self.update_filtered();
        }

        if result == EventResult::Handled {
            return result;
        }

        EventResult::Handled
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, EventContext, InputEvent, Modifiers, MouseButton, Point};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_command_new() {
        let cmd = Command::new("test", "Test Command");
        assert_eq!(cmd.id, "test");
        assert_eq!(cmd.label, "Test Command");
    }

    #[test]
    fn test_command_builder() {
        let cmd = Command::new("save", "Save File")
            .description("Save the current file")
            .keybinding("Cmd+S")
            .category("File");

        assert_eq!(cmd.description, Some("Save the current file".to_string()));
        assert_eq!(cmd.keybinding, Some("Cmd+S".to_string()));
        assert_eq!(cmd.category, Some("File".to_string()));
    }

    #[test]
    fn test_command_palette_new() {
        let palette = CommandPalette::new();
        assert!(!palette.is_open());
        assert_eq!(palette.max_visible_items, 8);
    }

    #[test]
    fn test_command_palette_open_close() {
        let mut palette = CommandPalette::new();

        palette.open();
        assert!(palette.is_open());

        palette.close();
        assert!(!palette.is_open());
    }

    #[test]
    fn test_command_palette_with_commands() {
        let commands = vec![
            Command::new("new", "New File"),
            Command::new("open", "Open File"),
            Command::new("save", "Save File"),
        ];

        let palette = CommandPalette::new().commands(commands);
        assert_eq!(palette.commands.len(), 3);
        assert_eq!(palette.filtered_commands.len(), 3);
    }

    #[test]
    fn test_command_palette_add_command() {
        let mut palette = CommandPalette::new();
        palette.add_command(Command::new("test", "Test"));

        assert_eq!(palette.commands.len(), 1);
    }

    #[test]
    fn test_command_palette_selection() {
        let mut palette = CommandPalette::new().commands(vec![
            Command::new("a", "A"),
            Command::new("b", "B"),
            Command::new("c", "C"),
        ]);

        assert_eq!(palette.selected_index, 0);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 1);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 2);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 2);

        palette.move_selection_up();
        assert_eq!(palette.selected_index, 1);
    }

    #[test]
    fn test_command_palette_filtering_by_query() {
        let mut palette = CommandPalette::new().commands(vec![
            Command::new("open", "Open File")
                .description("Open from disk")
                .category("File"),
            Command::new("build", "Build Project")
                .description("Compile assets")
                .category("Project"),
            Command::new("deploy", "Deploy").category("Ops"),
        ]);

        palette.search_input.set_value("open");
        palette.update_filtered();
        assert_eq!(palette.filtered_commands, vec![0]);

        palette.search_input.set_value("ops");
        palette.update_filtered();
        assert_eq!(palette.filtered_commands, vec![2]);

        palette.search_input.set_value("compile");
        palette.update_filtered();
        assert_eq!(palette.filtered_commands, vec![1]);

        palette.selected_index = 2;
        palette.scroll_offset = 1;
        palette.search_input.set_value("file");
        palette.update_filtered();
        assert_eq!(palette.selected_index, 0);
        assert_eq!(palette.scroll_offset, 0);
    }

    #[test]
    fn test_command_palette_scroll_offset_updates() {
        let mut palette = CommandPalette::new().max_visible_items(2).commands(vec![
            Command::new("a", "A"),
            Command::new("b", "B"),
            Command::new("c", "C"),
            Command::new("d", "D"),
        ]);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 1);
        assert_eq!(palette.scroll_offset, 0);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 2);
        assert_eq!(palette.scroll_offset, 1);

        palette.move_selection_down();
        assert_eq!(palette.selected_index, 3);
        assert_eq!(palette.scroll_offset, 2);
    }

    #[test]
    fn test_command_palette_select_current_invokes_callback() {
        let selected = Rc::new(RefCell::new(None));
        let closed = Rc::new(RefCell::new(false));
        let selected_clone = Rc::clone(&selected);
        let closed_clone = Rc::clone(&closed);

        let mut palette = CommandPalette::new()
            .commands(vec![
                Command::new("alpha", "Alpha"),
                Command::new("beta", "Beta"),
            ])
            .on_select(move |cmd| {
                *selected_clone.borrow_mut() = Some(cmd.id.clone());
            })
            .on_close(move || {
                *closed_clone.borrow_mut() = true;
            });

        palette.open();
        palette.select_current();

        assert_eq!(selected.borrow().as_deref(), Some("alpha"));
        assert!(!palette.is_open());
        assert!(*closed.borrow());
    }

    #[test]
    fn test_command_palette_event_keyboard() {
        let selected = Rc::new(RefCell::new(None));
        let selected_clone = Rc::clone(&selected);
        let mut palette = CommandPalette::new()
            .commands(vec![
                Command::new("alpha", "Alpha"),
                Command::new("beta", "Beta"),
            ])
            .on_select(move |cmd| {
                *selected_clone.borrow_mut() = Some(cmd.id.clone());
            });

        palette.open();

        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let mut cx = EventContext::new();

        let down = InputEvent::KeyDown {
            key: Key::Named(NamedKey::ArrowDown),
            modifiers: Modifiers::default(),
        };
        palette.event(&down, bounds, &mut cx);
        assert_eq!(palette.selected_index, 1);

        let enter = InputEvent::KeyDown {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        };
        palette.event(&enter, bounds, &mut cx);
        assert_eq!(selected.borrow().as_deref(), Some("beta"));
        assert!(!palette.is_open());

        palette.open();
        let escape = InputEvent::KeyDown {
            key: Key::Named(NamedKey::Escape),
            modifiers: Modifiers::default(),
        };
        palette.event(&escape, bounds, &mut cx);
        assert!(!palette.is_open());
    }

    #[test]
    fn test_command_palette_mouse_clicks() {
        let selected = Rc::new(RefCell::new(None));
        let selected_clone = Rc::clone(&selected);
        let mut palette = CommandPalette::new()
            .commands(vec![
                Command::new("alpha", "Alpha"),
                Command::new("beta", "Beta"),
            ])
            .on_select(move |cmd| {
                *selected_clone.borrow_mut() = Some(cmd.id.clone());
            });

        palette.open();
        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let mut cx = EventContext::new();

        let outside = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: 5.0,
            y: 5.0,
        };
        palette.event(&outside, bounds, &mut cx);
        assert!(!palette.is_open());

        palette.open();
        let palette_width = 500.0_f32.min(bounds.size.width - 40.0);
        let input_height = 48.0;
        let padding = theme::spacing::XS;
        let visible_items = palette
            .filtered_commands
            .len()
            .min(palette.max_visible_items);
        let list_height = visible_items as f32 * palette.item_height;
        let palette_height = input_height + padding + list_height + padding;

        let palette_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - palette_width) / 2.0,
            bounds.origin.y + 100.0,
            palette_width,
            palette_height,
        );
        let item_bounds = palette.item_bounds(&palette_bounds, 0);
        let click = Point::new(item_bounds.origin.x + 2.0, item_bounds.origin.y + 2.0);

        let inside = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: click.x,
            y: click.y,
        };
        palette.event(&inside, bounds, &mut cx);
        assert_eq!(selected.borrow().as_deref(), Some("alpha"));
        assert!(!palette.is_open());
    }
}
