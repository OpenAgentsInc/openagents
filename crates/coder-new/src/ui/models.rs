//! Model and generation settings picker for enabled provider plugins.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::{span, truncate};
use crate::{
    App,
    models::{Model, Picker, Stage},
    theme as t,
};

pub(super) fn render(frame: &mut Frame, app: &App) {
    let Some(picker) = app.model_picker.as_ref() else {
        return;
    };
    let area = frame.area();
    if area.width < 4 || area.height < 4 {
        return;
    }
    let rows = choices(picker);
    let model = match picker.stage {
        Stage::Models => picker.matching().get(picker.selected).copied(),
        Stage::Reasoning | Stage::Output => picker.pending.as_ref(),
    };
    let status = if let Some(error) = &picker.error {
        Some((error.clone(), t::DIFF_DELETE_FG))
    } else if picker.loading {
        Some((
            format!(
                "{} Refreshing model details",
                crate::tools::spinner(app.animation_frame)
            ),
            t::ACCENT_SKILL,
        ))
    } else {
        None
    };
    let search_rows = u16::from(picker.stage == Stage::Models);
    let status_rows = u16::from(status.is_some());
    let detail_rows = u16::from(model.is_some()) * 3;
    let wanted_height =
        (rows.len().max(1) as u16).saturating_add(search_rows + status_rows + detail_rows + 4);
    let width = (area.width / 2)
        .clamp(44, 80)
        .min(area.width.saturating_sub(2));
    let height = wanted_height.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let title = match picker.stage {
        Stage::Models => "Pick model",
        Stage::Reasoning => "Pick reasoning level",
        Stage::Output => "Maximum output tokens",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t::PROMPT_BORDER_ACTIVE))
        .style(Style::default().bg(t::BG_BASE).fg(t::TEXT_SECONDARY))
        .title(Span::styled(
            format!(" {} ", truncate(title, width.saturating_sub(4))),
            Style::default()
                .fg(t::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut y = inner.y;
    if search_rows > 0 {
        search(
            frame,
            Rect {
                y,
                height: 1,
                ..inner
            },
            picker,
        );
        y += 1;
    }
    if let Some((message, color)) = status {
        line(
            frame,
            Rect {
                y,
                height: 1,
                ..inner
            },
            &message,
            color,
        );
        y += 1;
    }

    let available = inner.bottom().saturating_sub(y);
    let details_height = if model.is_some() {
        available.saturating_sub(2).min(3)
    } else {
        0
    };
    let gap = u16::from(available >= details_height + 5);
    let list_height = available.saturating_sub(details_height + gap + 1);
    let selected = picker.selected.min(rows.len().saturating_sub(1));
    let start = selected
        .saturating_sub(usize::from(list_height) / 2)
        .min(rows.len().saturating_sub(usize::from(list_height)));
    if rows.is_empty() && list_height > 0 {
        line(
            frame,
            Rect {
                y,
                height: 1,
                ..inner
            },
            "No matching models",
            t::GRAY,
        );
    }
    for (index, choice) in rows
        .iter()
        .enumerate()
        .skip(start)
        .take(usize::from(list_height))
    {
        let row = Rect {
            y: y + (index - start) as u16,
            height: 1,
            ..inner
        };
        render_choice(frame, row, choice, index == selected);
    }
    y += list_height + gap;
    if let Some(model) = model {
        details(
            frame,
            Rect {
                y,
                height: details_height,
                ..inner
            },
            model,
        );
    }
    let footer = if inner.width >= 36 {
        "↑/↓ Move · Enter Select · Esc Back"
    } else {
        "↑/↓ · Enter · Esc"
    };
    line(
        frame,
        Rect {
            y: inner.bottom() - 1,
            height: 1,
            ..inner
        },
        footer,
        t::GRAY_BRIGHT,
    );
}

struct Choice {
    label: String,
    description: String,
}

fn choices(picker: &Picker) -> Vec<Choice> {
    let current = picker.pending.as_ref().is_some_and(|model| {
        model.plugin == picker.active_plugin && model.id == picker.active_model
    });
    match picker.stage {
        Stage::Models => picker
            .matching()
            .into_iter()
            .map(|model| Choice {
                label: if model.plugin == picker.active_plugin && model.id == picker.active_model {
                    format!("{} (current)", model.name)
                } else {
                    model.name.clone()
                },
                description: model.description.clone(),
            })
            .collect(),
        Stage::Reasoning => picker
            .reasoning_choices()
            .into_iter()
            .map(|effort| {
                let (label, description) = match effort.as_deref() {
                    None => ("Model default", "Use the model's default"),
                    Some("none") => ("None", "No reasoning"),
                    Some("minimal") => ("Minimal", "Minimal reasoning"),
                    Some("low") => ("Low", "Faster, lighter reasoning"),
                    Some("medium") => ("Medium", "Balanced reasoning"),
                    Some("high") => ("High", "Heavy reasoning"),
                    Some("xhigh") => ("Extra high", "Extended reasoning"),
                    Some("max") => ("Maximum", "Maximum reasoning"),
                    Some(other) => (other, ""),
                };
                Choice {
                    label: if current && effort == picker.active_options.reasoning {
                        format!("{label} (active)")
                    } else {
                        label.into()
                    },
                    description: description.into(),
                }
            })
            .collect(),
        Stage::Output => picker
            .output_choices()
            .into_iter()
            .map(|limit| {
                let label = limit.map_or_else(
                    || "Model default".into(),
                    |tokens| format!("{} tokens", number(tokens)),
                );
                Choice {
                    label: if current && limit == picker.active_options.max_tokens {
                        format!("{label} (active)")
                    } else {
                        label
                    },
                    description: if limit.is_none() {
                        "Use the model's default".into()
                    } else {
                        "Maximum generated output".into()
                    },
                }
            })
            .collect(),
    }
}

fn render_choice(frame: &mut Frame, area: Rect, choice: &Choice, selected: bool) {
    let background = if selected { t::BG_DARK } else { t::BG_BASE };
    let style = Style::default()
        .bg(background)
        .fg(t::TEXT_PRIMARY)
        .add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
    frame.render_widget(Block::default().style(style), area);
    let right = if area.width >= 42 {
        truncate(&choice.description, area.width / 3)
    } else {
        String::new()
    };
    let right_width = right.width() as u16;
    let label_width = area
        .width
        .saturating_sub(2 + right_width + u16::from(!right.is_empty()) * 2);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span(if selected { "❯ " } else { "  " }, t::ACCENT_MODEL),
            Span::styled(truncate(&choice.label, label_width), style),
        ])),
        area,
    );
    if !right.is_empty() {
        frame.render_widget(
            Paragraph::new(span(right, t::GRAY_BRIGHT)).right_aligned(),
            Rect {
                x: area.right() - right_width,
                width: right_width,
                ..area
            },
        );
    }
}

