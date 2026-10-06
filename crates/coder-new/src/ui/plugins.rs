//! Plugin management screens for the local presentation fixture.

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
    plugins::{ENDPOINT, SettingsFocus},
    theme as t,
};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.screen == Screen::PluginSettings {
        "OpenRouter BYOK"
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
        settings(frame, body, app);
    }
}

fn manager(frame: &mut Frame, area: Rect, app: &App) {
    let p = &app.plugins;
    let enabled = if p.enabled { "[ on  ]" } else { "[ off ]" };
    let status_color = match (p.enabled, p.key_configured) {
        (true, true)
            if app.mode == Mode::Live
                && matches!(p.connection, crate::plugins::Connection::Failed(_)) =>
        {
            t::DIFF_DELETE_FG
        }
        (true, true) => t::ACCENT_SUCCESS,
        (true, false) => t::COMMAND,
        _ => t::GRAY,
    };
    let mut lines = if area.width >= 56 {
        vec![
            Line::from(span(
                format!("  {:<30}{:<12}Status", "Plugin", "Enabled"),
                t::GRAY,
            )),
            Line::from(vec![
                span("❯ ", t::ACCENT_MODEL),
                Span::styled(
                    format!("{:<30}", "OpenRouter BYOK"),
                    Style::default()
                        .fg(t::TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
                span(
                    format!("{enabled:<12}"),
                    if p.enabled {
                        t::ACCENT_MODEL
                    } else {
                        t::GRAY_BRIGHT
                    },
                ),
                span(p.status(), status_color),
            ])
            .style(Style::default().bg(t::BG_DARK)),
        ]
    } else {
        vec![
            Line::from(span("❯ OpenRouter BYOK", t::TEXT_PRIMARY))
                .style(Style::default().bg(t::BG_DARK)),
            Line::from(vec![
                span(format!("  {enabled}  "), t::ACCENT_MODEL),
                span(p.status(), status_color),
            ]),
        ]
    };
    lines.extend([
        Line::default(),
        Line::from(span(
            "Space Turn on/off    Enter Configure    Esc Back",
            t::GRAY_BRIGHT,
        )),
        Line::default(),
        Line::from(span("Model provider", t::ACCENT_MODEL)),
        Line::from(span(
            "Use OpenRouter models with your own OpenRouter API key.",
            t::TEXT_SECONDARY,
        )),
        Line::from(span(
            "Requests go directly to OpenRouter. Billed by OpenRouter.",
            t::GRAY,
        )),
        Line::default(),
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
            area.width,
        ),
        detail(
            "Model",
            if p.model.is_empty() {
                "Account default"
            } else {
                &p.model
            },
            area.width,
        ),
        detail("Endpoint", ENDPOINT, area.width),
        detail("Connection", p.connection_label(), area.width),
    ]);
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

fn settings(frame: &mut Frame, area: Rect, app: &App) {
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
        "Model ID (optional)",
        p.field(false),
        p.focus == SettingsFocus::Model,
        area.width,
        &mut cursor,
    );
    lines.push(Line::from(span(
        truncate(
            "Leave blank to use your OpenRouter account default.",
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
            truncate("The key stays in memory until you quit.", area.width),
            t::GRAY,
        )));
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
    let max_scroll = (lines.len() as u16).saturating_sub(area.height);
    let scroll = focus_row
        .saturating_sub(area.height.saturating_sub(2))
        .min(max_scroll);
    frame.render_widget(Paragraph::new(Text::from(lines)).scroll((scroll, 0)), area);
    if let Some((column, row)) = cursor {
        frame.set_cursor_position((area.x + column, area.y + row - scroll));
    }
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
