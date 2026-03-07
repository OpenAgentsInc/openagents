use crate::app_state::PaneLoadState;
use crate::project_ops::{ProjectOpsPaneState, ProjectOpsPresentationMode};
use wgpui::{theme, Bounds, PaintContext, Point, Quad};

pub fn paint_project_ops_pane(
    bounds: Bounds,
    state: &ProjectOpsPaneState,
    paint: &mut PaintContext,
) {
    let header = Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 12.0,
        (bounds.size.width - 24.0).max(120.0),
        78.0,
    );
    paint.scene.draw_quad(
        Quad::new(header)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(8.0),
    );

    paint.scene.draw_text(paint.text.layout(
        "Project Ops",
        Point::new(header.origin.x + 12.0, header.origin.y + 16.0),
        16.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        state.summary.as_str(),
        Point::new(header.origin.x + 12.0, header.origin.y + 38.0),
        11.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "{} | {} | view:{} | team:{} | presentation:{} | sort:{} | operator:{}",
                state.source_badge,
                state.load_state.label(),
                state.active_saved_view,
                state.active_team.title,
                state.presentation_mode.label(),
                state.sort_preference.label(),
                state.operator_label,
            )
            .as_str(),
            Point::new(header.origin.x + 12.0, header.origin.y + 58.0),
            10.0,
            theme::text::MUTED,
        ),
    );

    let shell = Bounds::new(
        bounds.origin.x + 12.0,
        header.max_y() + 12.0,
        (bounds.size.width - 24.0).max(120.0),
        (bounds.size.height - header.size.height - 36.0).max(180.0),
    );
    paint.scene.draw_quad(
        Quad::new(shell)
            .with_background(theme::bg::APP)
            .with_corner_radius(8.0),
    );

    let toolbar = Bounds::new(
        shell.origin.x + 12.0,
        shell.origin.y + 12.0,
        shell.size.width - 24.0,
        92.0,
    );
    paint.scene.draw_quad(
        Quad::new(toolbar)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "Toolbar",
        Point::new(toolbar.origin.x + 12.0, toolbar.origin.y + 14.0),
        13.0,
        theme::text::PRIMARY,
    ));

    let mut chip_x = toolbar.origin.x + 12.0;
    let mut chip_y = toolbar.origin.y + 34.0;
    for view in &state.available_saved_views {
        let chip_width = (view.title.len() as f32 * 7.0 + 26.0).max(82.0);
        if chip_x + chip_width > toolbar.max_x() - 12.0 {
            chip_x = toolbar.origin.x + 12.0;
            chip_y += 22.0;
        }
        let chip_bounds = Bounds::new(chip_x, chip_y, chip_width, 18.0);
        let active = view.view_id == state.active_saved_view_id;
        paint.scene.draw_quad(
            Quad::new(chip_bounds)
                .with_background(if active {
                    theme::bg::APP
                } else {
                    theme::bg::HOVER
                })
                .with_corner_radius(9.0),
        );
        paint.scene.draw_text(paint.text.layout(
            view.title.as_str(),
            Point::new(chip_bounds.origin.x + 8.0, chip_bounds.origin.y + 12.0),
            10.0,
            if active {
                theme::text::PRIMARY
            } else {
                theme::text::SECONDARY
            },
        ));
        chip_x += chip_width + 8.0;
    }

    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Search: {} | sort:{} | team:{}",
                if state.search_query.trim().is_empty() {
                    "<empty>"
                } else {
                    state.search_query.as_str()
                },
                state.sort_preference.label(),
                state.active_team.team_key.as_str(),
            )
            .as_str(),
            Point::new(toolbar.origin.x + 12.0, toolbar.max_y() - 18.0),
            10.0,
            theme::text::MUTED,
        ),
    );
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Filters ({}) : {}",
                state.active_filter_chips.len(),
                if state.active_filter_chips.is_empty() {
                    "none".to_string()
                } else {
                    state.active_filter_chips.join(", ")
                }
            )
            .as_str(),
            Point::new(toolbar.origin.x + 180.0, toolbar.max_y() - 18.0),
            10.0,
            theme::text::MUTED,
        ),
    );
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Presentation: {} | bulk={} | {}",
                state.presentation_mode.label(),
                state.bulk_selected_work_item_ids.len(),
                if let Some(drag) = state.board_drag_state.as_ref() {
                    format!(
                        "drag={} from {}",
                        drag.work_item_id.as_str(),
                        drag.from_status.label()
                    )
                } else if state.presentation_mode == ProjectOpsPresentationMode::Board {
                    "board lanes show per-status counts and blocked rows".to_string()
                } else {
                    "list stays in stable updated-desc order".to_string()
                }
            )
            .as_str(),
            Point::new(toolbar.origin.x + 12.0, toolbar.max_y() - 36.0),
            10.0,
            theme::text::MUTED,
        ),
    );
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Teams ({}): {}",
                state.available_teams.len(),
                state
                    .available_teams
                    .iter()
                    .map(|team| {
                        if team.team_key == state.active_team.team_key {
                            format!("{}*", team.title)
                        } else {
                            team.title.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .as_str(),
            Point::new(toolbar.origin.x + 72.0, toolbar.origin.y + 14.0),
            10.0,
            theme::text::MUTED,
        ),
    );

    let content = Bounds::new(
        shell.origin.x + 12.0,
        toolbar.max_y() + 12.0,
        shell.size.width - 24.0,
        shell.size.height - toolbar.size.height - 36.0,
    );
    let list_width = if state.presentation_mode == ProjectOpsPresentationMode::Board {
        (content.size.width * 0.64).max(280.0)
    } else {
        (content.size.width * 0.56).max(220.0)
    };
    let list = Bounds::new(
        content.origin.x,
        content.origin.y,
        list_width,
        content.size.height,
    );
    let detail = Bounds::new(
        list.max_x() + 12.0,
        content.origin.y,
        (content.max_x() - list.max_x() - 12.0).max(140.0),
        content.size.height,
    );
    paint.scene.draw_quad(
        Quad::new(list)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(8.0),
    );
    paint.scene.draw_quad(
        Quad::new(detail)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(8.0),
    );

    let status_color = match state.load_state {
        PaneLoadState::Loading => theme::status::WARNING,
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Error => theme::status::ERROR,
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            detail.origin.x + 12.0,
            detail.origin.y + 14.0,
            10.0,
            10.0,
        ))
        .with_background(status_color)
        .with_corner_radius(5.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "Project Ops Detail",
        Point::new(detail.origin.x + 30.0, detail.origin.y + 12.0),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        state.status_note.as_str(),
        Point::new(detail.origin.x + 12.0, detail.origin.y + 38.0),
        11.0,
        theme::text::SECONDARY,
    ));

    let action = state.last_action.as_deref().unwrap_or("Project Ops idle");
    let mut detail_y = detail.origin.y + 84.0;
    paint.scene.draw_text(paint.text.layout(
        format!("Last action: {action}").as_str(),
        Point::new(detail.origin.x + 12.0, detail_y),
        11.0,
        theme::text::MUTED,
    ));
    detail_y += 20.0;
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Bulk: {} selected | {}",
                state.bulk_selected_work_item_ids.len(),
                state
                    .bulk_action_status
                    .as_deref()
                    .unwrap_or("no bulk action applied")
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ),
    );
    detail_y += 18.0;

    let projection_counts = format!(
        "Rows: work items={} | activity={} | cycles={} | views={} | teams={}",
        state.local_store.work_items.len(),
        state.local_store.activity_rows.len(),
        state.local_store.cycles.len(),
        state.local_store.saved_views.len(),
        state.local_store.teams.len()
    );
    paint.scene.draw_text(paint.text.layout(
        projection_counts.as_str(),
        Point::new(detail.origin.x + 12.0, detail_y),
        11.0,
        theme::text::PRIMARY,
    ));
    detail_y += 18.0;
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Pilot: commands={} views={} rebuilds={} last={}ms",
                state.pilot_metrics.command_counts.len(),
                state.pilot_metrics.view_counts.len(),
                state.pilot_metrics.projection_rebuild_count,
                state
                    .pilot_metrics
                    .last_projection_rebuild_duration_ms
                    .unwrap_or(0)
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ),
    );
    detail_y += 18.0;

    let sync = &state.sync_diagnostics;
    let replay_summary = match (sync.replay_cursor_seq, sync.replay_target_seq) {
        (Some(cursor), Some(target)) => format!("replay={cursor}/{target}"),
        (Some(cursor), None) => format!("replay={cursor}"),
        _ => "replay=idle".to_string(),
    };
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "PM sync: {} | lifecycle={} | {} | badge={} | team_badge={}",
                sync.bootstrap_state,
                sync.lifecycle_state.as_deref().unwrap_or("idle"),
                replay_summary,
                sync.source_badge,
                state.team_source_badge,
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            11.0,
            theme::text::MUTED,
        ),
    );
    detail_y += 18.0;
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Grants: required={} | granted={} | missing={}",
                sync.required_stream_grants.len(),
                sync.granted_stream_grants.len(),
                sync.missing_stream_grants.len(),
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ),
    );
    detail_y += 18.0;
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Granted raw: {}",
                if sync.granted_stream_grants.is_empty() {
                    "none".to_string()
                } else {
                    sync.granted_stream_grants.join(", ")
                }
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ),
    );
    detail_y += 18.0;
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Missing PM grants: {}",
                if sync.missing_stream_grants.is_empty() {
                    "none".to_string()
                } else {
                    sync.missing_stream_grants.join(", ")
                }
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            if sync.missing_stream_grants.is_empty() {
                theme::text::MUTED
            } else {
                theme::status::WARNING
            },
        ),
    );
    detail_y += 18.0;
    if let Some(bootstrap_note) = sync.bootstrap_note.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            format!("Bootstrap note: {bootstrap_note}").as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ));
        detail_y += 18.0;
    }
    if let Some(bootstrap_error) = sync.bootstrap_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            format!("Bootstrap error: {bootstrap_error}").as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::status::ERROR,
        ));
        detail_y += 18.0;
    }
    if sync.stale_cursor_recovery_required {
        paint.scene.draw_text(paint.text.layout(
            "Recovery: stale cursor requires explicit rebootstrap or checkpoint rewind.",
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::status::WARNING,
        ));
        detail_y += 18.0;
    } else if let Some(reason) = sync.last_disconnect_reason.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            format!("Last disconnect: {reason}").as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::text::MUTED,
        ));
        detail_y += 18.0;
    }
    for stream in sync.streams.iter().take(4) {
        paint.scene.draw_text(
            paint.text.layout(
                format!(
                    "{} grant={} checkpoint={} resume={}",
                    compact_stream_label(stream.stream_id.as_str()),
                    if stream.granted { "yes" } else { "no" },
                    stream.checkpoint_seq,
                    stream.resume_cursor_seq
                )
                .as_str(),
                Point::new(detail.origin.x + 12.0, detail_y),
                10.0,
                theme::text::MUTED,
            ),
        );
        detail_y += 16.0;
    }
    paint.scene.draw_text(paint.text.layout(
        "Policy: duplicates drop | out-of-order requires rebootstrap or rewind",
        Point::new(detail.origin.x + 12.0, detail_y),
        10.0,
        theme::text::MUTED,
    ));
    detail_y += 22.0;

    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Selected: {}",
                state
                    .selected_work_item_id
                    .as_ref()
                    .map(|id| id.as_str())
                    .unwrap_or("<none>")
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail_y),
            11.0,
            theme::text::PRIMARY,
        ),
    );
    detail_y += 18.0;
    if let Some(selection_notice) = state.selection_notice.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            selection_notice,
            Point::new(detail.origin.x + 12.0, detail_y),
            10.0,
            theme::status::WARNING,
        ));
        detail_y += 20.0;
    }

    if let Some(detail_draft) = state.detail_draft.as_ref() {
        let selected_team = state
            .selected_work_item()
            .map(|item| item.team_key.as_str().to_string())
            .unwrap_or_else(|| state.active_team.team_key.as_str().to_string());
        let parent_summary = detail_draft
            .parent_id
            .as_ref()
            .map(|parent_id| {
                state
                    .local_store
                    .work_items
                    .iter()
                    .find(|item| &item.work_item_id == parent_id)
                    .map(|item| format!("{} ({})", parent_id.as_str(), item.title))
                    .unwrap_or_else(|| format!("{} (missing)", parent_id.as_str()))
            })
            .unwrap_or_else(|| "none".to_string());
        let detail_lines = [
            format!("Title: {}", detail_draft.title),
            format!("Description: {}", detail_draft.description),
            format!(
                "Status: {} | Priority: {}",
                detail_draft.status.label(),
                detail_draft.priority.label()
            ),
            format!(
                "Assignee: {} | Cycle: {}",
                detail_draft.assignee.as_deref().unwrap_or("unassigned"),
                detail_draft
                    .cycle_id
                    .as_ref()
                    .map(|cycle_id| cycle_id.as_str())
                    .unwrap_or("none")
            ),
            format!(
                "Team: {} ({}) | Team view: {}",
                state.active_team.title,
                selected_team,
                state.active_team.default_saved_view_id
            ),
            format!(
                "Parent: {} | Due: {}",
                parent_summary,
                detail_draft
                    .due_at_unix_ms
                    .map(|due_at| due_at.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!(
                "Tags: {} | Blocked: {}",
                if detail_draft.area_tags.is_empty() {
                    "none".to_string()
                } else {
                    detail_draft.area_tags.join(", ")
                },
                detail_draft.blocked_reason.as_deref().unwrap_or("none")
            ),
            format!(
                "Created: {} | Updated: {} | Dirty: {}",
                detail_draft.created_at_unix_ms,
                detail_draft.updated_at_unix_ms,
                if detail_draft.dirty { "yes" } else { "no" }
            ),
        ];
        for (index, line) in detail_lines.iter().enumerate() {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(detail.origin.x + 12.0, detail_y + index as f32 * 18.0),
                10.5,
                theme::text::PRIMARY,
            ));
        }
        detail_y += detail_lines.len() as f32 * 18.0;
    } else {
        paint.scene.draw_text(paint.text.layout(
            "Select a work item to load the detail editor.",
            Point::new(detail.origin.x + 12.0, detail_y),
            10.5,
            theme::text::SECONDARY,
        ));
        detail_y += 22.0;
    }

    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Quick Create: title={} | team={} | priority={} | due={} | tags={} | desc={}",
                if state.quick_create_draft.title.is_empty() {
                    "<empty>"
                } else {
                    state.quick_create_draft.title.as_str()
                },
                state.quick_create_draft.team_key.as_str(),
                state.quick_create_draft.priority.label(),
                state
                    .quick_create_draft
                    .due_at_unix_ms
                    .map(|due_at| due_at.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                if state.quick_create_draft.area_tags.is_empty() {
                    "none".to_string()
                } else {
                    state.quick_create_draft.area_tags.join(", ")
                },
                if state.quick_create_draft.description.is_empty() {
                    "<empty>"
                } else {
                    state.quick_create_draft.description.as_str()
                }
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail.max_y() - 58.0),
            10.0,
            theme::text::MUTED,
        ),
    );
    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Save Status: {}",
                state
                    .detail_save_status
                    .as_deref()
                    .unwrap_or("no save applied yet")
            )
            .as_str(),
            Point::new(detail.origin.x + 12.0, detail.max_y() - 40.0),
            10.0,
            theme::text::MUTED,
        ),
    );
    paint.scene.draw_text(paint.text.layout(
        "Activity Timeline",
        Point::new(detail.origin.x + 12.0, detail_y + 14.0),
        12.0,
        theme::text::PRIMARY,
    ));
    if state.visible_activity_rows.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            state.activity_empty_state.as_str(),
            Point::new(detail.origin.x + 12.0, detail_y + 36.0),
            10.5,
            theme::text::SECONDARY,
        ));
    } else {
        for (index, row) in state.visible_activity_rows.iter().take(4).enumerate() {
            paint.scene.draw_text(
                paint.text.layout(
                    format!(
                        "{} | {} | {} | {}",
                        row.event_name.label(),
                        row.actor_label,
                        row.occurred_at_unix_ms,
                        row.summary
                    )
                    .as_str(),
                    Point::new(
                        detail.origin.x + 12.0,
                        detail_y + 36.0 + index as f32 * 18.0,
                    ),
                    10.0,
                    theme::text::PRIMARY,
                ),
            );
        }
    }

    paint.scene.draw_text(
        paint.text.layout(
            format!(
                "Visible in {} ({})",
                state.active_saved_view,
                state.visible_work_items.len()
            )
            .as_str(),
            Point::new(list.origin.x + 12.0, list.origin.y + 16.0),
            13.0,
            theme::text::PRIMARY,
        ),
    );
    if state.presentation_mode == ProjectOpsPresentationMode::Board {
        paint_board_presentation(list, state, paint);
    } else {
        paint_list_presentation(list, state, paint);
    }

    if let Some(error) = state.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            format!("Error: {error}").as_str(),
            Point::new(detail.origin.x + 12.0, detail.max_y() - 22.0),
            10.0,
            theme::status::ERROR,
        ));
    }
}

