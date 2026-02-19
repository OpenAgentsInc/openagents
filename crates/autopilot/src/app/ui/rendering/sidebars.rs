fn render_sidebars(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
) {
    let sidebar_bg = palette.chrome;

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

        let active_workspace_id = state.workspaces.active_workspace_id.as_ref();
        let status = active_workspace_id
            .and_then(|id| state.git.status_for_workspace(id));
        let file_count = status.map(|status| status.files.len()).unwrap_or(0);
        let panel_layout = git_diff_panel_layout(right_bounds, file_count);

        let header_run = state.text_system.layout_styled_mono(
            "GIT DIFF",
            Point::new(
                panel_layout.header_bounds.origin.x,
                panel_layout.header_bounds.origin.y,
            ),
            11.0,
            palette.text_muted,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(header_run);

        let totals_text = status
            .map(|status| {
                format!("+{} / -{}", status.total_additions, status.total_deletions)
            })
            .unwrap_or_else(|| "+0 / -0".to_string());
        let totals_width = state.text_system.measure_styled_mono(
            &totals_text,
            10.0,
            wgpui::text::FontStyle::default(),
        );
        let totals_x = panel_layout.header_bounds.origin.x
            + panel_layout.header_bounds.size.width
            - totals_width
            - 2.0;
        let totals_run = state.text_system.layout_styled_mono(
            &totals_text,
            Point::new(totals_x, panel_layout.header_bounds.origin.y + 1.0),
            10.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(totals_run);

        let diff_status = if active_workspace_id.is_none() {
            "Select a workspace to view changes.".to_string()
        } else if let Some(status) = status {
            if status.error.is_some() {
                "Git status unavailable.".to_string()
            } else if status.files.is_empty() {
                "Working tree clean".to_string()
            } else {
                format!(
                    "{} file{} changed",
                    status.files.len(),
                    if status.files.len() == 1 { "" } else { "s" }
                )
            }
        } else {
            "No git data yet.".to_string()
        };
        let diff_status_run = state.text_system.layout_styled_mono(
            &diff_status,
            Point::new(
                panel_layout.status_bounds.origin.x,
                panel_layout.status_bounds.origin.y,
            ),
            11.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(diff_status_run);

        let branch_label = status
            .map(|status| status.branch_name.as_str())
            .filter(|branch| !branch.trim().is_empty())
            .unwrap_or("unknown");
        let branch_run = state.text_system.layout_styled_mono(
            branch_label,
            Point::new(
                panel_layout.branch_bounds.origin.x,
                panel_layout.branch_bounds.origin.y,
            ),
            12.0,
            palette.text_primary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(branch_run);

        let list_origin = panel_layout.list_bounds.origin;
        let list_width = panel_layout.list_bounds.size.width;
        if let Some(status) = status {
            if let Some(error) = status.error.as_ref() {
                let error_text = truncate_preview(error, 140);
                let error_run = state.text_system.layout_styled_mono(
                    &error_text,
                    Point::new(list_origin.x, list_origin.y),
                    11.0,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(error_run);
            } else if status.files.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No changes detected.",
                    Point::new(list_origin.x, list_origin.y),
                    11.0,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for (index, row_bounds) in &panel_layout.row_bounds {
                    if let Some(file) = status.files.get(*index) {
                        let is_selected = state
                            .git
                            .selected_diff_path
                            .as_ref()
                            .map(|path| path == &file.path)
                            .unwrap_or(false);
                        if is_selected {
                            scene.draw_quad(
                                Quad::new(*row_bounds)
                                    .with_background(palette.panel_highlight)
                                    .with_border(palette.link, 1.0)
                                    .with_corner_radius(8.0),
                            );
                        }
                        let status_color = git_status_color(&file.status, palette);
                        let status_bounds = Bounds::new(
                            row_bounds.origin.x + 6.0,
                            row_bounds.origin.y + 8.0,
                            18.0,
                            18.0,
                        );
                        scene.draw_quad(
                            Quad::new(status_bounds)
                                .with_background(palette.panel_highlight)
                                .with_corner_radius(5.0),
                        );
                        let status_run = state.text_system.layout_styled_mono(
                            &file.status,
                            Point::new(status_bounds.origin.x + 5.0, status_bounds.origin.y + 4.0),
                            10.0,
                            status_color,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(status_run);

                        let path_text = truncate_preview(&file.path, 44);
                        let path_run = state.text_system.layout_styled_mono(
                            &path_text,
                            Point::new(row_bounds.origin.x + 30.0, row_bounds.origin.y + 7.0),
                            11.0,
                            palette.text_primary,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(path_run);

                        let additions_text = format!("+{}", file.additions);
                        let deletions_text = format!("-{}", file.deletions);
                        let sep_text = "/";
                        let add_width = state.text_system.measure_styled_mono(
                            &additions_text,
                            10.0,
                            wgpui::text::FontStyle::default(),
                        );
                        let sep_width = state.text_system.measure_styled_mono(
                            sep_text,
                            10.0,
                            wgpui::text::FontStyle::default(),
                        );
                        let del_width = state.text_system.measure_styled_mono(
                            &deletions_text,
                            10.0,
                            wgpui::text::FontStyle::default(),
                        );
                        let counts_width = add_width + sep_width + del_width + 4.0;
                        let counts_x = row_bounds.origin.x + list_width - counts_width - 4.0;
                        let counts_y = row_bounds.origin.y + 9.0;
                        let add_run = state.text_system.layout_styled_mono(
                            &additions_text,
                            Point::new(counts_x, counts_y),
                            10.0,
                            Hsla::new(140.0, 0.6, 0.45, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(add_run);
                        let sep_run = state.text_system.layout_styled_mono(
                            sep_text,
                            Point::new(counts_x + add_width + 2.0, counts_y),
                            10.0,
                            palette.text_dim,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(sep_run);
                        let del_run = state.text_system.layout_styled_mono(
                            &deletions_text,
                            Point::new(counts_x + add_width + sep_width + 4.0, counts_y),
                            10.0,
                            Hsla::new(0.0, 0.6, 0.55, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(del_run);
                    }
                }
            }
        } else if active_workspace_id.is_some() {
            let empty_run = state.text_system.layout_styled_mono(
                "Loading git status...",
                Point::new(list_origin.x, list_origin.y),
                11.0,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(empty_run);
        }

        let approvals = state.workspaces.approvals_for_active();
        if let Some(approvals_layout) =
            approvals_panel_layout(right_bounds, &panel_layout, approvals.len())
        {
            scene.draw_quad(
                Quad::new(approvals_layout.panel_bounds)
                    .with_background(palette.panel_highlight)
                    .with_border(palette.panel_border, 1.0)
                    .with_corner_radius(8.0),
            );
            let heading_x = approvals_layout.panel_bounds.origin.x + 10.0;
            let heading_y = approvals_layout.panel_bounds.origin.y + 10.0;
            let approvals_heading_run = state.text_system.layout_styled_mono(
                "APPROVALS",
                Point::new(heading_x, heading_y),
                11.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(approvals_heading_run);

            if approvals.is_empty() {
                let approvals_body_run = state.text_system.layout_styled_mono(
                    "No approvals pending.",
                    Point::new(heading_x, heading_y + 18.0),
                    11.0,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(approvals_body_run);
            } else {
                for (index, card_bounds) in &approvals_layout.card_bounds {
                    if let Some(request) = approvals.get(*index) {
                        scene.draw_quad(
                            Quad::new(*card_bounds)
                                .with_background(palette.panel)
                                .with_border(palette.panel_border, 1.0)
                                .with_corner_radius(8.0),
                        );
                        let method_text = truncate_preview(&request.method, 40);
                        let method_run = state.text_system.layout_styled_mono(
                            &method_text,
                            Point::new(card_bounds.origin.x + 8.0, card_bounds.origin.y + 6.0),
                            11.0,
                            palette.text_primary,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(method_run);

                        let params_text = serde_json::to_string_pretty(&request.params)
                            .unwrap_or_default();
                        let params_text = truncate_preview(&params_text, 120);
                        let params_run = state.text_system.layout_styled_mono(
                            &params_text,
                            Point::new(card_bounds.origin.x + 8.0, card_bounds.origin.y + 22.0),
                            10.0,
                            palette.text_dim,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(params_run);
                    }
                }
                for (index, bounds) in &approvals_layout.decline_bounds {
                    if approvals.get(*index).is_some() {
                        scene.draw_quad(
                            Quad::new(*bounds)
                                .with_background(palette.panel)
                                .with_border(palette.panel_border, 1.0)
                                .with_corner_radius(6.0),
                        );
                        let run = state.text_system.layout_styled_mono(
                            "Decline",
                            Point::new(bounds.origin.x + 6.0, bounds.origin.y + 3.0),
                            10.0,
                            palette.text_secondary,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(run);
                    }
                }
                for (index, bounds) in &approvals_layout.approve_bounds {
                    if approvals.get(*index).is_some() {
                        scene.draw_quad(
                            Quad::new(*bounds)
                                .with_background(palette.link)
                                .with_border(palette.panel_border, 1.0)
                                .with_corner_radius(6.0),
                        );
                        let run = state.text_system.layout_styled_mono(
                            "Approve",
                            Point::new(bounds.origin.x + 6.0, bounds.origin.y + 3.0),
                            10.0,
                            palette.text_primary,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(run);
                    }
                }
            }
        }
    }
}

fn git_status_color(status: &str, palette: &UiPalette) -> Hsla {
    match status {
        "A" => Hsla::new(140.0, 0.6, 0.45, 1.0),
        "M" => Hsla::new(45.0, 0.65, 0.5, 1.0),
        "D" => Hsla::new(0.0, 0.6, 0.55, 1.0),
        "R" => Hsla::new(200.0, 0.6, 0.55, 1.0),
        "T" => Hsla::new(280.0, 0.5, 0.55, 1.0),
        _ => palette.text_dim,
    }
}
