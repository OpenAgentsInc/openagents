//! Bundled plugin management and local configuration screens.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use super::{span, truncate};
use crate::{
    App, Draft, Mode, Screen,
    models::OPENROUTER_PLUGIN,
    plugin_definition::DEFINITIONS,
    plugins::{ENDPOINT, SettingsFocus},
    theme as t,
};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.screen == Screen::PluginSettings {
        app.plugins.selected_definition().name
    } else {
        "Plugins"
    };
    let context_width = if area.width >= 48 && app.mode == Mode::Live {
        24
    } else if area.width >= 42 {
        17
    } else {
        0
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(title, area.width.saturating_sub(context_width + 2)),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Rect { height: 1, ..area },
    );
    if context_width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                span(
                    if app.mode == Mode::Live {
                        "live · "
                    } else {
                        ""
                    },
                    t::ACCENT_MODEL,
                ),
                span("openagents", t::PATH),
                span(" / main", t::GRAY),
            ]))
            .right_aligned(),
            Rect {
                x: area.right() - context_width,
                width: context_width,
                height: 1,
                ..area
            },
        );
    }
    let body = Rect {
        y: area.y + 2,
        height: area.height.saturating_sub(2),
        ..area
    };
    if app.screen == Screen::Plugins {
        manager(frame, body, app);
    } else {
        match app.plugins.selected_definition().id {
            OPENROUTER_PLUGIN => router_settings(frame, body, app),
            "jev" => jev_settings(frame, body, app),
            "acp-subagents" => acp_settings(frame, body, app),
            _ => plugin_info(frame, body, app),
        }
    }
}

fn manager(frame: &mut Frame, area: Rect, app: &App) {
    let p = &app.plugins;
    let wide = area.width >= 56;
    let mut rows = Vec::new();
    if wide {
        rows.push(Line::from(span(
            format!("  {:<30}{:<12}Status", "Plugin", "Enabled"),
            t::GRAY,
        )));
    }
    for (index, definition) in DEFINITIONS.iter().enumerate() {
        let selected = index == p.selected.min(DEFINITIONS.len() - 1);
        let enabled = p.enabled_for(definition.id);
        let state = if enabled { "[ on  ]" } else { "[ off ]" };
        let status = p.status_for(definition.id);
        let mut row = Line::from(vec![
            span(if selected { "❯ " } else { "  " }, t::ACCENT_MODEL),
            Span::styled(
                if wide {
                    format!("{:<30}", definition.name)
                } else {
                    truncate(definition.name, area.width.saturating_sub(2))
                },
                Style::default()
                    .fg(t::TEXT_PRIMARY)
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]);
        if wide {
            row.spans.extend([
                span(
                    format!("{state:<12}"),
                    if enabled { t::ACCENT_MODEL } else { t::GRAY },
                ),
                span(status, status_color(status)),
            ]);
        }
        if selected {
            row = row.style(Style::default().bg(t::BG_DARK));
        }
        rows.push(row);
        if !wide {
            rows.push(Line::from(vec![
                span(
                    format!("  {state} "),
                    if enabled { t::ACCENT_MODEL } else { t::GRAY },
                ),
                span(
                    truncate(status, area.width.saturating_sub(10)),
                    status_color(status),
                ),
            ]));
        }
    }
    let hints = if area.width >= 64 {
        vec!["Up/Down Select · Space Turn on/off · Enter Configure · Esc Back"]
    } else if area.width >= 28 {
        vec![
            "Up/Down Select · Space Turn on/off",
            "Enter Configure · Esc Back",
        ]
    } else {
        vec![
            "Up/Down Select",
            "Space Turn on/off",
            "Enter Configure",
            "Esc Back",
        ]
    };
    let list_height =
        (rows.len() as u16).min(area.height.saturating_sub(hints.len() as u16 + 1).max(1));
    let selected = p.selected.min(DEFINITIONS.len() - 1) as u16;
    let selected_row = if wide { 1 + selected } else { selected * 2 };
    let selected_bottom = selected_row + u16::from(!wide);
    let scroll = selected_bottom.saturating_sub(list_height.saturating_sub(1));
    frame.render_widget(
        Paragraph::new(Text::from(rows)).scroll((scroll, 0)),
        Rect {
            height: list_height,
            ..area
        },
    );
    let mut lines = vec![Line::default()];
    lines.extend(
        hints
            .into_iter()
            .map(|hint| Line::from(span(truncate(hint, area.width), t::GRAY_BRIGHT))),
    );
    lines.push(Line::default());
    lines.extend(plugin_details(app, area.width));
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(ratatui::widgets::Wrap { trim: false }),
        Rect {
            y: area.y + list_height,
            height: area.height.saturating_sub(list_height),
            ..area
        },
    );
}

