use serde::{Deserialize, Serialize};

use crate::mesh::{CadMeshBounds, CadMeshPayload};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadSectionAxis {
    X,
    Y,
    Z,
}

impl CadSectionAxis {
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadSectionPlane {
    pub axis: CadSectionAxis,
    pub offset_normalized: f32,
}

impl CadSectionPlane {
    pub fn new(axis: CadSectionAxis, offset_normalized: f32) -> Self {
        Self {
            axis,
            offset_normalized,
        }
    }

    fn clamped_offset(self) -> f32 {
        self.offset_normalized.clamp(-1.0, 1.0)
    }

    fn axis_plane_coordinate(self, bounds: CadMeshBounds) -> f32 {
        let (min, max) = match self.axis {
            CadSectionAxis::X => (bounds.min_mm[0], bounds.max_mm[0]),
            CadSectionAxis::Y => (bounds.min_mm[1], bounds.max_mm[1]),
            CadSectionAxis::Z => (bounds.min_mm[2], bounds.max_mm[2]),
        };
        let min = min.min(max);
        let max = max.max(min);
        min + (max - min) * ((self.clamped_offset() + 1.0) * 0.5)
    }
}

pub fn clip_mesh_payload(
    payload: &CadMeshPayload,
    plane: CadSectionPlane,
    tolerance_mm: f32,
) -> Result<CadMeshPayload, String> {
    if payload.vertices.is_empty() {
        return Err("section clipping requires non-empty mesh vertices".to_string());
    }
    if payload.triangle_indices.is_empty() {
        return Err("section clipping requires non-empty mesh triangles".to_string());
    }
    if !plane.offset_normalized.is_finite() {
        return Err("section clipping requires finite plane offset".to_string());
    }
    if !tolerance_mm.is_finite() || tolerance_mm < 0.0 {
        return Err("section clipping requires finite, non-negative tolerance".to_string());
    }

    let plane_coordinate = plane.axis_plane_coordinate(payload.bounds);
    let mut index_map = vec![None; payload.vertices.len()];
    let mut clipped_vertices = Vec::new();
    for (index, vertex) in payload.vertices.iter().enumerate() {
        let axis_value = axis_coordinate(vertex.position_mm, plane.axis);
        if axis_value + tolerance_mm >= plane_coordinate {
            let remapped = clipped_vertices.len() as u32;
            index_map[index] = Some(remapped);
            clipped_vertices.push(*vertex);
        }
    }
    if clipped_vertices.is_empty() {
        return Err("section clipping removed all vertices".to_string());
    }

    let mut clipped_indices = Vec::new();
    for triangle in payload.triangle_indices.chunks_exact(3) {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;
        let Some(n0) = index_map[i0] else {
            continue;
        };
        let Some(n1) = index_map[i1] else {
            continue;
        };
        let Some(n2) = index_map[i2] else {
            continue;
        };
        clipped_indices.extend_from_slice(&[n0, n1, n2]);
    }
    if clipped_indices.is_empty() {
        return Err("section clipping removed all triangles".to_string());
    }

    let clipped_edges = payload
        .edges
        .iter()
        .filter_map(|edge| {
            let start = index_map
                .get(edge.start_vertex as usize)
                .copied()
                .flatten()?;
            let end = index_map.get(edge.end_vertex as usize).copied().flatten()?;
            (start != end).then_some(crate::mesh::CadMeshEdgeSegment {
                start_vertex: start,
                end_vertex: end,
                flags: edge.flags,
            })
        })
        .collect::<Vec<_>>();

    let bounds = bounds_from_vertices(&clipped_vertices)
        .ok_or_else(|| "section clipping could not produce bounded vertices".to_string())?;

    Ok(CadMeshPayload {
        mesh_id: format!(
            "{}#section:{}:{:.3}",
            payload.mesh_id,
            plane.axis.label(),
            plane.clamped_offset()
        ),
        document_revision: payload.document_revision,
        variant_id: payload.variant_id.clone(),
        topology: payload.topology,
        vertices: clipped_vertices,
        triangle_indices: clipped_indices,
        edges: clipped_edges,
        material_slots: payload.material_slots.clone(),
        bounds,
    })
}

fn axis_coordinate(position_mm: [f32; 3], axis: CadSectionAxis) -> f32 {
    match axis {
        CadSectionAxis::X => position_mm[0],
        CadSectionAxis::Y => position_mm[1],
        CadSectionAxis::Z => position_mm[2],
    }
}

fn bounds_from_vertices(vertices: &[crate::mesh::CadMeshVertex]) -> Option<CadMeshBounds> {
    let first = vertices.first()?;
    let mut min = first.position_mm;
    let mut max = first.position_mm;
    for vertex in vertices.iter().skip(1) {
        let position = vertex.position_mm;
        min[0] = min[0].min(position[0]);
        min[1] = min[1].min(position[1]);
        min[2] = min[2].min(position[2]);
        max[0] = max[0].max(position[0]);
        max[1] = max[1].max(position[1]);
        max[2] = max[2].max(position[2]);
    }
    Some(CadMeshBounds {
        min_mm: min,
        max_mm: max,
    })
}

#[cfg(test)]
mod tests {
    use super::{CadSectionAxis, CadSectionPlane, clip_mesh_payload};
    use crate::mesh::{
        CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
        CadMeshVertex,
    };

    fn fixture_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.section.fixture".to_string(),
            document_revision: 7,
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
                    position_mm: [0.0, 12.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [6.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.5, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [6.0, 12.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.5, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [12.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [12.0, 12.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 2, 3, 4, 3, 5, 4],
            edges: vec![
                CadMeshEdgeSegment {
                    start_vertex: 0,
                    end_vertex: 1,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 2,
                    end_vertex: 3,
                    flags: 1,
                },
                CadMeshEdgeSegment {
                    start_vertex: 4,
                    end_vertex: 5,
                    flags: 2,
                },
            ],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [12.0, 12.0, 0.0],
            },
        }
    }

    #[test]
    fn section_clip_is_deterministic_and_reindexes_triangles() {
        let payload = fixture_mesh();
        let plane = CadSectionPlane::new(CadSectionAxis::X, 0.0);
        let first = clip_mesh_payload(&payload, plane, 0.0).expect("clip should succeed");
        let second =
            clip_mesh_payload(&payload, plane, 0.0).expect("clip should stay deterministic");
        assert_eq!(first, second);
        assert!(!first.vertices.is_empty());
        assert!(!first.triangle_indices.is_empty());
        assert_eq!(first.triangle_indices.len() % 3, 0);
        assert!(
            first
                .vertices
                .iter()
                .all(|vertex| vertex.position_mm[0] >= 6.0)
        );
        assert!(
            first
                .triangle_indices
                .iter()
                .all(|index| (*index as usize) < first.vertices.len())
        );
    }

    #[test]
    fn section_clip_returns_explicit_error_when_plane_removes_all_triangles() {
        let payload = fixture_mesh();
        let plane = CadSectionPlane::new(CadSectionAxis::X, 1.0);
        let error =
            clip_mesh_payload(&payload, plane, 0.0).expect_err("all triangles should be clipped");
        assert!(
            error.contains("removed all triangles"),
            "expected explicit clipped-all-triangles error, got: {error}"
        );
    }

    #[test]
    fn section_axis_labels_are_stable() {
        assert_eq!(CadSectionAxis::X.label(), "x");
        assert_eq!(CadSectionAxis::Y.label(), "y");
        assert_eq!(CadSectionAxis::Z.label(), "z");
    }
}
