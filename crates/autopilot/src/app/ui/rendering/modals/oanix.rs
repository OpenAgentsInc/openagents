use crate::app::oanix::{
    format_backend_summary, format_bool, format_bytes, format_gpu_summary, format_relay_summary,
};

fn draw_oanix_row(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    label_x: f32,
    value_x: f32,
    y: &mut f32,
    line_height: f32,
    label: &str,
    value: &str,
    value_color: Hsla,
) {
            let label_run = state.text_system.layout_styled_mono(
                label,
                Point::new(label_x, *y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(label_run);
            let value_run = state.text_system.layout_styled_mono(
                value,
                Point::new(value_x, *y),
                12.0,
                value_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(value_run);
            *y += line_height;
}

fn draw_oanix_wrapped_row(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    label_x: f32,
    value_x: f32,
    y: &mut f32,
    line_height: f32,
    label: &str,
    value: &str,
    value_color: Hsla,
    max_chars: usize,
) {
            let lines = wrap_text(value, max_chars);
            if lines.is_empty() {
                return;
            }
            for (idx, line) in lines.into_iter().enumerate() {
                let row_label = if idx == 0 { label } else { "" };
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    y,
                    line_height,
                    row_label,
                    &line,
                    value_color,
                );
            }
}

fn render_oanix_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
) {
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = OANIX_MODAL_WIDTH;
            let modal_height = OANIX_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "OANIX Manifest",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            if state.autopilot.oanix_manifest_rx.is_some() {
                let pending_run = state.text_system.layout_styled_mono(
                    "Discovery in progress...",
                    Point::new(modal_x + 16.0, y),
                    11.0,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(pending_run);
                y += 18.0;
            }

            let manifest = state.autopilot.oanix_manifest.clone();
            let Some(manifest) = manifest else {
                let hint_run = state.text_system.layout_styled_mono(
                    "No manifest available. Press R to refresh.",
                    Point::new(modal_x + 16.0, y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(hint_run);
                return;
            };

            let line_height = 18.0;
            let section_gap = 10.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 170.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let draw_section = |title: &str,
                                state: &mut AppState,
                                scene: &mut Scene,
                                y: &mut f32| {
                let header = state.text_system.layout_styled_mono(
                    title,
                    Point::new(label_x, *y),
                    12.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(header);
                *y += line_height;
            };

            draw_section("Hardware", state, scene, &mut y);
            let cpu_value = format!(
                "{} cores • {}",
                manifest.hardware.cpu_cores,
                manifest.hardware.cpu_model
            );
            draw_oanix_wrapped_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "CPU",
                &cpu_value,
                palette.text_secondary,
                max_chars,
            );
            let ram_value = format!(
                "{} total • {} free",
                format_bytes(manifest.hardware.ram_bytes),
                format_bytes(manifest.hardware.ram_available)
            );
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "RAM",
                &ram_value,
                palette.text_secondary,
            );
            if manifest.hardware.gpus.is_empty() {
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "GPU",
                    "None",
                    palette.text_secondary,
                );
            } else {
                let max_gpus = 3usize;
                for (idx, gpu) in manifest.hardware.gpus.iter().take(max_gpus).enumerate() {
                    let label = if idx == 0 { "GPU" } else { "" };
                    let summary = truncate_preview(&format_gpu_summary(gpu), max_chars);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        label,
                        &summary,
                        palette.text_secondary,
                    );
                }
                if manifest.hardware.gpus.len() > max_gpus {
                    let extra = format!("+{} more", manifest.hardware.gpus.len() - max_gpus);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        "",
                        &extra,
                        palette.text_faint,
                    );
                }
            }

            y += section_gap;
            draw_section("Compute", state, scene, &mut y);
            let backend_count = manifest.compute.backends.len();
            let backend_summary = format!(
                "{} backends • {} models",
                backend_count, manifest.compute.total_models
            );
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Backends",
                &backend_summary,
                palette.text_secondary,
            );
            if backend_count == 0 {
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Details",
                    "No backends detected",
                    palette.text_faint,
                );
            } else {
                let max_backends = 3usize;
                for (idx, backend) in manifest.compute.backends.iter().take(max_backends).enumerate() {
                    let label = if idx == 0 { "Details" } else { "" };
                    let summary = truncate_preview(&format_backend_summary(backend), max_chars);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        label,
                        &summary,
                        palette.text_secondary,
                    );
                }
                if backend_count > max_backends {
                    let extra = format!("+{} more", backend_count - max_backends);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        "",
                        &extra,
                        palette.text_faint,
                    );
                }
            }

            y += section_gap;
            draw_section("Network", state, scene, &mut y);
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Internet",
                format_bool(manifest.network.has_internet),
                palette.text_secondary,
            );
            let relay_count = manifest.network.relays.len();
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Relays",
                &format!("{}", relay_count),
                palette.text_secondary,
            );
            if relay_count > 0 {
                let max_relays = 2usize;
                for (idx, relay) in manifest.network.relays.iter().take(max_relays).enumerate() {
                    let label = if idx == 0 { "Relay info" } else { "" };
                    let summary = truncate_preview(&format_relay_summary(relay), max_chars);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        label,
                        &summary,
                        palette.text_secondary,
                    );
                }
                if relay_count > max_relays {
                    let extra = format!("+{} more", relay_count - max_relays);
                    draw_oanix_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        "",
                        &extra,
                        palette.text_faint,
                    );
                }
            }
            let providers_summary = format!(
                "{} providers • {} pylons • {} online",
                manifest.network.total_providers,
                manifest.network.pylon_count,
                manifest.network.pylons_online
            );
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Providers",
                &providers_summary,
                palette.text_secondary,
            );

            y += section_gap;
            draw_section("Identity", state, scene, &mut y);
            let initialized = format_bool(manifest.identity.initialized);
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Initialized",
                initialized,
                palette.text_secondary,
            );
            let npub = manifest
                .identity
                .npub
                .as_deref()
                .map(|value| truncate_preview(value, max_chars))
                .unwrap_or_else(|| "Not set".to_string());
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Npub",
                &npub,
                palette.text_secondary,
            );
            let balance = manifest
                .identity
                .wallet_balance_sats
                .map(|value| format!("{} sats", value))
                .unwrap_or_else(|| "Unavailable".to_string());
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Balance",
                &balance,
                palette.text_secondary,
            );
            let network = manifest
                .identity
                .network
                .as_deref()
                .unwrap_or("Unknown");
            draw_oanix_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Network",
                network,
                palette.text_secondary,
            );

            y += section_gap;
            draw_section("Workspace", state, scene, &mut y);
            if let Some(workspace) = &manifest.workspace {
                let root = truncate_preview(&workspace.root.to_string_lossy(), max_chars);
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Root",
                    &root,
                    palette.text_secondary,
                );
                let project = workspace
                    .project_name
                    .as_deref()
                    .unwrap_or("Unknown");
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Project",
                    project,
                    palette.text_secondary,
                );
                let directives_summary = format!(
                    "{} directives • {} open issues • {} pending",
                    workspace.directives.len(),
                    workspace.open_issues,
                    workspace.pending_issues
                );
                draw_oanix_wrapped_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Tracking",
                    &directives_summary,
                    palette.text_secondary,
                    max_chars,
                );
            } else {
                draw_oanix_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Workspace",
                    "Not detected",
                    palette.text_faint,
                );
            }

            let footer_y = modal_y + modal_height - 24.0;
            let footer = state.text_system.layout_styled_mono(
                "R refresh • Esc close",
                Point::new(modal_x + 16.0, footer_y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}
