use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin};
use wgpui::PaintContext;

use crate::state::AppState;

pub(crate) fn build_repo_selector(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Dots grid background
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.15))
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // Centered layout - max width 500px
    let max_content_width: f32 = 500.0;
    let content_width = max_content_width.min(width - 48.0);
    let content_x = (width - content_width) / 2.0;

    // Calculate total content height for vertical centering
    let header_height = 24.0;
    let header_to_subtitle_gap = 24.0;
    let subtitle_height = 14.0;
    let subtitle_to_list_gap = 32.0;
    let row_height = 40.0;
    let max_visible_repos = 8; // Limit visible repos for centering calculation
    let visible_repos = state.repos.len().min(max_visible_repos);
    let list_height = if state.repos_loading || state.repos.is_empty() {
        40.0 // Single line for loading/empty state
    } else {
        visible_repos as f32 * row_height
    };

    let total_content_height = header_height + header_to_subtitle_gap + subtitle_height + subtitle_to_list_gap + list_height;
    let mut y = ((height - total_content_height) / 2.0).max(40.0);

    // Header
    let header = format!(
        "Welcome, {}",
        state.user.github_username.as_deref().unwrap_or("User")
    );
    let header_width = text_system.measure(&header, 24.0);
    let header_x = (width - header_width) / 2.0;
    let header_run = text_system.layout(
        &header,
        Point::new(header_x, y),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(header_run);

    // Logout button (top right - fixed position)
    let logout_text = "Log out";
    let logout_size = 12.0;
    let logout_width = text_system.measure(logout_text, logout_size) + 16.0;
    let logout_x = width - 24.0 - logout_width;
    let logout_y = 20.0; // Fixed top position
    state.button_bounds = Bounds::new(logout_x, logout_y, logout_width, 24.0);

    // Red border, transparent background
    let border_color = if state.button_hovered {
        theme::status::ERROR
    } else {
        theme::status::ERROR.with_alpha(0.7)
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.0))
            .with_border(border_color, 1.0),
    );

    let logout_run = text_system.layout(
        logout_text,
        Point::new(logout_x + 8.0, logout_y + 4.0),
        logout_size,
        border_color,
    );
    scene.draw_text(logout_run);

    y += header_height + header_to_subtitle_gap;

    let subtitle = "Select a repository:";
    let subtitle_width = text_system.measure(subtitle, 14.0);
    let subtitle_x = (width - subtitle_width) / 2.0;
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(subtitle_x, y),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    y += subtitle_height + subtitle_to_list_gap;

    state.repo_bounds.clear();

    if state.repos_loading {
        let loading_text = "Loading repositories...";
        let loading_width = text_system.measure(loading_text, 14.0);
        let loading_x = (width - loading_width) / 2.0;
        let loading_run = text_system.layout(
            loading_text,
            Point::new(loading_x, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(loading_run);
    } else if state.repos.is_empty() {
        let empty_text = "No repositories found";
        let empty_width = text_system.measure(empty_text, 14.0);
        let empty_x = (width - empty_width) / 2.0;
        let empty_run = text_system.layout(
            empty_text,
            Point::new(empty_x, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        for (i, repo) in state.repos.iter().enumerate() {
            let row_y = y + (i as f32 * row_height) - state.scroll_offset;

            if row_y + row_height < y || row_y > height {
                state.repo_bounds.push(Bounds::ZERO);
                continue;
            }

            let row_bounds = Bounds::new(content_x, row_y, content_width, row_height - 4.0);
            state.repo_bounds.push(row_bounds);

            let is_hovered = state.hovered_repo_idx == Some(i);
            let is_selected = state.selected_repo.as_ref() == Some(&repo.full_name);

            let row_bg = if is_selected {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(row_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            // Center text vertically in row
            let text_y = row_y + (row_height - 4.0 - 14.0) / 2.0;
            let name_run = text_system.layout(
                &repo.full_name,
                Point::new(content_x + 12.0, text_y),
                14.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(name_run);

            if repo.private {
                let badge_text = "Private";
                let badge_x = content_x + 12.0 + text_system.measure(&repo.full_name, 14.0) + 12.0;
                let badge_bounds = Bounds::new(badge_x, text_y - 1.0, 50.0, 16.0);
                scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(theme::status::WARNING.with_alpha(0.2))
                        .with_border(theme::status::WARNING, 1.0),
                );
                let badge_run = text_system.layout(
                    badge_text,
                    Point::new(badge_x + 6.0, text_y),
                    10.0,
                    theme::status::WARNING,
                );
                scene.draw_text(badge_run);
            }

            // Description removed
        }

        let total_height = state.repos.len() as f32 * row_height;
        let visible_height = height - y;
        if total_height > visible_height {
            let scroll_track_height = visible_height - 20.0;
            let scroll_thumb_height = (visible_height / total_height) * scroll_track_height;
            let scroll_thumb_y = y + 10.0 + (state.scroll_offset / total_height) * scroll_track_height;

            scene.draw_quad(
                Quad::new(Bounds::new(content_x + content_width - 8.0, y, 4.0, scroll_track_height))
                    .with_background(theme::bg::SURFACE),
            );

            scene.draw_quad(
                Quad::new(Bounds::new(
                    content_x + content_width - 8.0,
                    scroll_thumb_y,
                    4.0,
                    scroll_thumb_height,
                ))
                    .with_background(theme::text::MUTED),
            );

            let max_scroll = total_height - visible_height;
            state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);
        }
    }

    // Sidebar disabled - Code Blocks, Files, Workspace panels removed
    // Clear sidebar-related state
    state.markdown_demo.bounds = Bounds::ZERO;
    state.markdown_demo.clear_hover();
    state.editor_workspace.bounds = Bounds::ZERO;
    state.editor_workspace.buffer_list_bounds = Bounds::ZERO;
    state.editor_workspace.buffer_row_bounds.clear();
    state.editor_workspace.split_toggle_bounds = Bounds::ZERO;
    state.editor_workspace.new_buffer_bounds = Bounds::ZERO;
    state.editor_workspace.hovered_buffer_idx = None;
    state.editor_workspace.hovered_tab = None;
    state.editor_workspace.hovered_split_toggle = false;
    state.editor_workspace.hovered_new_buffer = false;
    state.editor_workspace.clear_hover();
    for pane in &mut state.editor_workspace.panes {
        pane.bounds = Bounds::ZERO;
        pane.editor_bounds = Bounds::ZERO;
        pane.tab_bounds.clear();
    }
    state.file_list_bounds = Bounds::ZERO;
    state.file_entry_bounds.clear();
    state.file_open_bounds = Bounds::ZERO;
    state.file_save_bounds = Bounds::ZERO;
    state.file_open_hovered = false;
    state.file_save_hovered = false;
    state.hovered_file_idx = None;
}
