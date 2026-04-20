use crate::app_state::{
    AutopilotChatState, ChatPaneInputs, CodingProjectPaneInputs, CodingProjectPaneState,
    CodingProjectTaskStatus, RenderState,
};
use crate::pane_renderer::{paint_action_button, paint_primary_button, split_text_for_display};
use crate::pane_system::{
    coding_project_board_bounds, coding_project_board_column_bounds,
    coding_project_chat_composer_bounds, coding_project_chat_panel_bounds,
    coding_project_chat_send_button_bounds, coding_project_chat_transcript_bounds,
    coding_project_overview_panel_bounds, coding_project_task_card_bounds_with_scroll,
    coding_project_task_detail_add_note_button_bounds,
    coding_project_task_detail_close_button_bounds, coding_project_task_detail_note_input_bounds,
    coding_project_task_detail_popup_bounds, pane_content_bounds,
};
use crate::ui_style::{self, AppTextRole};
use wgpui::{Bounds, Component, Hsla, InputEvent, PaintContext, Point, Quad, theme};

const PANEL_HEADER_HEIGHT: f32 = 24.0;
const OVERVIEW_LINE_HEIGHT: f32 = 14.0;
const CHAT_LINE_HEIGHT: f32 = 14.0;
const BOARD_CARD_HEIGHT: f32 = 48.0;
const BOARD_CARD_GAP: f32 = 8.0;

pub fn paint_coding_project_pane(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    coding_project: &CodingProjectPaneState,
    chat_inputs: &mut ChatPaneInputs,
    coding_project_inputs: &mut CodingProjectPaneInputs,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(content_bounds)
            .with_background(theme::bg::APP.with_alpha(0.20))
            .with_border(theme::border::DEFAULT.with_alpha(0.65), 1.0)
            .with_corner_radius(4.0),
    );

    paint_overview_panel(content_bounds, coding_project, paint);
    paint_board_panel(content_bounds, coding_project, paint);
    paint_chat_panel(content_bounds, autopilot_chat, coding_project, chat_inputs, paint);

    if coding_project.task_detail_open {
        paint_task_detail_overlay(content_bounds, coding_project, coding_project_inputs, paint);
    }
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane_bounds = state
        .panes
        .iter()
        .filter(|pane| pane.kind == crate::app_state::PaneKind::CodingProject)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane_bounds else {
        return false;
    };
    let content_bounds = pane_content_bounds(bounds);

    let composer_bounds = coding_project_chat_composer_bounds(content_bounds);
    let composer_handled = state
        .chat_inputs
        .composer
        .event(event, composer_bounds, &mut state.event_context)
        .is_handled();

    let note_handled = if state.coding_project.task_detail_open {
        state
            .coding_project_inputs
            .task_note
            .event(
                event,
                coding_project_task_detail_note_input_bounds(content_bounds),
                &mut state.event_context,
            )
            .is_handled()
    } else {
        false
    };

    composer_handled || note_handled
}

pub fn dispatch_scroll_event(
    state: &mut RenderState,
    cursor_position: Point,
    scroll_dy: f32,
) -> bool {
    if scroll_dy.abs() <= f32::EPSILON {
        return false;
    }
    let top_pane_bounds = state
        .panes
        .iter()
        .filter(|pane| pane.kind == crate::app_state::PaneKind::CodingProject)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane_bounds else {
        return false;
    };
    if !bounds.contains(cursor_position) {
        return false;
    }
    let content_bounds = pane_content_bounds(bounds);

    if state.coding_project.task_detail_open {
        return false;
    }

    let overview = coding_project_overview_panel_bounds(content_bounds);
    if overview.contains(cursor_position) {
        let viewport = overview_inner_clip(overview);
        let max_scroll = (overview_content_height(state, overview) - viewport.size.height).max(0.0);
        return state.coding_project.scroll_overview_by(scroll_dy, max_scroll);
    }

    let board = coding_project_board_bounds(content_bounds);
    if board.contains(cursor_position) {
        let max_scroll = board_max_scroll(content_bounds, state);
        return state.coding_project.scroll_board_by(scroll_dy, max_scroll);
    }

    let chat = coding_project_chat_transcript_bounds(content_bounds);
    if chat.contains(cursor_position) {
        let max_scroll = chat_max_scroll(chat_render_line_count(state, chat), chat.size.height);
        return state.coding_project.scroll_chat_by(scroll_dy, max_scroll);
    }

    false
}

