fn render_help_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
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

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Help",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

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
            let oanix_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenOanix, "Ctrl+Shift+O");
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
                        format!("{} - OANIX manifest", oanix_key),
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
                        "/skills - manage skills; /skills reload".to_string(),
                        "@file - insert file; !command - run bash and insert output".to_string(),
                    ],
                ),
                (
                    "Wallet",
                    vec!["/wallet - open wallet status; /wallet refresh - refresh".to_string()],
                ),
                (
                    "OANIX",
                    vec![
                        "/oanix - open manifest; /oanix refresh - refresh".to_string(),
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
                let heading = state.text_system.layout_styled_mono(
                    title,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(heading);
                y += line_height;

                for line in lines {
                    for wrapped in wrap_text(&line, max_chars) {
                        let text_run = state.text_system.layout_styled_mono(
                            &wrapped,
                            Point::new(modal_x + 20.0, y),
                            11.0,
                            palette.text_muted,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(text_run);
                        y += line_height;
                    }
                }

                y += section_gap;
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Esc/F1 to close",
                Point::new(modal_x + 16.0, y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
