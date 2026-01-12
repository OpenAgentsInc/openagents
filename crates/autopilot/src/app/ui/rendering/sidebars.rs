fn render_sidebars(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
) {
    let sidebar_bg = palette.panel;

    if let Some(left_bounds) = sidebar_layout.left {
        scene.draw_quad(
            Quad::new(left_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        let header_y = left_bounds.origin.y + 18.0;
        let header_run = state.text_system.layout_styled_mono(
            "Workspaces",
            Point::new(left_bounds.origin.x + 16.0, header_y),
            14.0,
            palette.text_primary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(header_run);

        let btn_bounds = new_session_button_bounds(left_bounds);
        let btn_bg = if state.new_session_button_hovered {
            palette.panel_highlight
        } else {
            palette.panel
        };
        scene.draw_quad(
            Quad::new(btn_bounds)
                .with_background(btn_bg)
                .with_border(palette.panel_border, 1.0)
                .with_corner_radius(btn_bounds.size.height / 2.0),
        );
        let btn_text_y = btn_bounds.origin.y + (btn_bounds.size.height - 12.0) / 2.0;
        let btn_run = state.text_system.layout_styled_mono(
            "+",
            Point::new(btn_bounds.origin.x + 7.0, btn_text_y),
            12.0,
            palette.text_primary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(btn_run);

        let list_layout = workspace_list_layout(left_bounds, state.workspaces.workspaces.len());
        if state.workspaces.workspaces.is_empty() {
            if let Some(empty_bounds) = list_layout.empty_bounds {
                let empty_run = state.text_system.layout_styled_mono(
                    "Add a workspace to start.",
                    Point::new(empty_bounds.origin.x, empty_bounds.origin.y),
                    12.0,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            }
        } else {
            for (index, workspace) in state.workspaces.workspaces.iter().enumerate() {
                if index >= list_layout.rows.len() {
                    break;
                }
                let row_bounds = list_layout.rows[index];
                let is_active = state
                    .workspaces
                    .active_workspace_id
                    .as_ref()
                    .map(|id| id == &workspace.id)
                    .unwrap_or(false);
                if is_active {
                    scene.draw_quad(
                        Quad::new(Bounds::new(
                            row_bounds.origin.x - 4.0,
                            row_bounds.origin.y,
                            3.0,
                            row_bounds.size.height,
                        ))
                        .with_background(palette.link),
                    );
                }
                let name_color = if is_active {
                    palette.text_primary
                } else {
                    palette.text_secondary
                };
                let name_run = state.text_system.layout_styled_mono(
                    &workspace.name,
                    Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 7.0),
                    13.0,
                    name_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(name_run);
                if !workspace.connected {
                    let connect_bounds = list_layout.connect_pills[index];
                    scene.draw_quad(
                        Quad::new(connect_bounds)
                            .with_background(palette.panel_highlight)
                            .with_border(palette.panel_border, 1.0)
                            .with_corner_radius(connect_bounds.size.height / 2.0),
                    );
                    let connect_run = state.text_system.layout_styled_mono(
                        "connect",
                        Point::new(connect_bounds.origin.x + 8.0, connect_bounds.origin.y + 3.0),
                        10.0,
                        palette.text_secondary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(connect_run);
                }
            }
        }
    }

    if let Some(right_bounds) = sidebar_layout.right {
        scene.draw_quad(
            Quad::new(right_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        let panel_padding = 12.0;
        let panel_x = right_bounds.origin.x + panel_padding;
        let panel_width = (right_bounds.size.width - panel_padding * 2.0).max(0.0);
        let mut y = right_bounds.origin.y + panel_padding;

        let diff_heading_run = state.text_system.layout_styled_mono(
            "GIT DIFF",
            Point::new(panel_x, y),
            11.0,
            palette.text_muted,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(diff_heading_run);
        y += 18.0;

        let diff_status = if state.workspaces.active_workspace_id.is_some() {
            "No git data yet."
        } else {
            "Select a workspace to view changes."
        };
        let diff_status_run = state.text_system.layout_styled_mono(
            diff_status,
            Point::new(panel_x, y),
            11.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(diff_status_run);
        y += 28.0;

        let approvals_height =
            (right_bounds.origin.y + right_bounds.size.height - y - panel_padding).max(0.0);
        if approvals_height > 32.0 {
            let approvals_bounds = Bounds::new(panel_x, y, panel_width, approvals_height);
            scene.draw_quad(
                Quad::new(approvals_bounds)
                    .with_background(palette.panel_highlight)
                    .with_border(palette.panel_border, 1.0)
                    .with_corner_radius(8.0),
            );
            let heading_x = approvals_bounds.origin.x + 10.0;
            let heading_y = approvals_bounds.origin.y + 10.0;
            let approvals_heading_run = state.text_system.layout_styled_mono(
                "APPROVALS",
                Point::new(heading_x, heading_y),
                11.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(approvals_heading_run);
            let approvals_body_run = state.text_system.layout_styled_mono(
                "No approvals pending.",
                Point::new(heading_x, heading_y + 18.0),
                11.0,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(approvals_body_run);
        }
    }
}

fn render_topbar(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
) {
    let topbar_bounds = Bounds::new(
        sidebar_layout.main.origin.x,
        sidebar_layout.main.origin.y,
        sidebar_layout.main.size.width,
        TOPBAR_HEIGHT,
    );
    scene.draw_quad(Quad::new(topbar_bounds).with_background(palette.panel));
    scene.draw_quad(
        Quad::new(Bounds::new(
            topbar_bounds.origin.x,
            topbar_bounds.origin.y + topbar_bounds.size.height - 1.0,
            topbar_bounds.size.width,
            1.0,
        ))
        .with_background(palette.panel_border),
    );

    let workspace = state
        .workspaces
        .active_workspace_id
        .as_ref()
        .and_then(|id| state.workspaces.workspaces.iter().find(|ws| &ws.id == id));
    let branch_label = "unknown";
    let branch_text = state.text_system.layout_styled_mono(
        branch_label,
        Point::new(topbar_bounds.origin.x + 14.0, topbar_bounds.origin.y + 14.0),
        11.0,
        palette.text_secondary,
        wgpui::text::FontStyle::default(),
    );
    let pill_width = state
        .text_system
        .measure_styled_mono(branch_label, 11.0, wgpui::text::FontStyle::default())
        .max(1.0)
        + 16.0;
    scene.draw_quad(
        Quad::new(Bounds::new(
            topbar_bounds.origin.x + 10.0,
            topbar_bounds.origin.y + 10.0,
            pill_width,
            20.0,
        ))
        .with_background(palette.panel_highlight)
        .with_border(palette.panel_border, 1.0)
        .with_corner_radius(10.0),
    );
    scene.draw_text(branch_text);

    let title_x = topbar_bounds.origin.x + 10.0 + pill_width + 12.0;
    let title = workspace
        .map(|ws| ws.name.as_str())
        .unwrap_or("No workspace");
    let title_run = state.text_system.layout_styled_mono(
        title,
        Point::new(title_x, topbar_bounds.origin.y + 12.0),
        14.0,
        palette.text_primary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);

    let path_text = workspace
        .map(|ws| ws.path.as_str())
        .unwrap_or("Add a workspace to begin.");
    let path_run = state.text_system.layout_styled_mono(
        truncate_preview(path_text, 80).as_str(),
        Point::new(title_x, topbar_bounds.origin.y + 28.0),
        10.0,
        palette.text_muted,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(path_run);
}
