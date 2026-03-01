use openagents_cad::analysis::{
    DENSITY_ALUMINUM_6061_KG_M3, edge_properties, estimate_body_properties, face_properties,
};
use openagents_cad::contracts::CadSelectionKind;
use openagents_cad::mesh::CadMeshPayload;
use wgpui::{
    Bounds, Hsla, MESH_EDGE_FLAG_SELECTED, MESH_EDGE_FLAG_SILHOUETTE, MeshEdge, MeshPrimitive,
    MeshVertex, PaintContext, Point, Quad, theme,
};

use crate::app_state::{
    CadCameraViewSnap, CadDemoPaneState, CadDemoWarningState, CadHiddenLineMode, CadProjectionMode,
    CadVariantViewportState,
};
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{
    cad_demo_context_menu_bounds, cad_demo_context_menu_row_bounds,
    cad_demo_cycle_variant_button_bounds, cad_demo_hidden_line_mode_button_bounds,
    cad_demo_hotkey_profile_button_bounds, cad_demo_projection_mode_button_bounds,
    cad_demo_reset_button_bounds, cad_demo_reset_camera_button_bounds,
    cad_demo_snap_endpoint_button_bounds, cad_demo_snap_grid_button_bounds,
    cad_demo_snap_midpoint_button_bounds, cad_demo_snap_origin_button_bounds,
    cad_demo_timeline_panel_bounds, cad_demo_timeline_row_bounds, cad_demo_view_cube_bounds,
    cad_demo_view_snap_front_button_bounds, cad_demo_view_snap_iso_button_bounds,
    cad_demo_view_snap_right_button_bounds, cad_demo_view_snap_top_button_bounds,
    cad_demo_warning_filter_code_button_bounds, cad_demo_warning_filter_severity_button_bounds,
    cad_demo_warning_marker_bounds, cad_demo_warning_panel_bounds, cad_demo_warning_row_bounds,
};

const PAD: f32 = 12.0;
const HEADER_LINE_HEIGHT: f32 = 14.0;
const SUBHEADER_GAP: f32 = 4.0;
const VIEWPORT_TOP_GAP: f32 = 10.0;
const FOOTER_RESERVED: f32 = 18.0;
const VIEWPORT_MESH_PAD: f32 = 12.0;
const VARIANT_TILE_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadCameraPose {
    pub projection_mode: CadProjectionMode,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub orbit_yaw_deg: f32,
    pub orbit_pitch_deg: f32,
}

