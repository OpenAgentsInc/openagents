use serde::{Deserialize, Serialize};

use crate::mesh::CadMeshPayload;

const QUERY_VIEWPORT_PAD_PX: f32 = 12.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadPickEntityKind {
    Body,
    Face,
    Edge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadPickProjectionMode {
    Orthographic,
    Perspective,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadPickViewport {
    pub origin_px: [f32; 2],
    pub size_px: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadPickCameraPose {
    pub projection_mode: CadPickProjectionMode,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub orbit_yaw_deg: f32,
    pub orbit_pitch_deg: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadPickQuery {
    pub viewport: CadPickViewport,
    pub camera: CadPickCameraPose,
    pub point_px: [f32; 2],
    pub tolerance_px: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadPickHit {
    pub kind: CadPickEntityKind,
    pub entity_id: String,
    pub mesh_id: String,
    pub variant_id: String,
    pub distance_px: f32,
    pub point_mm: [f32; 3],
}

pub fn pick_mesh_hit(payload: &CadMeshPayload, query: CadPickQuery) -> Option<CadPickHit> {
    let projected = project_vertices(payload, query)?;
    let tolerance_px = if query.tolerance_px > 0.0 {
        query.tolerance_px
    } else {
        (6.0 / query.camera.zoom.max(0.25)).clamp(2.0, 12.0)
    };
    let point = query.point_px;

    let mut best: Option<(u8, CadPickHit)> = None;

    for (edge_index, edge) in payload.edges.iter().enumerate() {
        let start = projected.screen.get(edge.start_vertex as usize)?;
        let end = projected.screen.get(edge.end_vertex as usize)?;
        let distance = distance_point_to_segment(point, *start, *end);
        if distance <= tolerance_px {
            let start_world = projected.world.get(edge.start_vertex as usize)?;
            let end_world = projected.world.get(edge.end_vertex as usize)?;
            let hit = CadPickHit {
                kind: CadPickEntityKind::Edge,
                entity_id: format!("edge.{edge_index}"),
                mesh_id: payload.mesh_id.clone(),
                variant_id: payload.variant_id.clone(),
                distance_px: distance,
                point_mm: midpoint3(*start_world, *end_world),
            };
            merge_best_hit(&mut best, 0, hit);
        }
    }

    for (face_index, triangle) in payload.triangle_indices.chunks_exact(3).enumerate() {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;
        let p0 = *projected.screen.get(i0)?;
        let p1 = *projected.screen.get(i1)?;
        let p2 = *projected.screen.get(i2)?;
        let distance = point_triangle_distance(point, p0, p1, p2);
        if distance <= tolerance_px {
            let w0 = *projected.world.get(i0)?;
            let w1 = *projected.world.get(i1)?;
            let w2 = *projected.world.get(i2)?;
            let hit = CadPickHit {
                kind: CadPickEntityKind::Face,
                entity_id: format!("face.{face_index}"),
                mesh_id: payload.mesh_id.clone(),
                variant_id: payload.variant_id.clone(),
                distance_px: distance,
                point_mm: centroid3(w0, w1, w2),
            };
            merge_best_hit(&mut best, 1, hit);
        }
    }

    let expanded_min = [
        projected.min_screen[0] - tolerance_px,
        projected.min_screen[1] - tolerance_px,
    ];
    let expanded_max = [
        projected.max_screen[0] + tolerance_px,
        projected.max_screen[1] + tolerance_px,
    ];
    if point[0] >= expanded_min[0]
        && point[0] <= expanded_max[0]
        && point[1] >= expanded_min[1]
        && point[1] <= expanded_max[1]
    {
        let distance = distance_point_to_rect(point, projected.min_screen, projected.max_screen);
        let hit = CadPickHit {
            kind: CadPickEntityKind::Body,
            entity_id: "body.0".to_string(),
            mesh_id: payload.mesh_id.clone(),
            variant_id: payload.variant_id.clone(),
            distance_px: distance,
            point_mm: projected.center_world,
        };
        merge_best_hit(&mut best, 2, hit);
    }

    best.map(|(_, hit)| hit)
}

fn merge_best_hit(best: &mut Option<(u8, CadPickHit)>, priority: u8, hit: CadPickHit) {
    match best {
        None => *best = Some((priority, hit)),
        Some((existing_priority, existing_hit)) => {
            let better_priority = priority < *existing_priority;
            let better_distance =
                priority == *existing_priority && hit.distance_px < existing_hit.distance_px;
            if better_priority || better_distance {
                *best = Some((priority, hit));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectedMesh {
    world: Vec<[f32; 3]>,
    screen: Vec<[f32; 2]>,
    center_world: [f32; 3],
    min_screen: [f32; 2],
    max_screen: [f32; 2],
}

fn project_vertices(payload: &CadMeshPayload, query: CadPickQuery) -> Option<ProjectedMesh> {
    if payload.vertices.is_empty() {
        return None;
    }
    if query.viewport.size_px[0] <= 1.0 || query.viewport.size_px[1] <= 1.0 {
        return None;
    }
    let yaw_rad = query.camera.orbit_yaw_deg.to_radians();
    let pitch_rad = query.camera.orbit_pitch_deg.to_radians();
    let center = [
        (payload.bounds.min_mm[0] + payload.bounds.max_mm[0]) * 0.5,
        (payload.bounds.min_mm[1] + payload.bounds.max_mm[1]) * 0.5,
        (payload.bounds.min_mm[2] + payload.bounds.max_mm[2]) * 0.5,
    ];

    let mut transformed = payload
        .vertices
        .iter()
        .map(|vertex| rotate_about_center(vertex.position_mm, center, yaw_rad, pitch_rad))
        .collect::<Vec<_>>();
    if transformed.is_empty() {
        return None;
    }

    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for point in &transformed {
        min_z = min_z.min(point[2]);
        max_z = max_z.max(point[2]);
    }
    for point in &mut transformed {
        *point = project_xy_for_mode(*point, center, query.camera.projection_mode, min_z, max_z);
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in &transformed {
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    let model_width = (max_x - min_x).abs().max(0.0001);
    let model_height = (max_y - min_y).abs().max(0.0001);
    let available_width = (query.viewport.size_px[0] - QUERY_VIEWPORT_PAD_PX * 2.0).max(1.0);
    let available_height = (query.viewport.size_px[1] - QUERY_VIEWPORT_PAD_PX * 2.0).max(1.0);
    let fit_scale = (available_width / model_width)
        .min(available_height / model_height)
        .max(0.0001);
    let scale = (fit_scale * query.camera.zoom.max(0.01)).max(0.0001);
    let scaled_width = model_width * scale;
    let scaled_height = model_height * scale;
    let origin_x = query.viewport.origin_px[0]
        + ((query.viewport.size_px[0] - scaled_width) * 0.5)
        + query.camera.pan_x;
    let origin_y = query.viewport.origin_px[1]
        + ((query.viewport.size_px[1] - scaled_height) * 0.5)
        + query.camera.pan_y;

    let mut min_screen = [f32::INFINITY, f32::INFINITY];
    let mut max_screen = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut screen = Vec::with_capacity(transformed.len());
    for point in &transformed {
        let sx = origin_x + ((point[0] - min_x) * scale);
        let sy = origin_y + ((max_y - point[1]) * scale);
        min_screen[0] = min_screen[0].min(sx);
        min_screen[1] = min_screen[1].min(sy);
        max_screen[0] = max_screen[0].max(sx);
        max_screen[1] = max_screen[1].max(sy);
        screen.push([sx, sy]);
    }

    Some(ProjectedMesh {
        world: transformed,
        screen,
        center_world: center,
        min_screen,
        max_screen,
    })
}

fn project_xy_for_mode(
    position: [f32; 3],
    center: [f32; 3],
    mode: CadPickProjectionMode,
    min_z: f32,
    max_z: f32,
) -> [f32; 3] {
    if mode == CadPickProjectionMode::Orthographic {
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

fn midpoint3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
    ]
}

fn centroid3(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    [
        (a[0] + b[0] + c[0]) / 3.0,
        (a[1] + b[1] + c[1]) / 3.0,
        (a[2] + b[2] + c[2]) / 3.0,
    ]
}

fn distance_point_to_segment(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }
    let t = (((point[0] - start[0]) * dx) + ((point[1] - start[1]) * dy)) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj = [start[0] + t * dx, start[1] + t * dy];
    ((point[0] - proj[0]).powi(2) + (point[1] - proj[1]).powi(2)).sqrt()
}

fn point_triangle_distance(point: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    if point_in_triangle(point, a, b, c) {
        return 0.0;
    }
    distance_point_to_segment(point, a, b)
        .min(distance_point_to_segment(point, b, c))
        .min(distance_point_to_segment(point, c, a))
}

fn point_in_triangle(point: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let area = triangle_area(a, b, c);
    if area.abs() <= f32::EPSILON {
        return false;
    }
    let w0 = triangle_area(point, b, c) / area;
    let w1 = triangle_area(a, point, c) / area;
    let w2 = triangle_area(a, b, point) / area;
    const EPS: f32 = 1e-5;
    w0 >= -EPS && w1 >= -EPS && w2 >= -EPS && w0 <= 1.0 + EPS && w1 <= 1.0 + EPS && w2 <= 1.0 + EPS
}

fn triangle_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn distance_point_to_rect(point: [f32; 2], min: [f32; 2], max: [f32; 2]) -> f32 {
    let dx = if point[0] < min[0] {
        min[0] - point[0]
    } else if point[0] > max[0] {
        point[0] - max[0]
    } else {
        0.0
    };
    let dy = if point[1] < min[1] {
        min[1] - point[1]
    } else if point[1] > max[1] {
        point[1] - max[1]
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{
        CadPickCameraPose, CadPickEntityKind, CadPickProjectionMode, CadPickQuery, CadPickViewport,
        pick_mesh_hit, project_vertices,
    };
    use crate::mesh::{
        CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
        CadMeshVertex,
    };

    fn square_mesh_payload() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.square".to_string(),
            document_revision: 1,
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
                    position_mm: [100.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [100.0, 100.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 100.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 2, 3],
            edges: vec![
                CadMeshEdgeSegment {
                    start_vertex: 0,
                    end_vertex: 1,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 1,
                    end_vertex: 2,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 2,
                    end_vertex: 3,
                    flags: 0,
                },
                CadMeshEdgeSegment {
                    start_vertex: 3,
                    end_vertex: 0,
                    flags: 0,
                },
            ],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [100.0, 100.0, 0.0],
            },
        }
    }

    fn default_query() -> CadPickQuery {
        CadPickQuery {
            viewport: CadPickViewport {
                origin_px: [20.0, 20.0],
                size_px: [500.0, 360.0],
            },
            camera: CadPickCameraPose {
                projection_mode: CadPickProjectionMode::Orthographic,
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
                orbit_yaw_deg: 0.0,
                orbit_pitch_deg: 0.0,
            },
            point_px: [240.0, 180.0],
            tolerance_px: 8.0,
        }
    }

    #[test]
    fn picks_face_for_triangle_interior() {
        let payload = square_mesh_payload();
        let mut query = default_query();
        let projected = project_vertices(&payload, query).expect("projection should succeed");
        let center = [
            (projected.min_screen[0] + projected.max_screen[0]) * 0.5,
            (projected.min_screen[1] + projected.max_screen[1]) * 0.5,
        ];
        query.point_px = center;
        let hit = pick_mesh_hit(&payload, query).expect("face hit expected");
        assert_eq!(hit.kind, CadPickEntityKind::Face);
        assert_eq!(hit.mesh_id, "mesh.square");
        assert_eq!(hit.variant_id, "variant.baseline");
        assert!(hit.entity_id.starts_with("face."));
    }

    #[test]
    fn picks_edge_with_tolerance() {
        let payload = square_mesh_payload();
        let mut query = default_query();
        let projected = project_vertices(&payload, query).expect("projection should succeed");
        let top_mid = [
            (projected.screen[0][0] + projected.screen[1][0]) * 0.5,
            (projected.screen[0][1] + projected.screen[1][1]) * 0.5 + 1.0,
        ];
        query.point_px = top_mid;
        query.tolerance_px = 6.0;
        let hit = pick_mesh_hit(&payload, query).expect("edge hit expected");
        assert_eq!(hit.kind, CadPickEntityKind::Edge);
        assert_eq!(hit.entity_id, "edge.0");
    }

    #[test]
    fn picks_body_when_faces_and_edges_are_absent() {
        let mut payload = square_mesh_payload();
        payload.triangle_indices.clear();
        payload.edges.clear();
        let mut query = default_query();
        let projected = project_vertices(&payload, query).expect("projection should succeed");
        query.point_px = [
            (projected.min_screen[0] + projected.max_screen[0]) * 0.5,
            (projected.min_screen[1] + projected.max_screen[1]) * 0.5,
        ];
        let hit = pick_mesh_hit(&payload, query).expect("body hit expected");
        assert_eq!(hit.kind, CadPickEntityKind::Body);
        assert_eq!(hit.entity_id, "body.0");
    }

    #[test]
    fn hit_precision_is_stable_across_zoom_levels() {
        let payload = square_mesh_payload();
        for zoom in [0.6, 1.0, 1.8, 2.4] {
            let mut query = default_query();
            query.camera.zoom = zoom;
            query.tolerance_px = 0.0;
            let projected = project_vertices(&payload, query).expect("projection should succeed");
            let right_edge_mid = [
                (projected.screen[1][0] + projected.screen[2][0]) * 0.5 - 0.5,
                (projected.screen[1][1] + projected.screen[2][1]) * 0.5,
            ];
            query.point_px = right_edge_mid;
            let hit = pick_mesh_hit(&payload, query).expect("edge hit should remain stable");
            assert_eq!(hit.kind, CadPickEntityKind::Edge);
            assert_eq!(hit.entity_id, "edge.1");
        }
    }
}
