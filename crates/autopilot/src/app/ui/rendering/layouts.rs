pub(crate) fn composer_bar_layout(
    text_system: &mut TextSystem,
    bar_bounds: Bounds,
    labels: &ComposerLabels,
) -> ComposerBarLayout {
    let font_size = 11.0;
    let mut x = bar_bounds.origin.x;
    let y = bar_bounds.origin.y + (bar_bounds.size.height - COMPOSER_PILL_HEIGHT) / 2.0;

    let mut measure = |text: &str| {
        text_system
            .measure_styled_mono(text, font_size, wgpui::text::FontStyle::default())
            .max(1.0)
            + COMPOSER_PILL_PADDING_X * 2.0
    };

    let mut next_bounds = |text: &str| {
        let mut width = measure(text);
        let remaining = (bar_bounds.origin.x + bar_bounds.size.width - x).max(0.0);
        if width > remaining {
            width = remaining.max(COMPOSER_PILL_HEIGHT);
        }
        let bounds = Bounds::new(x, y, width, COMPOSER_PILL_HEIGHT);
        x += width + COMPOSER_PILL_GAP;
        bounds
    };

    let model_bounds = next_bounds(&labels.model);
    let effort_bounds = next_bounds(&labels.effort);
    let access_bounds = next_bounds(&labels.access);
    let skill_bounds = next_bounds(&labels.skill);

    ComposerBarLayout {
        model_bounds,
        effort_bounds,
        access_bounds,
        skill_bounds,
    }
}

pub(crate) fn composer_menu_layout(
    anchor_bounds: Bounds,
    item_count: usize,
) -> ComposerMenuLayout {
    let visible_count = item_count.min(COMPOSER_MENU_MAX_ITEMS);
    let menu_width = anchor_bounds.size.width.max(COMPOSER_MENU_MIN_WIDTH);
    let menu_height =
        visible_count as f32 * COMPOSER_MENU_ITEM_HEIGHT + COMPOSER_MENU_PADDING * 2.0;
    let mut menu_y = anchor_bounds.origin.y - menu_height - 6.0;
    if menu_y < 8.0 {
        menu_y = anchor_bounds.origin.y + anchor_bounds.size.height + 6.0;
    }
    let menu_x = anchor_bounds.origin.x;
    let bounds = Bounds::new(menu_x, menu_y, menu_width, menu_height);

    let mut item_bounds = Vec::with_capacity(visible_count);
    for index in 0..visible_count {
        let y = menu_y + COMPOSER_MENU_PADDING + index as f32 * COMPOSER_MENU_ITEM_HEIGHT;
        item_bounds.push((
            index,
            Bounds::new(
                menu_x + COMPOSER_MENU_PADDING,
                y,
                menu_width - COMPOSER_MENU_PADDING * 2.0,
                COMPOSER_MENU_ITEM_HEIGHT,
            ),
        ));
    }

    ComposerMenuLayout { bounds, item_bounds }
}

pub(crate) fn approvals_panel_layout(
    right_bounds: Bounds,
    git_layout: &GitDiffPanelLayout,
    approvals_count: usize,
) -> Option<ApprovalsLayout> {
    let panel_x = git_layout.list_bounds.origin.x;
    let panel_width = git_layout.list_bounds.size.width;
    let panel_y = git_layout.list_bounds.origin.y + git_layout.list_bounds.size.height + 12.0;
    let panel_height =
        right_bounds.origin.y + right_bounds.size.height - panel_y - 12.0;
    if panel_height < 40.0 {
        return None;
    }
    let panel_bounds = Bounds::new(panel_x, panel_y, panel_width, panel_height);
    let header_height = 18.0;
    let list_top = panel_y + header_height + 8.0;
    let available_height = panel_y + panel_height - list_top - 8.0;
    let card_height = 78.0;
    let gap = 8.0;
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + gap) / (card_height + gap)) as usize
    };
    let visible_count = approvals_count.min(max_cards);

    let mut card_bounds = Vec::with_capacity(visible_count);
    let mut approve_bounds = Vec::with_capacity(visible_count);
    let mut decline_bounds = Vec::with_capacity(visible_count);

    for index in 0..visible_count {
        let y = list_top + index as f32 * (card_height + gap);
        let card = Bounds::new(panel_x + 6.0, y, panel_width - 12.0, card_height);
        let button_height = 18.0;
        let button_width = 60.0;
        let button_gap = 8.0;
        let button_y = y + card_height - button_height - 8.0;
        let approve_x = card.origin.x + card.size.width - button_width - 8.0;
        let decline_x = approve_x - button_width - button_gap;

        card_bounds.push((index, card));
        approve_bounds.push((
            index,
            Bounds::new(approve_x, button_y, button_width, button_height),
        ));
        decline_bounds.push((
            index,
            Bounds::new(decline_x, button_y, button_width, button_height),
        ));
    }

    Some(ApprovalsLayout {
        panel_bounds,
        card_bounds,
        approve_bounds,
        decline_bounds,
    })
}

