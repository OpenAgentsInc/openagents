use crate::app_state::PaneLoadState;
use crate::project_ops::ProjectOpsPaneState;
use wgpui::{Bounds, PaintContext, Point, Quad, theme};

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
    paint.scene.draw_text(paint.text.layout(
        format!(
            "{} | {} | view:{}",
            state.source_badge,
            state.load_state.label(),
            state.active_saved_view
        )
        .as_str(),
        Point::new(header.origin.x + 12.0, header.origin.y + 58.0),
        10.0,
        theme::text::MUTED,
    ));

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

    let status_color = match state.load_state {
        PaneLoadState::Loading => theme::status::WARNING,
        PaneLoadState::Ready => theme::status::SUCCESS,
        PaneLoadState::Error => theme::status::ERROR,
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(shell.origin.x + 12.0, shell.origin.y + 14.0, 10.0, 10.0))
            .with_background(status_color)
            .with_corner_radius(5.0),
    );
    paint.scene.draw_text(paint.text.layout(
        "Shell Status",
        Point::new(shell.origin.x + 30.0, shell.origin.y + 12.0),
        13.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        state.status_note.as_str(),
        Point::new(shell.origin.x + 12.0, shell.origin.y + 38.0),
        11.0,
        theme::text::SECONDARY,
    ));

    let action = state
        .last_action
        .as_deref()
        .unwrap_or("Project Ops idle");
    paint.scene.draw_text(paint.text.layout(
        format!("Last action: {action}").as_str(),
        Point::new(shell.origin.x + 12.0, shell.origin.y + 86.0),
        11.0,
        theme::text::MUTED,
    ));

    let projection_counts = format!(
        "Rows: work items={} | activity={} | cycles={} | views={}",
        state.local_store.work_items.len(),
        state.local_store.activity_rows.len(),
        state.local_store.cycles.len(),
        state.local_store.saved_views.len()
    );
    paint.scene.draw_text(paint.text.layout(
        projection_counts.as_str(),
        Point::new(shell.origin.x + 12.0, shell.origin.y + 104.0),
        11.0,
        theme::text::PRIMARY,
    ));

    let checkpoint_status = format!(
        "Checkpoints: wi={:?} act={:?} cyc={:?} view={:?}",
        state
            .local_store
            .checkpoint_for(crate::project_ops::PROJECT_OPS_WORK_ITEMS_STREAM_ID),
        state
            .local_store
            .checkpoint_for(crate::project_ops::PROJECT_OPS_ACTIVITY_PROJECTION_STREAM_ID),
        state
            .local_store
            .checkpoint_for(crate::project_ops::PROJECT_OPS_CYCLES_STREAM_ID),
        state
            .local_store
            .checkpoint_for(crate::project_ops::PROJECT_OPS_SAVED_VIEWS_STREAM_ID),
    );
    paint.scene.draw_text(paint.text.layout(
        checkpoint_status.as_str(),
        Point::new(shell.origin.x + 12.0, shell.origin.y + 124.0),
        11.0,
        theme::text::MUTED,
    ));

    let next_steps = [
        "Next: Step 0 command reducer and apply loop",
        "Next: built-in saved views and toolbar",
        "Next: detail editor and activity timeline",
    ];
    for (index, line) in next_steps.iter().enumerate() {
        paint.scene.draw_text(paint.text.layout(
            line,
            Point::new(shell.origin.x + 12.0, shell.origin.y + 156.0 + index as f32 * 18.0),
            11.0,
            theme::text::PRIMARY,
        ));
    }

    if let Some(error) = state.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            format!("Error: {error}").as_str(),
            Point::new(shell.origin.x + 12.0, shell.max_y() - 22.0),
            10.0,
            theme::status::ERROR,
        ));
    }
}