fn chat_render_line_count(state: &RenderState, transcript: Bounds) -> usize {
    let wrap_chars = wrap_chars_for_width((transcript.size.width - 56.0).max(56.0), 7.2, 12);
    let mut lines = 0usize;
    for message in &state.autopilot_chat.messages {
        let wrapped = split_text_for_display(message.content.as_str(), wrap_chars);
        lines += wrapped.len().max(1);
    }
    lines
}

fn paint_overview_panel(
    content_bounds: Bounds,
    coding_project: &CodingProjectPaneState,
    paint: &mut PaintContext,
) {
    let bounds = coding_project_overview_panel_bounds(content_bounds);
    let overview_orange = Hsla::from_hex(0xFFA122);
    paint_panel(bounds, "Overview", overview_orange, Some(overview_orange), paint);

    let helper = ui_style::app_text_style(AppTextRole::Helper);
    let body = ui_style::app_text_style(AppTextRole::Supporting);
    let clip = overview_inner_clip(bounds);
    let paragraph_wrap_chars = wrap_chars_for_width(clip.size.width, 8.4, 16);
    let review_wrap_chars = wrap_chars_for_width((clip.size.width - 12.0).max(48.0), 8.0, 12);
    let mut y = clip.origin.y + 2.0 - coding_project.overview_scroll_offset();

    paint.scene.push_clip(clip);

    let overview_lines = [
        "Purpose: route coding work through a staged kanban with explicit review gates.",
        "Scope level adapts by request size: small bug fixes get lightweight flow; new apps keep full planning and review lifecycle.",
        "Use task details to track updates and attach notes as new constraints appear.",
    ];

    for paragraph in overview_lines {
        for line in split_text_for_display(paragraph, paragraph_wrap_chars) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(clip.origin.x, y),
                body.font_size,
                body.color,
            ));
            y += OVERVIEW_LINE_HEIGHT;
        }
        y += 4.0;
    }

    y += 2.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "Review Requests",
        Point::new(clip.origin.x, y),
        ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
        overview_orange,
    ));
    y += OVERVIEW_LINE_HEIGHT + 2.0;

    if coding_project.review_requests.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            "No review requests pending.",
            Point::new(clip.origin.x, y),
            body.font_size,
            body.color,
        ));
    } else {
        for request in coding_project.review_requests.iter() {
            paint.scene.draw_text(paint.text.layout_mono(
                "?",
                Point::new(clip.origin.x, y),
                helper.font_size,
                theme::status::WARNING,
            ));
            for line in split_text_for_display(request, review_wrap_chars) {
                paint.scene.draw_text(paint.text.layout(
                    &line,
                    Point::new(clip.origin.x + 12.0, y),
                    body.font_size,
                    body.color,
                ));
                y += OVERVIEW_LINE_HEIGHT;
            }
            y += 2.0;
        }
    }

    paint.scene.pop_clip();
}

fn paint_board_panel(
    content_bounds: Bounds,
    coding_project: &CodingProjectPaneState,
    paint: &mut PaintContext,
) {
    let bounds = coding_project_board_bounds(content_bounds);
    let board_green = Hsla::from_hex(0x53D08C);
    paint_panel(bounds, "Project Board", board_green, Some(board_green), paint);

    let columns = [
        ("Backlog", CodingProjectTaskStatus::Backlog),
        ("In Progress", CodingProjectTaskStatus::InProgress),
        ("Review", CodingProjectTaskStatus::Review),
        ("Done", CodingProjectTaskStatus::Done),
    ];
    for (column_index, (label, status)) in columns.iter().enumerate() {
        let column_bounds = coding_project_board_column_bounds(content_bounds, column_index);
        paint.scene.draw_quad(
            Quad::new(column_bounds)
                .with_background(theme::bg::SURFACE.with_alpha(0.36))
                .with_border(theme::border::DEFAULT.with_alpha(0.58), 1.0)
                .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            label,
            Point::new(column_bounds.origin.x + 8.0, column_bounds.origin.y + 8.0),
            ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
            status_color(*status),
        ));
    }

    if coding_project.task_detail_open {
        return;
    }

    paint.scene.push_clip(bounds);
    for (index, task) in coding_project.tasks.iter().enumerate() {
        let Some(card) = coding_project_task_card_bounds_with_scroll(
            content_bounds,
            &coding_project.tasks,
            index,
            coding_project.board_scroll_offset(),
        ) else {
            continue;
        };
        let selected = coding_project.selected_task_index == Some(index) && coding_project.task_detail_open;
        paint.scene.draw_quad(
            Quad::new(card)
                .with_background(if selected {
                    theme::accent::PRIMARY.with_alpha(0.14)
                } else {
                    theme::bg::ELEVATED.with_alpha(0.60)
                })
                .with_border(theme::border::DEFAULT.with_alpha(0.62), 1.0)
                .with_corner_radius(3.0),
        );
        paint.scene.draw_text(paint.text.layout(
            &task.title,
            Point::new(card.origin.x + 8.0, card.origin.y + 8.0),
            ui_style::app_text_style(AppTextRole::PrimaryRow).font_size,
            ui_style::app_text_style(AppTextRole::PrimaryRow).color,
        ));
        let summary_wrap_chars = wrap_chars_for_width((card.size.width - 14.0).max(56.0), 7.0, 10);
        let summary = split_text_for_display(&task.summary, summary_wrap_chars)
            .into_iter()
            .next()
            .unwrap_or_default();
        paint.scene.draw_text(paint.text.layout(
            &summary,
            Point::new(card.origin.x + 8.0, card.origin.y + 26.0),
            ui_style::app_text_style(AppTextRole::SecondaryMetadata).font_size,
            ui_style::app_text_style(AppTextRole::SecondaryMetadata).color,
        ));
    }
    paint.scene.pop_clip();
}