pub(crate) fn session_list_layout(
    logical_width: f32,
    logical_height: f32,
    session_count: usize,
    selected: usize,
    checkpoint_height: f32,
) -> SessionListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    let content_top = modal_y + SESSION_MODAL_PADDING + 46.0;
    let footer_y = modal_y + modal_height - 24.0;
    let checkpoint_height = checkpoint_height.max(0.0);
    let checkpoint_bounds = if checkpoint_height > 0.0 {
        let y = footer_y - 12.0 - checkpoint_height;
        Some(Bounds::new(
            modal_x + SESSION_MODAL_PADDING,
            y,
            modal_width - SESSION_MODAL_PADDING * 2.0,
            checkpoint_height,
        ))
    } else {
        None
    };

    let card_area_bottom = checkpoint_bounds
        .as_ref()
        .map(|bounds| bounds.origin.y - 16.0)
        .unwrap_or(footer_y - 16.0);
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = session_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(session_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > session_count {
            start = session_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SessionListLayout {
        modal_bounds,
        card_bounds,
        checkpoint_bounds,
    }
}

pub(crate) fn agent_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    if state.catalogs.active_agent.is_some() {
        y += 18.0;
    }
    y += 18.0;
    if state.catalogs.agent_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.agent_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

pub(crate) fn skill_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    y += 18.0;
    if state.catalogs.skill_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.skill_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

pub(crate) fn agent_list_layout(
    logical_width: f32,
    logical_height: f32,
    agent_count: usize,
    selected: usize,
    content_top: f32,
) -> AgentListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP))
            as usize
    };

    let visible_count = agent_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(agent_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > agent_count {
            start = agent_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    AgentListLayout { card_bounds }
}

pub(crate) fn skill_list_layout(
    logical_width: f32,
    logical_height: f32,
    skill_count: usize,
    selected: usize,
    content_top: f32,
) -> SkillListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SKILL_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = skill_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(skill_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > skill_count {
            start = skill_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SKILL_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SKILL_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SkillListLayout { card_bounds }
}

pub(crate) fn hook_event_layout(
    logical_width: f32,
    logical_height: f32,
    event_count: usize,
    selected: usize,
) -> HookEventLayout {
    let modal_width = HOOK_MODAL_WIDTH;
    let modal_height = HOOK_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);

    let content_top = modal_y + 64.0;
    let content_bottom = modal_y + modal_height - 32.0;
    let content_height = (content_bottom - content_top).max(0.0);
    let list_width = 260.0;
    let list_bounds = Bounds::new(modal_x + 16.0, content_top, list_width, content_height);
    let inspector_bounds = Bounds::new(
        list_bounds.origin.x + list_width + 16.0,
        content_top,
        modal_width - list_width - 48.0,
        content_height,
    );

    let max_rows = (content_height / HOOK_EVENT_ROW_HEIGHT).floor().max(0.0) as usize;
    let visible_count = event_count.min(max_rows.max(1));
    let mut row_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(event_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > event_count {
            start = event_count.saturating_sub(visible_count);
        }
        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * HOOK_EVENT_ROW_HEIGHT;
            let bounds = Bounds::new(list_bounds.origin.x, y, list_bounds.size.width, HOOK_EVENT_ROW_HEIGHT);
            row_bounds.push((index, bounds));
        }
    }

    HookEventLayout {
        list_bounds,
        inspector_bounds,
        row_bounds,
    }
}
