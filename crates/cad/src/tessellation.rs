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