fn compact_stream_label(stream_id: &str) -> &str {
    match stream_id {
        crate::project_ops::PROJECT_OPS_WORK_ITEMS_STREAM_ID => "work_items",
        crate::project_ops::PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID => "activity",
        crate::project_ops::PROJECT_OPS_CYCLES_STREAM_ID => "cycles",
        crate::project_ops::PROJECT_OPS_SAVED_VIEWS_STREAM_ID => "saved_views",
        crate::project_ops::PROJECT_OPS_TEAMS_STREAM_ID => "teams",
        _ => stream_id,
    }
}

fn paint_list_presentation(list: Bounds, state: &ProjectOpsPaneState, paint: &mut PaintContext) {
    if state.visible_work_items.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            state.empty_state_copy.as_str(),
            Point::new(list.origin.x + 12.0, list.origin.y + 40.0),
            11.0,
            theme::text::SECONDARY,
        ));
        return;
    }

    for (index, item) in state.visible_work_items.iter().take(8).enumerate() {
        let row_y = list.origin.y + 40.0 + index as f32 * 22.0;
        let row_bounds = Bounds::new(
            list.origin.x + 8.0,
            row_y - 10.0,
            list.size.width - 16.0,
            18.0,
        );
        let selected = state
            .selected_work_item_id
            .as_ref()
            .is_some_and(|selected| selected == &item.work_item_id);
        paint.scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(if selected {
                    theme::bg::SURFACE
                } else {
                    theme::bg::APP.with_alpha(0.04)
                })
                .with_corner_radius(6.0),
        );
        let assignee = item.assignee.as_deref().unwrap_or("unassigned");
        let cycle = item
            .cycle_id
            .as_ref()
            .map(|cycle| cycle.as_str())
            .unwrap_or("no-cycle");
        let blocked = if item.is_blocked() { " blocked" } else { "" };
        paint.scene.draw_text(
            paint.text.layout(
                format!(
                    "{} | {} | {} | team:{} | {} | {}{}",
                    item.title,
                    item.status.label(),
                    item.priority.label(),
                    item.team_key.as_str(),
                    assignee,
                    cycle,
                    blocked
                )
                .as_str(),
                Point::new(list.origin.x + 14.0, row_y),
                10.5,
                if selected {
                    theme::text::PRIMARY
                } else {
                    theme::text::SECONDARY
                },
            ),
        );
    }
}

