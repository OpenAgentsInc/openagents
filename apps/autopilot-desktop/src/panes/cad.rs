use openagents_cad::analysis::{
    DENSITY_ALUMINUM_6061_KG_M3, edge_properties, estimate_body_properties, face_properties,
};
use openagents_cad::contracts::CadSelectionKind;
use openagents_cad::drafting::hidden_line::{
    DraftingTriangleMesh, project_mesh as project_drafting_mesh,
};
use openagents_cad::drafting::{
    ProjectedView as DraftingProjectedView, Visibility as DraftingVisibility,
};
use openagents_cad::kernel_math::Point3;
use openagents_cad::mesh::CadMeshPayload;
use wgpui::{
    Bounds, Hsla, MESH_EDGE_FLAG_SELECTED, MESH_EDGE_FLAG_SILHOUETTE, MeshEdge, MeshPrimitive,
    MeshVertex, PaintContext, Point, Quad, theme,
};

use crate::app_state::{
    CadCameraViewSnap, CadDemoPaneState, CadDemoWarningState, CadDrawingViewMode,
    CadHiddenLineMode, CadProjectionMode, CadVariantViewportState, CadViewportLayout,
};
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{
    cad_demo_context_menu_bounds, cad_demo_context_menu_row_bounds,
    cad_demo_cycle_variant_button_bounds, cad_demo_dimension_panel_bounds,
    cad_demo_dimension_row_bounds, cad_demo_drawing_add_detail_button_bounds,
    cad_demo_drawing_clear_details_button_bounds, cad_demo_drawing_dimensions_button_bounds,
    cad_demo_drawing_direction_button_bounds, cad_demo_drawing_hidden_lines_button_bounds,
    cad_demo_drawing_mode_button_bounds, cad_demo_drawing_reset_view_button_bounds,
    cad_demo_gripper_jaw_button_bounds, cad_demo_hidden_line_mode_button_bounds,
    cad_demo_hotkey_profile_button_bounds, cad_demo_material_button_bounds,
    cad_demo_projection_mode_button_bounds, cad_demo_reset_button_bounds,
    cad_demo_reset_camera_button_bounds, cad_demo_section_offset_button_bounds,
    cad_demo_section_plane_button_bounds, cad_demo_snap_endpoint_button_bounds,
    cad_demo_snap_grid_button_bounds, cad_demo_snap_midpoint_button_bounds,
    cad_demo_snap_origin_button_bounds, cad_demo_timeline_panel_bounds,
    cad_demo_timeline_row_bounds, cad_demo_view_cube_bounds,
    cad_demo_view_snap_front_button_bounds, cad_demo_view_snap_iso_button_bounds,
    cad_demo_view_snap_right_button_bounds, cad_demo_view_snap_top_button_bounds,
    cad_demo_viewport_layout_button_bounds, cad_demo_warning_filter_code_button_bounds,
    cad_demo_warning_filter_severity_button_bounds, cad_demo_warning_marker_bounds,
    cad_demo_warning_panel_bounds, cad_demo_warning_row_bounds,
};

const PAD: f32 = 12.0;
const HEADER_LINE_HEIGHT: f32 = 14.0;
const SUBHEADER_GAP: f32 = 4.0;
const VIEWPORT_TOP_GAP: f32 = 10.0;
const FOOTER_RESERVED: f32 = 18.0;
const VIEWPORT_MESH_PAD: f32 = 12.0;
const VARIANT_TILE_GAP: f32 = 8.0;
const ENGINEERING_OVERLAY_WIDTH: f32 = 220.0;
const ENGINEERING_OVERLAY_LINE_HEIGHT: f32 = 9.0;
const ENGINEERING_OVERLAY_LINE_COUNT: usize = 6;
const CAD_CAMERA_MIN_ZOOM: f32 = 0.35;
const CAD_CAMERA_MAX_ZOOM: f32 = 1.0;
const CAD_PERSPECTIVE_FIT_EXPANSION: f32 = 1.22;

fn legacy_cad_pane_enabled() -> bool {
    std::env::var_os("OPENAGENTS_CAD_USE_LEGACY_PANE").is_some()
}

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
    let jaw_bounds = cad_demo_gripper_jaw_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let hidden_line_bounds = cad_demo_hidden_line_mode_button_bounds(content_bounds);
    let reset_camera_bounds = cad_demo_reset_camera_button_bounds(content_bounds);
    let projection_bounds = cad_demo_projection_mode_button_bounds(content_bounds);
    let viewport_layout_bounds = cad_demo_viewport_layout_button_bounds(content_bounds);
    let drawing_mode_bounds = cad_demo_drawing_mode_button_bounds(content_bounds);
    let drawing_direction_bounds = cad_demo_drawing_direction_button_bounds(content_bounds);
    let drawing_hidden_bounds = cad_demo_drawing_hidden_lines_button_bounds(content_bounds);
    let drawing_dimensions_bounds = cad_demo_drawing_dimensions_button_bounds(content_bounds);
    let drawing_reset_bounds = cad_demo_drawing_reset_view_button_bounds(content_bounds);
    let drawing_add_detail_bounds = cad_demo_drawing_add_detail_button_bounds(content_bounds);
    let drawing_clear_details_bounds = cad_demo_drawing_clear_details_button_bounds(content_bounds);
    let snap_grid_bounds = cad_demo_snap_grid_button_bounds(content_bounds);
    let snap_origin_bounds = cad_demo_snap_origin_button_bounds(content_bounds);
    let snap_endpoint_bounds = cad_demo_snap_endpoint_button_bounds(content_bounds);
    let snap_midpoint_bounds = cad_demo_snap_midpoint_button_bounds(content_bounds);
    let hotkey_bounds = cad_demo_hotkey_profile_button_bounds(content_bounds);
    let section_plane_bounds = cad_demo_section_plane_button_bounds(content_bounds);
    let section_offset_bounds = cad_demo_section_offset_button_bounds(content_bounds);
    let material_bounds = cad_demo_material_button_bounds(content_bounds);
    let body_top = (cycle_bounds
        .max_y()
        .max(reset_bounds.max_y())
        .max(jaw_bounds.max_y())
        .max(hidden_line_bounds.max_y())
        .max(reset_camera_bounds.max_y())
        .max(projection_bounds.max_y())
        .max(viewport_layout_bounds.max_y())
        .max(drawing_mode_bounds.max_y())
        .max(drawing_direction_bounds.max_y())
        .max(drawing_hidden_bounds.max_y())
        .max(drawing_dimensions_bounds.max_y())
        .max(drawing_reset_bounds.max_y())
        .max(drawing_add_detail_bounds.max_y())
        .max(drawing_clear_details_bounds.max_y())
        .max(snap_grid_bounds.max_y())
        .max(snap_origin_bounds.max_y())
        .max(snap_endpoint_bounds.max_y())
        .max(snap_midpoint_bounds.max_y())
        .max(hotkey_bounds.max_y())
        .max(section_plane_bounds.max_y())
        .max(section_offset_bounds.max_y())
        .max(material_bounds.max_y())
        + 8.0)
        .min(content_bounds.max_y());
    Bounds::new(
        content_bounds.origin.x,
        body_top,
        content_bounds.size.width,
        (content_bounds.max_y() - body_top).max(0.0),
    )
}

