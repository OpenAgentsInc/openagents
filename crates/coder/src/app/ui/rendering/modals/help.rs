fn render_help_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
    scroll_offset: f32,
) {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = HELP_MODAL_WIDTH;
            let modal_height = HELP_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            // Content area bounds for clipping
            let content_top = modal_y + 36.0;  // Below title
            let content_bottom = modal_y + modal_height - 30.0;  // Above footer

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Help (scroll to see more)",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            // Apply scroll offset to content starting position
            y -= scroll_offset;

            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;
            let line_height = 14.0;
            let section_gap = 6.0;

            let interrupt =
                keybinding_labels(&state.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
            let palette_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenCommandPalette,
                "Ctrl+K",
            );
            let settings_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
            let agent_backends_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenAgentBackends,
                "Ctrl+Shift+B",
            );
            let dvm_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenDvm, "Ctrl+Shift+P");
            let gateway_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenGateway,
                "Ctrl+Shift+G",
            );
            let lm_router_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenLmRouter,
                "Ctrl+Shift+L",
            );
            let nexus_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenNexus,
                "Ctrl+Shift+X",
            );
            let nip90_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenNip90, "Ctrl+Shift+J");
            let oanix_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenOanix, "Ctrl+Shift+O");
            let directives_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenDirectives,
                "Ctrl+Shift+T",
            );
            let issues_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenIssues, "Ctrl+Shift+I");
            let tracker_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenIssueTracker,
                "Ctrl+Shift+A",
            );
            let nip28_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenNip28, "Ctrl+Shift+N");
            let left_sidebar = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleLeftSidebar,
                "Ctrl+[",
            );
            let right_sidebar = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleRightSidebar,
                "Ctrl+]",
            );
            let toggle_sidebars = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleSidebars,
                "Ctrl+\\",
            );

            let sections: Vec<(&str, Vec<String>)> = vec![
                (
                    "Hotkeys",
                    vec![
                        "F1 - Help".to_string(),
                        "Enter - Send message".to_string(),
                        format!("{} - Interrupt request", interrupt),
                        format!("{} - Command palette", palette_key),
                        format!("{} - Settings", settings_key),
                        format!("{} - Agent backends", agent_backends_key),
                        format!("{} - DVM providers", dvm_key),
                        format!("{} - Gateway health", gateway_key),
                        format!("{} - LM router", lm_router_key),
                        format!("{} - Nexus stats", nexus_key),
                        format!("{} - NIP-90 jobs", nip90_key),
                        format!("{} - OANIX manifest", oanix_key),
                        format!("{} - Directives", directives_key),
                        format!("{} - Issues", issues_key),
                        format!("{} - Issue tracker", tracker_key),
                        format!("{} - NIP-28 chat", nip28_key),
                        format!("{} - Toggle left sidebar", left_sidebar),
                        format!("{} - Toggle right sidebar", right_sidebar),
                        format!("{} - Toggle both sidebars", toggle_sidebars),
                    ],
                ),
                (
                    "Core",
                    vec![
                        "/clear - reset chat; /compact - compact context; /undo - undo last exchange"
                            .to_string(),
                        "/cancel - cancel active run; /bug - report issue".to_string(),
                    ],
                ),
                (
                    "Sessions",
                    vec![
                        "/session list - list sessions; /session resume <id> - resume".to_string(),
                        "/session fork - fork current; /session export - export markdown".to_string(),
                    ],
                ),
                (
                    "Permissions",
                    vec![
                        "/permission rules - manage rules".to_string(),
                        "/permission allow|deny <tool|bash:pattern>".to_string(),
                    ],
                ),
                (
                    "Tools, MCP, Hooks",
                    vec![
                        "/tools - list tools; /tools enable|disable <tool>".to_string(),
                        "/mcp - open MCP servers; /mcp add|remove <name> <json>".to_string(),
                        "/hooks - hook panel; /hooks reload - reload scripts".to_string(),
                    ],
                ),
                (
                    "Agents, Skills",
                    vec![
                        "/agents - manage agents; /agent select <name>; /agent clear".to_string(),
                        "/agent-backends - backend status; /agent-backends refresh".to_string(),
                        "/skills - manage skills; /skills reload".to_string(),
                        "@file - insert file; !command - run bash and insert output".to_string(),
                    ],
                ),
                (
                    "Network Services",
                    vec![
                        "/dvm - DVM providers; /gateway - gateway health".to_string(),
                        "/lm-router - LM router; /nexus - Nexus stats".to_string(),
                        "/nip90 - NIP-90 jobs; /nip28 - NIP-28 chat".to_string(),
                    ],
                ),
                (
                    "Workspace",
                    vec![
                        "/oanix - OANIX manifest; /directives - workspace directives".to_string(),
                        "/issues - workspace issues; /issue-tracker - autopilot issues".to_string(),
                    ],
                ),
            ];

            for (title, lines) in sections {
                // Only draw if within visible content area
                if y >= content_top - line_height && y <= content_bottom {
                    let heading = state.text_system.layout_styled_mono(
                        title,
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        palette.text_primary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(heading);
                }
                y += line_height;

                for line in lines {
                    for wrapped in wrap_text(&line, max_chars) {
                        // Only draw if within visible content area
                        if y >= content_top - line_height && y <= content_bottom {
                            let text_run = state.text_system.layout_styled_mono(
                                &wrapped,
                                Point::new(modal_x + 20.0, y),
                                11.0,
                                palette.text_muted,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(text_run);
                        }
                        y += line_height;
                    }
                }

                y += section_gap;
            }

            // Draw footer (always visible, at fixed position)
            let footer_y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Esc/F1 to close",
                Point::new(modal_x + 16.0, footer_y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
