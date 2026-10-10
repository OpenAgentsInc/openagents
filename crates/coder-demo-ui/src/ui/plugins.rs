//! Bundled plugin management and local configuration screens.

use unicode_width::UnicodeWidthStr;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
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
    let context_width = if area.width >= 42 { 17 } else { 0 };
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
                span("openagents", t::TEXT_PRIMARY),
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
            "boat-cloud" | "gce-cloud" => cloud_settings(frame, body, app),
            "acp-subagents" => acp_settings(frame, body, app),
            crate::brainstorm::PLUGIN => brainstorm_settings(frame, body, app),
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
                "A model you pick runs on OpenRouter, billed by OpenRouter. Auto runs on OpenAgents when you're signed in.",
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
                detail(
                    "Model",
                    if p.model == crate::models::DEFAULT_MODEL {
                        "auto"
                    } else {
                        &p.model
                    },
                    width,
                ),
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
                detail("Gateway", p.bundled.gateway_label(), width),
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
                    &format!("{} detected", p.bundled.acp_choices().len()),
                    width,
                ),
                Line::from(span(
                    "Installed agents appear automatically. Choose which to use.",
                    t::GRAY,
                )),
            ]);
        }
        crate::brainstorm::PLUGIN => {
            lines.push(Line::default());
            lines.extend([
                detail("Recipient", &p.bundled.brainstorm.preferences.origin, width),
                detail("Perspective", "Brainstorm house", width),
                Line::from(span(
                    "Explicit queries and public keys go to this recipient.",
                    t::GRAY,
                )),
                Line::from(span("Opening or enabling makes no service read.", t::GRAY)),
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

fn brainstorm_settings(frame: &mut Frame, area: Rect, app: &App) {
    use crate::brainstorm::Focus;
    let settings = &app.plugins.bundled.brainstorm;
    let mut lines = vec![
        Line::from(span("Brainstorm house perspective", t::ACCENT_MODEL)),
        detail("Recipient", &settings.preferences.origin, area.width),
        Line::from(span(
            "Explicit queries and public keys go to this HTTPS recipient.",
            t::GRAY,
        )),
        Line::from(span(
            "Opening, saving, or enabling makes no service read.",
            t::GRAY,
        )),
        Line::from(span(
            "This integration uses unsigned HTTP observations, not personal or signed scores.",
            t::GRAY,
        )),
        Line::default(),
    ];
    let mut cursor = None;
    let origin = field(
        &mut lines,
        "HTTPS origin",
        settings.field(),
        settings.focus == Focus::Origin,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        "Save a changed recipient before testing it. Enable from the plugin list.",
        t::GRAY,
    )));
    lines.push(Line::default());
    let test = action(
        &mut lines,
        "Test connection (public discovery)",
        settings.focus == Focus::Test,
        t::ACCENT_SKILL,
    );
    let status = if settings.fixture {
        "Demo fixture · no service read"
    } else {
        settings.status()
    };
    lines.push(detail("Connection", status, area.width));
    if let crate::plugins::Connection::Failed(error) = &settings.connection {
        lines.push(Line::from(span(error, t::DIFF_DELETE_FG)));
    }
    if let Some(discovery) = &settings.discovery {
        lines.push(detail("House key", &discovery.house.pubkey, area.width));
        lines.push(detail(
            "Discovered",
            &format!("{} ms since Unix epoch", discovery.house.discovered_at_ms),
            area.width,
        ));
        lines.push(Line::from(span(
            "The separately discovered key is not bound atomically to score responses.",
            t::GRAY,
        )));
    }
    if let Some(error) = &settings.error {
        lines.push(Line::from(span(error, t::DIFF_DELETE_FG)));
    }
    lines.push(Line::default());
    let save = action(
        &mut lines,
        "Save settings",
        settings.focus == Focus::Save,
        t::ACCENT_MODEL,
    );
    let cancel = action(
        &mut lines,
        "Cancel (Esc)",
        settings.focus == Focus::Cancel,
        t::TEXT_SECONDARY,
    );
    lines.push(Line::default());
    lines.push(Line::from(span(crate::brainstorm::USAGE, t::GRAY_BRIGHT)));
    lines.push(Line::from(span(
        "Tab Move between fields · Enter Select · Esc Cancel pending discovery",
        t::GRAY,
    )));
    let focus = match settings.focus {
        Focus::Origin => origin,
        Focus::Test => test,
        Focus::Save => save,
        Focus::Cancel => cancel,
    };
    render_fields(frame, area, lines, focus, cursor, true);
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
        truncate("Leave empty for auto · Use /models to choose.", area.width),
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
        SettingsFocus::Gateway | SettingsFocus::Endpoint => key_row,
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
    let mut lines = vec![Line::from(span("Connection settings", t::ACCENT_MODEL))];
    let mut cursor = None;
    let gateway_row = action(
        &mut lines,
        &format!("Gateway: {}", p.gateway_label()),
        p.focus == SettingsFocus::Gateway,
        t::ACCENT_MODEL,
    );
    lines.push(Line::from(span(
        truncate("Enter or Left/Right Select gateway", area.width),
        t::GRAY,
    )));
    lines.push(Line::default());
    let endpoint_row = field(
        &mut lines,
        "API base URL",
        p.endpoint_field(),
        p.focus == SettingsFocus::Endpoint,
        area.width,
        &mut cursor,
    );
    lines.push(Line::default());
    let key_row = field(
        &mut lines,
        "Gateway API key",
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
                p.default_model()
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
        SettingsFocus::Gateway => gateway_row,
        SettingsFocus::Endpoint => endpoint_row,
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
    let agents = p.acp_choices();
    frame.render_widget(
        Paragraph::new(span("Detected on this computer", t::GRAY)),
        Rect { height: 1, ..area },
    );
    let mut footer = vec![Line::from(span(
        truncate(
            &format!(
                "{} detected · {} on",
                agents.len(),
                agents.iter().filter(|agent| agent.enabled).count()
            ),
            area.width,
        ),
        t::GRAY,
    ))];
    let hints: &[&str] = if area.width >= 66 {
        &["↑/↓ Select · Space/Enter Toggle · R Refresh · Esc Back"]
    } else {
        &["↑/↓ Select · Space/Enter Toggle", "R Refresh · Esc Back"]
    };
    footer.extend(
        hints
            .iter()
            .map(|hint| Line::from(span(truncate(hint, area.width), t::GRAY_BRIGHT))),
    );
    if let Some(error) = &p.storage_error {
        footer.push(Line::from(span(
            truncate(error, area.width),
            t::DIFF_DELETE_FG,
        )));
    }
    let footer_height = (footer.len() as u16).min(area.height.saturating_sub(3));
    let list = Rect {
        y: area.y + 2,
        height: area.height.saturating_sub(footer_height + 2),
        ..area
    };
    if agents.is_empty() {
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(span("No ACP agents detected.", t::TEXT_PRIMARY)),
                Line::default(),
                Line::from(span(
                    truncate("Install an ACP agent, then press R to refresh.", area.width),
                    t::GRAY,
                )),
            ])),
            list,
        );
    } else {
        let rows: Vec<_> = agents
            .iter()
            .enumerate()
            .map(|(index, agent)| {
                let selected = index == p.acp_selected;
                let mut spans = vec![
                    span(if selected { "❯ " } else { "  " }, t::ACCENT_MODEL),
                    span(
                        if agent.enabled { "[x] " } else { "[ ] " },
                        if agent.enabled {
                            t::ACCENT_MODEL
                        } else {
                            t::GRAY
                        },
                    ),
                    Span::styled(
                        if area.width >= 56 {
                            format!("{:<24}", truncate(&agent.name, 23))
                        } else {
                            truncate(&agent.name, area.width.saturating_sub(6))
                        },
                        Style::default()
                            .fg(t::TEXT_PRIMARY)
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                ];
                if area.width >= 56 {
                    spans.push(span(
                        truncate(
                            &agent
                                .program
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            area.width.saturating_sub(30),
                        ),
                        t::GRAY,
                    ));
                }
                let row = Line::from(spans);
                if selected {
                    row.style(Style::default().bg(t::BG_DARK))
                } else {
                    row
                }
            })
            .collect();
        let scroll = (p.acp_selected as u16).saturating_sub(list.height.saturating_sub(1));
        frame.render_widget(Paragraph::new(Text::from(rows)).scroll((scroll, 0)), list);
    }
    frame.render_widget(
        Paragraph::new(Text::from(footer)),
        Rect {
            y: list.bottom(),
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

fn cloud_settings(frame: &mut Frame, area: Rect, app: &App) {
    let Some(e) = &app.plugins.bundled.cloud_editor else {
        return;
    };
    let mode = if e.config.mode == crate::cloud_settings::Mode::Coder {
        "Coder runtime"
    } else {
        "Integrated agent"
    };
    let size = if e.placement == crate::cloud_settings::Placement::Gce {
        "Granted pool shape"
    } else {
        &e.config.size
    };
    let mut rows = vec![
        Line::from(span(
            "Credentials are selected by variable name. Values stay private.",
            t::GRAY,
        )),
        Line::from(""),
    ];
    let fields = [
        ("Mode", mode),
        ("Machine size", size),
        ("Template", e.template.text.as_str()),
        ("Credential variables", e.credentials.text.as_str()),
        ("Workspace paths", e.paths.text.as_str()),
        ("Save", ""),
        ("Cancel", ""),
    ];
    for (i, (name, value)) in fields.iter().enumerate() {
        rows.push(Line::from(span(
            format!("{} {name}: {value}", if e.focus == i { "❯" } else { " " }),
            if e.focus == i {
                t::TEXT_PRIMARY
            } else {
                t::TEXT_SECONDARY
            },
        )));
    }
    rows.push(Line::from(""));
    rows.push(Line::from(span(
        "Enter/Space: change choice · Tab: next · comma-separated names and paths",
        t::GRAY,
    )));
    if let Some(error) = &e.error {
        rows.push(Line::from(span(error, t::TEXT_PRIMARY)));
    }
    let at = if e.focus + 2 >= area.height as usize {
        (e.focus + 3).saturating_sub(area.height as usize)
    } else {
        0
    };
    frame.render_widget(Paragraph::new(rows).scroll((at as u16, 0)), area);
    let draft = match e.focus {
        2 => Some(&e.template),
        3 => Some(&e.credentials),
        4 => Some(&e.paths),
        _ => None,
    };
    if let Some(d) = draft {
        let label = fields[e.focus].0;
        let x = (4 + label.chars().count() + d.text[..d.cursor].width())
            .min(area.width.saturating_sub(1) as usize) as u16;
        let y = (e.focus + 2).saturating_sub(at) as u16;
        if y < area.height {
            frame.set_cursor_position((area.x + x, area.y + y));
        }
    }
}