fn cad_demo_basic_toolbar_bottom(content_bounds: Bounds) -> f32 {
    cad_demo_cycle_variant_button_bounds(content_bounds)
        .max_y()
        .max(cad_demo_gripper_jaw_button_bounds(content_bounds).max_y())
        .max(cad_demo_reset_button_bounds(content_bounds).max_y())
        .max(cad_demo_reset_camera_button_bounds(content_bounds).max_y())
        .max(cad_demo_projection_mode_button_bounds(content_bounds).max_y())
        .max(cad_demo_viewport_layout_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_mode_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_direction_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_hidden_lines_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_dimensions_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_reset_view_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_add_detail_button_bounds(content_bounds).max_y())
        .max(cad_demo_drawing_clear_details_button_bounds(content_bounds).max_y())
}

fn cad_demo_basic_viewport_bounds(content_bounds: Bounds) -> Bounds {
    let viewport_top =
        (cad_demo_basic_toolbar_bottom(content_bounds) + 8.0).min(content_bounds.max_y());
    let viewport_origin_x = (content_bounds.origin.x + 8.0).min(content_bounds.max_x());
    let viewport_origin_y = viewport_top.min(content_bounds.max_y());
    let viewport_width = (content_bounds.size.width - 16.0).max(1.0);
    let viewport_height = (content_bounds.max_y() - viewport_top - 18.0).max(1.0);
    Bounds::new(
        viewport_origin_x,
        viewport_origin_y,
        viewport_width,
        viewport_height,
    )
}

