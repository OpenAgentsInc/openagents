//! Bundled and installed plugin management and local configuration screens.

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
    plugins::{ENDPOINT, SettingsFocus},
    theme as t,
};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.screen == Screen::PluginSettings {
        app.plugins.selected_definition().name
    } else {
        "Plugins"
    };
    // The folder and branch Coder is working in (the demo, with no folder,
    // shows its sample repository).
    let (folder, branch) = match &app.cwd {
        Some(cwd) => (
            cwd.file_name()
                .map(|name| display_text(&name.to_string_lossy()))
                .unwrap_or_default(),
            app.branch.as_deref().map(display_text),
        ),
        None => ("openagents".to_owned(), Some("main".to_owned())),
    };
    let branch = branch
        .map(|branch| format!(" / {branch}"))
        .unwrap_or_default();
    let context = (folder.width() + branch.width()) as u16;
    let context_width = if area.width >= 42 && !folder.is_empty() {
        context.min(area.width / 2)
    } else {
        0
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(
                &display_text(title),
                area.width.saturating_sub(context_width + 2),
            ),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Rect { height: 1, ..area },
    );
    if context_width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                span(truncate(&folder, context_width), t::TEXT_PRIMARY),
                span(
                    truncate(&branch, context_width.saturating_sub(folder.width() as u16)),
                    t::GRAY,
                ),
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
    let definition_count = p.definitions().count();
    let wide = area.width >= 56;
    let mut rows = Vec::new();
    if wide {
        rows.push(Line::from(span(
            format!("  {:<30}{:<12}Status", "Plugin", "Enabled"),
            t::GRAY,
        )));
    }
    for (index, definition) in p.definitions().enumerate() {
        let name = display_text(definition.name);
        let selected = index == p.selected.min(definition_count - 1);
        let enabled = p.enabled_for(definition.id);
        // A row that cannot be switched here shows its state, not a switch.
        let state = match (fixed_row(app, definition.id), enabled) {
            (false, true) => "[ on  ]",
            (false, false) => "[ off ]",
            (true, true) => "on",
            (true, false) => "off",
        };
        let status = p.status_for(definition.id);
        let mut row = Line::from(vec![
            span(if selected { "❯ " } else { "  " }, t::ACCENT_MODEL),
            Span::styled(
                if wide {
                    format!("{:<30}", truncate(&name, 29))
                } else {
                    truncate(&name, area.width.saturating_sub(2))
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
    let hints = if selected_is_installed(app) && fixed_row(app, p.selected_definition().id) {
        if area.width >= 44 {
            vec!["Up/Down Select · Enter Details · Esc Back"]
        } else if area.width >= 28 {
            vec!["Up/Down Select", "Enter Details · Esc Back"]
        } else {
            vec!["Up/Down Select", "Enter Details", "Esc Back"]
        }
    } else {
        let enter = if selected_is_installed(app) {
            "Enter Details"
        } else {
            "Enter Configure"
        };
        let (wide_hint, second) = if selected_is_installed(app) {
            (
                "Up/Down Select · Space Turn on/off · Enter Details · Esc Back",
                "Enter Details · Esc Back",
            )
        } else {
            (
                "Up/Down Select · Space Turn on/off · Enter Configure · Esc Back",
                "Enter Configure · Esc Back",
            )
        };
        if area.width >= 64 {
            vec![wide_hint]
        } else if area.width >= 28 {
            vec!["Up/Down Select · Space Turn on/off", second]
        } else {
            vec!["Up/Down Select", "Space Turn on/off", enter, "Esc Back"]
        }
    };
    let list_height =
        (rows.len() as u16).min(area.height.saturating_sub(hints.len() as u16 + 1).max(1));
    let selected = p.selected.min(definition_count - 1) as u16;
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

fn selected_is_installed(app: &App) -> bool {
    #[cfg(unix)]
    return app.plugins.selected_installed().is_some();
    #[cfg(not(unix))]
    {
        let _ = app;
        false
    }
}

/// Whether `id` is an installed plugin this screen cannot turn on or off.
fn fixed_row(app: &App, id: &str) -> bool {
    #[cfg(unix)]
    return app.plugins.installed().iter().any(|plugin| plugin.id == id)
        && !app.plugins.installed_toggles();
    #[cfg(not(unix))]
    {
        let _ = (app, id);
        false
    }
}

/// Who published an installed plugin, in words: "Local" for one installed
/// from a folder here, otherwise a short npub.
#[cfg(unix)]
fn publisher(plugin: &background::plugins::Installed) -> String {
    let key = plugin.id.split_once(':').map_or("", |(key, _)| key);
    if key == background::plugins::LOCAL_KEY {
        return "Local".into();
    }
    let bytes: Option<Vec<u8>> = (0..32)
        .map(|index| {
            key.get(index * 2..index * 2 + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect();
    let Some(bytes) = bytes
        .filter(|_| key.len() == 64)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
    else {
        return "Unknown".into();
    };
    let npub = nostr::nip19::encode_npub(&bytes);
    format!("{}…{}", &npub[..12], &npub[npub.len() - 4..])
}

fn display_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn status_color(status: &str) -> ratatui::style::Color {
    match status {
        "Disabled" => t::GRAY,
        "Setup required" | "Checking" | "Update available" => t::COMMAND,
        crate::engine_update::UPDATE_REQUIRED => t::DIFF_DELETE_FG,
        "Unavailable" => t::DIFF_DELETE_FG,
        _ => t::ACCENT_SUCCESS,
    }
}

fn plugin_details(app: &App, width: u16) -> Vec<Line<'static>> {
    let p = &app.plugins;
    let definition = p.selected_definition();
    let mut lines = vec![
        Line::from(span(
            display_text(if definition.id == OPENROUTER_PLUGIN {
                "Model provider"
            } else {
                definition.name
            }),
            t::ACCENT_MODEL,
        )),
        Line::from(span(
            display_text(definition.description),
            t::TEXT_SECONDARY,
        )),
    ];
    #[cfg(unix)]
    if let Some(plugin) = p.selected_installed() {
        let local = plugin.id.starts_with(background::plugins::LOCAL_KEY);
        lines.push(Line::default());
        lines.extend([
            detail("Publisher", &publisher(plugin), width),
            detail("Version", &display_text(&plugin.version), width),
            detail(
                "From",
                if local {
                    "A folder on this computer"
                } else {
                    "The plugin catalog"
                },
                width,
            ),
        ]);
        if !p.installed_toggles() {
            lines.push(Line::default());
            lines.push(Line::from(span(
                "Can't turn this on or off here: Coder can't find its plugin folder.",
                t::GRAY,
            )));
        }
        if plugin.enabled
            && let Some((_, note)) = p.installed_note.as_ref().filter(|(id, _)| *id == plugin.id)
        {
            lines.push(Line::default());
            lines.push(Line::from(span(display_text(note), t::COMMAND)));
        }
        if let Some(error) = &p.installed_error {
            lines.push(Line::from(span(display_text(error), t::DIFF_DELETE_FG)));
        }
        return lines;
    }
    match definition.id {
        OPENROUTER_PLUGIN => {
            lines.push(Line::from(span(
                "A model you pick runs on OpenRouter, billed by OpenRouter. Auto runs on OpenAgents when you're logged in.",
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
                detail("Model", &crate::models::label(&p.model), width),
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
                detail("Provider", "Your model login here, or OpenRouter", width),
                detail("Working folder", "Current checkout", width),
            ]);
        }
        "openagents-cli" => {
            lines.push(Line::default());
            lines.extend([
                detail("Command", "openagents", width),
                Line::from(span(
                    "Installed with Coder. Run openagents --help to see its commands.",
                    t::GRAY,
                )),
            ]);
        }
        "acp-subagents" => {
            lines.push(Line::default());
            lines.extend([
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
            for agent in p.bundled.acp_choices() {
                if let Some(sentence) = p
                    .bundled
                    .engines
                    .state(&agent.id)
                    .filter(|state| matches!(state, crate::engine_update::State::Refused { .. }))
                    .and_then(|state| state.sentence())
                {
                    lines.push(Line::from(span(
                        sentence.replace("Press U", "Press Enter, select it, then press U"),
                        t::COMMAND,
                    )));
                }
            }
        }
        crate::brainstorm::PLUGIN => {
            lines.push(Line::default());
            lines.extend([
                detail("Recipient", &p.bundled.brainstorm.preferences.origin, width),
                detail("Perspective", "Brainstorm house", width),
                Line::from(span(
                    "Only what you type after /brainstorm goes to this website.",
                    t::GRAY,
                )),
                Line::from(span("Turning it on sends nothing.", t::GRAY)),
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
        if fixed_row(app, app.plugins.selected_definition().id) {
            "Esc Back"
        } else {
            "Esc Back · Turn on/off from the plugin list"
        },
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
            &format!("{} ms (Unix time)", discovery.house.discovered_at_ms),
            area.width,
        ));
        lines.push(Line::from(span(
            "Scores are not proven to come from this key.",
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

/// What Coder's checks cover on an ACP agent, for the person (#11508).
fn acp_checks_line(agent: &crate::bundled_runtime::AcpAgent) -> String {
    if coder_sdk::acp::gated(&agent.id) {
        format!(
            "Coder checks every step {} takes before it runs.",
            agent.name
        )
    } else {
        format!(
            "{} doesn't ask before every step, so Coder checks only the steps it asks about.",
            agent.name
        )
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
    let updatable = p.selected_updatable();
    let hints: &[&str] = match (updatable.is_some(), area.width >= 76, area.width >= 66) {
        (true, true, _) => &["↑/↓ Select · Space/Enter Toggle · U Update · R Refresh · Esc Back"],
        (true, false, _) => &[
            "↑/↓ Select · Space/Enter Toggle",
            "U Update · R Refresh · Esc Back",
        ],
        (false, _, true) => &["↑/↓ Select · Space/Enter Toggle · R Refresh · Esc Back"],
        (false, _, false) => &["↑/↓ Select · Space/Enter Toggle", "R Refresh · Esc Back"],
    };
    footer.extend(
        hints
            .iter()
            .map(|hint| Line::from(span(truncate(hint, area.width), t::GRAY_BRIGHT))),
    );
    // How far Coder's checks reach on the selected agent (#11508), before
    // the person delegates to it.
    if let Some(agent) = agents.get(p.acp_selected)
        && agent.transport == crate::bundled_runtime::AgentTransport::Acp
    {
        let text = acp_checks_line(agent);
        for range in coder_terminal::wrap_rows(&text, usize::from(area.width).max(1)) {
            footer.push(Line::from(span(text[range].trim().to_owned(), t::GRAY)));
        }
    }
    if let Some(agent) = updatable {
        let (text, color) = if p.engines.updating.as_deref() == Some(agent.id.as_str()) {
            (format!("Updating {}…", agent.name), t::COMMAND)
        } else if let Some(message) = &p.engines.message {
            (message.clone(), t::TEXT_SECONDARY)
        } else if let Some(sentence) = p
            .engines
            .state(&agent.id)
            .and_then(|state| state.sentence())
        {
            (sentence, t::COMMAND)
        } else {
            (String::new(), t::GRAY)
        };
        for range in coder_terminal::wrap_rows(&text, usize::from(area.width).max(1)) {
            footer.push(Line::from(span(text[range].trim().to_owned(), color)));
        }
    }
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
                let label = p.engines.state(&agent.id).and_then(|state| state.label());
                if let Some(label) = label {
                    spans.push(span(format!(" {label}"), status_color(label)));
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
    let draft = Draft {
        text,
        cursor,
        ..Default::default()
    };
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
    use crate::plugin_definition::DEFINITIONS;
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
        let selected = app
            .plugins
            .definitions()
            .position(|definition| definition.id == id)
            .unwrap();
        app.plugins.selected = selected;
    }

    #[cfg(unix)]
    fn installed(index: usize) -> background::plugins::Installed {
        background::plugins::Installed {
            id: format!("{}:fixture-{index}", background::plugins::LOCAL_KEY),
            slug: format!("fixture-{index}"),
            name: format!("Installed {index}"),
            summary: format!("Installed fixture {index} summary."),
            version: "1.2.3".into(),
            dir: std::path::PathBuf::from(format!("/fixture/extensions/fixture-{index}/1.2.3")),
            background: vec![],
            classes: vec![],
            enabled: index % 2 == 0,
        }
    }

    #[cfg(unix)]
    #[test]
    fn installed_rows_and_statuses_appear_when_the_open_manager_refreshes() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.screen = Screen::Plugins;
        assert!(!draw(&app, 110, 36).rows.join("\n").contains("Installed 0"));

        app.plugins
            .replace_installed(vec![installed(0), installed(1)]);
        let canvas = draw(&app, 110, 36);
        let enabled = canvas
            .rows
            .iter()
            .find(|row| row.contains("Installed 0"))
            .unwrap();
        assert!(!enabled.contains("[ on  ]"));
        assert!(enabled.contains("Enabled"));
        let disabled = canvas
            .rows
            .iter()
            .find(|row| row.contains("Installed 1"))
            .unwrap();
        assert!(!disabled.contains("[ off ]"));
        assert!(disabled.contains("Disabled"));

        // Without its plugin folder a row is no switch, and says why.
        select(&mut app, &installed(1).id);
        let text = draw(&app, 110, 36).rows.join("\n");
        assert!(text.contains("❯ Installed 1"));
        assert!(text.contains("Enter Details"));
        assert!(!text.contains("Space Turn on/off"));
        assert!(!text.contains("Enter Configure"));
        assert!(text.contains("Can't turn this on or off here"));

        let home = tempfile::tempdir().unwrap();
        app.plugins
            .use_extensions(background::Layout::new(home.path(), None).unwrap());
        let text = draw(&app, 110, 36).rows.join("\n");
        assert!(text.contains("Space Turn on/off · Enter Details"));
        let row = draw(&app, 110, 36)
            .rows
            .into_iter()
            .find(|row| row.contains("Installed 1"))
            .unwrap();
        assert!(row.contains("[ off ]"));
        assert!(!text.contains("Can't turn this on or off here"));
    }

    #[cfg(unix)]
    #[test]
    fn a_long_installed_catalog_keeps_the_selected_row_visible_after_resize() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.screen = Screen::Plugins;
        app.plugins
            .replace_installed((0..24).map(installed).collect());
        select(&mut app, &installed(23).id);

        for (width, height) in [(110, 16), (20, 10)] {
            let canvas = draw(&app, width, height);
            let text = canvas.rows.join("\n");
            assert!(text.contains("❯ Installed 23"), "{text}");
            assert!(text.contains("off"), "{text}");
            assert!(text.contains("Enter Details"), "{text}");
            assert!(!canvas.cursor_visible);
        }
    }

    #[cfg(unix)]
    #[test]
    fn installed_details_speak_plainly_without_keys_or_folders() {
        let plugin = installed(0);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.plugins.replace_installed(vec![plugin.clone()]);
        select(&mut app, &plugin.id);
        app.screen = Screen::PluginSettings;

        let canvas = draw(&app, 160, 24);
        let text = canvas.rows.join("\n");
        assert!(canvas.rows[0].contains(&plugin.name));
        assert!(text.contains(&plugin.summary));
        assert!(text.contains(&plugin.version));
        assert!(text.contains("Publisher") && text.contains("Local"));
        assert!(text.contains("A folder on this computer"));
        assert!(!text.contains(background::plugins::LOCAL_KEY));
        assert!(!text.contains(&"0".repeat(64)));
        assert!(!text.contains("/fixture/extensions"));
        assert!(!text.contains(&plugin.id));
        assert!(!canvas.cursor_visible);

        // A published plugin names its publisher by a short npub.
        let mut published = installed(2);
        published.id = format!("{}:fixture-2", "0b".repeat(32));
        app.plugins.replace_installed(vec![published.clone()]);
        select(&mut app, &published.id);
        let text = draw(&app, 160, 24).rows.join("\n");
        assert!(text.contains("npub1"), "{text}");
        assert!(text.contains("The plugin catalog"));
        assert!(!text.contains(&"0b".repeat(32)));
    }

    #[cfg(unix)]
    #[test]
    fn installed_metadata_does_not_emit_terminal_control_characters() {
        let mut plugin = installed(0);
        plugin.name = "Installed\u{1b}[31m 0".into();
        plugin.summary = "Description\n\u{1b}]52;c;probe\u{7}".into();
        plugin.version = "1.2.3\u{1b}[0m".into();
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.plugins.replace_installed(vec![plugin]);
        app.plugins.selected = DEFINITIONS.len();
        assert!(
            plugin_details(&app, 160)
                .iter()
                .flat_map(|line| &line.spans)
                .all(|span| { !span.content.chars().any(char::is_control) })
        );
        for screen in [Screen::Plugins, Screen::PluginSettings] {
            app.screen = screen;
            assert!(
                draw(&app, 160, 24)
                    .rows
                    .iter()
                    .all(|row| !row.chars().any(char::is_control))
            );
        }
    }

    /// Whether a bundled plugin's row reads on: its default, except that
    /// Claude Code and Codex are on only where they are signed in, as the
    /// settings decide (`bundled_settings`). A test's computer may hold
    /// neither sign-in.
    fn shows_on(definition: &crate::plugin_definition::PluginDefinition) -> bool {
        definition.default_enabled
            && match definition.id {
                crate::claude::PLUGIN => crate::claude::available(),
                crate::codex::PLUGIN => crate::codex::available(),
                _ => true,
            }
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
            let on = shows_on(definition);
            assert!(
                row.contains(if on { "[ on  ]" } else { "[ off ]" }),
                "{}: {row}",
                definition.id
            );
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
        assert!(text.contains("Add the API key for this gateway"));
        assert!(text.contains("TypeSafe direct"));
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
            assert!(
                text.contains(if shows_on(definition) {
                    "[ on  ]"
                } else {
                    "[ off ]"
                }),
                "{text}"
            );
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
            SettingsFocus::Gateway,
            SettingsFocus::Endpoint,
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
                if matches!(
                    focus,
                    SettingsFocus::ApiKey | SettingsFocus::Model | SettingsFocus::Endpoint
                ) {
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
                        SettingsFocus::Gateway => "Gateway: TypeSafe",
                        _ => unreachable!(),
                    };
                    assert!(text.contains(&format!("❯ {label}")), "{text}");
                }
            }
        }
    }

    #[test]
    fn acp_picker_shows_an_empty_state_without_a_text_editor() {
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        select(&mut app, "acp-subagents");
        app.plugins.bundled.begin_acp();
        let canvas = draw(&app, 80, 24);
        let text = canvas.rows.join("\n");
        assert!(text.contains("No ACP agents detected."));
        assert!(text.contains("R Refresh"));
        assert!(!text.contains("JSON"));
        assert!(!text.contains("[]"));
        assert!(!canvas.cursor_visible);
    }

    #[test]
    fn acp_picker_keeps_the_selected_checkbox_visible_after_resize() {
        let temporary = tempfile::tempdir().unwrap();
        let program = temporary
            .path()
            .join(if cfg!(windows) { "agent.exe" } else { "agent" });
        std::fs::write(&program, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        select(&mut app, "acp-subagents");
        app.plugins.bundled.acp_agents = (0..24)
            .map(|index| crate::bundled_runtime::AcpAgent {
                id: format!("fixture-{index}"),
                name: format!("Agent {index}"),
                program: program.clone(),
                transport: Default::default(),
                arguments: vec![],
                mode: None,
                enabled: true,
            })
            .collect();
        app.plugins.bundled.begin_acp();
        assert_eq!(app.plugins.bundled.acp_choices().len(), 24);
        app.plugins.bundled.acp_selected = 23;
        assert!(app.plugins.bundled.toggle_acp_agent());
        for (width, height) in [(80, 24), (20, 10)] {
            let canvas = draw(&app, width, height);
            assert!(!canvas.cursor_visible);
            assert!(canvas.rows.iter().any(|row| row.contains("❯ [ ] Agent 23")));
        }
    }

    #[test]
    #[cfg(unix)]
    fn cursor_shows_when_it_needs_an_update_and_offers_the_update() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = tempfile::tempdir().unwrap();
        let program = temporary.path().join("cursor-agent");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut app = App::default();
        app.plugins.bundled.set_live(true);
        app.plugins.bundled.discover_acp(&|name| {
            (name == acp_client::cursor::BIN_VAR).then(|| program.clone().into_os_string())
        });
        let cursor = app
            .plugins
            .bundled
            .acp_choices()
            .iter()
            .position(|agent| agent.id == "cursor")
            .unwrap();
        app.plugins.bundled.acp_selected = cursor;
        app.plugins.bundled.engines.set(
            "cursor",
            crate::engine_update::State::Refused {
                installed: "2026.06.24-00-45-58-9f61de7".into(),
            },
        );
        app.screen = Screen::Plugins;
        select(&mut app, "acp-subagents");
        let list = draw(&app, 100, 30).rows.join("\n");
        assert!(list.contains("Update required"), "{list}");
        assert!(list.contains("cursor-agent update"), "{list}");
        app.screen = Screen::PluginSettings;
        let text = draw(&app, 100, 30).rows.join("\n");
        // How far Coder's checks reach on it, said before delegating
        // (#11508).
        assert!(
            text.contains("Cursor doesn't ask before every step, so Coder checks only the steps it asks about."),
            "{text}"
        );
        assert!(text.contains("Cursor"), "{text}");
        assert!(text.contains("Update required"), "{text}");
        assert!(text.contains("U Update"), "{text}");
        let flowing = draw(&app, 100, 30)
            .rows
            .iter()
            .map(|row| row.trim())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            flowing.contains("Press U to update it, or run `cursor-agent update`"),
            "{text}"
        );
        // The hint is there only where U does something.
        app.plugins.bundled.set_live(false);
        let demo = draw(&app, 100, 30).rows.join("\n");
        assert!(!demo.contains("U Update"), "{demo}");
    }

    #[test]
    fn the_header_names_the_folder_and_branch_coder_is_in() {
        let mut app = App::default();
        app.screen = Screen::Plugins;
        app.cwd = Some("/tmp/work/slugkit".into());
        app.branch = Some("main".into());
        let canvas = draw(&app, 80, 24);
        assert!(
            canvas.rows[0].trim_end().ends_with("slugkit / main"),
            "{}",
            canvas.rows[0]
        );
        assert!(!canvas.rows[0].contains("openagents"));
    }

    #[test]
    fn bundled_tool_info_screens_speak_plainly() {
        let mut app = App::default();
        app.screen = Screen::PluginSettings;
        // The pages describe the plugin in words; the model's tool names
        // (microcoder, openagents_cli) are not for the person.
        for (id, tool, shown) in [
            ("microcoder", "microcoder", "Current checkout"),
            ("openagents-cli", "openagents_cli", "openagents --help"),
        ] {
            select(&mut app, id);
            let canvas = draw(&app, 80, 24);
            assert!(canvas.rows[0].contains(app.plugins.selected_definition().name));
            let screen = canvas.rows.join("\n");
            assert!(screen.contains(shown), "{screen}");
            assert!(!screen.contains(tool), "{screen}");
            assert!(!canvas.cursor_visible);
        }
    }
}

fn cloud_settings(frame: &mut Frame, area: Rect, app: &App) {
    let Some(e) = &app.plugins.bundled.cloud_editor else {
        return;
    };
    let mode = if e.config.mode == coder_cloud::Mode::Coder {
        "Coder runtime"
    } else {
        "Integrated agent"
    };
    let size = if e.placement == coder_cloud::Placement::Gce {
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