impl CadCameraPose {
    fn from_variant(state: &CadVariantViewportState, projection_mode: CadProjectionMode) -> Self {
        Self {
            projection_mode,
            zoom: state.camera_zoom,
            pan_x: state.camera_pan_x,
            pan_y: state.camera_pan_y,
            orbit_yaw_deg: state.camera_orbit_yaw_deg,
            orbit_pitch_deg: state.camera_orbit_pitch_deg,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadDemoPlaceholderLayout {
    pub header_origin: Point,
    pub subheader_origin: Point,
    pub viewport_bounds: Bounds,
    pub footer_origin: Point,
}

/// Compute a bounded placeholder layout that never exceeds pane content bounds.
pub fn placeholder_layout(content_bounds: Bounds) -> CadDemoPlaceholderLayout {
    let max_x = content_bounds.max_x();
    let max_y = content_bounds.max_y();
    let inner_width = (content_bounds.size.width - PAD * 2.0).max(0.0);

    let header_y = (content_bounds.origin.y + PAD).min(max_y);
    let subheader_y = (header_y + HEADER_LINE_HEIGHT + SUBHEADER_GAP).min(max_y);
    let viewport_top = (subheader_y + HEADER_LINE_HEIGHT + VIEWPORT_TOP_GAP).min(max_y);
    let viewport_bottom = (max_y - FOOTER_RESERVED).max(viewport_top);
    let viewport_height = (viewport_bottom - viewport_top).max(0.0);
    let viewport_bounds = Bounds::new(
        (content_bounds.origin.x + PAD).min(max_x),
        viewport_top,
        inner_width,
        viewport_height,
    );

    let footer_y = (max_y - 12.0).max(content_bounds.origin.y);
    CadDemoPlaceholderLayout {
        header_origin: Point::new(content_bounds.origin.x + PAD, header_y),
        subheader_origin: Point::new(content_bounds.origin.x + PAD, subheader_y),
        viewport_bounds,
        footer_origin: Point::new(content_bounds.origin.x + PAD, footer_y),
    }
}

fn cad_demo_body_bounds(content_bounds: Bounds) -> Bounds {
    let cycle_bounds = cad_demo_cycle_variant_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let hidden_line_bounds = cad_demo_hidden_line_mode_button_bounds(content_bounds);
    let reset_camera_bounds = cad_demo_reset_camera_button_bounds(content_bounds);
    let projection_bounds = cad_demo_projection_mode_button_bounds(content_bounds);
    let snap_grid_bounds = cad_demo_snap_grid_button_bounds(content_bounds);
    let snap_origin_bounds = cad_demo_snap_origin_button_bounds(content_bounds);
    let snap_endpoint_bounds = cad_demo_snap_endpoint_button_bounds(content_bounds);
    let snap_midpoint_bounds = cad_demo_snap_midpoint_button_bounds(content_bounds);
    let hotkey_bounds = cad_demo_hotkey_profile_button_bounds(content_bounds);
    let body_top = (cycle_bounds
        .max_y()
        .max(reset_bounds.max_y())
        .max(hidden_line_bounds.max_y())
        .max(reset_camera_bounds.max_y())
        .max(projection_bounds.max_y())
        .max(snap_grid_bounds.max_y())
        .max(snap_origin_bounds.max_y())
        .max(snap_endpoint_bounds.max_y())
        .max(snap_midpoint_bounds.max_y())
        .max(hotkey_bounds.max_y())
        + 8.0)
        .min(content_bounds.max_y());
    Bounds::new(
        content_bounds.origin.x,
        body_top,
        content_bounds.size.width,
        (content_bounds.max_y() - body_top).max(0.0),
    )
}

pub fn camera_interaction_bounds(content_bounds: Bounds) -> Bounds {
    placeholder_layout(cad_demo_body_bounds(content_bounds)).viewport_bounds
}

pub fn variant_tile_bounds(content_bounds: Bounds, tile_index: usize) -> Bounds {
    let viewport = camera_interaction_bounds(content_bounds);
    let gap = VARIANT_TILE_GAP;
    let tile_width = ((viewport.size.width - gap) * 0.5).max(2.0);
    let tile_height = ((viewport.size.height - gap) * 0.5).max(2.0);
    let col = (tile_index % 2) as f32;
    let row = ((tile_index / 2) % 2) as f32;
    Bounds::new(
        viewport.origin.x + col * (tile_width + gap),
        viewport.origin.y + row * (tile_height + gap),
        tile_width,
        tile_height,
    )
}

pub fn variant_tile_index_at_point(content_bounds: Bounds, point: Point) -> Option<usize> {
    let viewport = camera_interaction_bounds(content_bounds);
    if !viewport.contains(point) {
        return None;
    }
    (0..4).find(|index| variant_tile_bounds(content_bounds, *index).contains(point))
}

pub fn paint_cad_demo_placeholder_pane(
    content_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) {
    let cycle_bounds = cad_demo_cycle_variant_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let hidden_line_bounds = cad_demo_hidden_line_mode_button_bounds(content_bounds);
    let reset_camera_bounds = cad_demo_reset_camera_button_bounds(content_bounds);
    let projection_bounds = cad_demo_projection_mode_button_bounds(content_bounds);
    let snap_grid_bounds = cad_demo_snap_grid_button_bounds(content_bounds);
    let snap_origin_bounds = cad_demo_snap_origin_button_bounds(content_bounds);
    let snap_endpoint_bounds = cad_demo_snap_endpoint_button_bounds(content_bounds);
    let snap_midpoint_bounds = cad_demo_snap_midpoint_button_bounds(content_bounds);
    let hotkey_bounds = cad_demo_hotkey_profile_button_bounds(content_bounds);
    let warning_panel = cad_demo_warning_panel_bounds(content_bounds);
    let timeline_panel = cad_demo_timeline_panel_bounds(content_bounds);
    let severity_filter_bounds = cad_demo_warning_filter_severity_button_bounds(content_bounds);
    let code_filter_bounds = cad_demo_warning_filter_code_button_bounds(content_bounds);
    paint_action_button(cycle_bounds, "Cycle Variant", paint);
    paint_action_button(reset_bounds, "Reset Session", paint);
    paint_action_button(
        hidden_line_bounds,
        &format!("Render: {}", pane_state.hidden_line_mode.label()),
        paint,
    );
    paint_action_button(reset_camera_bounds, "Reset Camera", paint);
    paint_action_button(
        projection_bounds,
        &format!("Projection: {}", pane_state.projection_mode.label()),
        paint,
    );
    paint_action_button(
        snap_grid_bounds,
        if pane_state.snap_toggles.grid {
            "Grid Snap: On"
        } else {
            "Grid Snap: Off"
        },
        paint,
    );
    paint_action_button(
        snap_origin_bounds,
        if pane_state.snap_toggles.origin {
            "Origin Snap: On"
        } else {
            "Origin Snap: Off"
        },
        paint,
    );
    paint_action_button(
        snap_endpoint_bounds,
        if pane_state.snap_toggles.endpoint {
            "Endpoint Snap: On"
        } else {
            "Endpoint Snap: Off"
        },
        paint,
    );
    paint_action_button(
        snap_midpoint_bounds,
        if pane_state.snap_toggles.midpoint {
            "Midpoint Snap: On"
        } else {
            "Midpoint Snap: Off"
        },
        paint,
    );
    paint_action_button(
        hotkey_bounds,
        &format!("Hotkeys: {}", pane_state.hotkey_profile),
        paint,
    );
    paint_action_button(
        severity_filter_bounds,
        &format!("Severity: {}", pane_state.warning_filter_severity),
        paint,
    );
    paint_action_button(
        code_filter_bounds,
        &format!("Code: {}", pane_state.warning_filter_code),
        paint,
    );
    paint_view_cube_overlay(content_bounds, pane_state, paint);

    let body_bounds = cad_demo_body_bounds(content_bounds);
    let layout = placeholder_layout(body_bounds);

    if layout.header_origin.y + 10.0 <= content_bounds.max_y() {
        paint.scene.draw_text(paint.text.layout(
            "CAD demo placeholder",
            layout.header_origin,
            12.0,
            theme::text::PRIMARY,
        ));
    }

    if layout.subheader_origin.y + 10.0 <= content_bounds.max_y() {
        let variant_count = pane_state.variant_ids.len();
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "doc={} rev={} variants={variant_count}",
                pane_state.document_id, pane_state.document_revision
            ),
            layout.subheader_origin,
            10.0,
            theme::text::MUTED,
        ));
    }