pub fn camera_interaction_bounds(content_bounds: Bounds) -> Bounds {
    if legacy_cad_pane_enabled() {
        placeholder_layout(cad_demo_body_bounds(content_bounds)).viewport_bounds
    } else {
        cad_demo_basic_viewport_bounds(content_bounds)
    }
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
    if !legacy_cad_pane_enabled() {
        paint_cad_demo_basic_pane(content_bounds, pane_state, paint);
        return;
    }
    paint.scene.push_clip(content_bounds);
    let cycle_bounds = cad_demo_cycle_variant_button_bounds(content_bounds);
    let jaw_bounds = cad_demo_gripper_jaw_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let hidden_line_bounds = cad_demo_hidden_line_mode_button_bounds(content_bounds);
    let reset_camera_bounds = cad_demo_reset_camera_button_bounds(content_bounds);
    let projection_bounds = cad_demo_projection_mode_button_bounds(content_bounds);
    let viewport_layout_bounds = cad_demo_viewport_layout_button_bounds(content_bounds);
    let snap_grid_bounds = cad_demo_snap_grid_button_bounds(content_bounds);
    let snap_origin_bounds = cad_demo_snap_origin_button_bounds(content_bounds);
    let snap_endpoint_bounds = cad_demo_snap_endpoint_button_bounds(content_bounds);
    let snap_midpoint_bounds = cad_demo_snap_midpoint_button_bounds(content_bounds);
    let hotkey_bounds = cad_demo_hotkey_profile_button_bounds(content_bounds);
    let section_plane_bounds = cad_demo_section_plane_button_bounds(content_bounds);
    let section_offset_bounds = cad_demo_section_offset_button_bounds(content_bounds);
    let material_bounds = cad_demo_material_button_bounds(content_bounds);
    let warning_panel = cad_demo_warning_panel_bounds(content_bounds);
    let dimension_panel = cad_demo_dimension_panel_bounds(content_bounds);
    let timeline_panel = cad_demo_timeline_panel_bounds(content_bounds);
    let severity_filter_bounds = cad_demo_warning_filter_severity_button_bounds(content_bounds);
    let code_filter_bounds = cad_demo_warning_filter_code_button_bounds(content_bounds);
    paint_action_button(cycle_bounds, "Cycle Variant", paint);
    paint_action_button(
        jaw_bounds,
        if matches!(
            pane_state.active_design_profile(),
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
        ) {
            if pane_state.gripper_jaw_open {
                "Pose: Pinch"
            } else {
                "Pose: Tripod"
            }
        } else if pane_state.gripper_jaw_open {
            "Jaw: Close"
        } else {
            "Jaw: Open"
        },
        paint,
    );
    paint_action_button(reset_bounds, "Bootstrap Demo", paint);
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
        viewport_layout_bounds,
        &format!("Layout: {}", pane_state.viewport_layout.label()),
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
        section_plane_bounds,
        &format!(
            "Section: {}",
            pane_state
                .section_axis
                .map(|axis| axis.label().to_string())
                .unwrap_or_else(|| "off".to_string())
        ),
        paint,
    );
    let section_offset_label = if pane_state.section_axis.is_some() {
        format!("Slice: {:+.1}", pane_state.section_offset_normalized)
    } else {
        "Slice: --".to_string()
    };
    paint_action_button(section_offset_bounds, section_offset_label.as_str(), paint);
    paint_action_button(
        material_bounds,
        &format!(
            "Material: {}",
            pane_state
                .analysis_snapshot
                .material_id
                .as_deref()
                .unwrap_or(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID)
        ),
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
            "CAD Demo",
            layout.header_origin,
            12.0,
            theme::text::PRIMARY,
        ));
    }

    if layout.subheader_origin.y + 10.0 <= content_bounds.max_y() {
        let variant_count = pane_state.variant_ids.len();
        paint.scene.draw_text(paint.text.layout(
            &format!(
                "doc {} | rev {} | active {} | variants {}",
                pane_state.document_id,
                pane_state.document_revision,
                pane_state.active_variant_id,
                variant_count
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

        let base_mesh_payload = pane_state.last_good_mesh_payload.as_ref();
        let mut viewport_status = "4-up variant viewport ready".to_string();
        let mut section_clip_failed = false;
        let section_mesh_payload = if let Some(payload) = base_mesh_payload {
            if let Some(section_plane) = pane_state.section_plane() {
                match openagents_cad::section::clip_mesh_payload(
                    payload,
                    section_plane,
                    openagents_cad::policy::resolve_tolerance_mm(None) as f32,
                ) {
                    Ok(clipped) => {
                        viewport_status = format!("section {}", pane_state.section_summary());
                        Some(clipped)
                    }
                    Err(error) => {
                        section_clip_failed = true;
                        viewport_status = format!("section clip failed: {error}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let mesh_payload = if section_clip_failed {
            None
        } else {
            section_mesh_payload.as_ref().or(base_mesh_payload)
        };

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
                &tile_caption(tile_index, is_active, selection, hover),
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
                let selected_outline_active = selection.is_some();
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
                            let _ = error;
                            viewport_status = format!("tile {} mesh draw skipped", tile_index + 1);
                        }
                        paint.scene.pop_clip();
                    }
                    Err(error) => {
                        let _ = error;
                        viewport_status = format!("tile {} mesh conversion failed", tile_index + 1);
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

        paint.scene.push_clip(layout.viewport_bounds);
        paint_engineering_overlay(layout.viewport_bounds, pane_state, paint);
        paint.scene.pop_clip();

        if let Some(tile_index) = pane_state.measurement_tile_index
            && tile_index < 4
        {
            let tile_bounds = variant_tile_bounds(content_bounds, tile_index);
            for point in &pane_state.measurement_points {
                if tile_bounds.contains(*point) {
                    let marker = Bounds::new(point.x - 2.0, point.y - 2.0, 4.0, 4.0);
                    paint.scene.draw_quad(
                        Quad::new(marker)
                            .with_background(theme::status::SUCCESS)
                            .with_corner_radius(1.0),
                    );
                }
            }

            if let Some(distance_px) = pane_state.measurement_distance_px {
                let anchor = if pane_state.measurement_points.len() >= 2 {
                    let first = pane_state.measurement_points[0];
                    let second = pane_state.measurement_points[1];
                    Point::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5)
                } else {
                    pane_state
                        .measurement_points
                        .first()
                        .copied()
                        .unwrap_or(Point::new(
                            tile_bounds.origin.x + 8.0,
                            tile_bounds.origin.y + 16.0,
                        ))
                };
                let label_origin = Point::new(
                    anchor
                        .x
                        .clamp(tile_bounds.origin.x + 6.0, tile_bounds.max_x() - 120.0),
                    anchor
                        .y
                        .clamp(tile_bounds.origin.y + 10.0, tile_bounds.max_y() - 8.0),
                );
                let angle_deg = pane_state.measurement_angle_deg.unwrap_or(0.0);
                paint.scene.draw_text(paint.text.layout(
                    &format!("Measure d={distance_px:.2}px a={angle_deg:.2}deg"),
                    label_origin,
                    8.0,
                    theme::status::SUCCESS,
                ));
            }
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
        paint.scene.push_clip(warning_panel);
        paint.scene.draw_quad(
            Quad::new(warning_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            "Warnings",
            Point::new(warning_panel.origin.x + 6.0, warning_panel.origin.y + 10.0),
            9.0,
            theme::text::SECONDARY,
        ));
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
        paint.scene.pop_clip();
    }

    if dimension_panel.size.width > 2.0 && dimension_panel.size.height > 2.0 {
        paint.scene.push_clip(dimension_panel);
        paint.scene.draw_quad(
            Quad::new(dimension_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            "Dimensions",
            Point::new(
                dimension_panel.origin.x + 6.0,
                dimension_panel.origin.y + 10.0,
            ),
            9.0,
            theme::text::SECONDARY,
        ));

        for (visible_index, (dimension_index, dimension)) in pane_state
            .visible_dimension_slots()
            .into_iter()
            .take(4)
            .enumerate()
        {
            let row_bounds = cad_demo_dimension_row_bounds(content_bounds, visible_index);
            if row_bounds.max_y() > dimension_panel.max_y() {
                break;
            }
            let is_editing = pane_state
                .dimension_edit
                .as_ref()
                .is_some_and(|edit| edit.dimension_index == dimension_index);
            if is_editing {
                paint.scene.draw_quad(
                    Quad::new(row_bounds)
                        .with_background(theme::bg::ELEVATED)
                        .with_corner_radius(3.0),
                );
            }
            let value_text = if is_editing {
                pane_state
                    .dimension_edit
                    .as_ref()
                    .map(|edit| edit.draft_value.clone())
                    .unwrap_or_else(|| format!("{:.3}", dimension.value_mm))
            } else {
                format!("{:.3}", dimension.value_mm)
            };
            let prefix = if is_editing { ">" } else { "" };
            paint.scene.draw_text(paint.text.layout(
                &format!("{prefix}{}: {value_text} mm", dimension.label),
                Point::new(row_bounds.origin.x + 4.0, row_bounds.origin.y + 10.0),
                8.5,
                if is_editing {
                    theme::text::PRIMARY
                } else {
                    theme::text::MUTED
                },
            ));
        }

        if let Some(edit) = pane_state.dimension_edit.as_ref()
            && let Some(error) = edit.last_error.as_deref()
        {
            let error_origin = Point::new(
                dimension_panel.origin.x + 6.0,
                (dimension_panel.max_y() - 8.0).max(dimension_panel.origin.y + 12.0),
            );
            if error_origin.y + 8.0 <= dimension_panel.max_y() {
                paint.scene.draw_text(paint.text.layout(
                    error,
                    error_origin,
                    7.5,
                    theme::status::ERROR,
                ));
            }
        }
        paint.scene.pop_clip();
    }

    if timeline_panel.size.width > 2.0 && timeline_panel.size.height > 2.0 {
        paint.scene.push_clip(timeline_panel);
        paint.scene.draw_quad(
            Quad::new(timeline_panel)
                .with_background(theme::bg::SURFACE)
                .with_corner_radius(4.0)
                .with_border(theme::border::SUBTLE, 1.0),
        );
        paint.scene.draw_text(paint.text.layout(
            "Timeline",
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
        } else if let Some((inspect_title, inspect_lines)) =
            assembly_selection_inspect_lines(pane_state)
        {
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
        paint.scene.pop_clip();
    }

    if pane_state.context_menu.is_open && !pane_state.context_menu.items.is_empty() {
        let menu_bounds = cad_demo_context_menu_bounds(
            content_bounds,
            pane_state.context_menu.anchor,
            pane_state.context_menu.items.len(),
        );
        paint.scene.push_clip(menu_bounds);
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
        paint.scene.pop_clip();
    }

    if layout.footer_origin.y + 8.0 <= content_bounds.max_y() {
        let summary = footer_summary_line(pane_state);
        paint.scene.draw_text(paint.text.layout(
            &summary,
            layout.footer_origin,
            9.0,
            theme::text::MUTED,
        ));
    }
    paint.scene.pop_clip();
}

fn paint_cad_demo_basic_pane(
    content_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) {
    paint.scene.push_clip(content_bounds);

    let cycle_bounds = cad_demo_cycle_variant_button_bounds(content_bounds);
    let jaw_bounds = cad_demo_gripper_jaw_button_bounds(content_bounds);
    let reset_bounds = cad_demo_reset_button_bounds(content_bounds);
    let reset_camera_bounds = cad_demo_reset_camera_button_bounds(content_bounds);
    let projection_bounds = cad_demo_projection_mode_button_bounds(content_bounds);
    let viewport_layout_bounds = cad_demo_viewport_layout_button_bounds(content_bounds);
    let drawing_mode_bounds = cad_demo_drawing_mode_button_bounds(content_bounds);
    let drawing_direction_bounds = cad_demo_drawing_direction_button_bounds(content_bounds);
    let drawing_hidden_lines_bounds = cad_demo_drawing_hidden_lines_button_bounds(content_bounds);
    let drawing_dimensions_bounds = cad_demo_drawing_dimensions_button_bounds(content_bounds);
    let drawing_reset_view_bounds = cad_demo_drawing_reset_view_button_bounds(content_bounds);
    let drawing_add_detail_bounds = cad_demo_drawing_add_detail_button_bounds(content_bounds);
    let drawing_clear_details_bounds = cad_demo_drawing_clear_details_button_bounds(content_bounds);

    paint_action_button(cycle_bounds, "Variant", paint);
    paint_action_button(
        jaw_bounds,
        if matches!(
            pane_state.active_design_profile(),
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
        ) {
            if pane_state.gripper_jaw_open {
                "Pose: Pinch"
            } else {
                "Pose: Tripod"
            }
        } else if pane_state.gripper_jaw_open {
            "Jaw: Close"
        } else {
            "Jaw: Open"
        },
        paint,
    );
    paint_action_button(reset_bounds, "Open CAD", paint);
    paint_action_button(reset_camera_bounds, "Reset Camera", paint);
    paint_action_button(
        projection_bounds,
        &format!("Projection: {}", pane_state.projection_mode.label()),
        paint,
    );
    paint_action_button(
        viewport_layout_bounds,
        &format!("Layout: {}", pane_state.viewport_layout.label()),
        paint,
    );
    paint_action_button(
        drawing_mode_bounds,
        if pane_state.drawing_view_mode == CadDrawingViewMode::TwoD {
            "Mode: 2D"
        } else {
            "Mode: 3D"
        },
        paint,
    );
    paint_action_button(
        drawing_direction_bounds,
        &format!("Direction: {}", pane_state.drawing_view_direction.label()),
        paint,
    );
    paint_action_button(
        drawing_hidden_lines_bounds,
        if pane_state.drawing_show_hidden_lines {
            "Hidden: On"
        } else {
            "Hidden: Off"
        },
        paint,
    );
    paint_action_button(
        drawing_dimensions_bounds,
        if pane_state.drawing_show_dimensions {
            "Dims: On"
        } else {
            "Dims: Off"
        },
        paint,
    );
    paint_action_button(drawing_reset_view_bounds, "Reset 2D", paint);
    paint_action_button(drawing_add_detail_bounds, "Detail +", paint);
    paint_action_button(
        drawing_clear_details_bounds,
        &format!("Clear ({})", pane_state.drawing_detail_views.len()),
        paint,
    );

    let viewport_bounds = cad_demo_basic_viewport_bounds(content_bounds);
    paint.scene.draw_quad(
        Quad::new(viewport_bounds)
            .with_background(theme::bg::ELEVATED)
            .with_corner_radius(4.0)
            .with_border(theme::border::SUBTLE, 1.0),
    );

    let mut status = format!(
        "{} | rev {}",
        pane_state.active_variant_id, pane_state.document_revision
    );
    if pane_state.drawing_view_mode == CadDrawingViewMode::TwoD {
        status = paint_drawing_mode_viewport(viewport_bounds, pane_state, paint);
    } else {
        status = paint_basic_3d_viewport(viewport_bounds, pane_state, paint);
    }

    paint.scene.draw_text(paint.text.layout(
        if pane_state.drawing_view_mode == CadDrawingViewMode::TwoD {
            "CAD Drawing"
        } else {
            "CAD"
        },
        Point::new(
            viewport_bounds.origin.x + 8.0,
            viewport_bounds.origin.y + 12.0,
        ),
        10.0,
        theme::text::SECONDARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        &status,
        Point::new(
            viewport_bounds.origin.x + 8.0,
            viewport_bounds.origin.y + 24.0,
        ),
        9.0,
        theme::text::MUTED,
    ));

    let footer_y = (content_bounds.max_y() - 10.0).max(content_bounds.origin.y + 8.0);
    paint.scene.draw_text(paint.text.layout(
        &footer_summary_line(pane_state),
        Point::new(content_bounds.origin.x + 10.0, footer_y),
        9.0,
        theme::text::MUTED,
    ));

    paint.scene.pop_clip();
}

fn viewport_quad_tile_bounds(viewport: Bounds, tile_index: usize) -> Bounds {
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

fn paint_basic_3d_viewport(
    viewport_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) -> String {
    let Some(mesh_payload) = pane_state.last_good_mesh_payload.as_ref() else {
        return "waiting for CAD mesh payload".to_string();
    };

    if pane_state.viewport_layout == CadViewportLayout::Single {
        let active_variant_view = pane_state.variant_viewport(pane_state.active_variant_tile_index);
        let selected_outline_active =
            active_variant_view.is_some_and(|view| view.selected_ref.is_some());
        let camera_pose = if let Some(viewport_state) = active_variant_view {
            CadCameraPose::from_variant(viewport_state, pane_state.projection_mode)
        } else {
            let fallback = CadVariantViewportState::for_variant(&pane_state.active_variant_id);
            CadCameraPose::from_variant(&fallback, pane_state.projection_mode)
        };
        return match cad_mesh_to_viewport_primitive(
            mesh_payload,
            viewport_bounds,
            selected_outline_active,
            pane_state.hidden_line_mode,
            camera_pose,
        ) {
            Ok(mesh) => {
                paint.scene.push_clip(viewport_bounds);
                if let Err(error) = paint.scene.draw_mesh(mesh) {
                    let _ = error;
                    paint.scene.pop_clip();
                    "mesh draw skipped".to_string()
                } else {
                    paint.scene.pop_clip();
                    format!(
                        "{} | rev {}",
                        pane_state.active_variant_id, pane_state.document_revision
                    )
                }
            }
            Err(error) => format!("mesh conversion failed: {error}"),
        };
    }

    let mut status = "quad layout showing 4 variants".to_string();
    for tile_index in 0..4 {
        let tile_bounds = viewport_quad_tile_bounds(viewport_bounds, tile_index);
        if tile_bounds.size.width < 2.0 || tile_bounds.size.height < 2.0 {
            continue;
        }
        let variant_id = pane_state
            .variant_ids
            .get(tile_index)
            .cloned()
            .unwrap_or_else(|| "variant.unset".to_string());
        let tile_view = pane_state
            .variant_viewports
            .iter()
            .find(|view| view.variant_id == variant_id);
        let is_active = pane_state.active_variant_id == variant_id;

        let mut tile_quad = Quad::new(tile_bounds)
            .with_background(theme::bg::SURFACE)
            .with_corner_radius(4.0)
            .with_border(theme::border::SUBTLE, 1.0);
        if is_active {
            tile_quad = tile_quad.with_border(theme::text::PRIMARY, 1.0);
        }
        paint.scene.draw_quad(tile_quad);
        paint.scene.draw_text(paint.text.layout(
            &variant_id,
            Point::new(tile_bounds.origin.x + 6.0, tile_bounds.origin.y + 10.0),
            8.0,
            if is_active {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            },
        ));

        let selected_outline_active = tile_view.is_some_and(|view| view.selected_ref.is_some());
        let camera_pose = if let Some(view) = tile_view {
            CadCameraPose::from_variant(view, pane_state.projection_mode)
        } else {
            let fallback = CadVariantViewportState::for_variant(&variant_id);
            CadCameraPose::from_variant(&fallback, pane_state.projection_mode)
        };
        match cad_mesh_to_viewport_primitive(
            mesh_payload,
            tile_bounds,
            selected_outline_active,
            pane_state.hidden_line_mode,
            camera_pose,
        ) {
            Ok(mesh) => {
                paint.scene.push_clip(tile_bounds);
                if let Err(error) = paint.scene.draw_mesh(mesh) {
                    let _ = error;
                    status = format!("mesh draw skipped tile {}", tile_index + 1);
                }
                paint.scene.pop_clip();
            }
            Err(error) => {
                status = format!("mesh conversion failed tile {}: {}", tile_index + 1, error);
            }
        }
    }

    status
}

fn paint_drawing_mode_viewport(
    viewport_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) -> String {
    let Some(payload) = pane_state.last_good_mesh_payload.as_ref() else {
        return "drawing mode waiting for CAD mesh payload".to_string();
    };
    let Some(mesh) = cad_mesh_payload_to_drafting_mesh(payload) else {
        return "drawing mode mesh conversion failed".to_string();
    };

    let mut projected = project_drafting_mesh(
        &mesh,
        pane_state
            .drawing_view_direction
            .to_drafting_view_direction(),
    );
    if !pane_state.drawing_show_hidden_lines {
        projected
            .edges
            .retain(|edge| edge.visibility == DraftingVisibility::Visible);
    }

    if projected.edges.is_empty() {
        return "drawing mode has no projected edges".to_string();
    }

    let mesh = match drafting_projected_view_to_mesh(viewport_bounds, pane_state, &projected) {
        Ok(mesh) => mesh,
        Err(error) => return error,
    };

    paint.scene.push_clip(viewport_bounds);
    if paint.scene.draw_mesh(mesh).is_err() {
        paint.scene.pop_clip();
        return "drawing mode mesh draw skipped".to_string();
    }
    if pane_state.drawing_show_dimensions {
        let dim_origin = Point::new(
            viewport_bounds.origin.x + 8.0,
            viewport_bounds.max_y() - 20.0,
        );
        if dim_origin.y + 8.0 <= viewport_bounds.max_y() {
            paint.scene.draw_text(paint.text.layout(
                &format!(
                    "Dims W={:.1} H={:.1}",
                    projected.bounds.width(),
                    projected.bounds.height()
                ),
                dim_origin,
                8.0,
                theme::text::SECONDARY,
            ));
        }
    }
    if !pane_state.drawing_detail_views.is_empty() {
        for (index, detail) in pane_state.drawing_detail_views.iter().take(4).enumerate() {
            let y = viewport_bounds.origin.y + 10.0 + index as f32 * 10.0;
            if y + 8.0 > viewport_bounds.max_y() {
                break;
            }
            paint.scene.draw_text(paint.text.layout(
                &format!("Detail {} x{:.1}", detail.label, detail.scale),
                Point::new(
                    (viewport_bounds.max_x() - 110.0).max(viewport_bounds.origin.x + 6.0),
                    y,
                ),
                8.0,
                theme::text::MUTED,
            ));
        }
    }
    paint.scene.pop_clip();

    let hidden_count = projected
        .edges
        .iter()
        .filter(|edge| edge.visibility == DraftingVisibility::Hidden)
        .count();
    format!(
        "2d {} edges={} hidden={} zoom={:.2} details={}",
        pane_state.drawing_view_direction.label(),
        projected.edges.len(),
        hidden_count,
        pane_state.drawing_zoom,
        pane_state.drawing_detail_views.len(),
    )
}

fn cad_mesh_payload_to_drafting_mesh(payload: &CadMeshPayload) -> Option<DraftingTriangleMesh> {
    if payload.vertices.is_empty() || payload.triangle_indices.is_empty() {
        return None;
    }
    if !payload.triangle_indices.len().is_multiple_of(3) {
        return None;
    }
    let vertices = payload
        .vertices
        .iter()
        .map(|vertex| {
            Point3::new(
                f64::from(vertex.position_mm[0]),
                f64::from(vertex.position_mm[1]),
                f64::from(vertex.position_mm[2]),
            )
        })
        .collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(payload.triangle_indices.len() / 3);
    let vertex_len = vertices.len();
    for triangle in payload.triangle_indices.chunks_exact(3) {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;
        if i0 >= vertex_len || i1 >= vertex_len || i2 >= vertex_len {
            return None;
        }
        triangles.push([i0, i1, i2]);
    }
    Some(DraftingTriangleMesh {
        vertices,
        triangles,
    })
}

fn drafting_projected_view_to_mesh(
    viewport_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    view: &DraftingProjectedView,
) -> Result<MeshPrimitive, String> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for edge in &view.edges {
        min_x = min_x.min(edge.start.x.min(edge.end.x));
        min_y = min_y.min(edge.start.y.min(edge.end.y));
        max_x = max_x.max(edge.start.x.max(edge.end.x));
        max_y = max_y.max(edge.start.y.max(edge.end.y));
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return Err("drawing mode bounds are invalid".to_string());
    }
    let width = (max_x - min_x).max(1e-6);
    let height = (max_y - min_y).max(1e-6);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;

    let pad = 10.0f64;
    let scale_x = (f64::from(viewport_bounds.size.width) - pad * 2.0).max(1.0) / width;
    let scale_y = (f64::from(viewport_bounds.size.height) - pad * 2.0).max(1.0) / height;
    let scale = scale_x.min(scale_y).max(1e-6) * f64::from(pane_state.drawing_zoom.max(0.1));

    let viewport_center_x = f64::from(
        viewport_bounds.origin.x + viewport_bounds.size.width * 0.5 + pane_state.drawing_pan_x,
    );
    let viewport_center_y = f64::from(
        viewport_bounds.origin.y + viewport_bounds.size.height * 0.5 - pane_state.drawing_pan_y,
    );

    let mut vertices = Vec::with_capacity(view.edges.len() * 2);
    let mut edges = Vec::with_capacity(view.edges.len());
    for edge in &view.edges {
        let start_index = vertices.len() as u32;
        let color = if edge.visibility == DraftingVisibility::Hidden {
            [0.45, 0.48, 0.52, 0.72]
        } else {
            [0.84, 0.88, 0.92, 1.0]
        };
        for point in [edge.start, edge.end] {
            let px = viewport_center_x + (point.x - center_x) * scale;
            let py = viewport_center_y - (point.y - center_y) * scale;
            vertices.push(MeshVertex::new(
                [px as f32, py as f32, 0.0],
                [0.0, 0.0, 1.0],
                color,
            ));
        }
        let mut flags = 0;
        if edge.visibility == DraftingVisibility::Visible {
            flags |= MESH_EDGE_FLAG_SILHOUETTE;
        }
        edges.push(MeshEdge::new(start_index, start_index + 1).with_flags(flags));
    }

    let indices = if vertices.len() >= 3 {
        vec![0, 1, 2]
    } else if vertices.len() == 2 {
        vec![0, 1, 1]
    } else if vertices.len() == 1 {
        vec![0, 0, 0]
    } else {
        return Err("drawing mode produced no vertices".to_string());
    };

    Ok(MeshPrimitive::new(vertices, indices).with_edges(edges))
}

fn engineering_overlay_lines(
    pane_state: &CadDemoPaneState,
) -> [String; ENGINEERING_OVERLAY_LINE_COUNT] {
    let analysis = &pane_state.analysis_snapshot;
    let material_id = analysis
        .material_id
        .as_deref()
        .unwrap_or(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID);
    let volume = analysis
        .volume_mm3
        .map(|value| format!("{value:.1} mm^3"))
        .unwrap_or_else(|| "--".to_string());
    let mass = analysis
        .mass_kg
        .map(|value| format!("{value:.3} kg"))
        .unwrap_or_else(|| "--".to_string());
    let cost = analysis
        .estimated_cost_usd
        .map(|value| format!("${value:.2}"))
        .unwrap_or_else(|| "--".to_string());
    let deflection = analysis
        .max_deflection_mm
        .map(|value| format!("{value:.3} mm"))
        .unwrap_or_else(|| "--".to_string());
    let deflection_confidence = analysis
        .estimator_metadata
        .get("deflection.confidence")
        .cloned()
        .unwrap_or_else(|| "n/a".to_string());
    let cog = analysis
        .center_of_gravity_mm
        .map(|value| format!("{:.1}, {:.1}, {:.1}", value[0], value[1], value[2]))
        .unwrap_or_else(|| "--".to_string());
    [
        format!("Material: {material_id}"),
        format!("Volume: {volume}"),
        format!("Mass: {mass}"),
        format!("Cost: {cost}"),
        format!("Deflection: {deflection} ({deflection_confidence})"),
        format!("CoG: {cog}"),
    ]
}

fn tile_caption(
    tile_index: usize,
    is_active: bool,
    selection: Option<&str>,
    hover: Option<&str>,
) -> String {
    let mut caption = format!("{}{}", tile_index + 1, if is_active { "*" } else { "" });
    if selection.is_some() {
        caption.push_str(" sel");
    }
    if hover.is_some() {
        caption.push_str(" hov");
    }
    caption
}

fn footer_summary_line(pane_state: &CadDemoPaneState) -> String {
    let material_id = pane_state
        .analysis_snapshot
        .material_id
        .as_deref()
        .unwrap_or(openagents_cad::materials::DEFAULT_CAD_MATERIAL_ID);
    let mass_label = pane_state
        .analysis_snapshot
        .mass_kg
        .map(|value| format!("{value:.3}kg"))
        .unwrap_or_else(|| "--kg".to_string());
    let cost_label = pane_state
        .analysis_snapshot
        .estimated_cost_usd
        .map(|value| format!("${value:.2}"))
        .unwrap_or_else(|| "$--".to_string());
    let summary = format!(
        "{} @ tile{} | rev {} | warn {} | {} z{:.2} | mode {}/{} z{:.2} | section {} | material {} | {} {} | snaps {} | hotkeys {}",
        pane_state.active_variant_id,
        pane_state.active_variant_tile_index + 1,
        pane_state.document_revision,
        pane_state.warnings.len(),
        pane_state.projection_mode.label(),
        pane_state.camera_zoom,
        pane_state.drawing_view_mode.label(),
        pane_state.drawing_view_direction.label(),
        pane_state.drawing_zoom,
        pane_state.section_summary(),
        material_id,
        mass_label,
        cost_label,
        pane_state.snap_summary(),
        pane_state.hotkey_profile,
    );
    truncate_with_ellipsis(summary, 220)
}

fn truncate_with_ellipsis(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let mut truncated = String::new();
    for ch in value.chars().take(max_chars - 3) {
        truncated.push(ch);
    }
    truncated.push_str("...");
    truncated
}

fn engineering_overlay_bounds(viewport_bounds: Bounds) -> Bounds {
    let width = ENGINEERING_OVERLAY_WIDTH
        .min((viewport_bounds.size.width - 8.0).max(40.0))
        .max(80.0);
    let height = (22.0 + ENGINEERING_OVERLAY_LINE_COUNT as f32 * ENGINEERING_OVERLAY_LINE_HEIGHT)
        .min((viewport_bounds.size.height - 8.0).max(20.0))
        .max(20.0);
    let origin_x = (viewport_bounds.max_x() - width - 4.0).max(viewport_bounds.origin.x + 2.0);
    let origin_y = (viewport_bounds.origin.y + 14.0)
        .min((viewport_bounds.max_y() - height - 2.0).max(viewport_bounds.origin.y + 2.0));
    Bounds::new(origin_x, origin_y, width, height)
}

fn paint_engineering_overlay(
    viewport_bounds: Bounds,
    pane_state: &CadDemoPaneState,
    paint: &mut PaintContext,
) {
    let overlay_bounds = engineering_overlay_bounds(viewport_bounds);
    if overlay_bounds.size.width <= 2.0 || overlay_bounds.size.height <= 2.0 {
        return;
    }
    let lines = engineering_overlay_lines(pane_state);
    paint.scene.draw_quad(
        Quad::new(overlay_bounds)
            .with_background(Hsla::new(0.0, 0.0, 0.10, 0.92))
            .with_corner_radius(4.0)
            .with_border(theme::border::SUBTLE, 1.0),
    );
    let title_origin = Point::new(
        overlay_bounds.origin.x + 6.0,
        overlay_bounds.origin.y + 10.0,
    );
    if title_origin.y + 8.0 <= overlay_bounds.max_y() {
        paint.scene.draw_text(paint.text.layout(
            "Engineering",
            title_origin,
            8.0,
            theme::text::SECONDARY,
        ));
    }
    for (index, line) in lines.iter().enumerate() {
        let y = title_origin.y + 10.0 + index as f32 * ENGINEERING_OVERLAY_LINE_HEIGHT;
        if y + 8.0 > overlay_bounds.max_y() {
            break;
        }
        paint.scene.draw_text(paint.text.layout(
            line,
            Point::new(overlay_bounds.origin.x + 6.0, y),
            8.0,
            theme::text::PRIMARY,
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

fn assembly_selection_inspect_lines(
    pane_state: &CadDemoPaneState,
) -> Option<(String, Vec<String>)> {
    if let Some(joint_id) = pane_state.assembly_ui_state.selected_joint_id.as_deref()
        && let Some(joint) = pane_state
            .assembly_schema
            .joints
            .iter()
            .find(|joint| joint.id == joint_id)
    {
        let kind = match joint.kind {
            openagents_cad::assembly::CadJointKind::Fixed => "Fixed",
            openagents_cad::assembly::CadJointKind::Revolute { .. } => "Revolute",
            openagents_cad::assembly::CadJointKind::Slider { .. } => "Slider",
            openagents_cad::assembly::CadJointKind::Cylindrical { .. } => "Cylindrical",
            openagents_cad::assembly::CadJointKind::Ball => "Ball",
        };
        let semantics = joint.resolve_state_semantics(joint.state);
        let limits = semantics
            .limits
            .map(|(lower, upper)| format!("[{lower:.1}, {upper:.1}] {}", semantics.state_unit))
            .unwrap_or_else(|| "none".to_string());
        return Some((
            "Assembly Joint".to_string(),
            vec![
                format!("Joint: {}", joint.id),
                format!("Kind: {kind}"),
                format!(
                    "State: {:.3} {}",
                    semantics.effective_state, semantics.state_unit
                ),
                format!("Limits: {limits}"),
                format!(
                    "Parent: {} Child: {}",
                    joint.parent_instance_id.as_deref().unwrap_or("world"),
                    joint.child_instance_id
                ),
            ],
        ));
    }

    if let Some(instance_id) = pane_state.assembly_ui_state.selected_instance_id.as_deref()
        && let Some(instance) = pane_state
            .assembly_schema
            .instances
            .iter()
            .find(|instance| instance.id == instance_id)
    {
        let transform = instance
            .transform
            .unwrap_or_else(openagents_cad::assembly::CadTransform3D::identity);
        let is_ground =
            pane_state.assembly_schema.ground_instance_id.as_deref() == Some(instance_id);
        return Some((
            "Assembly Instance".to_string(),
            vec![
                format!("Instance: {}", instance.id),
                format!("PartDef: {}", instance.part_def_id),
                format!("Name: {}", instance.name.as_deref().unwrap_or("-")),
                format!(
                    "T: ({:.1}, {:.1}, {:.1}) mm",
                    transform.translation.x, transform.translation.y, transform.translation.z
                ),
                format!("Ground: {}", if is_ground { "yes" } else { "no" }),
            ],
        ));
    }

    None
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
    let bounds_extent_x = (payload.bounds.max_mm[0] - payload.bounds.min_mm[0]).abs();
    let bounds_extent_y = (payload.bounds.max_mm[1] - payload.bounds.min_mm[1]).abs();
    let bounds_extent_z = (payload.bounds.max_mm[2] - payload.bounds.min_mm[2]).abs();
    let bounds_diameter = (bounds_extent_x * bounds_extent_x
        + bounds_extent_y * bounds_extent_y
        + bounds_extent_z * bounds_extent_z)
        .sqrt()
        .max(1.0);
    let fit_diameter = if camera_pose.projection_mode == CadProjectionMode::Perspective {
        bounds_diameter * CAD_PERSPECTIVE_FIT_EXPANSION
    } else {
        bounds_diameter
    };
    let fit_scale = (available_width / fit_diameter)
        .min(available_height / fit_diameter)
        .max(0.0001);
    let zoom = if camera_pose.zoom.is_finite() {
        camera_pose.zoom
    } else {
        1.0
    }
    .clamp(CAD_CAMERA_MIN_ZOOM, CAD_CAMERA_MAX_ZOOM);
    let pan_x = if camera_pose.pan_x.is_finite() {
        camera_pose.pan_x
    } else {
        0.0
    }
    .clamp(
        -viewport_bounds.size.width * 0.45,
        viewport_bounds.size.width * 0.45,
    );
    let pan_y = if camera_pose.pan_y.is_finite() {
        camera_pose.pan_y
    } else {
        0.0
    }
    .clamp(
        -viewport_bounds.size.height * 0.45,
        viewport_bounds.size.height * 0.45,
    );
    let scale = (fit_scale * zoom).max(0.0001);
    let scaled_width = model_width * scale;
    let scaled_height = model_height * scale;
    let origin_x =
        viewport_bounds.origin.x + ((viewport_bounds.size.width - scaled_width) * 0.5) + pan_x;
    let origin_y =
        viewport_bounds.origin.y + ((viewport_bounds.size.height - scaled_height) * 0.5) + pan_y;

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
    let show_edges = hidden_line_mode != CadHiddenLineMode::Shaded;
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
        CadCameraPose, assembly_selection_inspect_lines, cad_mesh_payload_to_drafting_mesh,
        cad_mesh_to_viewport_primitive, drafting_projected_view_to_mesh,
        engineering_overlay_bounds, engineering_overlay_lines, footer_summary_line,
        placeholder_layout, selection_inspect_lines, tile_caption, truncate_with_ellipsis,
        variant_tile_bounds, variant_tile_index_at_point,
    };
    use openagents_cad::analysis::{DENSITY_ALUMINUM_6061_KG_M3, estimate_body_properties};
    use openagents_cad::contracts::CadSelectionKind;
    use openagents_cad::drafting::{
        EdgeType as DraftingEdgeType, Point2D as DraftingPoint2D,
        ProjectedEdge as DraftingProjectedEdge, ProjectedView as DraftingProjectedView,
        ViewDirection as DraftingViewDirection, Visibility as DraftingVisibility,
    };
    use openagents_cad::mesh::{
        CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
        CadMeshVertex,
    };
    use wgpui::{Bounds, MESH_EDGE_FLAG_SELECTED, MESH_EDGE_FLAG_SILHOUETTE, Point};

    use crate::app_state::{
        CadDemoPaneState, CadHiddenLineMode, CadProjectionMode, CadSectionAxis,
    };

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

    fn projected_extents(mesh: &wgpui::MeshPrimitive) -> (f32, f32, f32, f32) {
        let min_x = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        (min_x, max_x, min_y, max_y)
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
    fn engineering_overlay_bounds_stay_within_viewport() {
        let viewport = Bounds::new(30.0, 50.0, 360.0, 220.0);
        let overlay = engineering_overlay_bounds(viewport);
        assert!(overlay.origin.x >= viewport.origin.x);
        assert!(overlay.origin.y >= viewport.origin.y);
        assert!(overlay.max_x() <= viewport.max_x() + 0.001);
        assert!(overlay.max_y() <= viewport.max_y() + 0.001);
    }

    #[test]
    fn engineering_overlay_lines_reflect_live_analysis_values() {
        let mut state = CadDemoPaneState::default();
        state.analysis_snapshot.material_id = Some("al-6061-t6".to_string());
        state.analysis_snapshot.volume_mm3 = Some(1_240_000.0);
        state.analysis_snapshot.mass_kg = Some(2.73);
        state.analysis_snapshot.estimated_cost_usd = Some(128.45);
        state.analysis_snapshot.max_deflection_mm = Some(0.58);
        state.analysis_snapshot.center_of_gravity_mm = Some([40.0, 18.0, 72.0]);
        state.analysis_snapshot.estimator_metadata = std::collections::BTreeMap::from([(
            "deflection.confidence".to_string(),
            "medium".to_string(),
        )]);

        let first = engineering_overlay_lines(&state);
        assert!(first[0].contains("Material: al-6061-t6"));
        assert!(first[1].contains("Volume:"));
        assert!(first[2].contains("Mass:"));
        assert!(first[3].contains("Cost:"));
        assert!(first[4].contains("Deflection:"));
        assert!(first[4].contains("(medium)"));
        assert!(first[5].contains("CoG:"));

        state.analysis_snapshot.mass_kg = Some(2.10);
        let second = engineering_overlay_lines(&state);
        assert_ne!(first[2], second[2]);
    }

    #[test]
    fn tile_caption_is_compact_and_readable() {
        assert_eq!(
            tile_caption(0, true, Some("face.1"), Some("edge.2")),
            "1* sel hov"
        );
        assert_eq!(tile_caption(1, false, None, None), "2");
    }

    #[test]
    fn footer_summary_line_avoids_session_noise() {
        let mut state = CadDemoPaneState::default();
        state.document_revision = 42;
        state.analysis_snapshot.material_id = Some("al-6061-t6".to_string());
        state.analysis_snapshot.mass_kg = Some(2.731);
        state.analysis_snapshot.estimated_cost_usd = Some(128.44);
        let summary = footer_summary_line(&state);
        assert!(summary.contains("variant.baseline"));
        assert!(summary.contains("rev 42"));
        assert!(summary.contains("mode 3d/front"));
        assert!(summary.contains("material al-6061-t6"));
        assert!(summary.contains("2.731kg"));
        assert!(summary.contains("$128.44"));
        assert!(!summary.contains("session="));
        assert!(!summary.contains("orbit="));
    }

    #[test]
    fn footer_summary_line_truncates_for_long_tokens() {
        let mut state = CadDemoPaneState::default();
        state.active_variant_id =
            "variant.extremely-long-identifier-with-many-segments-and-extra-context".to_string();
        state.hotkey_profile =
            "profile.with.a.very.long.name.for.testing.clip.behavior".to_string();
        let summary = footer_summary_line(&state);
        assert!(summary.chars().count() <= 220);
    }

    #[test]
    fn truncate_with_ellipsis_respects_limit() {
        let value = "abcdefghijklmnopqrstuvwxyz".to_string();
        let truncated = truncate_with_ellipsis(value, 10);
        assert_eq!(truncated, "abcdefg...");
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
    fn cad_mesh_payload_to_drafting_mesh_rejects_invalid_indices() {
        let mut payload = CadMeshPayload::default();
        payload.vertices = vec![CadMeshVertex::default()];
        payload.triangle_indices = vec![0, 1, 2];
        assert!(cad_mesh_payload_to_drafting_mesh(&payload).is_none());
    }

    #[test]
    fn drafting_projected_view_to_mesh_is_deterministic() {
        let viewport = Bounds::new(10.0, 20.0, 280.0, 160.0);
        let mut state = CadDemoPaneState::default();
        state.drawing_zoom = 1.25;
        state.drawing_pan_x = 14.0;
        state.drawing_pan_y = -8.0;

        let mut view = DraftingProjectedView::new(DraftingViewDirection::Front);
        view.add_edge(DraftingProjectedEdge::new(
            DraftingPoint2D::new(-20.0, -10.0),
            DraftingPoint2D::new(20.0, -10.0),
            DraftingVisibility::Visible,
            DraftingEdgeType::Sharp,
            0.0,
        ));
        view.add_edge(DraftingProjectedEdge::new(
            DraftingPoint2D::new(20.0, -10.0),
            DraftingPoint2D::new(20.0, 15.0),
            DraftingVisibility::Hidden,
            DraftingEdgeType::Silhouette,
            0.0,
        ));

        let first = drafting_projected_view_to_mesh(viewport, &state, &view)
            .expect("projection mapping should succeed");
        let second = drafting_projected_view_to_mesh(viewport, &state, &view)
            .expect("projection mapping should remain deterministic");
        assert_eq!(first, second);
        assert_eq!(first.edges.len(), 2);
        assert_eq!(first.indices.len(), 3);
        assert!(!first.vertices.is_empty());
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
    fn mesh_projection_clamps_zoom_to_stay_within_viewport_fit() {
        let viewport = Bounds::new(5.0, 7.0, 240.0, 160.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.zoom.clamp".to_string(),
            document_revision: 1,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [-40.0, -30.0, -20.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, -30.0, -20.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, 30.0, 20.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [-40.0, 30.0, 20.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 2, 3],
            edges: vec![CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            }],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [-40.0, -30.0, -20.0],
                max_mm: [40.0, 30.0, 20.0],
            },
        };

        let zoomed = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                zoom: 3.5,
                ..default_camera_pose()
            },
        )
        .expect("zoomed projection should still succeed");

        for vertex in &zoomed.vertices {
            assert!(vertex.position[0] >= viewport.origin.x - 0.001);
            assert!(vertex.position[0] <= viewport.max_x() + 0.001);
            assert!(vertex.position[1] >= viewport.origin.y - 0.001);
            assert!(vertex.position[1] <= viewport.max_y() + 0.001);
        }
    }

    #[test]
    fn mesh_projection_uses_orientation_stable_fit_scale() {
        let viewport = Bounds::new(0.0, 0.0, 220.0, 140.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.fit.stable".to_string(),
            document_revision: 1,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [-60.0, -10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [60.0, -10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [60.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [-60.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 2, 3],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [-60.0, -10.0, 0.0],
                max_mm: [60.0, 10.0, 0.0],
            },
        };

        let yaw_0 = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                orbit_yaw_deg: 0.0,
                orbit_pitch_deg: 0.0,
                ..default_camera_pose()
            },
        )
        .expect("yaw=0 projection should succeed");
        let yaw_90 = cad_mesh_to_viewport_primitive(
            &payload,
            viewport,
            false,
            CadHiddenLineMode::Shaded,
            CadCameraPose {
                orbit_yaw_deg: 90.0,
                orbit_pitch_deg: 0.0,
                ..default_camera_pose()
            },
        )
        .expect("yaw=90 projection should succeed");

        let (min_x0, max_x0, min_y0, max_y0) = projected_extents(&yaw_0);
        let (min_x90, max_x90, min_y90, max_y90) = projected_extents(&yaw_90);
        let longest_0 = (max_x0 - min_x0).max(max_y0 - min_y0);
        let longest_90 = (max_x90 - min_x90).max(max_y90 - min_y90);
        assert!((longest_0 - longest_90).abs() <= 0.5);
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
            CadHiddenLineMode::ShadedEdges,
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
    fn assembly_selection_inspect_lines_show_instance_details() {
        let mut state = CadDemoPaneState::default();
        state
            .select_assembly_instance("arm-1")
            .expect("select known assembly instance");

        let (title, lines) =
            assembly_selection_inspect_lines(&state).expect("assembly instance inspect lines");
        assert_eq!(title, "Assembly Instance");
        assert!(lines.iter().any(|line| line.contains("Instance: arm-1")));
        assert!(lines.iter().any(|line| line.contains("PartDef: arm")));
        assert!(lines.iter().any(|line| line.contains("Ground: no")));
    }

    #[test]
    fn assembly_selection_inspect_lines_show_joint_edit_state() {
        let mut state = CadDemoPaneState::default();
        state
            .select_assembly_joint("joint.hinge")
            .expect("select known assembly joint");
        let semantics = state
            .set_selected_assembly_joint_state(120.0)
            .expect("joint state edit should succeed");
        assert!(semantics.was_clamped);
        assert_eq!(semantics.effective_state, 90.0);

        let (title, lines) =
            assembly_selection_inspect_lines(&state).expect("assembly joint inspect lines");
        assert_eq!(title, "Assembly Joint");
        assert!(lines.iter().any(|line| line.contains("Joint: joint.hinge")));
        assert!(lines.iter().any(|line| line.contains("State: 90.000 deg")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Limits: [-90.0, 90.0] deg"))
        );
    }

    #[test]
    fn selection_inspect_lines_remain_available_with_section_mode_enabled() {
        let payload = CadMeshPayload {
            mesh_id: "mesh.inspect.section".to_string(),
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
        state.section_axis = Some(CadSectionAxis::X);
        state.section_offset_normalized = 0.2;
        let _ = state.selection_store.set_primary(
            CadSelectionKind::Body,
            "body.0",
            Some("body.0".to_string()),
        );

        let inspect = selection_inspect_lines(&state).expect("inspect should remain available");
        assert_eq!(inspect.0, "Body Inspect");
        assert!(inspect.1.iter().any(|line| line.contains("Volume")));
    }

    #[test]
    fn section_clipping_remains_compatible_with_hidden_line_modes() {
        let viewport = Bounds::new(10.0, 10.0, 180.0, 120.0);
        let payload = CadMeshPayload {
            mesh_id: "mesh.variant.section".to_string(),
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
                    position_mm: [0.0, 20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [12.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.5, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [12.0, 20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.5, 1.0],
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
                    position_mm: [20.0, 20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 2, 3, 4, 3, 5, 4],
            edges: vec![
                CadMeshEdgeSegment {
                    start_vertex: 2,
                    end_vertex: 3,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 4,
                    end_vertex: 5,
                    flags: 0,
                },
            ],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [20.0, 20.0, 0.0],
            },
        };
        let sectioned = openagents_cad::section::clip_mesh_payload(
            &payload,
            openagents_cad::section::CadSectionPlane::new(
                openagents_cad::section::CadSectionAxis::X,
                0.0,
            ),
            openagents_cad::policy::resolve_tolerance_mm(None) as f32,
        )
        .expect("section clipping should succeed");
        let shaded_edges = cad_mesh_to_viewport_primitive(
            &sectioned,
            viewport,
            false,
            CadHiddenLineMode::ShadedEdges,
            default_camera_pose(),
        )
        .expect("sectioned shaded+edges projection should succeed");
        let wireframe = cad_mesh_to_viewport_primitive(
            &sectioned,
            viewport,
            false,
            CadHiddenLineMode::Wireframe,
            default_camera_pose(),
        )
        .expect("sectioned wireframe projection should succeed");

        assert!(!shaded_edges.edges.is_empty());
        assert!(!wireframe.edges.is_empty());
        assert_eq!(
            wireframe,
            cad_mesh_to_viewport_primitive(
                &sectioned,
                viewport,
                false,
                CadHiddenLineMode::Wireframe,
                default_camera_pose(),
            )
            .expect("wireframe should remain deterministic")
        );
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
