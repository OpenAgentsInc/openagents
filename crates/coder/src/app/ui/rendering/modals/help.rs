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
            let wallet_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenWallet, "Ctrl+Shift+W");
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
            let spark_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenSparkWallet,
                "Ctrl+Shift+S",
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
            let rlm_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenRlm, "Ctrl+Shift+R");
            let rlm_trace_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenRlmTrace, "Ctrl+Shift+Y");
            let pylon_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenPylonEarnings,
                "Ctrl+Shift+E",
            );
            let pylon_jobs_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenPylonJobs,
                "Ctrl+Shift+U",
            );
            let dspy_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenDspy, "Ctrl+Shift+D");
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
                        "Shift+Tab - Cycle permission mode".to_string(),
                        format!("{} - Interrupt request", interrupt),
                        format!("{} - Command palette", palette_key),
                        format!("{} - Settings", settings_key),
                        format!("{} - Wallet", wallet_key),
                        format!("{} - Agent backends", agent_backends_key),
                        format!("{} - DVM providers", dvm_key),
                        format!("{} - Gateway health", gateway_key),
                        format!("{} - LM router", lm_router_key),
                        format!("{} - Nexus stats", nexus_key),
                        format!("{} - Spark wallet", spark_key),
                        format!("{} - NIP-90 jobs", nip90_key),
                        format!("{} - OANIX manifest", oanix_key),
                        format!("{} - Directives", directives_key),
                        format!("{} - Issues", issues_key),
                        format!("{} - Issue tracker", tracker_key),
                        format!("{} - RLM runs", rlm_key),
                        format!("{} - RLM trace", rlm_trace_key),
                        format!("{} - Pylon earnings", pylon_key),
                        format!("{} - Pylon jobs", pylon_jobs_key),
                        format!("{} - DSPy status", dspy_key),
                        format!("{} - NIP-28 chat", nip28_key),
                        format!("{} - Toggle left sidebar", left_sidebar),
                        format!("{} - Toggle right sidebar", right_sidebar),
                        format!("{} - Toggle both sidebars", toggle_sidebars),
                    ],
                ),
                (
                    "Core",
                    vec![
                        "/model - choose model; /output-style <name> - style output".to_string(),
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
                        "/permission mode <default|plan|acceptEdits|bypassPermissions|dontAsk>"
                            .to_string(),
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
                    "Agents, Skills, Prompts",
                    vec![
                        "/agents - manage agents; /agent select <name>; /agent clear".to_string(),
                        "/agent-backends - backend status; /agent-backends refresh".to_string(),
                        "/skills - manage skills; /skills reload".to_string(),
                        "@file - insert file; !command - run bash and insert output".to_string(),
                    ],
                ),
                (
                    "Wallet",
                    vec!["/wallet - open wallet status; /wallet refresh - refresh".to_string()],
                ),
                (
                    "DVM Providers",
                    vec![
                        "/dvm - open providers".to_string(),
                        "/dvm connect <relay_url>; /dvm kind <id>".to_string(),
                        "/dvm refresh - refresh".to_string(),
                    ],
                ),
                (
                    "Gateway",
                    vec![
                        "/gateway - open health".to_string(),
                        "/gateway refresh - refresh".to_string(),
                    ],
                ),
                (
                    "LM Router",
                    vec![
                        "/lm-router - open status".to_string(),
                        "/lm-router refresh - refresh".to_string(),
                    ],
                ),
                (
                    "Nexus",
                    vec![
                        "/nexus - open stats".to_string(),
                        "/nexus connect <stats_url>; /nexus refresh".to_string(),
                    ],
                ),
                (
                    "Spark Wallet",
                    vec![
                        "/spark - open status".to_string(),
                        "/spark refresh - refresh".to_string(),
                    ],
                ),
                (
                    "NIP-90 Jobs",
                    vec![
                        "/nip90 - open job monitor".to_string(),
                        "/nip90 connect <relay_url>; /nip90 refresh".to_string(),
                    ],
                ),
                (
                    "OANIX",
                    vec![
                        "/oanix - open manifest; /oanix refresh - refresh".to_string(),
                    ],
                ),
                (
                    "Directives",
                    vec![
                        "/directives - open directives; /directives refresh - refresh".to_string(),
                    ],
                ),
                (
                    "Issues",
                    vec![
                        "/issues - open workspace issues; /issues refresh - refresh".to_string(),
                    ],
                ),
                (
                    "Issue Tracker",
                    vec![
                        "/issue-tracker - open tracker; /issue-tracker refresh - refresh"
                            .to_string(),
                    ],
                ),
                (
                    "RLM",
                    vec![
                        "/rlm - open run history; /rlm refresh - refresh".to_string(),
                        "/rlm trace [run_id] - open trace events".to_string(),
                    ],
                ),
                (
                    "Pylon",
                    vec![
                        "/pylon - open earnings; /pylon refresh - refresh".to_string(),
                        "/pylon earnings - open earnings; /pylon earnings refresh".to_string(),
                        "/pylon jobs - open jobs; /pylon jobs refresh".to_string(),
                    ],
                ),
                (
                    "DSPy",
                    vec![
                        "/dspy - open status; /dspy refresh - refresh".to_string(),
                        "/dspy auto on|off; /dspy background on|off".to_string(),
                    ],
                ),
                (
                    "NIP-28 Chat",
                    vec![
                        "/nip28 - open chat; /nip28 connect <relay_url>".to_string(),
                        "/nip28 channel <id|name>; /nip28 send <message>".to_string(),
                        "/nip28 refresh - reconnect".to_string(),
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