    if layout.viewport_bounds.size.width > 1.0 && layout.viewport_bounds.size.height > 1.0 {
        paint.scene.draw_quad(
            Quad::new(layout.viewport_bounds)
                .with_background(theme::bg::ELEVATED)
                .with_corner_radius(4.0),
        );

        let mesh_payload = pane_state.last_good_mesh_payload.as_ref();
        let mut viewport_status = "4-up variant viewport ready".to_string();

        for tile_index in 0..4 {
            let tile_bounds = variant_tile_bounds(content_bounds, tile_index);
            if tile_bounds.size.width < 2.0 || tile_bounds.size.height < 2.0 {
                continue;
            }
            let is_active = pane_state.active_variant_tile_index == tile_index;
            let variant_view = pane_state.variant_viewport(tile_index);
            let tile_label = variant_view
                .map(|view| view.variant_id.as_str())
                .or_else(|| pane_state.variant_ids.get(tile_index).map(String::as_str))
                .unwrap_or("variant.unset");
            let selection = variant_view.and_then(|view| view.selected_ref.as_deref());
            let hover = variant_view.and_then(|view| view.hovered_ref.as_deref());
            let selection_suffix = selection
                .map(|value| format!(" sel={value}"))
                .unwrap_or_default();
            let hover_suffix = hover
                .map(|value| format!(" hov={value}"))
                .unwrap_or_default();

            let mut tile_quad = Quad::new(tile_bounds)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0);
            if is_active {
                tile_quad = tile_quad.with_border(theme::text::PRIMARY, 1.0);
            }
            paint.scene.draw_quad(tile_quad);

            let tile_label_origin =
                Point::new(tile_bounds.origin.x + 6.0, tile_bounds.origin.y + 10.0);
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "{}{}{}{}",
                    tile_index + 1,
                    if is_active { "*" } else { "" },
                    selection_suffix,
                    hover_suffix
                ),
                tile_label_origin,
                8.0,
                theme::text::MUTED,
            ));
            let tile_variant_origin =
                Point::new(tile_bounds.origin.x + 6.0, tile_bounds.origin.y + 20.0);
            if tile_variant_origin.y + 8.0 <= tile_bounds.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    tile_label,
                    tile_variant_origin,
                    8.0,
                    theme::text::SECONDARY,
                ));
            }

            if let Some(payload) = mesh_payload {
                let selected_outline_active = selection.is_some() || hover.is_some();
                let camera_pose = if let Some(viewport_state) = variant_view {
                    CadCameraPose::from_variant(viewport_state, pane_state.projection_mode)
                } else {
                    let fallback = CadVariantViewportState::for_variant(tile_label);
                    CadCameraPose::from_variant(&fallback, pane_state.projection_mode)
                };
                match cad_mesh_to_viewport_primitive(
                    payload,
                    tile_bounds,
                    selected_outline_active,
                    pane_state.hidden_line_mode,
                    camera_pose,
                ) {
                    Ok(mesh) => {
                        paint.scene.push_clip(tile_bounds);
                        if let Err(error) = paint.scene.draw_mesh(mesh) {
                            viewport_status =
                                format!("tile {} mesh draw skipped: {error}", tile_index + 1);
                        }
                        paint.scene.pop_clip();
                    }
                    Err(error) => {
                        viewport_status =
                            format!("tile {} mesh conversion failed: {error}", tile_index + 1);
                    }
                }
            }
        }

        let viewport_label = Point::new(
            layout.viewport_bounds.origin.x + 8.0,
            (layout.viewport_bounds.origin.y + 10.0).min(layout.viewport_bounds.max_y()),
        );
        if viewport_label.y + 9.0 <= content_bounds.max_y() {
            paint.scene.draw_text(paint.text.layout(
                &viewport_status,
                viewport_label,
                10.0,
                theme::text::MUTED,
            ));
        }

        let visible_warning_indices = visible_warning_indices(pane_state);
        for (marker_index, warning_index) in visible_warning_indices.iter().take(8).enumerate() {
            let marker_bounds = cad_demo_warning_marker_bounds(content_bounds, marker_index);
            let warning = &pane_state.warnings[*warning_index];
            let mut marker = Quad::new(marker_bounds).with_corner_radius(2.0);
            marker = marker.with_background(warning_color(warning));
            if pane_state.focused_warning_index == Some(*warning_index)
                || pane_state.warning_hover_index == Some(*warning_index)
            {
                marker = marker.with_border(theme::text::PRIMARY, 1.0);
            }
            paint.scene.draw_quad(marker);
        }
    }

    if warning_panel.size.width > 2.0 && warning_panel.size.height > 2.0 {
        paint.scene.draw_quad(
            Quad::new(warning_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        let visible_warning_indices = visible_warning_indices(pane_state);
        for (row_index, warning_index) in visible_warning_indices.iter().take(8).enumerate() {
            let row_bounds = cad_demo_warning_row_bounds(content_bounds, row_index);
            if row_bounds.max_y() > warning_panel.max_y() {
                break;
            }
            let warning = &pane_state.warnings[*warning_index];
            if pane_state.focused_warning_index == Some(*warning_index) {
                paint.scene.draw_quad(
                    Quad::new(row_bounds)
                        .with_background(theme::bg::ELEVATED)
                        .with_corner_radius(3.0),
                );
            }
            let row_text = format!("[{}] {}", warning.severity, warning.code);
            paint.scene.draw_text(paint.text.layout(
                &row_text,
                Point::new(row_bounds.origin.x + 4.0, row_bounds.origin.y + 9.0),
                9.0,
                warning_color(warning),
            ));
            let detail_y = row_bounds.origin.y + 18.0;
            if detail_y + 8.0 <= warning_panel.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    &warning.message,
                    Point::new(row_bounds.origin.x + 4.0, detail_y),
                    8.0,
                    theme::text::MUTED,
                ));
            }
        }
    }

    if timeline_panel.size.width > 2.0 && timeline_panel.size.height > 2.0 {
        paint.scene.draw_quad(
            Quad::new(timeline_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            "Feature Timeline",
            Point::new(
                timeline_panel.origin.x + 6.0,
                timeline_panel.origin.y + 10.0,
            ),
            10.0,
            theme::text::PRIMARY,
        ));

        for visible_index in 0..10 {
            let actual_index = pane_state.timeline_scroll_offset + visible_index;
            if actual_index >= pane_state.timeline_rows.len() {
                break;
            }
            let row = &pane_state.timeline_rows[actual_index];
            let row_bounds = cad_demo_timeline_row_bounds(content_bounds, visible_index);
            if row_bounds.max_y() > timeline_panel.max_y() {
                break;
            }
            if pane_state.timeline_selected_index == Some(actual_index) {
                paint.scene.draw_quad(
                    Quad::new(row_bounds)
                        .with_background(theme::bg::ELEVATED)
                        .with_corner_radius(3.0),
                );
            }
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "{} [{}] {} ({})",
                    row.feature_name, row.status_badge, row.op_type, row.provenance
                ),
                Point::new(row_bounds.origin.x + 4.0, row_bounds.origin.y + 9.0),
                9.0,
                timeline_row_color(row),
            ));
        }

        if let Some((inspect_title, inspect_lines)) = selection_inspect_lines(pane_state) {
            let inspector_origin = Point::new(
                timeline_panel.origin.x + 6.0,
                (timeline_panel.max_y() - 52.0).max(timeline_panel.origin.y + 14.0),
            );
            paint.scene.draw_text(paint.text.layout(
                &format!("{inspect_title}:"),
                inspector_origin,
                9.0,
                theme::text::SECONDARY,
            ));
            for (offset, line) in inspect_lines.iter().take(5).enumerate() {
                let y = inspector_origin.y + 10.0 + offset as f32 * 9.0;
                if y + 8.0 > timeline_panel.max_y() {
                    break;
                }
                paint.scene.draw_text(paint.text.layout(
                    line,
                    Point::new(inspector_origin.x, y),
                    8.0,
                    theme::text::MUTED,
                ));
            }
        } else if pane_state.timeline_selected_index.is_some() {
            let inspector_origin = Point::new(
                timeline_panel.origin.x + 6.0,
                (timeline_panel.max_y() - 34.0).max(timeline_panel.origin.y + 14.0),
            );
            paint.scene.draw_text(paint.text.layout(
                "Params:",
                inspector_origin,
                9.0,
                theme::text::SECONDARY,
            ));
            for (offset, (name, value)) in pane_state
                .selected_feature_params
                .iter()
                .take(2)
                .enumerate()
            {
                let y = inspector_origin.y + 10.0 + offset as f32 * 9.0;
                if y + 8.0 > timeline_panel.max_y() {
                    break;
                }
                paint.scene.draw_text(paint.text.layout(
                    &format!("{name}={value}"),
                    Point::new(inspector_origin.x, y),
                    8.0,
                    theme::text::MUTED,
                ));
            }
        }
    }

    if pane_state.context_menu.is_open && !pane_state.context_menu.items.is_empty() {
        let menu_bounds = cad_demo_context_menu_bounds(
            content_bounds,
            pane_state.context_menu.anchor,
            pane_state.context_menu.items.len(),
        );
        paint.scene.draw_quad(
            Quad::new(menu_bounds)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(6.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "Context {}",
                pane_state.context_menu.target_kind.label().to_ascii_uppercase()
            ),
            Point::new(menu_bounds.origin.x + 6.0, menu_bounds.origin.y + 10.0),
            8.0,
            theme::text::MUTED,
        ));
        for (index, item) in pane_state.context_menu.items.iter().enumerate() {
            let row_bounds = cad_demo_context_menu_row_bounds(menu_bounds, index);
            if row_bounds.max_y() > menu_bounds.max_y() - 2.0 {
                break;
            }
            paint.scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(Hsla::new(0.0, 0.0, 0.14, 0.55))
                    .with_corner_radius(3.0),
            );
            paint.scene.draw_text(paint.text.layout(
                item.label.as_str(),
                Point::new(row_bounds.origin.x + 5.0, row_bounds.origin.y + 10.0),
                8.5,
                theme::text::PRIMARY,
            ));
        }
    }

    if layout.footer_origin.y + 8.0 <= content_bounds.max_y() {
        let focus_suffix = pane_state
            .focused_geometry_ref
            .as_ref()
            .map(|value| format!(" focus={value}"))
            .unwrap_or_default();
        let three_d_mouse_status = pane_state.three_d_mouse_status();
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "session={} active={} warnings={}{} cam({}; z={:.2} pan={:.0},{:.0} orbit={:.0}/{:.0}) snaps[{}] hotkeys[{}] 3dmouse[{}]",
                pane_state.session_id,
                format!(
                    "{}@tile{}",
                    pane_state.active_variant_id,
                    pane_state.active_variant_tile_index + 1
                ),
                pane_state.warnings.len(),
                focus_suffix,
                pane_state.projection_mode.label(),
                pane_state.camera_zoom,
                pane_state.camera_pan_x,
                pane_state.camera_pan_y,
                pane_state.camera_orbit_yaw_deg,
                pane_state.camera_orbit_pitch_deg,
                pane_state.snap_summary(),
                pane_state.hotkey_profile,
                three_d_mouse_status,
            ),
            layout.footer_origin,
            9.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_view_cube_overlay(
    content_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) {
    let cube_bounds = cad_demo_view_cube_bounds(content_bounds);
    if cube_bounds.size.width <= 2.0 || cube_bounds.size.height <= 2.0 {
        return;
    }
    paint.scene.draw_quad(
        Quad::new(cube_bounds)
            .with_background(theme::bg::SURFACE)
            .with_corner_radius(4.0)
            .with_border(theme::border::SUBTLE, 1.0),
    );
    let label_origin = Point::new(cube_bounds.origin.x + 6.0, cube_bounds.origin.y + 10.0);
    if label_origin.y + 8.0 <= cube_bounds.max_y() {
        paint.scene.draw_text(paint.text.layout(
            "View Cube",
            label_origin,
            8.0,
            theme::text::MUTED,
        ));
    }

    let active = pane_state.active_view_snap();
    paint_view_snap_button(
        cad_demo_view_snap_top_button_bounds(content_bounds),
        "Top",
        active == Some(CadCameraViewSnap::Top),
        paint,
    );
    paint_view_snap_button(
        cad_demo_view_snap_front_button_bounds(content_bounds),
        "Front",
        active == Some(CadCameraViewSnap::Front),
        paint,
    );
    paint_view_snap_button(
        cad_demo_view_snap_right_button_bounds(content_bounds),
        "Right",
        active == Some(CadCameraViewSnap::Right),
        paint,
    );
    paint_view_snap_button(
        cad_demo_view_snap_iso_button_bounds(content_bounds),
        "Iso",
        active == Some(CadCameraViewSnap::Isometric),
        paint,
    );
}