fn status_color(status: &str) -> ratatui::style::Color {
    match status {
        "Disabled" => t::GRAY,
        "Setup required" | "Checking" => t::COMMAND,
        "Unavailable" => t::DIFF_DELETE_FG,
        _ => t::ACCENT_SUCCESS,
    }
}

fn plugin_details(app: &App, width: u16) -> Vec<Line<'static>> {
    let p = &app.plugins;
    let definition = p.selected_definition();
    let mut lines = vec![
        Line::from(span(
            if definition.id == OPENROUTER_PLUGIN {
                "Model provider"
            } else {
                definition.name
            },
            t::ACCENT_MODEL,
        )),
        Line::from(span(definition.description, t::TEXT_SECONDARY)),
    ];
    match definition.id {
        OPENROUTER_PLUGIN => {
            lines.push(Line::from(span(
                "Requests go directly to OpenRouter. Billed by OpenRouter.",
                t::GRAY,
            )));
            lines.push(Line::default());
            lines.extend([
                detail(
                    "API key",
                    if p.key_configured {
                        if app.mode == Mode::Live {
                            "Added"
                        } else {
                            "Added · not verified"
                        }
                    } else {
                        "Not configured"
                    },
                    width,
                ),
                detail("Model", &p.model, width),
                detail(
                    "Reasoning",
                    p.options.reasoning.as_deref().unwrap_or("Model default"),
                    width,
                ),
                detail("Endpoint", ENDPOINT, width),
                detail("Connection", p.connection_label(), width),
            ]);
        }
        "jev" => {
            lines.push(Line::default());
            lines.extend([
                detail("Tool", "jev", width),
                detail("API key", p.bundled.key_label(), width),
                detail("Model", p.bundled.jev_model(), width),
                detail("Endpoint", p.bundled.jev_endpoint(), width),
                detail("Connection", p.bundled.connection_label(), width),
            ]);
        }
        "microcoder" => {
            lines.push(Line::default());
            lines.extend([
                detail("Tool", "microcoder", width),
                detail(
                    "Provider",
                    "Local model login or configured OpenRouter",
                    width,
                ),
                detail("Working folder", "Current checkout", width),
                Line::from(span(
                    "The host applies command, step, and time limits.",
                    t::GRAY,
                )),
            ]);
        }
        "openagents-cli" => {
            lines.push(Line::default());
            lines.extend([
                detail("Tool", "openagents_cli", width),
                detail("Command", "openagents", width),
                Line::from(span(
                    "Installed with Coder. Use --help to discover commands.",
                    t::GRAY,
                )),
                Line::from(span(
                    "Runs argument arrays in the current working folder.",
                    t::GRAY,
                )),
            ]);
        }
        "acp-subagents" => {
            lines.push(Line::default());
            lines.extend([
                detail("Tool", "acp_subagent", width),
                detail(
                    "Agents",
                    &format!("{} configured", p.bundled.acp_agents.len()),
                    width,
                ),
                Line::from(span(
                    "Configure named local executables that speak ACP over stdio.",
                    t::GRAY,
                )),
            ]);
        }
        _ => {}
    }
    if app.mode == Mode::Live {
        let error = if definition.id == OPENROUTER_PLUGIN {
            &p.storage_error
        } else {
            &p.bundled.storage_error
        };
        if let Some(error) = error {
            lines.push(Line::from(span(error, t::DIFF_DELETE_FG)));
        }
    }
    lines
}

fn plugin_info(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = plugin_details(app, area.width);
    lines.push(Line::default());
    lines.push(Line::from(span(
        "Esc Back · Turn on/off from the plugin list",
        t::GRAY_BRIGHT,
    )));
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
}

