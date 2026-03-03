use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::eval::DeterministicRebuildResult;
use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::mesh::{
    CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
    CadMeshVertex,
};
use crate::{CadError, CadResult};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadTessellationReceipt {
    pub document_revision: u64,
    pub variant_id: String,
    pub rebuild_hash: String,
    pub mesh_id: String,
    pub mesh_hash: String,
    pub feature_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub edge_count: usize,
    pub material_slot_count: usize,
}

/// Deterministically tessellate rebuild output into renderer-facing mesh payload.
pub fn tessellate_rebuild_result(
    graph: &FeatureGraph,
    rebuild: &DeterministicRebuildResult,
    document_revision: u64,
    variant_id: &str,
) -> CadResult<(CadMeshPayload, CadTessellationReceipt)> {
    if variant_id.trim().is_empty() {
        return Err(CadError::InvalidParameter {
            name: "variant_id".to_string(),
            reason: "variant id must not be empty".to_string(),
        });
    }
    graph.validate()?;
    let expected_ids = graph.deterministic_topo_order()?;
    if expected_ids != rebuild.ordered_feature_ids {
        return Err(CadError::EvalFailed {
            reason: "rebuild feature order does not match graph topological order".to_string(),
        });
    }

    let node_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let profile = RackVariantProfile::from_variant(variant_id);
    let mut builder = TessellationBuilder::default();

    for record in &rebuild.records {
        let Some(node) = node_by_id.get(record.feature_id.as_str()) else {
            return Err(CadError::EvalFailed {
                reason: format!("missing feature node {}", record.feature_id),
            });
        };
        tessellate_feature_node(&mut builder, node, &profile, &record.geometry_hash)?;
    }

    let bounds = builder.bounds().ok_or_else(|| CadError::EvalFailed {
        reason: "tessellation produced zero vertices".to_string(),
    })?;
    let mesh_id = format!(
        "mesh.{}.{}",
        sanitize_mesh_segment(variant_id),
        rebuild.rebuild_hash
    );
    let payload = CadMeshPayload {
        mesh_id: mesh_id.clone(),
        document_revision,
        variant_id: variant_id.to_string(),
        topology: CadMeshTopology::Triangles,
        vertices: builder.vertices,
        triangle_indices: builder.triangle_indices,
        edges: builder.edges,
        material_slots: material_slots(),
        bounds,
    };
    let encoded = payload.to_binary_payload()?;
    let receipt = CadTessellationReceipt {
        document_revision,
        variant_id: variant_id.to_string(),
        rebuild_hash: rebuild.rebuild_hash.clone(),
        mesh_id,
        mesh_hash: encoded.deterministic_hash,
        feature_count: rebuild.records.len(),
        vertex_count: payload.vertices.len(),
        triangle_count: payload.triangle_indices.len() / 3,
        edge_count: payload.edges.len(),
        material_slot_count: payload.material_slots.len(),
    };
    Ok((payload, receipt))
}

