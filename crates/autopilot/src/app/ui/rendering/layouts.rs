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