fn paint_chat_panel(
    content_bounds: Bounds,
    autopilot_chat: &AutopilotChatState,
    coding_project: &CodingProjectPaneState,
    chat_inputs: &mut ChatPaneInputs,
    paint: &mut PaintContext,
) {
    let panel = coding_project_chat_panel_bounds(content_bounds);
    paint_panel(panel, "Chat", Hsla::from_hex(0x2DA6F2), None, paint);

    let transcript = coding_project_chat_transcript_bounds(content_bounds);
    paint.scene.push_clip(transcript);
    let wrap_chars = wrap_chars_for_width((transcript.size.width - 56.0).max(56.0), 7.2, 12);
    let chat_font_size = ui_style::app_text_style(AppTextRole::SecondaryMetadata).font_size;
    let mut y = transcript.origin.y + 6.0 - coding_project.chat_scroll_offset();
    for message in &autopilot_chat.messages {
        let color = match message.role {
            crate::app_state::AutopilotRole::User => theme::accent::PRIMARY,
            crate::app_state::AutopilotRole::Codex => theme::text::MUTED,
        };
        let wrapped = split_text_for_display(message.content.as_str(), wrap_chars);
        if wrapped.is_empty() {
            let rendered = match message.role {
                crate::app_state::AutopilotRole::User => "<you".to_string(),
                crate::app_state::AutopilotRole::Codex => "agent>".to_string(),
            };
            let run = paint
                .text
                .layout_mono(rendered.as_str(), Point::ZERO, chat_font_size, color);
            let x = match message.role {
                crate::app_state::AutopilotRole::User => {
                    (transcript.max_x() - run.bounds().size.width).max(transcript.origin.x)
                }
                crate::app_state::AutopilotRole::Codex => transcript.origin.x,
            };
            paint.scene.draw_text(paint.text.layout_mono(
                rendered.as_str(),
                Point::new(x, y),
                chat_font_size,
                color,
            ));
            y += CHAT_LINE_HEIGHT;
            continue;
        }
        for (line_index, line) in wrapped.into_iter().enumerate() {
            let rendered = match message.role {
                crate::app_state::AutopilotRole::User => {
                    if line_index == 0 {
                        format!("<you {line}")
                    } else {
                        line
                    }
                }
                crate::app_state::AutopilotRole::Codex => {
                    if line_index == 0 {
                        format!("agent> {line}")
                    } else {
                        format!("       {line}")
                    }
                }
            };
            let run = paint
                .text
                .layout_mono(rendered.as_str(), Point::ZERO, chat_font_size, color);
            let x = match message.role {
                crate::app_state::AutopilotRole::User => {
                    (transcript.max_x() - run.bounds().size.width).max(transcript.origin.x)
                }
                crate::app_state::AutopilotRole::Codex => transcript.origin.x,
            };
            paint.scene.draw_text(paint.text.layout_mono(
                rendered.as_str(),
                Point::new(x, y),
                chat_font_size,
                color,
            ));
            y += CHAT_LINE_HEIGHT;
        }
    }
    paint.scene.pop_clip();

    let composer = coding_project_chat_composer_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            composer.origin.x - 2.0,
            composer.origin.y - 2.0,
            composer.size.width + 4.0,
            composer.size.height + 4.0,
        ))
        .with_background(theme::bg::SURFACE.with_alpha(0.35))
        .with_border(theme::border::DEFAULT.with_alpha(0.35), 1.0)
        .with_corner_radius(4.0),
    );
    chat_inputs.composer.set_max_width(composer.size.width);
    chat_inputs.composer.paint(composer, paint);
    paint_primary_button(coding_project_chat_send_button_bounds(content_bounds), ">", paint);
}