fn detail(label: &str, value: &str, width: u16) -> Line<'static> {
    if width < 40 {
        Line::from(vec![
            span(format!("{label}  "), t::GRAY),
            span(value, t::TEXT_SECONDARY),
        ])
    } else {
        Line::from(vec![
            span(format!("{label:<16}"), t::GRAY),
            span(value, t::TEXT_SECONDARY),
        ])
    }
}

fn router_settings(frame: &mut Frame, area: Rect, app: &App) {
    let p = &app.plugins;
    let mut lines = vec![
        Line::from(span("Connection settings", t::ACCENT_MODEL)),
        detail("Endpoint", ENDPOINT, area.width),
        Line::default(),
    ];
    let mut cursor = None;
    let key_row = field(
        &mut lines,
        "OpenRouter API key",
        p.field(true),
        p.focus == SettingsFocus::ApiKey,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        truncate(p.error.unwrap_or(p.key_label()), area.width),
        if p.error.is_some() {
            t::DIFF_DELETE_FG
        } else {
            t::GRAY
        },
    )));
    lines.push(Line::default());
    let model_row = field(
        &mut lines,
        "Model ID",
        p.field(false),
        p.focus == SettingsFocus::Model,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        truncate(
            "Default: openrouter/free · Use /models to choose.",
            area.width,
        ),
        t::GRAY,
    )));
    lines.push(Line::default());
    let test_row = action(
        &mut lines,
        "Test API key",
        p.focus == SettingsFocus::TestKey,
        t::ACCENT_SKILL,
    );
    let connection_label = if matches!(p.connection, crate::plugins::Connection::Checking) {
        format!(
            "{} {}",
            crate::tools::spinner(app.animation_frame),
            p.connection_label()
        )
    } else {
        p.connection_label().into()
    };
    lines.push(Line::from(span(
        truncate(&connection_label, area.width),
        match p.connection {
            crate::plugins::Connection::Verified => t::ACCENT_SUCCESS,
            crate::plugins::Connection::Failed(_) => t::DIFF_DELETE_FG,
            _ => t::GRAY,
        },
    )));
    if app.mode == Mode::Live {
        lines.push(Line::from(span(
            truncate(p.storage_label(), area.width),
            t::GRAY,
        )));
        if let Some(error) = &p.storage_error {
            lines.push(Line::from(span(
                truncate(error, area.width),
                t::DIFF_DELETE_FG,
            )));
        }
    }
    lines.push(Line::default());
    let save_row = action(
        &mut lines,
        "Save settings",
        p.focus == SettingsFocus::Save,
        t::ACCENT_MODEL,
    );
    let remove_row = action(
        &mut lines,
        "Remove API key",
        p.focus == SettingsFocus::RemoveKey,
        t::DIFF_DELETE_FG,
    );
    let cancel_row = action(
        &mut lines,
        "Cancel (Esc)",
        p.focus == SettingsFocus::Cancel,
        t::TEXT_SECONDARY,
    );
    lines.push(Line::default());
    lines.push(Line::from(span(
        truncate("Tab Move between fields · Enter Select", area.width),
        t::GRAY,
    )));
    let focus_row = match p.focus {
        SettingsFocus::ApiKey => key_row,
        SettingsFocus::Model => model_row,
        SettingsFocus::TestKey => test_row,
        SettingsFocus::Save => save_row,
        SettingsFocus::RemoveKey => remove_row,
        SettingsFocus::Cancel => cancel_row,
    };
    render_fields(
        frame,
        area,
        lines,
        focus_row,
        cursor,
        app.model_picker.is_none(),
    );
}

