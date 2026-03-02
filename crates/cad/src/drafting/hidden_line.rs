use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::kernel_math::{Point3, Vec3};

use super::edge_extract::DEFAULT_SHARP_ANGLE;
use super::projection::ViewMatrix;
use super::types::{
    EdgeType, MeshEdge, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility,
};

const DEFAULT_EDGE_SAMPLES: usize = 5;
const EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftingTriangleMesh {
    pub vertices: Vec<Point3>,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DraftingProjectionOptions {
    pub sharp_angle_degrees: f64,
    pub edge_samples: usize,
}

impl Default for DraftingProjectionOptions {
    fn default() -> Self {
        Self {
            sharp_angle_degrees: DEFAULT_SHARP_ANGLE,
            edge_samples: DEFAULT_EDGE_SAMPLES,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Triangle3D {
    v0: Point3,
    v1: Point3,
    v2: Point3,
    normal: Vec3,
}

pub fn project_mesh(mesh: &DraftingTriangleMesh, view_direction: ViewDirection) -> ProjectedView {
    project_mesh_with_options(mesh, view_direction, DraftingProjectionOptions::default())
}

pub fn project_mesh_with_options(
    mesh: &DraftingTriangleMesh,
    view_direction: ViewDirection,
    options: DraftingProjectionOptions,
) -> ProjectedView {
    let view_matrix = ViewMatrix::from_view_direction(view_direction);
    let view_vector = view_direction.view_vector();
    let triangles = build_triangles(mesh);
    let edges = extract_drawing_edges(mesh, &triangles, view_vector, options.sharp_angle_degrees);

    let mut projected = ProjectedView::new(view_direction);
    for edge in edges {
        let v0 = mesh.vertices[edge.v0 as usize];
        let v1 = mesh.vertices[edge.v1 as usize];

        let (p0, d0) = view_matrix.project(v0);
        let (p1, d1) = view_matrix.project(v1);
        let visibility = check_edge_visibility(
            v0,
            v1,
            &triangles,
            &view_matrix,
            view_vector,
            options.edge_samples.max(1),
        );
        let edge = ProjectedEdge::new(p0, p1, visibility, edge.edge_type, (d0 + d1) * 0.5);
        if edge.length() >= 1e-6 {
            projected.add_edge(edge);
        }
    }

    projected
}

pub fn classify_edge_visibility(
    edges: &[MeshEdge],
    mesh: &DraftingTriangleMesh,
    view_direction: ViewDirection,
    options: DraftingProjectionOptions,
) -> Vec<(MeshEdge, Visibility)> {
    let view_matrix = ViewMatrix::from_view_direction(view_direction);
    let view_vector = view_direction.view_vector();
    let triangles = build_triangles(mesh);

    edges
        .iter()
        .map(|edge| {
            let v0 = mesh.vertices[edge.v0 as usize];
            let v1 = mesh.vertices[edge.v1 as usize];
            let visibility = check_edge_visibility(
                v0,
                v1,
                &triangles,
                &view_matrix,
                view_vector,
                options.edge_samples.max(1),
            );
            (*edge, visibility)
        })
        .collect()
}

fn extract_drawing_edges(
    mesh: &DraftingTriangleMesh,
    triangles: &[Triangle3D],
    view_vector: Vec3,
    sharp_angle_degrees: f64,
) -> Vec<MeshEdge> {
    #[derive(Debug, Clone)]
    struct EdgeAdj {
        tri0: usize,
        tri1: Option<usize>,
    }

    let mut edge_map: HashMap<(usize, usize), EdgeAdj> = HashMap::new();
    for (triangle_index, tri) in mesh.triangles.iter().enumerate() {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            edge_map
                .entry(key)
                .and_modify(|entry| entry.tri1 = Some(triangle_index))
                .or_insert(EdgeAdj {
                    tri0: triangle_index,
                    tri1: None,
                });
        }
    }

    let sharp_threshold = sharp_angle_degrees.to_radians();
    let mut result = Vec::new();

    for ((v0, v1), adjacency) in edge_map {
        let tri0 = &triangles[adjacency.tri0];
        let edge_type = if let Some(tri1_index) = adjacency.tri1 {
            let tri1 = &triangles[tri1_index];
            let front0 = tri0.normal.dot(view_vector) > 0.0;
            let front1 = tri1.normal.dot(view_vector) > 0.0;
            if front0 != front1 {
                Some(EdgeType::Silhouette)
            } else {
                let dot = tri0.normal.dot(tri1.normal).clamp(-1.0, 1.0);
                let angle = dot.acos();
                if angle >= sharp_threshold {
                    Some(EdgeType::Sharp)
                } else {
                    None
                }
            }
        } else {
            Some(EdgeType::Boundary)
        };

        if let Some(edge_type) = edge_type {
            result.push(MeshEdge::new(
                v0 as u32,
                v1 as u32,
                adjacency.tri0 as u32,
                adjacency.tri1.map(|value| value as u32),
                edge_type,
            ));
        }
    }

    result.sort_by(|left, right| {
        left.v0
            .cmp(&right.v0)
            .then_with(|| left.v1.cmp(&right.v1))
            .then_with(|| left.tri0.cmp(&right.tri0))
    });
    result
}

fn build_triangles(mesh: &DraftingTriangleMesh) -> Vec<Triangle3D> {
    mesh.triangles
        .iter()
        .map(|triangle| {
            let v0 = mesh.vertices[triangle[0]];
            let v1 = mesh.vertices[triangle[1]];
            let v2 = mesh.vertices[triangle[2]];
            let normal = triangle_normal(v0, v1, v2);
            Triangle3D { v0, v1, v2, normal }
        })
        .collect()
}

fn triangle_normal(v0: Point3, v1: Point3, v2: Point3) -> Vec3 {
    let e1 = Vec3::new(v1.x - v0.x, v1.y - v0.y, v1.z - v0.z);
    let e2 = Vec3::new(v2.x - v0.x, v2.y - v0.y, v2.z - v0.z);
    e1.cross(e2).normalized().unwrap_or_else(Vec3::z)
}

fn check_edge_visibility(
    v0: Point3,
    v1: Point3,
    triangles: &[Triangle3D],
    view_matrix: &ViewMatrix,
    view_vector: Vec3,
    edge_samples: usize,
) -> Visibility {
    for index in 0..edge_samples {
        let t = (index as f64 + 0.5) / edge_samples as f64;
        let sample = Point3::new(
            v0.x + t * (v1.x - v0.x),
            v0.y + t * (v1.y - v0.y),
            v0.z + t * (v1.z - v0.z),
        );
        let (sample_2d, sample_depth) = view_matrix.project(sample);
        if is_point_occluded(sample_2d, sample_depth, triangles, view_matrix, view_vector) {
            return Visibility::Hidden;
        }
    }

    Visibility::Visible
}

fn is_point_occluded(
    point_2d: Point2D,
    point_depth: f64,
    triangles: &[Triangle3D],
    view_matrix: &ViewMatrix,
    view_vector: Vec3,
) -> bool {
    for triangle in triangles {
        if triangle.normal.dot(view_vector) <= 0.0 {
            continue;
        }

        let (t0_2d, t0_depth) = view_matrix.project(triangle.v0);
        let (t1_2d, t1_depth) = view_matrix.project(triangle.v1);
        let (t2_2d, t2_depth) = view_matrix.project(triangle.v2);

        let min_depth = t0_depth.min(t1_depth).min(t2_depth);
        if min_depth >= point_depth - EPSILON {
            continue;
        }

        if !point_in_triangle_2d(point_2d, t0_2d, t1_2d, t2_2d) {
            continue;
        }

        if let Some(interpolated_depth) =
            interpolate_depth_at_point(point_2d, t0_2d, t1_2d, t2_2d, t0_depth, t1_depth, t2_depth)
            && interpolated_depth < point_depth - EPSILON
        {
            return true;
        }
    }

    false
}

fn point_in_triangle_2d(point: Point2D, a: Point2D, b: Point2D, c: Point2D) -> bool {
    let v0 = Vec3::new(c.x - a.x, c.y - a.y, 0.0);
    let v1 = Vec3::new(b.x - a.x, b.y - a.y, 0.0);
    let v2 = Vec3::new(point.x - a.x, point.y - a.y, 0.0);

    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() <= 1e-12 {
        return false;
    }

    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    let epsilon = 1e-8;
    u >= -epsilon && v >= -epsilon && (u + v) <= 1.0 + epsilon
}

fn interpolate_depth_at_point(
    point: Point2D,
    a: Point2D,
    b: Point2D,
    c: Point2D,
    da: f64,
    db: f64,
    dc: f64,
) -> Option<f64> {
    let v0 = Vec3::new(c.x - a.x, c.y - a.y, 0.0);
    let v1 = Vec3::new(b.x - a.x, b.y - a.y, 0.0);
    let v2 = Vec3::new(point.x - a.x, point.y - a.y, 0.0);

    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() <= 1e-12 {
        return None;
    }

    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let w = 1.0 - u - v;

    Some(w * da + v * db + u * dc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cube_mesh(size: f64) -> DraftingTriangleMesh {
        let s = size;
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(s, 0.0, 0.0),
            Point3::new(s, s, 0.0),
            Point3::new(0.0, s, 0.0),
            Point3::new(0.0, 0.0, s),
            Point3::new(s, 0.0, s),
            Point3::new(s, s, s),
            Point3::new(0.0, s, s),
        ];

        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [2, 3, 7],
            [2, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];

        DraftingTriangleMesh {
            vertices,
            triangles,
        }
    }

    #[test]
    fn isometric_projection_keeps_all_cube_edges() {
        let mesh = make_cube_mesh(1.0);
        let projected = project_mesh(&mesh, ViewDirection::ISOMETRIC_STANDARD);
        assert_eq!(projected.edges.len(), 12);
    }

    #[test]
    fn front_view_has_visible_and_hidden_edges() {
        let mesh = make_cube_mesh(1.0);
        let projected = project_mesh(&mesh, ViewDirection::Front);
        assert!(projected.num_visible() > 0);
        assert!(projected.num_hidden() > 0);
        assert!(projected.bounds.is_valid());
    }

    #[test]
    fn classify_visibility_is_deterministic() {
        let mesh = make_cube_mesh(1.0);
        let triangles = build_triangles(&mesh);
        let edges = extract_drawing_edges(
            &mesh,
            &triangles,
            ViewDirection::Front.view_vector(),
            DEFAULT_SHARP_ANGLE,
        );

        let first = classify_edge_visibility(
            &edges,
            &mesh,
            ViewDirection::Front,
            DraftingProjectionOptions::default(),
        );
        let second = classify_edge_visibility(
            &edges,
            &mesh,
            ViewDirection::Front,
            DraftingProjectionOptions::default(),
        );
        assert_eq!(first, second);
    }
}