fn paint_task_detail_overlay(
    content_bounds: Bounds,
    coding_project: &CodingProjectPaneState,
    coding_project_inputs: &mut CodingProjectPaneInputs,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(content_bounds).with_background(theme::bg::APP.with_alpha(0.86)),
    );

    let popup = coding_project_task_detail_popup_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(popup)
            .with_background(theme::bg::SURFACE.with_alpha(1.0))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(6.0),
    );

    let Some(task) = coding_project.selected_task() else {
        return;
    };
    let popup_content = popup_content_clip(popup);
    let popup_wrap_chars = wrap_chars_for_width(popup_content.size.width, 7.4, 18);
    let popup_bullet_wrap_chars = wrap_chars_for_width((popup_content.size.width - 12.0).max(64.0), 7.4, 14);

    paint.scene.draw_text(paint.text.layout_mono(
        "Task Detail",
        Point::new(popup.origin.x + 12.0, popup.origin.y + 10.0),
        ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
        ui_style::app_text_style(AppTextRole::SectionHeading).color,
    ));
    paint_action_button(
        coding_project_task_detail_close_button_bounds(content_bounds),
        "Close",
        paint,
    );

    paint.scene.push_clip(popup_content);

    let mut y = popup_content.origin.y;
    for line in split_text_for_display(&task.title, popup_wrap_chars)
        .into_iter()
        .take(2)
    {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(popup_content.origin.x, y),
            ui_style::app_text_style(AppTextRole::PrimaryRow).font_size,
            ui_style::app_text_style(AppTextRole::PrimaryRow).color,
        ));
        y += 14.0;
    }

    y += 6.0;
    for line in split_text_for_display(&task.details, popup_wrap_chars)
        .into_iter()
        .take(6)
    {
        paint.scene.draw_text(paint.text.layout(
            &line,
            Point::new(popup_content.origin.x, y),
            ui_style::app_text_style(AppTextRole::Supporting).font_size,
            ui_style::app_text_style(AppTextRole::Supporting).color,
        ));
        y += 14.0;
    }

    y += 8.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "Updates",
        Point::new(popup_content.origin.x, y),
        ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
        theme::accent::PRIMARY,
    ));
    y += 14.0;

    for update in task.updates.iter().take(4) {
        let wrapped = split_text_for_display(update, popup_bullet_wrap_chars);
        if let Some(first) = wrapped.first() {
            paint.scene.draw_text(paint.text.layout(
                &format!("- {first}"),
                Point::new(popup_content.origin.x, y),
                ui_style::app_text_style(AppTextRole::Supporting).font_size,
                ui_style::app_text_style(AppTextRole::Supporting).color,
            ));
            y += 14.0;
        }
        for line in wrapped.into_iter().skip(1) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(popup_content.origin.x + 10.0, y),
                ui_style::app_text_style(AppTextRole::Supporting).font_size,
                ui_style::app_text_style(AppTextRole::Supporting).color,
            ));
            y += 14.0;
        }
    }

    y += 6.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "Notes",
        Point::new(popup_content.origin.x, y),
        ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
        Hsla::from_hex(0x63DD8A),
    ));
    y += 14.0;

    for note in task.notes.iter().rev().take(4) {
        let wrapped = split_text_for_display(note, popup_bullet_wrap_chars);
        if let Some(first) = wrapped.first() {
            paint.scene.draw_text(paint.text.layout(
                &format!("* {first}"),
                Point::new(popup_content.origin.x, y),
                ui_style::app_text_style(AppTextRole::Supporting).font_size,
                ui_style::app_text_style(AppTextRole::Supporting).color,
            ));
            y += 14.0;
        }
        for line in wrapped.into_iter().skip(1) {
            paint.scene.draw_text(paint.text.layout(
                &line,
                Point::new(popup_content.origin.x + 10.0, y),
                ui_style::app_text_style(AppTextRole::Supporting).font_size,
                ui_style::app_text_style(AppTextRole::Supporting).color,
            ));
            y += 14.0;
        }
    }
    paint.scene.pop_clip();

    let note_input = coding_project_task_detail_note_input_bounds(content_bounds);
    coding_project_inputs.task_note.set_max_width(note_input.size.width);
    coding_project_inputs.task_note.paint(note_input, paint);
    paint_primary_button(
        coding_project_task_detail_add_note_button_bounds(content_bounds),
        "Add",
        paint,
    );
}