fn tessellate_feature_node(
    builder: &mut TessellationBuilder,
    node: &FeatureNode,
    profile: &RackVariantProfile,
    geometry_hash: &str,
) -> CadResult<()> {
    let jitter = hash_jitter(geometry_hash);
    match node.operation_key.as_str() {
        "primitive.box.v1" => {
            let width_mm = node_param_f32(node, "width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.width_mm);
            let depth_mm = node_param_f32(node, "depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.depth_mm);
            let height_mm = node_param_f32(node, "height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.height_mm);
            let min = [-width_mm * 0.5, -depth_mm * 0.5, 0.0 + jitter];
            let max = [width_mm * 0.5, depth_mm * 0.5, height_mm];
            builder.add_box(min, max, 0, 1);
            Ok(())
        }
        "cut.hole.v1" => {
            let width_mm = node_param_f32(node, "width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.width_mm);
            let depth_mm = node_param_f32(node, "depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.depth_mm);
            let height_mm = node_param_f32(node, "height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.height_mm);
            let radius_mm = node_param_f32(node, "mount_hole_radius_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.mount_hole_radius_mm);
            let depth_z = node_param_f32(node, "mount_hole_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(height_mm * 0.15);
            let z = height_mm * 0.65;
            let x_margin = width_mm * 0.42;
            let y_margin = depth_mm * 0.38;
            let centers = [
                [-x_margin, -y_margin, z],
                [x_margin, -y_margin, z],
                [-x_margin, y_margin, z],
                [x_margin, y_margin, z],
            ];
            for center in centers {
                builder.add_cylinder_z(center, radius_mm, depth_z, 14, 1, 2);
            }
            Ok(())
        }
        "linear.pattern.v1" => {
            let depth_mm = node_param_f32(node, "depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.depth_mm);
            let height_mm = node_param_f32(node, "height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.height_mm);
            let vent_spacing_mm = node_param_f32(node, "vent_spacing_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.vent_spacing_mm);
            let vent_count = node_param_u32(node, "vent_count")
                .map(|value| value.clamp(1, 16) as usize)
                .unwrap_or(profile.vent_count);
            let vent_width = vent_spacing_mm * 0.42;
            let vent_depth = depth_mm * 0.72;
            let start_x = -((vent_count as f32 - 1.0) * vent_spacing_mm) * 0.5;
            for index in 0..vent_count {
                let x = start_x + (index as f32 * vent_spacing_mm);
                let min = [
                    x - (vent_width * 0.5),
                    -vent_depth * 0.5,
                    height_mm * 0.72 + jitter,
                ];
                let max = [x + (vent_width * 0.5), vent_depth * 0.5, height_mm * 0.9];
                builder.add_box(min, max, 1, 4);
            }
            Ok(())
        }
        "fillet.placeholder.v1" => {
            let width_mm = node_param_f32(node, "width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.width_mm);
            let depth_mm = node_param_f32(node, "depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.depth_mm);
            let height_mm = node_param_f32(node, "height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.height_mm);
            let wall_mm = node_param_f32(node, "wall_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(profile.wall_mm);
            let z = height_mm + (wall_mm * 0.05) + jitter;
            let corners = [
                [-width_mm * 0.5, -depth_mm * 0.5, z],
                [width_mm * 0.5, -depth_mm * 0.5, z],
                [width_mm * 0.5, depth_mm * 0.5, z],
                [-width_mm * 0.5, depth_mm * 0.5, z],
            ];
            let mut vertex_indices = [0_u32; 4];
            for (index, corner) in corners.into_iter().enumerate() {
                vertex_indices[index] = builder.push_vertex(corner, [0.0, 0.0, 1.0], [0.0, 0.0], 1);
            }
            for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                builder.push_edge(vertex_indices[a], vertex_indices[b], 8);
            }
            Ok(())
        }
        "gripper.base_plate.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_width_mm);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_depth_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let hole_diameter_mm = node_param_f32(node, "servo_mount_hole_diameter_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.servo_mount_hole_diameter_mm);
            let hole_half_mm = (hole_diameter_mm * 0.5).max(0.6);
            let x_offset_mm = (base_width_mm * 0.22).max(8.0);
            builder.add_plate_with_dual_holes(
                base_width_mm,
                base_depth_mm,
                base_thickness_mm,
                x_offset_mm,
                hole_half_mm,
                0,
                1,
                0.0 + jitter * 0.2,
            );
            Ok(())
        }
        "gripper.finger.left.v1" | "gripper.finger.right.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let jaw_open_mm = node_param_f32(node, "jaw_open_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.jaw_open_mm);
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_length_mm);
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_thickness_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let finger_height_mm = node_param_f32(node, "finger_height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or((finger_thickness_mm * 2.4).max(10.0));
            let side_sign = if node.operation_key == "gripper.finger.left.v1" {
                1.0
            } else {
                -1.0
            };
            let finger_center_y = side_sign * ((jaw_open_mm * 0.5) + (finger_thickness_mm * 0.5));
            let min = [
                -finger_length_mm * 0.5,
                finger_center_y - finger_thickness_mm * 0.5,
                base_thickness_mm,
            ];
            let max = [
                finger_length_mm * 0.5,
                finger_center_y + finger_thickness_mm * 0.5,
                base_thickness_mm + finger_height_mm,
            ];
            builder.add_box(min, max, 1, 6);
            Ok(())
        }
        "gripper.servo_mount_holes.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_width_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let hole_diameter_mm = node_param_f32(node, "servo_mount_hole_diameter_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.servo_mount_hole_diameter_mm);
            let radius = (hole_diameter_mm * 0.5).max(0.6);
            let x_offset = (base_width_mm * 0.22).max(8.0);
            let top_z = base_thickness_mm + jitter * 0.1;
            let bottom_z = 0.0 + jitter * 0.1;
            for center_x in [-x_offset, x_offset] {
                builder.add_circle_edge_loop([center_x, 0.0, top_z], radius, 16, 4, 1);
                builder.add_circle_edge_loop([center_x, 0.0, bottom_z], radius, 16, 4, 1);
                let top_vertex = builder.push_vertex(
                    [center_x + radius, 0.0, top_z],
                    [1.0, 0.0, 0.0],
                    [0.0, 0.0],
                    1,
                );
                let bottom_vertex = builder.push_vertex(
                    [center_x + radius, 0.0, bottom_z],
                    [1.0, 0.0, 0.0],
                    [0.0, 0.0],
                    1,
                );
                builder.push_edge(bottom_vertex, top_vertex, 4);
            }
            Ok(())
        }
        "gripper.edge_marker.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_width_mm);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_depth_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let z = base_thickness_mm + jitter + 0.02;
            let corners = [
                [-base_width_mm * 0.5, -base_depth_mm * 0.5, z],
                [base_width_mm * 0.5, -base_depth_mm * 0.5, z],
                [base_width_mm * 0.5, base_depth_mm * 0.5, z],
                [-base_width_mm * 0.5, base_depth_mm * 0.5, z],
            ];
            let mut v = [0_u32; 4];
            for (index, corner) in corners.into_iter().enumerate() {
                v[index] = builder.push_vertex(corner, [0.0, 0.0, 1.0], [0.0, 0.0], 1);
            }
            for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                builder.push_edge(v[a], v[b], 8);
            }
            let chamfer_mm = (base_thickness_mm * 0.18).clamp(0.4, 2.8);
            let half_w = base_width_mm * 0.5;
            let half_d = base_depth_mm * 0.5;
            let z0 = (base_thickness_mm - chamfer_mm).max(0.0);
            let z1 = base_thickness_mm;
            let edge_band = chamfer_mm.max(0.6);
            builder.add_box(
                [-half_w, half_d - edge_band, z0],
                [half_w, half_d, z1],
                1,
                6,
            );
            builder.add_box(
                [-half_w, -half_d, z0],
                [half_w, -half_d + edge_band, z1],
                1,
                6,
            );
            builder.add_box(
                [-half_w, -half_d, z0],
                [-half_w + edge_band, half_d, z1],
                1,
                6,
            );
            builder.add_box(
                [half_w - edge_band, -half_d, z0],
                [half_w, half_d, z1],
                1,
                6,
            );
            Ok(())
        }
        "gripper.flexure.left.v1" | "gripper.flexure.right.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let jaw_open_mm = node_param_f32(node, "jaw_open_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.jaw_open_mm);
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_length_mm);
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_thickness_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let compliant_joint_count = node_param_u32(node, "compliant_joint_count")
                .map(|value| value.clamp(1, 8) as usize)
                .unwrap_or(2);
            let flexure_thickness_mm = node_param_f32(node, "flexure_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(1.2)
                .clamp(0.4, 4.0);
            let finger_height_mm = (finger_thickness_mm * 2.4).max(10.0);
            let side_sign = if node.operation_key == "gripper.flexure.left.v1" {
                1.0
            } else {
                -1.0
            };
            let finger_center_y = side_sign * ((jaw_open_mm * 0.5) + (finger_thickness_mm * 0.5));
            let inner_face_y = finger_center_y - (side_sign * (finger_thickness_mm * 0.5));
            let flexure_span_x = (finger_length_mm * 0.66).max(8.0);
            let start_x = -flexure_span_x * 0.5;
            let step_x = flexure_span_x / (compliant_joint_count as f32 + 1.0);
            let flexure_depth =
                (flexure_thickness_mm * 1.6).clamp(0.6, finger_thickness_mm.max(0.8));
            let flexure_center_y = inner_face_y
                - (side_sign * ((flexure_depth * 0.5) + (flexure_thickness_mm * 0.12)));
            let z_base = base_thickness_mm + (finger_height_mm * 0.28);
            let z_peak = z_base + (flexure_thickness_mm * 1.8).max(0.8);

            for index in 0..compliant_joint_count {
                let x_center = start_x + ((index + 1) as f32 * step_x);
                let wave = (((index as f32) * 0.35) + jitter).sin() * flexure_thickness_mm * 0.08;
                builder.add_box(
                    [
                        x_center - (flexure_thickness_mm * 0.65),
                        flexure_center_y - (flexure_depth * 0.5),
                        z_base + wave,
                    ],
                    [
                        x_center + (flexure_thickness_mm * 0.65),
                        flexure_center_y + (flexure_depth * 0.5),
                        z_peak + wave,
                    ],
                    1,
                    6,
                );
            }
            builder.add_box(
                [
                    start_x,
                    flexure_center_y - (flexure_thickness_mm * 0.22),
                    z_base + jitter * 0.05,
                ],
                [
                    -start_x,
                    flexure_center_y + (flexure_thickness_mm * 0.22),
                    z_base + flexure_thickness_mm,
                ],
                1,
                6,
            );
            Ok(())
        }
        "gripper.compliant_pads.v1" => {
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let jaw_open_mm = node_param_f32(node, "jaw_open_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.jaw_open_mm);
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_length_mm);
            let finger_height_mm = node_param_f32(node, "finger_height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or((gripper.finger_thickness_mm * 2.4).max(10.0));
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.finger_thickness_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let flexure_thickness_mm = node_param_f32(node, "flexure_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(1.2)
                .clamp(0.4, 4.0);
            let pad_depth = (flexure_thickness_mm * 1.8).clamp(0.8, finger_thickness_mm * 0.9);
            let pad_length = (finger_length_mm * 0.42).clamp(10.0, finger_length_mm * 0.8);
            let pad_height = (finger_height_mm * 0.30).clamp(2.0, finger_height_mm * 0.8);
            let pad_z0 = base_thickness_mm + (finger_height_mm * 0.24);
            let pad_z1 = pad_z0 + pad_height;
            let y_inset = flexure_thickness_mm * 0.15;
            for side_sign in [1.0_f32, -1.0_f32] {
                let finger_center_y =
                    side_sign * ((jaw_open_mm * 0.5) + (finger_thickness_mm * 0.5));
                let inner_face_y = finger_center_y - (side_sign * (finger_thickness_mm * 0.5));
                let pad_center_y = inner_face_y - (side_sign * ((pad_depth * 0.5) + y_inset));
                builder.add_box(
                    [
                        -pad_length * 0.5,
                        pad_center_y - (pad_depth * 0.5),
                        pad_z0 + jitter * 0.04,
                    ],
                    [pad_length * 0.5, pad_center_y + (pad_depth * 0.5), pad_z1],
                    1,
                    6,
                );
            }
            Ok(())
        }
        "gripper.single_drive_linkage.v1" => {
            if !node_param_bool(node, "single_servo_drive").unwrap_or(true) {
                return Ok(());
            }
            let gripper = GripperVariantProfile::from_variant(
                node.params
                    .get("variant")
                    .map(String::as_str)
                    .unwrap_or("variant.baseline"),
            );
            let jaw_open_mm = node_param_f32(node, "jaw_open_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.jaw_open_mm);
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_width_mm);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(gripper.base_thickness_mm);
            let rod_half_x = (base_width_mm * 0.24).max(8.0);
            let rod_half_y = (jaw_open_mm * 0.38).max(6.0);
            let rod_thickness = (base_thickness_mm * 0.22).clamp(1.4, 4.0);
            let z0 = base_thickness_mm + (rod_thickness * 0.8) + (jitter * 0.1);
            let z1 = z0 + rod_thickness;
            builder.add_box(
                [-rod_half_x, -rod_half_y, z0],
                [rod_half_x, rod_half_y, z1],
                1,
                6,
            );
            for side_sign in [-1.0_f32, 1.0_f32] {
                let x_center = side_sign * (rod_half_x * 0.72);
                builder.add_box(
                    [x_center - 1.2, -rod_half_y, z1],
                    [x_center + 1.2, rod_half_y, z1 + (rod_thickness * 1.5)],
                    1,
                    6,
                );
            }
            let hub_radius = (rod_thickness * 1.1).max(1.2);
            builder.add_cylinder_z(
                [0.0, 0.0, base_thickness_mm + rod_thickness],
                hub_radius,
                rod_thickness * 1.8,
                14,
                1,
                4,
            );
            Ok(())
        }
        "hand3.base_plate.v1" => {
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(90.0);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(58.0);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(8.0);
            let hole_diameter_mm = node_param_f32(node, "servo_mount_hole_diameter_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(2.9);
            let channel_diameter_mm = node_param_f32(node, "tendon_channel_diameter_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(1.8)
                .clamp(0.6, 4.0);
            let hole_half_mm = (hole_diameter_mm * 0.5).max(0.6);
            let x_offset_mm = (base_width_mm * 0.18).max(7.5);
            builder.add_plate_with_dual_holes(
                base_width_mm,
                base_depth_mm,
                base_thickness_mm,
                x_offset_mm,
                hole_half_mm,
                0,
                1,
                0.0 + jitter * 0.2,
            );
            let channel_manifold_half = (channel_diameter_mm * 1.6).max(1.0);
            builder.add_box(
                [
                    -(base_width_mm * 0.12),
                    -(base_depth_mm * 0.20),
                    base_thickness_mm * 0.4,
                ],
                [
                    base_width_mm * 0.12,
                    base_depth_mm * 0.10,
                    base_thickness_mm * 0.85 + channel_manifold_half * 0.2,
                ],
                1,
                6,
            );
            Ok(())
        }
        "hand3.finger.digit.v1" => {
            let digit_slot = node
                .params
                .get("digit_slot")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0) as f32;
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(68.0);
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(7.0);
            let finger_height_mm = node_param_f32(node, "finger_height_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or((finger_thickness_mm * 2.4).max(10.0));
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(58.0);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(8.0);
            let finger_spacing_mm = node_param_f32(node, "finger_spacing_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(12.0);
            let pose_preset = node
                .params
                .get("pose_preset")
                .map(String::as_str)
                .unwrap_or("open");
            let (reach_scale, spread_scale) = match pose_preset {
                "pinch" => (0.86, 0.78),
                "tripod" => (1.04, 1.08),
                _ => (0.94, 1.0),
            };
            let x_center = digit_slot * finger_spacing_mm * spread_scale;
            let y0 = base_depth_mm * 0.22;
            let mut y1 = y0 + (finger_length_mm * reach_scale);
            if digit_slot.abs() < 0.01 && pose_preset == "tripod" {
                y1 += finger_length_mm * 0.08;
            }
            let z0 = base_thickness_mm;
            let z1 = z0 + finger_height_mm;
            builder.add_box(
                [
                    x_center - (finger_thickness_mm * 0.5),
                    y0,
                    z0 + jitter * 0.05,
                ],
                [x_center + (finger_thickness_mm * 0.5), y1, z1],
                1,
                6,
            );
            let tip_length = (finger_thickness_mm * 1.4).clamp(2.0, 8.0);
            builder.add_box(
                [
                    x_center - (finger_thickness_mm * 0.36),
                    y1 - tip_length,
                    z1 - (finger_height_mm * 0.25),
                ],
                [
                    x_center + (finger_thickness_mm * 0.36),
                    y1 + (finger_thickness_mm * 0.12),
                    z1,
                ],
                1,
                6,
            );
            Ok(())
        }
        "hand3.thumb.opposable.v1" => {
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(68.0);
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(7.0);
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(90.0);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(58.0);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(8.0);
            let thumb_angle_deg = node_param_f32(node, "thumb_base_angle_deg")
                .filter(|value| *value > 0.0)
                .unwrap_or(42.0);
            let pose_preset = node
                .params
                .get("pose_preset")
                .map(String::as_str)
                .unwrap_or("open");
            let pose_angle_delta = match pose_preset {
                "pinch" => 8.0,
                "tripod" => -6.0,
                _ => 0.0,
            };
            let angle_rad = (thumb_angle_deg + pose_angle_delta).to_radians();
            let thumb_length = finger_length_mm * 0.78;
            let start = [-base_width_mm * 0.34, -base_depth_mm * 0.08];
            let end = [
                start[0] + (angle_rad.cos() * thumb_length),
                start[1] + (angle_rad.sin() * thumb_length),
            ];
            let thumb_thickness = finger_thickness_mm * 0.9;
            let thumb_height = (thumb_thickness * 2.2).max(9.0);
            for segment in 0..4 {
                let t0 = segment as f32 / 4.0;
                let t1 = (segment as f32 + 1.0) / 4.0;
                let p0 = [
                    start[0] + ((end[0] - start[0]) * t0),
                    start[1] + ((end[1] - start[1]) * t0),
                ];
                let p1 = [
                    start[0] + ((end[0] - start[0]) * t1),
                    start[1] + ((end[1] - start[1]) * t1),
                ];
                let z0 = base_thickness_mm + (segment as f32 * 0.24) + jitter * 0.04;
                let z1 = z0 + thumb_height;
                builder.add_box(
                    [
                        p0[0].min(p1[0]) - (thumb_thickness * 0.5),
                        p0[1].min(p1[1]) - (thumb_thickness * 0.5),
                        z0,
                    ],
                    [
                        p0[0].max(p1[0]) + (thumb_thickness * 0.5),
                        p0[1].max(p1[1]) + (thumb_thickness * 0.5),
                        z1,
                    ],
                    1,
                    6,
                );
            }
            Ok(())
        }
        "hand3.tendon.channel.v1" => {
            let digit = node
                .params
                .get("digit")
                .map(String::as_str)
                .unwrap_or("index");
            let channel_diameter_mm = node_param_f32(node, "channel_diameter_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(1.8)
                .clamp(0.6, 4.0);
            let channel_half = channel_diameter_mm * 0.5;
            let pose_preset = node
                .params
                .get("pose_preset")
                .map(String::as_str)
                .unwrap_or("open");
            if digit == "thumb" {
                let finger_length_mm = node_param_f32(node, "finger_length_mm")
                    .filter(|value| *value > 0.0)
                    .unwrap_or(68.0);
                let base_width_mm = node_param_f32(node, "base_width_mm")
                    .filter(|value| *value > 0.0)
                    .unwrap_or(90.0);
                let base_depth_mm = node_param_f32(node, "base_depth_mm")
                    .filter(|value| *value > 0.0)
                    .unwrap_or(58.0);
                let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                    .filter(|value| *value > 0.0)
                    .unwrap_or(8.0);
                let thumb_angle_deg = node_param_f32(node, "thumb_base_angle_deg")
                    .filter(|value| *value > 0.0)
                    .unwrap_or(42.0);
                let pose_angle_delta = match pose_preset {
                    "pinch" => 8.0,
                    "tripod" => -6.0,
                    _ => 0.0,
                };
                let angle_rad = (thumb_angle_deg + pose_angle_delta).to_radians();
                let thumb_length = finger_length_mm * 0.74;
                let start = [-base_width_mm * 0.34, -base_depth_mm * 0.08];
                for segment in 0..5 {
                    let t0 = segment as f32 / 5.0;
                    let t1 = (segment as f32 + 1.0) / 5.0;
                    let p0 = [
                        start[0] + (angle_rad.cos() * thumb_length * t0),
                        start[1] + (angle_rad.sin() * thumb_length * t0),
                    ];
                    let p1 = [
                        start[0] + (angle_rad.cos() * thumb_length * t1),
                        start[1] + (angle_rad.sin() * thumb_length * t1),
                    ];
                    let z0 = base_thickness_mm + (segment as f32 * 0.36) + jitter * 0.03;
                    builder.add_box(
                        [
                            p0[0].min(p1[0]) - channel_half,
                            p0[1].min(p1[1]) - channel_half,
                            z0,
                        ],
                        [
                            p0[0].max(p1[0]) + channel_half,
                            p0[1].max(p1[1]) + channel_half,
                            z0 + channel_diameter_mm,
                        ],
                        0,
                        4,
                    );
                }
                return Ok(());
            }

            let digit_slot = node
                .params
                .get("digit_slot")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0) as f32;
            let finger_length_mm = node_param_f32(node, "finger_length_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(68.0);
            let finger_thickness_mm = node_param_f32(node, "finger_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(7.0);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(58.0);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(8.0);
            let finger_spacing_mm = node_param_f32(node, "finger_spacing_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(12.0);
            let spread_scale = if pose_preset == "pinch" {
                0.78
            } else if pose_preset == "tripod" {
                1.08
            } else {
                1.0
            };
            let x_center = digit_slot * finger_spacing_mm * spread_scale;
            let y0 = base_depth_mm * 0.32;
            let y1 = y0 + (finger_length_mm * 0.72);
            let z0 = base_thickness_mm + (finger_thickness_mm * 1.1) + jitter * 0.02;
            builder.add_box(
                [x_center - channel_half, y0, z0],
                [x_center + channel_half, y1, z0 + channel_diameter_mm],
                0,
                4,
            );
            builder.add_circle_edge_loop([x_center, y0, z0], channel_half, 12, 4, 0);
            builder.add_circle_edge_loop([x_center, y1, z0], channel_half, 12, 4, 0);
            Ok(())
        }
        "hand3.edge_marker.v1" => {
            let base_width_mm = node_param_f32(node, "base_width_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(90.0);
            let base_depth_mm = node_param_f32(node, "base_depth_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(58.0);
            let base_thickness_mm = node_param_f32(node, "base_thickness_mm")
                .filter(|value| *value > 0.0)
                .unwrap_or(8.0);
            let z = base_thickness_mm + jitter + 0.02;
            let corners = [
                [-base_width_mm * 0.5, -base_depth_mm * 0.5, z],
                [base_width_mm * 0.5, -base_depth_mm * 0.5, z],
                [base_width_mm * 0.5, base_depth_mm * 0.5, z],
                [-base_width_mm * 0.5, base_depth_mm * 0.5, z],
            ];
            let mut v = [0_u32; 4];
            for (index, corner) in corners.into_iter().enumerate() {
                v[index] = builder.push_vertex(corner, [0.0, 0.0, 1.0], [0.0, 0.0], 1);
            }
            for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                builder.push_edge(v[a], v[b], 8);
            }
            Ok(())
        }
        other => Err(CadError::EvalFailed {
            reason: format!(
                "tessellation has no handler for operation_key={other} feature_id={}",
                node.id
            ),
        }),
    }
}

fn node_param_f32(node: &FeatureNode, key: &str) -> Option<f32> {
    node.params.get(key)?.parse::<f32>().ok()
}

fn node_param_u32(node: &FeatureNode, key: &str) -> Option<u32> {
    node.params.get(key)?.parse::<u32>().ok()
}

fn node_param_bool(node: &FeatureNode, key: &str) -> Option<bool> {
    let value = node.params.get(key)?.trim().to_ascii_lowercase();
    match value.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn material_slots() -> Vec<CadMeshMaterialSlot> {
    vec![
        CadMeshMaterialSlot {
            slot: 0,
            reserved: 0,
            base_color_rgba: [0.79, 0.81, 0.84, 1.0],
            roughness: 0.44,
            metallic: 0.78,
        },
        CadMeshMaterialSlot {
            slot: 1,
            reserved: 0,
            base_color_rgba: [0.23, 0.83, 0.67, 1.0],
            roughness: 0.32,
            metallic: 0.18,
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RackVariantProfile {
    width_mm: f32,
    depth_mm: f32,
    height_mm: f32,
    wall_mm: f32,
    mount_hole_radius_mm: f32,
    vent_count: usize,
    vent_spacing_mm: f32,
}

impl RackVariantProfile {
    fn from_variant(variant_id: &str) -> Self {
        match variant_id {
            "variant.lightweight" => Self {
                width_mm: 382.0,
                depth_mm: 228.0,
                height_mm: 86.0,
                wall_mm: 4.0,
                mount_hole_radius_mm: 4.6,
                vent_count: 10,
                vent_spacing_mm: 18.5,
            },
            "variant.low-cost" => Self {
                width_mm: 392.0,
                depth_mm: 220.0,
                height_mm: 82.0,
                wall_mm: 5.0,
                mount_hole_radius_mm: 4.0,
                vent_count: 7,
                vent_spacing_mm: 23.0,
            },
            "variant.stiffness" => Self {
                width_mm: 398.0,
                depth_mm: 232.0,
                height_mm: 94.0,
                wall_mm: 7.0,
                mount_hole_radius_mm: 4.8,
                vent_count: 12,
                vent_spacing_mm: 14.5,
            },
            _ => Self {
                width_mm: 390.0,
                depth_mm: 226.0,
                height_mm: 88.0,
                wall_mm: 6.0,
                mount_hole_radius_mm: 4.3,
                vent_count: 8,
                vent_spacing_mm: 20.0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GripperVariantProfile {
    jaw_open_mm: f32,
    finger_length_mm: f32,
    finger_thickness_mm: f32,
    base_width_mm: f32,
    base_depth_mm: f32,
    base_thickness_mm: f32,
    servo_mount_hole_diameter_mm: f32,
}

impl GripperVariantProfile {
    fn from_variant(variant_id: &str) -> Self {
        match variant_id {
            "variant.wide-jaw" => Self {
                jaw_open_mm: 64.0,
                finger_length_mm: 68.0,
                finger_thickness_mm: 8.0,
                base_width_mm: 94.0,
                base_depth_mm: 56.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
            },
            "variant.long-reach" => Self {
                jaw_open_mm: 42.0,
                finger_length_mm: 88.0,
                finger_thickness_mm: 8.0,
                base_width_mm: 80.0,
                base_depth_mm: 52.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
            },
            "variant.stiff-finger" => Self {
                jaw_open_mm: 40.0,
                finger_length_mm: 65.0,
                finger_thickness_mm: 10.0,
                base_width_mm: 82.0,
                base_depth_mm: 54.0,
                base_thickness_mm: 10.0,
                servo_mount_hole_diameter_mm: 3.2,
            },
            _ => Self {
                jaw_open_mm: 42.0,
                finger_length_mm: 65.0,
                finger_thickness_mm: 8.0,
                base_width_mm: 78.0,
                base_depth_mm: 52.0,
                base_thickness_mm: 8.0,
                servo_mount_hole_diameter_mm: 2.9,
            },
        }
    }
}

#[derive(Default)]
struct TessellationBuilder {
    vertices: Vec<CadMeshVertex>,
    triangle_indices: Vec<u32>,
    edges: Vec<CadMeshEdgeSegment>,
}

impl TessellationBuilder {
    fn push_vertex(
        &mut self,
        position_mm: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        material_slot: u16,
    ) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CadMeshVertex {
            position_mm,
            normal,
            uv,
            material_slot,
            flags: 0,
        });
        index
    }

    fn push_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.triangle_indices.extend([a, b, c]);
    }

    fn push_edge(&mut self, start_vertex: u32, end_vertex: u32, flags: u32) {
        self.edges.push(CadMeshEdgeSegment {
            start_vertex,
            end_vertex,
            flags,
        });
    }

    fn add_box(&mut self, min: [f32; 3], max: [f32; 3], material_slot: u16, edge_flags: u32) {
        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let corners = [
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [min[0], max[1], min[2]],
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ];
        let mut v = [0_u32; 8];
        for (index, corner) in corners.into_iter().enumerate() {
            v[index] = self.push_vertex(
                corner,
                normalized([
                    corner[0] - center[0],
                    corner[1] - center[1],
                    corner[2] - center[2],
                ]),
                [0.0, 0.0],
                material_slot,
            );
        }
        for (a, b, c) in [
            (0, 2, 1),
            (0, 3, 2),
            (4, 5, 6),
            (4, 6, 7),
            (0, 1, 5),
            (0, 5, 4),
            (1, 2, 6),
            (1, 6, 5),
            (2, 3, 7),
            (2, 7, 6),
            (3, 0, 4),
            (3, 4, 7),
        ] {
            self.push_triangle(v[a], v[b], v[c]);
        }
        for (a, b) in [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            self.push_edge(v[a], v[b], edge_flags);
        }
    }

    fn add_cylinder_z(
        &mut self,
        center: [f32; 3],
        radius_mm: f32,
        height_mm: f32,
        segments: usize,
        material_slot: u16,
        edge_flags: u32,
    ) {
        let z0 = center[2] - (height_mm * 0.5);
        let z1 = center[2] + (height_mm * 0.5);
        let mut bottom = Vec::<u32>::with_capacity(segments);
        let mut top = Vec::<u32>::with_capacity(segments);
        for segment in 0..segments {
            let t = (segment as f32 / segments as f32) * std::f32::consts::TAU;
            let (sin, cos) = t.sin_cos();
            let x = center[0] + (cos * radius_mm);
            let y = center[1] + (sin * radius_mm);
            bottom.push(self.push_vertex([x, y, z0], [cos, sin, 0.0], [0.0, 0.0], material_slot));
            top.push(self.push_vertex([x, y, z1], [cos, sin, 0.0], [0.0, 1.0], material_slot));
        }

        let top_center = self.push_vertex(center, [0.0, 0.0, 1.0], [0.5, 0.5], material_slot);
        let bottom_center = self.push_vertex(
            [center[0], center[1], z0],
            [0.0, 0.0, -1.0],
            [0.5, 0.5],
            material_slot,
        );
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let b0 = bottom[segment];
            let b1 = bottom[next];
            let t0 = top[segment];
            let t1 = top[next];
            self.push_triangle(b0, t1, t0);
            self.push_triangle(b0, b1, t1);
            self.push_triangle(top_center, t0, t1);
            self.push_triangle(bottom_center, b1, b0);
            self.push_edge(b0, b1, edge_flags);
            self.push_edge(t0, t1, edge_flags);
        }
    }

    fn add_circle_edge_loop(
        &mut self,
        center: [f32; 3],
        radius_mm: f32,
        segments: usize,
        edge_flags: u32,
        material_slot: u16,
    ) {
        if segments < 3 || radius_mm <= f32::EPSILON {
            return;
        }
        let mut indices = Vec::<u32>::with_capacity(segments);
        for segment in 0..segments {
            let t = (segment as f32 / segments as f32) * std::f32::consts::TAU;
            let (sin, cos) = t.sin_cos();
            indices.push(self.push_vertex(
                [
                    center[0] + (cos * radius_mm),
                    center[1] + (sin * radius_mm),
                    center[2],
                ],
                [0.0, 0.0, 1.0],
                [0.0, 0.0],
                material_slot,
            ));
        }
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            self.push_edge(indices[segment], indices[next], edge_flags);
        }
    }

    fn add_plate_with_dual_holes(
        &mut self,
        width_mm: f32,
        depth_mm: f32,
        thickness_mm: f32,
        hole_offset_x_mm: f32,
        hole_half_mm: f32,
        material_slot: u16,
        edge_flags: u32,
        z_base: f32,
    ) {
        let half_w = width_mm * 0.5;
        let half_d = depth_mm * 0.5;
        let c0 = -hole_offset_x_mm;
        let c1 = hole_offset_x_mm;
        let hole_margin = 0.8;
        let hole_half = hole_half_mm.max(0.6);
        let z0 = z_base;
        let z1 = z_base + thickness_mm;

        // Fallback when holes cannot be represented without degeneracy.
        if c0 - hole_half <= -half_w + hole_margin
            || c1 + hole_half >= half_w - hole_margin
            || hole_half >= half_d - hole_margin
        {
            self.add_box(
                [-half_w, -half_d, z0],
                [half_w, half_d, z1],
                material_slot,
                edge_flags,
            );
            return;
        }

        // Construct the plate by union of rectangular slabs around two through-holes.
        let mut add_slab = |min_x: f32, min_y: f32, max_x: f32, max_y: f32| {
            if max_x - min_x <= 0.05 || max_y - min_y <= 0.05 {
                return;
            }
            self.add_box(
                [min_x, min_y, z0],
                [max_x, max_y, z1],
                material_slot,
                edge_flags,
            );
        };

        add_slab(-half_w, -half_d, c0 - hole_half, half_d);
        add_slab(c0 + hole_half, -half_d, c1 - hole_half, half_d);
        add_slab(c1 + hole_half, -half_d, half_w, half_d);

        add_slab(c0 - hole_half, hole_half, c0 + hole_half, half_d);
        add_slab(c0 - hole_half, -half_d, c0 + hole_half, -hole_half);
        add_slab(c1 - hole_half, hole_half, c1 + hole_half, half_d);
        add_slab(c1 - hole_half, -half_d, c1 + hole_half, -hole_half);
    }

    fn bounds(&self) -> Option<CadMeshBounds> {
        let mut iter = self.vertices.iter();
        let first = iter.next()?;
        let mut min_mm = first.position_mm;
        let mut max_mm = first.position_mm;
        for vertex in iter {
            for axis in 0..3 {
                min_mm[axis] = min_mm[axis].min(vertex.position_mm[axis]);
                max_mm[axis] = max_mm[axis].max(vertex.position_mm[axis]);
            }
        }
        Some(CadMeshBounds { min_mm, max_mm })
    }
}

fn hash_jitter(geometry_hash: &str) -> f32 {
    let seed = u64::from_str_radix(geometry_hash, 16).unwrap_or(0);
    (seed % 19) as f32 * 0.01
}

fn normalized(v: [f32; 3]) -> [f32; 3] {
    let len_sq = (v[0] * v[0]) + (v[1] * v[1]) + (v[2] * v[2]);
    if len_sq <= f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    let inv = len_sq.sqrt().recip();
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

fn sanitize_mesh_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::tessellate_rebuild_result;
    use crate::eval::evaluate_feature_graph_deterministic;
    use crate::feature_graph::{FeatureGraph, FeatureNode};
    use std::collections::BTreeMap;

    fn gripper_graph_for_variant(variant_id: &str) -> FeatureGraph {
        gripper_graph_for_variant_with_underactuation(variant_id, false)
    }

    fn gripper_graph_for_variant_with_underactuation(
        variant_id: &str,
        underactuated: bool,
    ) -> FeatureGraph {
        let mut jaw_open_mm: f64 = 42.0;
        let mut finger_length_mm: f64 = 65.0;
        let mut finger_thickness_mm: f64 = 8.0;
        match variant_id {
            "variant.wide-jaw" => {
                jaw_open_mm += 22.0;
            }
            "variant.long-reach" => {
                finger_length_mm += 23.0;
            }
            "variant.stiff-finger" => {
                jaw_open_mm -= 2.0;
                finger_thickness_mm += 2.0;
            }
            _ => {}
        }

        let mut base_params = BTreeMap::new();
        base_params.insert("variant".to_string(), variant_id.to_string());
        base_params.insert("base_width_mm".to_string(), "78.0".to_string());
        base_params.insert("base_depth_mm".to_string(), "52.0".to_string());
        base_params.insert("base_thickness_mm".to_string(), "8.0".to_string());
        base_params.insert(
            "servo_mount_hole_diameter_mm".to_string(),
            "2.9".to_string(),
        );
        base_params.insert("print_fit_mm".to_string(), "0.15".to_string());
        base_params.insert("print_clearance_mm".to_string(), "0.35".to_string());

        let mut left_finger_params = BTreeMap::new();
        left_finger_params.insert("variant".to_string(), variant_id.to_string());
        left_finger_params.insert("jaw_open_mm".to_string(), format!("{jaw_open_mm:.4}"));
        left_finger_params.insert(
            "finger_length_mm".to_string(),
            format!("{finger_length_mm:.4}"),
        );
        left_finger_params.insert(
            "finger_thickness_mm".to_string(),
            format!("{finger_thickness_mm:.4}"),
        );
        left_finger_params.insert("base_thickness_mm".to_string(), "8.0".to_string());
        left_finger_params.insert(
            "finger_height_mm".to_string(),
            format!("{:.4}", (finger_thickness_mm * 2.4).max(10.0)),
        );

        let right_finger_params = left_finger_params.clone();

        let mut hole_params = BTreeMap::new();
        hole_params.insert("variant".to_string(), variant_id.to_string());
        hole_params.insert("base_width_mm".to_string(), "78.0".to_string());
        hole_params.insert("base_thickness_mm".to_string(), "8.0".to_string());
        hole_params.insert(
            "servo_mount_hole_diameter_mm".to_string(),
            "2.9".to_string(),
        );

        let mut edge_params = BTreeMap::new();
        edge_params.insert("variant".to_string(), variant_id.to_string());
        edge_params.insert("base_width_mm".to_string(), "78.0".to_string());
        edge_params.insert("base_depth_mm".to_string(), "52.0".to_string());
        edge_params.insert("base_thickness_mm".to_string(), "8.0".to_string());

        let mut nodes = vec![
            FeatureNode {
                id: "feature.gripper.base".to_string(),
                name: "gripper_base".to_string(),
                operation_key: "gripper.base_plate.v1".to_string(),
                depends_on: Vec::new(),
                params: base_params,
            },
            FeatureNode {
                id: "feature.gripper.finger.left".to_string(),
                name: "gripper_finger_left".to_string(),
                operation_key: "gripper.finger.left.v1".to_string(),
                depends_on: vec!["feature.gripper.base".to_string()],
                params: left_finger_params.clone(),
            },
            FeatureNode {
                id: "feature.gripper.finger.right".to_string(),
                name: "gripper_finger_right".to_string(),
                operation_key: "gripper.finger.right.v1".to_string(),
                depends_on: vec!["feature.gripper.base".to_string()],
                params: right_finger_params.clone(),
            },
            FeatureNode {
                id: "feature.gripper.servo_mount_holes".to_string(),
                name: "gripper_servo_mount_holes".to_string(),
                operation_key: "gripper.servo_mount_holes.v1".to_string(),
                depends_on: vec!["feature.gripper.base".to_string()],
                params: hole_params,
            },
            FeatureNode {
                id: "feature.gripper.edge_marker".to_string(),
                name: "gripper_edge_marker".to_string(),
                operation_key: "gripper.edge_marker.v1".to_string(),
                depends_on: vec!["feature.gripper.base".to_string()],
                params: edge_params,
            },
        ];

        if underactuated {
            let mut flexure_left = BTreeMap::new();
            flexure_left.insert("variant".to_string(), variant_id.to_string());
            flexure_left.insert("jaw_open_mm".to_string(), format!("{jaw_open_mm:.4}"));
            flexure_left.insert(
                "finger_length_mm".to_string(),
                format!("{finger_length_mm:.4}"),
            );
            flexure_left.insert(
                "finger_thickness_mm".to_string(),
                format!("{finger_thickness_mm:.4}"),
            );
            flexure_left.insert("base_thickness_mm".to_string(), "8.0".to_string());
            flexure_left.insert("compliant_joint_count".to_string(), "3".to_string());
            flexure_left.insert("flexure_thickness_mm".to_string(), "1.2".to_string());
            let flexure_right = flexure_left.clone();

            let mut compliant_pads = BTreeMap::new();
            compliant_pads.insert("variant".to_string(), variant_id.to_string());
            compliant_pads.insert("jaw_open_mm".to_string(), format!("{jaw_open_mm:.4}"));
            compliant_pads.insert(
                "finger_length_mm".to_string(),
                format!("{finger_length_mm:.4}"),
            );
            compliant_pads.insert(
                "finger_height_mm".to_string(),
                format!("{:.4}", (finger_thickness_mm * 2.4).max(10.0)),
            );
            compliant_pads.insert(
                "finger_thickness_mm".to_string(),
                format!("{finger_thickness_mm:.4}"),
            );
            compliant_pads.insert("base_thickness_mm".to_string(), "8.0".to_string());
            compliant_pads.insert("flexure_thickness_mm".to_string(), "1.2".to_string());

            let mut linkage = BTreeMap::new();
            linkage.insert("variant".to_string(), variant_id.to_string());
            linkage.insert("single_servo_drive".to_string(), "1".to_string());
            linkage.insert("jaw_open_mm".to_string(), format!("{jaw_open_mm:.4}"));
            linkage.insert("base_width_mm".to_string(), "78.0".to_string());
            linkage.insert("base_thickness_mm".to_string(), "8.0".to_string());

            nodes.extend([
                FeatureNode {
                    id: "feature.gripper.flexure.left".to_string(),
                    name: "gripper_flexure_left".to_string(),
                    operation_key: "gripper.flexure.left.v1".to_string(),
                    depends_on: vec!["feature.gripper.finger.left".to_string()],
                    params: flexure_left,
                },
                FeatureNode {
                    id: "feature.gripper.flexure.right".to_string(),
                    name: "gripper_flexure_right".to_string(),
                    operation_key: "gripper.flexure.right.v1".to_string(),
                    depends_on: vec!["feature.gripper.finger.right".to_string()],
                    params: flexure_right,
                },
                FeatureNode {
                    id: "feature.gripper.compliant_pads".to_string(),
                    name: "gripper_compliant_pads".to_string(),
                    operation_key: "gripper.compliant_pads.v1".to_string(),
                    depends_on: vec![
                        "feature.gripper.finger.left".to_string(),
                        "feature.gripper.finger.right".to_string(),
                    ],
                    params: compliant_pads,
                },
                FeatureNode {
                    id: "feature.gripper.single_drive_linkage".to_string(),
                    name: "gripper_single_drive_linkage".to_string(),
                    operation_key: "gripper.single_drive_linkage.v1".to_string(),
                    depends_on: vec![
                        "feature.gripper.base".to_string(),
                        "feature.gripper.flexure.left".to_string(),
                        "feature.gripper.flexure.right".to_string(),
                    ],
                    params: linkage,
                },
            ]);
        }

        FeatureGraph { nodes }
    }

    fn three_finger_thumb_graph(
        variant_id: &str,
        tendon_channel_diameter_mm: f64,
        pose_preset: &str,
    ) -> FeatureGraph {
        let mut nodes = vec![
            FeatureNode {
                id: "feature.hand3.base".to_string(),
                name: "hand3_base".to_string(),
                operation_key: "hand3.base_plate.v1".to_string(),
                depends_on: Vec::new(),
                params: BTreeMap::from([
                    ("variant".to_string(), variant_id.to_string()),
                    ("base_width_mm".to_string(), "90.0".to_string()),
                    ("base_depth_mm".to_string(), "58.0".to_string()),
                    ("base_thickness_mm".to_string(), "8.0".to_string()),
                    (
                        "servo_mount_hole_diameter_mm".to_string(),
                        "2.9".to_string(),
                    ),
                    (
                        "tendon_channel_diameter_mm".to_string(),
                        format!("{tendon_channel_diameter_mm:.3}"),
                    ),
                ]),
            },
            FeatureNode {
                id: "feature.hand3.thumb".to_string(),
                name: "hand3_thumb".to_string(),
                operation_key: "hand3.thumb.opposable.v1".to_string(),
                depends_on: vec!["feature.hand3.base".to_string()],
                params: BTreeMap::from([
                    ("variant".to_string(), variant_id.to_string()),
                    ("finger_length_mm".to_string(), "68.0".to_string()),
                    ("finger_thickness_mm".to_string(), "7.0".to_string()),
                    ("base_width_mm".to_string(), "90.0".to_string()),
                    ("base_depth_mm".to_string(), "58.0".to_string()),
                    ("base_thickness_mm".to_string(), "8.0".to_string()),
                    ("thumb_base_angle_deg".to_string(), "48.0".to_string()),
                    ("pose_preset".to_string(), pose_preset.to_string()),
                ]),
            },
        ];
        for (digit_slot, digit_name) in [(-1_i32, "index"), (0_i32, "middle"), (1_i32, "ring")] {
            nodes.push(FeatureNode {
                id: format!("feature.hand3.finger.{digit_name}"),
                name: format!("hand3_finger_{digit_name}"),
                operation_key: "hand3.finger.digit.v1".to_string(),
                depends_on: vec!["feature.hand3.base".to_string()],
                params: BTreeMap::from([
                    ("variant".to_string(), variant_id.to_string()),
                    ("digit_slot".to_string(), digit_slot.to_string()),
                    ("finger_length_mm".to_string(), "68.0".to_string()),
                    ("finger_thickness_mm".to_string(), "7.0".to_string()),
                    ("finger_height_mm".to_string(), "16.8".to_string()),
                    ("base_depth_mm".to_string(), "58.0".to_string()),
                    ("base_thickness_mm".to_string(), "8.0".to_string()),
                    ("finger_spacing_mm".to_string(), "12.0".to_string()),
                    ("jaw_open_mm".to_string(), "34.0".to_string()),
                    ("pose_preset".to_string(), pose_preset.to_string()),
                ]),
            });
            nodes.push(FeatureNode {
                id: format!("feature.hand3.tendon.{digit_name}"),
                name: format!("hand3_tendon_{digit_name}"),
                operation_key: "hand3.tendon.channel.v1".to_string(),
                depends_on: vec![format!("feature.hand3.finger.{digit_name}")],
                params: BTreeMap::from([
                    ("variant".to_string(), variant_id.to_string()),
                    ("digit".to_string(), digit_name.to_string()),
                    ("digit_slot".to_string(), digit_slot.to_string()),
                    ("finger_length_mm".to_string(), "68.0".to_string()),
                    ("finger_thickness_mm".to_string(), "7.0".to_string()),
                    ("base_thickness_mm".to_string(), "8.0".to_string()),
                    ("base_depth_mm".to_string(), "58.0".to_string()),
                    ("finger_spacing_mm".to_string(), "12.0".to_string()),
                    (
                        "channel_diameter_mm".to_string(),
                        format!("{tendon_channel_diameter_mm:.3}"),
                    ),
                    ("pose_preset".to_string(), pose_preset.to_string()),
                ]),
            });
        }
        nodes.push(FeatureNode {
            id: "feature.hand3.tendon.thumb".to_string(),
            name: "hand3_tendon_thumb".to_string(),
            operation_key: "hand3.tendon.channel.v1".to_string(),
            depends_on: vec!["feature.hand3.thumb".to_string()],
            params: BTreeMap::from([
                ("variant".to_string(), variant_id.to_string()),
                ("digit".to_string(), "thumb".to_string()),
                ("finger_length_mm".to_string(), "68.0".to_string()),
                ("finger_thickness_mm".to_string(), "7.0".to_string()),
                ("base_thickness_mm".to_string(), "8.0".to_string()),
                ("base_depth_mm".to_string(), "58.0".to_string()),
                ("base_width_mm".to_string(), "90.0".to_string()),
                ("thumb_base_angle_deg".to_string(), "48.0".to_string()),
                (
                    "channel_diameter_mm".to_string(),
                    format!("{tendon_channel_diameter_mm:.3}"),
                ),
                ("pose_preset".to_string(), pose_preset.to_string()),
            ]),
        });
        nodes.push(FeatureNode {
            id: "feature.hand3.edge_marker".to_string(),
            name: "hand3_edge_marker".to_string(),
            operation_key: "hand3.edge_marker.v1".to_string(),
            depends_on: vec!["feature.hand3.base".to_string()],
            params: BTreeMap::from([
                ("variant".to_string(), variant_id.to_string()),
                ("base_width_mm".to_string(), "90.0".to_string()),
                ("base_depth_mm".to_string(), "58.0".to_string()),
                ("base_thickness_mm".to_string(), "8.0".to_string()),
            ]),
        });
        FeatureGraph { nodes }
    }

    #[test]
    fn gripper_tessellation_is_deterministic_for_identical_inputs() {
        let graph = gripper_graph_for_variant("variant.baseline");
        let rebuild = evaluate_feature_graph_deterministic(&graph)
            .expect("deterministic rebuild should succeed");

        let (mesh_a, receipt_a) =
            tessellate_rebuild_result(&graph, &rebuild, 7, "variant.baseline")
                .expect("first tessellation should succeed");
        let (mesh_b, receipt_b) =
            tessellate_rebuild_result(&graph, &rebuild, 7, "variant.baseline")
                .expect("second tessellation should succeed");

        assert_eq!(receipt_a.mesh_hash, receipt_b.mesh_hash);
        assert_eq!(receipt_a.rebuild_hash, receipt_b.rebuild_hash);
        assert_eq!(mesh_a.vertices.len(), mesh_b.vertices.len());
        assert_eq!(mesh_a.triangle_indices, mesh_b.triangle_indices);
        assert_eq!(mesh_a.edges, mesh_b.edges);
        assert!(receipt_a.vertex_count > 0);
        assert!(receipt_a.triangle_count > 0);
        assert!(receipt_a.edge_count > 0);
    }

    #[test]
    fn gripper_variant_profiles_change_bounds_and_mesh_hash() {
        let baseline_graph = gripper_graph_for_variant("variant.baseline");
        let long_reach_graph = gripper_graph_for_variant("variant.long-reach");
        let baseline_rebuild = evaluate_feature_graph_deterministic(&baseline_graph)
            .expect("baseline rebuild should succeed");
        let long_reach_rebuild = evaluate_feature_graph_deterministic(&long_reach_graph)
            .expect("long-reach rebuild should succeed");

        let (baseline_mesh, baseline_receipt) =
            tessellate_rebuild_result(&baseline_graph, &baseline_rebuild, 11, "variant.baseline")
                .expect("baseline tessellation should succeed");
        let (long_reach_mesh, long_reach_receipt) = tessellate_rebuild_result(
            &long_reach_graph,
            &long_reach_rebuild,
            11,
            "variant.long-reach",
        )
        .expect("long-reach tessellation should succeed");

        assert_ne!(baseline_receipt.mesh_hash, long_reach_receipt.mesh_hash);
        let baseline_span_x = baseline_mesh.bounds.max_mm[0] - baseline_mesh.bounds.min_mm[0];
        let long_reach_span_x = long_reach_mesh.bounds.max_mm[0] - long_reach_mesh.bounds.min_mm[0];
        assert!(
            long_reach_span_x > baseline_span_x,
            "long-reach variant should increase total X envelope span"
        );
    }

    #[test]
    fn gripper_base_hole_diameter_changes_mesh_hash() {
        let mut tight_graph = gripper_graph_for_variant("variant.baseline");
        let mut wide_graph = gripper_graph_for_variant("variant.baseline");
        let base_tight = tight_graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "feature.gripper.base")
            .expect("base node present");
        base_tight.params.insert(
            "servo_mount_hole_diameter_mm".to_string(),
            "2.6".to_string(),
        );
        let base_wide = wide_graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "feature.gripper.base")
            .expect("base node present");
        base_wide.params.insert(
            "servo_mount_hole_diameter_mm".to_string(),
            "4.0".to_string(),
        );

        let tight_rebuild = evaluate_feature_graph_deterministic(&tight_graph)
            .expect("tight rebuild should succeed");
        let wide_rebuild =
            evaluate_feature_graph_deterministic(&wide_graph).expect("wide rebuild should succeed");
        let (_, tight_receipt) =
            tessellate_rebuild_result(&tight_graph, &tight_rebuild, 21, "variant.baseline")
                .expect("tight tessellation should succeed");
        let (_, wide_receipt) =
            tessellate_rebuild_result(&wide_graph, &wide_rebuild, 21, "variant.baseline")
                .expect("wide tessellation should succeed");
        assert_ne!(tight_receipt.mesh_hash, wide_receipt.mesh_hash);
    }

    #[test]
    fn underactuated_gripper_ops_change_mesh_hash() {
        let baseline_graph =
            gripper_graph_for_variant_with_underactuation("variant.baseline", false);
        let underactuated_graph =
            gripper_graph_for_variant_with_underactuation("variant.baseline", true);
        let baseline_rebuild = evaluate_feature_graph_deterministic(&baseline_graph)
            .expect("baseline rebuild should succeed");
        let underactuated_rebuild = evaluate_feature_graph_deterministic(&underactuated_graph)
            .expect("underactuated rebuild should succeed");
        let (_, baseline_receipt) =
            tessellate_rebuild_result(&baseline_graph, &baseline_rebuild, 31, "variant.baseline")
                .expect("baseline tessellation should succeed");
        let (underactuated_mesh, underactuated_receipt) = tessellate_rebuild_result(
            &underactuated_graph,
            &underactuated_rebuild,
            31,
            "variant.baseline",
        )
        .expect("underactuated tessellation should succeed");

        assert_ne!(baseline_receipt.mesh_hash, underactuated_receipt.mesh_hash);
        assert!(
            underactuated_mesh.vertices.len() > 0,
            "underactuated mesh should produce vertices"
        );
        assert!(
            underactuated_mesh.edges.len() > 0,
            "underactuated mesh should produce edge segments"
        );
    }

    #[test]
    fn underactuated_gripper_tessellation_is_deterministic() {
        let graph = gripper_graph_for_variant_with_underactuation("variant.baseline", true);
        let rebuild = evaluate_feature_graph_deterministic(&graph)
            .expect("underactuated rebuild should succeed");
        let (mesh_a, receipt_a) =
            tessellate_rebuild_result(&graph, &rebuild, 33, "variant.baseline")
                .expect("underactuated tessellation A should succeed");
        let (mesh_b, receipt_b) =
            tessellate_rebuild_result(&graph, &rebuild, 33, "variant.baseline")
                .expect("underactuated tessellation B should succeed");

        assert_eq!(receipt_a.mesh_hash, receipt_b.mesh_hash);
        assert_eq!(mesh_a.triangle_indices, mesh_b.triangle_indices);
        assert_eq!(mesh_a.edges, mesh_b.edges);
    }

    #[test]
    fn three_finger_thumb_tessellation_is_deterministic_for_identical_inputs() {
        let graph = three_finger_thumb_graph("variant.baseline", 1.6, "tripod");
        let rebuild =
            evaluate_feature_graph_deterministic(&graph).expect("three-finger rebuild should pass");
        let (mesh_a, receipt_a) =
            tessellate_rebuild_result(&graph, &rebuild, 41, "variant.baseline")
                .expect("three-finger tessellation A should succeed");
        let (mesh_b, receipt_b) =
            tessellate_rebuild_result(&graph, &rebuild, 41, "variant.baseline")
                .expect("three-finger tessellation B should succeed");
        assert_eq!(receipt_a.mesh_hash, receipt_b.mesh_hash);
        assert_eq!(mesh_a.triangle_indices, mesh_b.triangle_indices);
        assert_eq!(mesh_a.edges, mesh_b.edges);
        assert!(mesh_a.vertices.len() > 0);
    }

    #[test]
    fn three_finger_thumb_tendon_channel_diameter_changes_mesh_hash() {
        let narrow_graph = three_finger_thumb_graph("variant.baseline", 1.0, "pinch");
        let wide_graph = three_finger_thumb_graph("variant.baseline", 2.4, "pinch");
        let narrow_rebuild = evaluate_feature_graph_deterministic(&narrow_graph)
            .expect("narrow tendon rebuild should pass");
        let wide_rebuild = evaluate_feature_graph_deterministic(&wide_graph)
            .expect("wide tendon rebuild should pass");
        let (_, narrow_receipt) =
            tessellate_rebuild_result(&narrow_graph, &narrow_rebuild, 43, "variant.baseline")
                .expect("narrow tendon tessellation should succeed");
        let (_, wide_receipt) =
            tessellate_rebuild_result(&wide_graph, &wide_rebuild, 43, "variant.baseline")
                .expect("wide tendon tessellation should succeed");
        assert_ne!(narrow_receipt.mesh_hash, wide_receipt.mesh_hash);
    }
}