fn jev_settings(frame: &mut Frame, area: Rect, app: &App) {
    let p = &app.plugins.bundled;
    let mut lines = vec![
        Line::from(span("Connection settings", t::ACCENT_MODEL)),
        detail("Endpoint", p.jev_endpoint(), area.width),
        Line::default(),
    ];
    let mut cursor = None;
    let key_row = field(
        &mut lines,
        "TypeSafe API key",
        p.field(true),
        p.focus == SettingsFocus::ApiKey,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        truncate(p.error.unwrap_or(p.key_label()), area.width),
        if p.error.is_some() {
            t::DIFF_DELETE_FG
        } else {
            t::GRAY
        },
    )));
    lines.push(Line::default());
    let model_row = field(
        &mut lines,
        "Jev model ID",
        p.field(false),
        p.focus == SettingsFocus::Model,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        truncate(
            &format!(
                "Default: {} · Typed decisions and probabilities",
                jev::defaults::MODEL
            ),
            area.width,
        ),
        t::GRAY,
    )));
    lines.push(Line::default());
    let test_row = action(
        &mut lines,
        "Test API key",
        p.focus == SettingsFocus::TestKey,
        t::ACCENT_SKILL,
    );
    let connection = if matches!(p.connection, crate::plugins::Connection::Checking) {
        format!(
            "{} {}",
            crate::tools::spinner(app.animation_frame),
            p.connection_label()
        )
    } else {
        p.connection_label().into()
    };
    lines.push(Line::from(span(
        truncate(&connection, area.width),
        match p.connection {
            crate::plugins::Connection::Verified => t::ACCENT_SUCCESS,
            crate::plugins::Connection::Failed(_) => t::DIFF_DELETE_FG,
            _ => t::GRAY,
        },
    )));
    if app.mode == Mode::Live {
        lines.push(Line::from(span(
            "Saved with Coder plugin settings",
            t::GRAY,
        )));
        if let Some(error) = &p.storage_error {
            lines.push(Line::from(span(
                truncate(error, area.width),
                t::DIFF_DELETE_FG,
            )));
        }
    }
    lines.push(Line::default());
    let save_row = action(
        &mut lines,
        "Save settings",
        p.focus == SettingsFocus::Save,
        t::ACCENT_MODEL,
    );
    let remove_row = action(
        &mut lines,
        "Remove API key",
        p.focus == SettingsFocus::RemoveKey,
        t::DIFF_DELETE_FG,
    );
    let cancel_row = action(
        &mut lines,
        "Cancel (Esc)",
        p.focus == SettingsFocus::Cancel,
        t::TEXT_SECONDARY,
    );
    lines.push(Line::default());
    lines.push(Line::from(span(
        truncate("Tab Move between fields · Enter Select", area.width),
        t::GRAY,
    )));
    let focus_row = match p.focus {
        SettingsFocus::ApiKey => key_row,
        SettingsFocus::Model => model_row,
        SettingsFocus::TestKey => test_row,
        SettingsFocus::Save => save_row,
        SettingsFocus::RemoveKey => remove_row,
        SettingsFocus::Cancel => cancel_row,
    };
    render_fields(
        frame,
        area,
        lines,
        focus_row,
        cursor,
        app.model_picker.is_none(),
    );
}

fn render_fields(
    frame: &mut Frame,
    area: Rect,
    lines: Vec<Line<'static>>,
    focus_row: u16,
    cursor: Option<(u16, u16)>,
    show_cursor: bool,
) {
    let max_scroll = (lines.len() as u16).saturating_sub(area.height);
    let scroll = focus_row
        .saturating_sub(area.height.saturating_sub(2))
        .min(max_scroll);
    frame.render_widget(Paragraph::new(Text::from(lines)).scroll((scroll, 0)), area);
    if let Some((column, row)) = cursor.filter(|(_, row)| {
        show_cursor && *row >= scroll && *row < scroll.saturating_add(area.height)
    }) {
        frame.set_cursor_position((
            area.x + column.min(area.width.saturating_sub(1)),
            area.y + row - scroll,
        ));
    }
}