fn paint_view_snap_button(bounds: Bounds, label: &str, active: bool, paint: &mut PaintContext) {
    if bounds.size.width <= 2.0 || bounds.size.height <= 2.0 {
        return;
    }
    let mut quad = Quad::new(bounds).with_corner_radius(2.0);
    if active {
        quad = quad
            .with_background(theme::bg::ELEVATED)
            .with_border(theme::text::PRIMARY, 1.0);
    } else {
        quad = quad
            .with_background(theme::bg::APP)
            .with_border(theme::border::SUBTLE, 1.0);
    }
    paint.scene.draw_quad(quad);
    let text_origin = Point::new(
        bounds.origin.x + 4.0,
        bounds.origin.y + bounds.size.height * 0.5,
    );
    paint.scene.draw_text(paint.text.layout(
        label,
        text_origin,
        8.0,
        if active {
            theme::text::PRIMARY
        } else {
            theme::text::SECONDARY
        },
    ));
}

fn visible_warning_indices(state: &CadDemoPaneState) -> Vec<usize> {
    state
        .warnings
        .iter()
        .enumerate()
        .filter(|(_, warning)| warning_passes_filter(state, warning))
        .map(|(index, _)| index)
        .collect()
}

fn warning_passes_filter(state: &CadDemoPaneState, warning: &CadDemoWarningState) -> bool {
    let severity_ok = state.warning_filter_severity == "all"
        || warning
            .severity
            .eq_ignore_ascii_case(&state.warning_filter_severity);
    let code_ok = state.warning_filter_code == "all"
        || warning
            .code
            .eq_ignore_ascii_case(&state.warning_filter_code);
    severity_ok && code_ok
}