fn paint_board_presentation(board: Bounds, state: &ProjectOpsPaneState, paint: &mut PaintContext) {
    if state.visible_work_items.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            state.empty_state_copy.as_str(),
            Point::new(board.origin.x + 12.0, board.origin.y + 40.0),
            11.0,
            theme::text::SECONDARY,
        ));
        return;
    }

    let gutter = 10.0;
    let lane_width = ((board.size.width - gutter) / 2.0).max(120.0);
    let lane_height = ((board.size.height - gutter * 2.0 - 40.0) / 3.0).max(88.0);
    for (index, lane) in state.board_lanes.iter().enumerate() {
        let column = (index % 2) as f32;
        let row = (index / 2) as f32;
        let lane_bounds = Bounds::new(
            board.origin.x + column * (lane_width + gutter),
            board.origin.y + 32.0 + row * (lane_height + gutter),
            lane_width,
            lane_height,
        );
        let is_drag_source = state
            .board_drag_state
            .as_ref()
            .is_some_and(|drag| drag.from_status == lane.status);
        paint.scene.draw_quad(
            Quad::new(lane_bounds)
                .with_background(if is_drag_source {
                    theme::bg::SURFACE
                } else {
                    theme::bg::APP.with_alpha(0.08)
                })
                .with_corner_radius(8.0),
        );
        paint.scene.draw_text(
            paint.text.layout(
                format!(
                    "{} | count:{} | blocked:{}",
                    lane.title, lane.work_item_count, lane.blocked_count
                )
                .as_str(),
                Point::new(lane_bounds.origin.x + 8.0, lane_bounds.origin.y + 14.0),
                10.5,
                theme::text::PRIMARY,
            ),
        );
        paint.scene.draw_text(paint.text.layout(
            if is_drag_source {
                "drag source"
            } else if state.board_drag_state.is_some() {
                "drop target -> command/event status change"
            } else {
                "same filtered PM projection as list view"
            },
            Point::new(lane_bounds.origin.x + 8.0, lane_bounds.origin.y + 30.0),
            9.5,
            theme::text::MUTED,
        ));

        if lane.items.is_empty() {
            paint.scene.draw_text(paint.text.layout(
                lane.empty_state_copy.as_str(),
                Point::new(lane_bounds.origin.x + 8.0, lane_bounds.origin.y + 50.0),
                9.5,
                theme::text::SECONDARY,
            ));
            continue;
        }

        for (item_index, item) in lane.items.iter().take(4).enumerate() {
            let card_y = lane_bounds.origin.y + 52.0 + item_index as f32 * 22.0;
            let card_bounds = Bounds::new(
                lane_bounds.origin.x + 6.0,
                card_y - 10.0,
                lane_bounds.size.width - 12.0,
                18.0,
            );
            let selected = state
                .selected_work_item_id
                .as_ref()
                .is_some_and(|selected| selected == &item.work_item_id);
            let dragged = state
                .board_drag_state
                .as_ref()
                .is_some_and(|drag| drag.work_item_id == item.work_item_id);
            paint.scene.draw_quad(
                Quad::new(card_bounds)
                    .with_background(if dragged || selected {
                        theme::bg::HOVER
                    } else {
                        theme::bg::APP.with_alpha(0.06)
                    })
                    .with_corner_radius(6.0),
            );
            paint.scene.draw_text(
                paint.text.layout(
                    format!(
                        "{} | {}{}",
                        item.title,
                        item.priority.label(),
                        if item.is_blocked() { " | blocked" } else { "" }
                    )
                    .as_str(),
                    Point::new(lane_bounds.origin.x + 10.0, card_y),
                    9.5,
                    if selected {
                        theme::text::PRIMARY
                    } else {
                        theme::text::SECONDARY
                    },
                ),
            );
        }

        let overflow = lane.items.len().saturating_sub(4);
        if overflow > 0 {
            paint.scene.draw_text(paint.text.layout(
                format!("+{} more", overflow).as_str(),
                Point::new(lane_bounds.origin.x + 8.0, lane_bounds.max_y() - 10.0),
                9.0,
                theme::text::MUTED,
            ));
        }
    }
}