fn acp_settings(frame: &mut Frame, area: Rect, app: &App) {
    let p = &app.plugins.bundled;
    let instructions = [
        "Local agents that speak ACP over stdio",
        "Paste a JSON array. Each agent needs id, name, and program.",
        "Optional: arguments, mode, enabled. Programs run locally.",
    ];
    let instruction_height = 3.min(area.height.saturating_sub(3));
    frame.render_widget(
        Paragraph::new(Text::from(
            instructions
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    Line::from(span(
                        truncate(text, area.width),
                        if index == 0 { t::ACCENT_MODEL } else { t::GRAY },
                    ))
                })
                .collect::<Vec<_>>(),
        )),
        Rect {
            height: instruction_height,
            ..area
        },
    );
    let error = p.acp_error.as_ref().or(p.storage_error.as_ref());
    let footer_height =
        (2 + u16::from(error.is_some())).min(area.height.saturating_sub(instruction_height + 2));
    let editor = Rect {
        y: area.y + instruction_height,
        height: area
            .height
            .saturating_sub(instruction_height + footer_height),
        ..area
    };
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE));
    let inner = block.inner(editor);
    frame.render_widget(block, editor);
    if inner.width > 0 && inner.height > 0 {
        let (rows, cursor) = p.acp_draft.wrapped(inner.width);
        let scroll = cursor.1.saturating_sub(inner.height.saturating_sub(1));
        frame.render_widget(
            Paragraph::new(Text::from(
                rows.into_iter()
                    .map(|row| Line::from(span(row, t::TEXT_PRIMARY)))
                    .collect::<Vec<_>>(),
            ))
            .scroll((scroll, 0)),
            inner,
        );
        if app.model_picker.is_none() {
            frame.set_cursor_position((
                inner.x + cursor.0.min(inner.width.saturating_sub(1)),
                inner.y + cursor.1.saturating_sub(scroll),
            ));
        }
    }
    let mut footer = vec![
        Line::from(span(
            truncate("Ctrl+S Save settings · Esc Cancel", area.width),
            t::GRAY_BRIGHT,
        )),
        Line::from(span(
            truncate(
                &format!(
                    "{} configured · Empty array removes all agents",
                    p.acp_agents.len()
                ),
                area.width,
            ),
            t::GRAY,
        )),
    ];
    if let Some(error) = error {
        footer.push(Line::from(span(
            truncate(error, area.width),
            t::DIFF_DELETE_FG,
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(footer)),
        Rect {
            y: editor.bottom(),
            height: footer_height,
            ..area
        },
    );
}

fn field(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    (text, cursor): (String, usize),
    focused: bool,
    width: u16,
    position: &mut Option<(u16, u16)>,
) -> u16 {
    lines.push(Line::from(span(
        truncate(label, width),
        if focused {
            t::TEXT_PRIMARY
        } else {
            t::GRAY_BRIGHT
        },
    )));
    let rule = "─".repeat(usize::from(width));
    let color = if focused {
        t::PROMPT_BORDER_ACTIVE
    } else {
        t::BG_LIGHT
    };
    lines.push(Line::from(span(rule.clone(), color)));
    let row = lines.len() as u16;
    let draft = Draft { text, cursor };
    let usable = width.saturating_sub(3).max(1);
    let (wrapped, location) = draft.wrapped(usable);
    let text = &wrapped[if focused { usize::from(location.1) } else { 0 }];
    // Show the cursor's segment when a long key or model exceeds the field width.
    let prefix = if focused { " ❯ " } else { "   " };
    lines.push(Line::from(vec![
        span(prefix, t::GRAY_BRIGHT),
        span(text, t::TEXT_PRIMARY),
    ]));
    lines.push(Line::from(span(rule, color)));
    if focused {
        *position = Some((3 + location.0.min(usable.saturating_sub(1)), row));
    }
    row
}

fn action(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    focused: bool,
    color: ratatui::style::Color,
) -> u16 {
    let row = lines.len() as u16;
    lines.push(Line::from(vec![
        span(if focused { "❯ " } else { "  " }, t::ACCENT_MODEL),
        Span::styled(
            label.to_owned(),
            Style::default().fg(color).add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ),
    ]));
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    struct Canvas {
        rows: Vec<String>,
        cursor: (u16, u16),
        cursor_visible: bool,
    }

    fn draw(app: &App, width: u16, height: u16) -> Canvas {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), app))
            .unwrap();
        let cursor = terminal.get_cursor_position().unwrap();
        let backend = terminal.backend();
        Canvas {
            rows: (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| backend.buffer()[(x, y)].symbol())
                        .collect()
                })
                .collect(),
            cursor: (cursor.x, cursor.y),
            cursor_visible: backend.cursor_visible(),
        }
    }

    fn select(app: &mut App, id: &str) {
        app.plugins.selected = DEFINITIONS
            .iter()
            .position(|definition| definition.id == id)
            .unwrap();
    }

    #[test]
    fn manager_shows_all_bundled_plugins_their_defaults_and_selection_details() {
        let mut app = App::default();
        app.screen = Screen::Plugins;
        let canvas = draw(&app, 110, 36);
        for definition in DEFINITIONS {
            let row = canvas
                .rows
                .iter()
                .find(|row| row.contains(definition.name))
                .unwrap();
            assert!(row.contains(if definition.default_enabled {
                "[ on  ]"
            } else {
                "[ off ]"
            }));
            assert!(row.contains(app.plugins.status_for(definition.id)));
        }
        let text = canvas.rows.join("\n");
        assert!(text.contains("Up/Down Select"));
        assert!(text.contains("Space Turn on/off"));
        assert!(text.contains("Enter Configure"));
        assert!(text.contains("Not configured"));
        select(&mut app, "jev");
        let text = draw(&app, 110, 36).rows.join("\n");
        assert!(text.contains("❯ Jev"));
        assert!(text.contains("Add your TypeSafe API key"));
        assert!(text.contains(app.plugins.bundled.jev_model()));
    }

    #[test]
    fn a_narrow_manager_keeps_each_selected_plugin_visible() {
        let mut app = App::default();
        app.screen = Screen::Plugins;
        for (index, definition) in DEFINITIONS.iter().enumerate() {
            app.plugins.selected = index;
            let canvas = draw(&app, 20, 10);
            let text = canvas.rows.join("\n");
            assert!(text.contains(&format!("❯ {}", definition.name)), "{text}");
            assert!(text.contains(if definition.default_enabled {
                "[ on  ]"
            } else {
                "[ off ]"
            }));
            assert!(!canvas.cursor_visible);
        }
    }

    #[test]
    fn jev_masks_keys_and_keeps_fields_and_actions_visible_after_resize() {
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        select(&mut app, "jev");
        app.plugins.bundled.begin_settings();
        app.plugins.bundled.paste("masking-probe-👩‍💻界");
        for focus in [
            SettingsFocus::ApiKey,
            SettingsFocus::Model,
            SettingsFocus::TestKey,
            SettingsFocus::Save,
            SettingsFocus::RemoveKey,
            SettingsFocus::Cancel,
        ] {
            app.plugins.bundled.focus = focus;
            for (width, height) in [(80, 24), (20, 10)] {
                let canvas = draw(&app, width, height);
                let text = canvas.rows.join("\n");
                assert!(text.contains("Jev"));
                assert!(!text.contains("masking-probe"));
                assert!(!text.contains("👩‍💻"));
                assert!(!text.contains('界'));
                if matches!(focus, SettingsFocus::ApiKey | SettingsFocus::Model) {
                    assert!(canvas.cursor_visible);
                    assert!(canvas.cursor.0 < width && canvas.cursor.1 < height);
                    if focus == SettingsFocus::ApiKey {
                        assert!(text.contains('•'));
                    }
                } else {
                    assert!(!canvas.cursor_visible);
                    let label = match focus {
                        SettingsFocus::TestKey => "Test API key",
                        SettingsFocus::Save => "Save settings",
                        SettingsFocus::RemoveKey => "Remove API key",
                        SettingsFocus::Cancel => "Cancel",
                        _ => unreachable!(),
                    };
                    assert!(text.contains(&format!("❯ {label}")), "{text}");
                }
            }
        }
    }

    #[test]
    fn acp_editor_starts_with_an_empty_array_and_tracks_a_long_cursor() {
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        select(&mut app, "acp-subagents");
        app.plugins.bundled.begin_acp();
        let canvas = draw(&app, 80, 24);
        assert!(canvas.rows.join("\n").contains("[]"));
        assert!(canvas.rows.join("\n").contains("Ctrl+S Save settings"));
        assert!(canvas.cursor_visible);
        app.plugins.bundled.acp_draft.insert(&"\n".repeat(40));
        app.plugins
            .bundled
            .acp_draft
            .insert("cursor follows this line");
        for (width, height) in [(80, 24), (20, 10)] {
            let canvas = draw(&app, width, height);
            assert!(canvas.cursor_visible);
            assert!(canvas.cursor.0 < width && canvas.cursor.1 < height);
            assert!(canvas.rows.iter().any(|row| row.contains("line")));
        }
    }

    #[test]
    fn bundled_tool_info_screens_name_the_registered_tools() {
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        for (id, tool) in [
            ("microcoder", "microcoder"),
            ("openagents-cli", "openagents_cli"),
        ] {
            select(&mut app, id);
            let canvas = draw(&app, 80, 24);
            assert!(canvas.rows[0].contains(app.plugins.selected_definition().name));
            assert!(canvas.rows.join("\n").contains(tool));
            assert!(!canvas.cursor_visible);
        }
    }
}