fn warning_color(warning: &CadDemoWarningState) -> wgpui::Hsla {
    if warning.severity.eq_ignore_ascii_case("critical") {
        return theme::status::ERROR;
    }
    if warning.severity.eq_ignore_ascii_case("warning") {
        return theme::status::WARNING;
    }
    theme::text::MUTED
}

fn timeline_row_color(row: &crate::app_state::CadTimelineRowState) -> wgpui::Hsla {
    match row.status_badge.as_str() {
        "fail" => theme::status::ERROR,
        "warn" => theme::status::WARNING,
        _ => theme::text::SECONDARY,
    }
}

fn selection_inspect_lines(pane_state: &CadDemoPaneState) -> Option<(String, Vec<String>)> {
    let primary = pane_state.selection_store.state().primary.as_ref()?;
    let payload = pane_state.last_good_mesh_payload.as_ref()?;
    match primary.kind {
        CadSelectionKind::Body => {
            let analysis = &pane_state.analysis_snapshot;
            let estimate = estimate_body_properties(payload, DENSITY_ALUMINUM_6061_KG_M3)?;
            let volume_mm3 = analysis.volume_mm3?;
            let mass_kg = analysis.mass_kg?;
            let cog = analysis.center_of_gravity_mm?;
            Some((
                "Body Inspect".to_string(),
                vec![
                    format!("Volume: {:.1} mm^3", volume_mm3),
                    format!("Area: {:.1} mm^2", estimate.surface_area_mm2),
                    format!("Mass: {:.4} kg", mass_kg),
                    format!("CoG: ({:.1}, {:.1}, {:.1}) mm", cog[0], cog[1], cog[2]),
                    format!(
                        "BBox: {:.1} x {:.1} x {:.1} mm",
                        estimate.bounds_size_mm[0],
                        estimate.bounds_size_mm[1],
                        estimate.bounds_size_mm[2]
                    ),
                ],
            ))
        }
        CadSelectionKind::Face => {
            let face_index = parse_entity_index(primary.entity_id.as_str(), "face")?;
            let face = face_properties(payload, face_index)?;
            Some((
                "Face Inspect".to_string(),
                vec![
                    format!("Face: {}", face.face_index),
                    format!("Area: {:.1} mm^2", face.area_mm2),
                    format!(
                        "Normal: ({:.2}, {:.2}, {:.2})",
                        face.normal[0], face.normal[1], face.normal[2]
                    ),
                ],
            ))
        }
        CadSelectionKind::Edge => {
            let edge_index = parse_entity_index(primary.entity_id.as_str(), "edge")?;
            let edge = edge_properties(payload, edge_index)?;
            Some((
                "Edge Inspect".to_string(),
                vec![
                    format!("Edge: {}", edge.edge_index),
                    format!("Length: {:.2} mm", edge.length_mm),
                    format!("Type: {}", edge.edge_type.label()),
                ],
            ))
        }
    }
}

fn parse_entity_index(entity_id: &str, prefix: &str) -> Option<usize> {
    let value = entity_id.strip_prefix(prefix)?.strip_prefix('.')?;
    value.parse::<usize>().ok()
}