fn paint_panel(
    bounds: Bounds,
    title: &str,
    accent: Hsla,
    border_tint: Option<Hsla>,
    paint: &mut PaintContext,
) {
    let border_color = border_tint
        .map(|tint| tint.with_alpha(0.55))
        .unwrap_or_else(|| theme::border::DEFAULT.with_alpha(0.74));
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::APP.with_alpha(0.24))
            .with_border(border_color, 1.0)
            .with_corner_radius(4.0),
    );
    let header = Bounds::new(
        bounds.origin.x + 1.0,
        bounds.origin.y + 1.0,
        (bounds.size.width - 2.0).max(0.0),
        PANEL_HEADER_HEIGHT,
    );
    paint.scene.draw_quad(Quad::new(header).with_background(accent.with_alpha(0.10)));
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            3.0,
            (bounds.size.height - 2.0).max(0.0),
        ))
        .with_background(accent.with_alpha(0.74)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        ui_style::app_text_style(AppTextRole::SectionHeading).font_size,
        accent,
    ));
}

fn status_color(status: CodingProjectTaskStatus) -> Hsla {
    match status {
        CodingProjectTaskStatus::Backlog => theme::text::MUTED,
        CodingProjectTaskStatus::InProgress => theme::accent::PRIMARY,
        CodingProjectTaskStatus::Review => theme::status::WARNING,
        CodingProjectTaskStatus::Done => theme::status::SUCCESS,
    }
}

fn overview_inner_clip(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + PANEL_HEADER_HEIGHT + 8.0,
        (bounds.size.width - 20.0).max(40.0),
        (bounds.size.height - PANEL_HEADER_HEIGHT - 14.0).max(40.0),
    )
}

fn popup_content_clip(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 34.0,
        (bounds.size.width - 24.0).max(120.0),
        (bounds.size.height - 106.0).max(80.0),
    )
}

fn overview_content_height(state: &RenderState, overview_bounds: Bounds) -> f32 {
    let clip = overview_inner_clip(overview_bounds);
    let paragraph_wrap_chars = wrap_chars_for_width(clip.size.width, 8.4, 16);
    let review_wrap_chars = wrap_chars_for_width((clip.size.width - 12.0).max(48.0), 8.0, 12);
    let mut lines = 0usize;
    let overview_lines = [
        "Purpose: route coding work through a staged kanban with explicit review gates.",
        "Scope level adapts by request size: small bug fixes get lightweight flow; new apps keep full planning and review lifecycle.",
        "Use task details to track updates and attach notes as new constraints appear.",
    ];
    for paragraph in overview_lines {
        lines += split_text_for_display(paragraph, paragraph_wrap_chars).len();
        lines += 1;
    }
    lines += 2;
    for request in &state.coding_project.review_requests {
        lines += split_text_for_display(request, review_wrap_chars).len();
        lines += 1;
    }
    lines as f32 * OVERVIEW_LINE_HEIGHT + 16.0
}

fn wrap_chars_for_width(width: f32, avg_char_px: f32, min_chars: usize) -> usize {
    (((width - 8.0).max(8.0) / avg_char_px).floor() as usize).max(min_chars)
}

fn board_max_scroll(content_bounds: Bounds, state: &RenderState) -> f32 {
    let mut backlog = 0usize;
    let mut progress = 0usize;
    let mut review = 0usize;
    let mut done = 0usize;
    for task in &state.coding_project.tasks {
        match task.status {
            CodingProjectTaskStatus::Backlog => backlog += 1,
            CodingProjectTaskStatus::InProgress => progress += 1,
            CodingProjectTaskStatus::Review => review += 1,
            CodingProjectTaskStatus::Done => done += 1,
        }
    }
    let max_cards = backlog.max(progress).max(review).max(done) as f32;
    let content_height = 24.0 + max_cards * (BOARD_CARD_HEIGHT + BOARD_CARD_GAP) + 16.0;
    let viewport_height = coding_project_board_column_bounds(content_bounds, 0).size.height;
    (content_height - viewport_height).max(0.0)
}

fn chat_max_scroll(line_count: usize, viewport_height: f32) -> f32 {
    let content_height = line_count as f32 * CHAT_LINE_HEIGHT + 12.0;
    (content_height - viewport_height).max(0.0)
}