fn search(frame: &mut Frame, area: Rect, picker: &Picker) {
    let prefix = "Search: ";
    let prefix_width = (prefix.width() as u16).min(area.width);
    let usable = area.width.saturating_sub(prefix_width);
    let (segments, cursor) = picker.query.wrapped(usable.max(1));
    let text = segments
        .get(usize::from(cursor.1))
        .map_or("", String::as_str);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            span(truncate(prefix, prefix_width), t::GRAY_BRIGHT),
            span(truncate(text, usable), t::TEXT_PRIMARY),
        ])),
        area,
    );
    if usable > 0 {
        frame.set_cursor_position((area.x + prefix_width + cursor.0.min(usable - 1), area.y));
    }
}

fn details(frame: &mut Frame, area: Rect, model: &Model) {
    let mut lines = vec![
        (format!("Provider  {}", model.provider), t::ACCENT_MODEL),
        (
            format!(
                "ID  {}",
                if crate::models::pinned(&model.id) {
                    model.id.as_str()
                } else {
                    crate::models::AUTO
                }
            ),
            t::TEXT_SECONDARY,
        ),
    ];
    let mut limits = Vec::new();
    if let Some(context) = model.context_length {
        limits.push(format!("Context {}", number(context)));
    }
    if let Some(output) = model.max_output_tokens {
        limits.push(format!("Output {}", number(output)));
    }
    lines.push((
        if limits.is_empty() {
            model.description.clone()
        } else {
            limits.join(" · ")
        },
        t::GRAY_BRIGHT,
    ));
    for (index, (text, color)) in lines.iter().take(usize::from(area.height)).enumerate() {
        line(
            frame,
            Rect {
                y: area.y + index as u16,
                height: 1,
                ..area
            },
            text,
            *color,
        );
    }
}

fn line(frame: &mut Frame, area: Rect, text: &str, color: ratatui::style::Color) {
    frame.render_widget(
        Paragraph::new(span(truncate(text, area.width), color)),
        area,
    );
}

fn number(value: u32) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            result.push(',');
        }
        result.push(digit);
    }
    result
}