fn cad_mesh_to_viewport_primitive(
    payload: &CadMeshPayload,
    viewport_bounds: Bounds,
    selected_outline_active: bool,
    hidden_line_mode: CadHiddenLineMode,
    camera_pose: CadCameraPose,
) -> Result<MeshPrimitive, String> {
    if payload.vertices.is_empty() {
        return Err("mesh payload has no vertices".to_string());
    }
    if payload.triangle_indices.is_empty() {
        return Err("mesh payload has no triangle indices".to_string());
    }

    let model_center = [
        (payload.bounds.min_mm[0] + payload.bounds.max_mm[0]) * 0.5,
        (payload.bounds.min_mm[1] + payload.bounds.max_mm[1]) * 0.5,
        (payload.bounds.min_mm[2] + payload.bounds.max_mm[2]) * 0.5,
    ];
    let yaw_rad = camera_pose.orbit_yaw_deg.to_radians();
    let pitch_rad = camera_pose.orbit_pitch_deg.to_radians();

    let transformed_positions = payload
        .vertices
        .iter()
        .map(|vertex| rotate_about_center(vertex.position_mm, model_center, yaw_rad, pitch_rad))
        .collect::<Vec<_>>();
    let transformed_normals = payload
        .vertices
        .iter()
        .map(|vertex| rotate_vector_yaw_pitch(vertex.normal, yaw_rad, pitch_rad))
        .collect::<Vec<_>>();

    let transformed_min_z = transformed_positions
        .iter()
        .map(|position| position[2])
        .fold(f32::INFINITY, f32::min);
    let transformed_max_z = transformed_positions
        .iter()
        .map(|position| position[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let projected_positions = transformed_positions
        .iter()
        .map(|position| {
            project_xy_for_mode(
                *position,
                model_center,
                camera_pose.projection_mode,
                transformed_min_z,
                transformed_max_z,
            )
        })
        .collect::<Vec<_>>();

    let min_x = projected_positions
        .iter()
        .map(|position| position[0])
        .fold(f32::INFINITY, f32::min);
    let max_x = projected_positions
        .iter()
        .map(|position| position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = projected_positions
        .iter()
        .map(|position| position[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = projected_positions
        .iter()
        .map(|position| position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_z = projected_positions
        .iter()
        .map(|position| position[2])
        .fold(f32::INFINITY, f32::min);
    let model_width = (max_x - min_x).abs().max(1.0);
    let model_height = (max_y - min_y).abs().max(1.0);
    let available_width = (viewport_bounds.size.width - VIEWPORT_MESH_PAD * 2.0).max(1.0);
    let available_height = (viewport_bounds.size.height - VIEWPORT_MESH_PAD * 2.0).max(1.0);
    let fit_scale = (available_width / model_width)
        .min(available_height / model_height)
        .max(0.0001);
    let scale = (fit_scale * camera_pose.zoom).max(0.0001);
    let scaled_width = model_width * scale;
    let scaled_height = model_height * scale;
    let origin_x = viewport_bounds.origin.x
        + ((viewport_bounds.size.width - scaled_width) * 0.5)
        + camera_pose.pan_x;
    let origin_y = viewport_bounds.origin.y
        + ((viewport_bounds.size.height - scaled_height) * 0.5)
        + camera_pose.pan_y;

    let slot_to_color = payload
        .material_slots
        .iter()
        .map(|slot| (slot.slot, slot.base_color_rgba))
        .collect::<std::collections::BTreeMap<_, _>>();

    let vertices = payload
        .vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            let fill_color = styled_fill_color(
                slot_to_color
                    .get(&vertex.material_slot)
                    .copied()
                    .unwrap_or([0.78, 0.80, 0.83, 1.0]),
                hidden_line_mode,
            );
            project_vertex(
                projected_positions[index],
                transformed_normals[index],
                min_x,
                max_y,
                min_z,
                origin_x,
                origin_y,
                scale,
                fill_color,
            )
        })
        .collect::<Vec<_>>();
    let show_edges = hidden_line_mode != CadHiddenLineMode::Shaded || selected_outline_active;
    let edges = if show_edges {
        payload
            .edges
            .iter()
            .map(|edge| {
                let mut flags = edge.flags | MESH_EDGE_FLAG_SILHOUETTE;
                if selected_outline_active {
                    flags |= MESH_EDGE_FLAG_SELECTED;
                }
                MeshEdge::new(edge.start_vertex, edge.end_vertex).with_flags(flags)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let primitive =
        MeshPrimitive::new(vertices, payload.triangle_indices.clone()).with_edges(edges);
    primitive
        .validate()
        .map_err(|error| error.to_string())
        .map(|_| primitive)
}

fn project_xy_for_mode(
    position: [f32; 3],
    center: [f32; 3],
    mode: CadProjectionMode,
    min_z: f32,
    max_z: f32,
) -> [f32; 3] {
    if mode == CadProjectionMode::Orthographic {
        return position;
    }
    let depth_span = (max_z - min_z).abs().max(1.0);
    let depth_ratio = ((position[2] - min_z) / depth_span).clamp(0.0, 1.0);
    let perspective = 0.78 + depth_ratio * 0.44;
    [
        center[0] + (position[0] - center[0]) * perspective,
        center[1] + (position[1] - center[1]) * perspective,
        position[2],
    ]
}

fn project_vertex(
    transformed_position: [f32; 3],
    transformed_normal: [f32; 3],
    min_x: f32,
    max_y: f32,
    min_z: f32,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
    color: [f32; 4],
) -> MeshVertex {
    let projected_x = origin_x + ((transformed_position[0] - min_x) * scale);
    let projected_y = origin_y + ((max_y - transformed_position[1]) * scale);
    let projected_z = (transformed_position[2] - min_z) * scale * 0.25;
    MeshVertex::new(
        [projected_x, projected_y, projected_z],
        transformed_normal,
        color,
    )
}

fn rotate_about_center(
    position: [f32; 3],
    center: [f32; 3],
    yaw_rad: f32,
    pitch_rad: f32,
) -> [f32; 3] {
    let local = [
        position[0] - center[0],
        position[1] - center[1],
        position[2] - center[2],
    ];
    let rotated = rotate_vector_yaw_pitch(local, yaw_rad, pitch_rad);
    [
        rotated[0] + center[0],
        rotated[1] + center[1],
        rotated[2] + center[2],
    ]
}

fn rotate_vector_yaw_pitch(vector: [f32; 3], yaw_rad: f32, pitch_rad: f32) -> [f32; 3] {
    let (sin_yaw, cos_yaw) = yaw_rad.sin_cos();
    let (sin_pitch, cos_pitch) = pitch_rad.sin_cos();
    let yaw_x = vector[0] * cos_yaw - vector[1] * sin_yaw;
    let yaw_y = vector[0] * sin_yaw + vector[1] * cos_yaw;
    let yaw_z = vector[2];

    let pitch_y = yaw_y * cos_pitch - yaw_z * sin_pitch;
    let pitch_z = yaw_y * sin_pitch + yaw_z * cos_pitch;
    [yaw_x, pitch_y, pitch_z]
}

fn styled_fill_color(base: [f32; 4], mode: CadHiddenLineMode) -> [f32; 4] {
    match mode {
        CadHiddenLineMode::Shaded => base,
        CadHiddenLineMode::ShadedEdges => [
            (base[0] * 0.72) + 0.10,
            (base[1] * 0.72) + 0.10,
            (base[2] * 0.72) + 0.10,
            0.86,
        ],
        CadHiddenLineMode::Wireframe => [0.28, 0.32, 0.36, 0.06],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadCameraPose, cad_mesh_to_viewport_primitive, placeholder_layout, selection_inspect_lines,
        variant_tile_bounds, variant_tile_index_at_point,
    };
    use openagents_cad::analysis::{DENSITY_ALUMINUM_6061_KG_M3, estimate_body_properties};
    use openagents_cad::contracts::CadSelectionKind;
    use openagents_cad::mesh::{
        CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
        CadMeshVertex,
    };
    use wgpui::{Bounds, MESH_EDGE_FLAG_SELECTED, MESH_EDGE_FLAG_SILHOUETTE, Point};

    use crate::app_state::{CadDemoPaneState, CadHiddenLineMode, CadProjectionMode};

    fn default_camera_pose() -> CadCameraPose {
        CadCameraPose {
            projection_mode: CadProjectionMode::Orthographic,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            orbit_yaw_deg: 26.0,
            orbit_pitch_deg: 18.0,
        }
    }

    #[test]
    fn placeholder_layout_stays_within_large_bounds() {
        let content = Bounds::new(20.0, 30.0, 640.0, 420.0);
        let layout = placeholder_layout(content);
        assert!(layout.header_origin.x >= content.origin.x);
        assert!(layout.header_origin.y >= content.origin.y);
        assert!(layout.subheader_origin.y >= content.origin.y);
        assert!(layout.viewport_bounds.origin.x >= content.origin.x);
        assert!(layout.viewport_bounds.origin.y >= content.origin.y);
        assert!(layout.viewport_bounds.max_x() <= content.max_x() + 0.001);
        assert!(layout.viewport_bounds.max_y() <= content.max_y() + 0.001);
        assert!(layout.footer_origin.y <= content.max_y() + 0.001);
    }

    #[test]
    fn placeholder_layout_stays_within_tight_bounds() {
        let content = Bounds::new(4.0, 6.0, 160.0, 92.0);
        let layout = placeholder_layout(content);
        assert!(layout.viewport_bounds.size.width >= 0.0);
        assert!(layout.viewport_bounds.size.height >= 0.0);
        assert!(layout.viewport_bounds.max_x() <= content.max_x() + 0.001);
        assert!(layout.viewport_bounds.max_y() <= content.max_y() + 0.001);
        assert!(layout.footer_origin.y >= content.origin.y);
        assert!(layout.footer_origin.y <= content.max_y() + 0.001);
    }

    #[test]
    fn variant_tiles_are_deterministic_and_pickable() {
        let content = Bounds::new(20.0, 20.0, 820.0, 520.0);
        let first = variant_tile_bounds(content, 0);
        let second = variant_tile_bounds(content, 1);
        let third = variant_tile_bounds(content, 2);
        let fourth = variant_tile_bounds(content, 3);
        assert!(first.max_x() <= second.min_x() + 0.001);
        assert!(first.max_y() <= third.min_y() + 0.001);
        assert!(third.max_x() <= fourth.min_x() + 0.001);

        let hit_first = variant_tile_index_at_point(
            content,
            Point::new(first.origin.x + 4.0, first.origin.y + 4.0),
        );
        let hit_fourth = variant_tile_index_at_point(
            content,
            Point::new(fourth.origin.x + 4.0, fourth.origin.y + 4.0),
        );
        assert_eq!(hit_first, Some(0));
        assert_eq!(hit_fourth, Some(3));
    }

    #[test]
    fn mesh_projection_stays_within_viewport_and_is_deterministic() {
        let viewport = Bounds::new(120.0, 80.0, 320.0, 180.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.variant.baseline".to_string(),
            document_revision: 2,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [-20.0, -10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, -10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, 30.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [-20.0, 30.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 2, 3],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 1,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [-20.0, -10.0, 0.0],
                max_mm: [40.0, 30.0, 0.0],
            },
        };

        let first = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect("mesh projection should succeed");
        let second = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect("mesh projection should remain deterministic");
        assert_eq!(first, second);
        for vertex in &first.vertices {
            assert!(vertex.position[0] >= viewport.origin.x - 0.001);
            assert!(vertex.position[0] <= viewport.max_x() + 0.001);
            assert!(vertex.position[1] >= viewport.origin.y - 0.001);
            assert!(vertex.position[1] <= viewport.max_y() + 0.001);
        }
    }

    #[test]
    fn mesh_projection_rejects_empty_payload() {
        let payload = CadMeshPayload::default();
        let viewport = Bounds::new(0.0, 0.0, 120.0, 80.0);
        let error = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect_err("empty payload should fail");
        assert_eq!(error, "mesh payload has no vertices");
    }

    #[test]
    fn mesh_projection_sets_selected_outline_edge_flags() {
        let viewport = Bounds::new(20.0, 20.0, 200.0, 120.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.variant.baseline".to_string(),
            document_revision: 2,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [10.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [10.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [10.0, 10.0, 0.0],
            },
        };
        let mesh = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            true,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect("selected projection should succeed");
        assert_eq!(mesh.edges.len(), 1);
        assert_ne!(mesh.edges[0].flags & MESH_EDGE_FLAG_SELECTED, 0);
    }

    #[test]
    fn selection_inspect_lines_surface_volume_mass_and_bounds_for_body_selection() {
        let payload = CadMeshPayload {
            mesh_id: "mesh.inspect".to_string(),
            document_revision: 3,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [10.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 10.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [10.0, 10.0, 10.0],
            },
        };
        let estimate = estimate_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("body estimate should exist");
        let mut state = CadDemoPaneState::default();
        state.last_good_mesh_payload = Some(payload);
        state.analysis_snapshot.volume_mm3 = Some(estimate.volume_mm3);
        state.analysis_snapshot.mass_kg = Some(estimate.mass_kg);
        state.analysis_snapshot.center_of_gravity_mm = Some(estimate.center_of_gravity_mm);
        let _ = state.selection_store.set_primary(
            CadSelectionKind::Body,
            "body.0",
            Some("body.0".to_string()),
        );

        let (title, lines) =
            selection_inspect_lines(&state).expect("body inspect lines should be visible");
        assert_eq!(title, "Body Inspect");
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("Volume"));
        assert!(lines[1].contains("Area"));
        assert!(lines[2].contains("Mass"));
        assert!(lines[3].contains("CoG"));
        assert!(lines[4].contains("BBox"));
    }

    #[test]
    fn selection_inspect_lines_return_face_area_normal_and_edge_length_type() {
        let payload = CadMeshPayload {
            mesh_id: "mesh.inspect.subentities".to_string(),
            document_revision: 5,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [10.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![
                CadMeshEdgeSegment {
                    start_vertex: 0,
                    end_vertex: 1,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 1,
                    end_vertex: 2,
                    flags: 7,
                },
            ],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [10.0, 10.0, 0.0],
            },
        };

        let mut face_state = CadDemoPaneState::default();
        face_state.last_good_mesh_payload = Some(payload.clone());
        let _ = face_state.selection_store.set_primary(
            CadSelectionKind::Face,
            "face.0",
            Some("face.0".to_string()),
        );
        let face_before =
            selection_inspect_lines(&face_state).expect("face lines should be available");
        face_state.orbit_camera_by_drag(19.0, -8.0);
        let face_after = selection_inspect_lines(&face_state)
            .expect("face lines should remain stable across camera motion");
        assert_eq!(face_before, face_after);
        assert_eq!(face_before.0, "Face Inspect");
        assert!(face_before.1.iter().any(|line| line.contains("Area")));
        assert!(face_before.1.iter().any(|line| line.contains("Normal")));

        let mut edge_state = CadDemoPaneState::default();
        edge_state.last_good_mesh_payload = Some(payload);
        let _ = edge_state.selection_store.set_primary(
            CadSelectionKind::Edge,
            "edge.1",
            Some("edge.1".to_string()),
        );
        let edge_lines =
            selection_inspect_lines(&edge_state).expect("edge lines should be available");
        assert_eq!(edge_lines.0, "Edge Inspect");
        assert!(edge_lines.1.iter().any(|line| line.contains("Length")));
        assert!(edge_lines.1.iter().any(|line| line.contains("Type")));
    }

    #[test]
    fn render_modes_style_fill_and_edge_baselines() {
        let viewport = Bounds::new(10.0, 10.0, 180.0, 120.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.variant.baseline".to_string(),
            document_revision: 2,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [20.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [10.0, 20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.5, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [20.0, 20.0, 0.0],
            },
        };

        let shaded = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect("shaded projection should succeed");
        let shaded_edges = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::ShadedEdges,
            default_camera_pose(),
        )
        .expect("shaded+edges projection should succeed");
        let wireframe = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Wireframe,
            default_camera_pose(),
        )
        .expect("wireframe projection should succeed");
        let wireframe_again = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Wireframe,
            default_camera_pose(),
        )
        .expect("wireframe projection should remain deterministic");

        assert_eq!(shaded.edges.len(), 0);
        assert_eq!(shaded_edges.edges.len(), 1);
        assert_eq!(wireframe.edges.len(), 1);
        assert_ne!(shaded_edges.edges[0].flags & MESH_EDGE_FLAG_SILHOUETTE, 0);
        assert_ne!(wireframe.edges[0].flags & MESH_EDGE_FLAG_SILHOUETTE, 0);
        assert!(shaded.vertices.iter().all(|vertex| vertex.color[3] >= 0.99));
        assert!(
            shaded_edges
                .vertices
                .iter()
                .all(|vertex| vertex.color[3] < 0.95)
        );
        assert!(
            wireframe
                .vertices
                .iter()
                .all(|vertex| vertex.color[3] <= 0.10)
        );
        assert_eq!(wireframe, wireframe_again);
    }

    #[test]
    fn camera_pose_affects_projection_deterministically() {
        let viewport = Bounds::new(0.0, 0.0, 220.0, 140.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.variant.baseline".to_string(),
            document_revision: 3,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [-30.0, -20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [30.0, -20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 24.0, 10.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [-30.0, -20.0, 0.0],
                max_mm: [30.0, 24.0, 10.0],
            },
        };

        let baseline = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            default_camera_pose(),
        )
        .expect("baseline projection should succeed");
        let moved = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                projection_mode: CadProjectionMode::Orthographic,
                zoom: 1.3,
                pan_x: 16.0,
                pan_y: -12.0,
                orbit_yaw_deg: 46.0,
                orbit_pitch_deg: 8.0,
            },
        )
        .expect("moved projection should succeed");
        let moved_again = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                projection_mode: CadProjectionMode::Orthographic,
                zoom: 1.3,
                pan_x: 16.0,
                pan_y: -12.0,
                orbit_yaw_deg: 46.0,
                orbit_pitch_deg: 8.0,
            },
        )
        .expect("moved projection should be deterministic");

        assert_eq!(moved, moved_again);
        assert_ne!(baseline.vertices[0].position, moved.vertices[0].position);
    }

    #[test]
    fn perspective_projection_mode_changes_vertex_projection_deterministically() {
        let viewport = Bounds::new(0.0, 0.0, 220.0, 140.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.perspective.compare".to_string(),
            document_revision: 1,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [-30.0, -20.0, -10.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [30.0, -20.0, 10.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 30.0, 25.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [-30.0, -20.0, -10.0],
                max_mm: [30.0, 30.0, 25.0],
            },
        };

        let ortho = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                projection_mode: CadProjectionMode::Orthographic,
                ..default_camera_pose()
            },
        )
        .expect("orthographic projection should succeed");

        let perspective = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                projection_mode: CadProjectionMode::Perspective,
                ..default_camera_pose()
            },
        )
        .expect("perspective projection should succeed");

        let perspective_again = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                projection_mode: CadProjectionMode::Perspective,
                ..default_camera_pose()
            },
        )
        .expect("perspective projection should be deterministic");

        assert_eq!(perspective, perspective_again);
        assert_ne!(ortho.vertices[0].position, perspective.vertices[0].position);
    }
}
