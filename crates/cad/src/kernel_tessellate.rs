use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::kernel_geom::{SurfaceKind, SurfaceRecord};
use crate::kernel_math::Point3;
use crate::kernel_primitives::BRepSolid;
use crate::{CadError, CadResult};

const EPSILON: f64 = 1e-9;

/// Output triangle mesh for kernel tessellation parity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriangleMesh {
    /// Flat array of vertex positions `[x0, y0, z0, ...]`.
    pub vertices: Vec<f32>,
    /// Flat array of triangle indices `[i0, i1, i2, ...]`.
    pub indices: Vec<u32>,
    /// Flat array of vertex normals `[nx0, ny0, nz0, ...]`.
    pub normals: Vec<f32>,
}

impl TriangleMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn num_vertices(&self) -> usize {
        self.vertices.len() / 3
    }

    pub fn num_triangles(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn merge(&mut self, other: &TriangleMesh) {
        let offset = self.num_vertices() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.normals.extend_from_slice(&other.normals);
        self.indices
            .extend(other.indices.iter().map(|index| index + offset));
    }
}

/// Tessellation quality parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TessellationParams {
    pub circle_segments: u32,
    pub height_segments: u32,
    pub latitude_segments: u32,
}

impl Default for TessellationParams {
    fn default() -> Self {
        Self {
            circle_segments: 32,
            height_segments: 1,
            latitude_segments: 16,
        }
    }
}

impl TessellationParams {
    pub fn from_segments(segments: u32) -> Self {
        Self {
            circle_segments: segments.max(3),
            height_segments: 1,
            latitude_segments: (segments / 2).max(4),
        }
    }
}

/// Tessellate a B-rep solid using vcad-style segment controls.
pub fn tessellate_brep(brep: &BRepSolid, segments: u32) -> CadResult<TriangleMesh> {
    let params = TessellationParams::from_segments(segments);
    tessellate_solid(brep, &params)
}

/// Tessellate a B-rep solid into a triangle mesh.
///
/// This parity lane supports solids emitted by `kernel_primitives` constructors
/// (cube, cylinder, sphere, cone/frustum).
pub fn tessellate_solid(brep: &BRepSolid, params: &TessellationParams) -> CadResult<TriangleMesh> {
    if brep.geometry.surfaces.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "kernel tessellation requires at least one surface".to_string(),
        });
    }

    let vertices: Vec<Point3> = brep
        .topology
        .vertices
        .values()
        .map(|vertex| vertex.point)
        .collect();
    if vertices.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "kernel tessellation requires at least one topology vertex".to_string(),
        });
    }

    let kind = classify_primitive(brep, &vertices)?;
    match kind {
        PrimitiveKind::Cube { min, max } => Ok(tessellate_cube(min, max)),
        PrimitiveKind::Cylinder {
            radius,
            z_min,
            z_max,
        } => Ok(tessellate_cylinder(
            radius,
            z_min,
            z_max,
            params.circle_segments,
        )),
        PrimitiveKind::Sphere { center, radius } => Ok(tessellate_sphere(
            center,
            radius,
            params.circle_segments,
            params.latitude_segments,
        )),
        PrimitiveKind::Cone {
            radius_bottom,
            radius_top,
            z_min,
            z_max,
        } => Ok(tessellate_cone(
            radius_bottom,
            radius_top,
            z_min,
            z_max,
            params.circle_segments,
            params.height_segments,
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PrimitiveKind {
    Cube {
        min: [f64; 3],
        max: [f64; 3],
    },
    Cylinder {
        radius: f64,
        z_min: f64,
        z_max: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Cone {
        radius_bottom: f64,
        radius_top: f64,
        z_min: f64,
        z_max: f64,
    },
}

fn classify_primitive(brep: &BRepSolid, vertices: &[Point3]) -> CadResult<PrimitiveKind> {
    let counts = brep.topology.counts();
    let (min, max) = bounds(vertices);

    let mut plane_count = 0;
    let mut cylinder_count = 0;
    let mut sphere_count = 0;
    let mut cone_count = 0;
    for surface in &brep.geometry.surfaces {
        match surface.kind() {
            SurfaceKind::Plane => plane_count += 1,
            SurfaceKind::Cylinder => cylinder_count += 1,
            SurfaceKind::Sphere => sphere_count += 1,
            SurfaceKind::Cone => cone_count += 1,
            _ => {}
        }
    }

    if counts.face_count == 6 && plane_count == 6 {
        return Ok(PrimitiveKind::Cube { min, max });
    }

    if counts.face_count == 3 && cylinder_count == 1 && plane_count == 2 {
        let radius = max_radial(vertices);
        return Ok(PrimitiveKind::Cylinder {
            radius,
            z_min: min[2],
            z_max: max[2],
        });
    }

    if counts.face_count == 1 && sphere_count == 1 {
        for surface in &brep.geometry.surfaces {
            if let SurfaceRecord::Sphere(sphere) = surface {
                return Ok(PrimitiveKind::Sphere {
                    center: [sphere.center.x, sphere.center.y, sphere.center.z],
                    radius: sphere.radius,
                });
            }
        }
    }

    if cone_count == 1 && (counts.face_count == 2 || counts.face_count == 3) {
        let (radius_bottom, radius_top) = radial_extrema(vertices, min[2], max[2]);
        return Ok(PrimitiveKind::Cone {
            radius_bottom,
            radius_top,
            z_min: min[2],
            z_max: max[2],
        });
    }

    Err(CadError::InvalidFeatureGraph {
        reason: format!(
            "kernel tessellation cannot classify primitive topology (faces={}, surfaces={})",
            counts.face_count,
            brep.geometry.surfaces.len()
        ),
    })
}

fn tessellate_cube(min: [f64; 3], max: [f64; 3]) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

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

    let faces = [
        ([0, 1, 2, 3], [0.0, 0.0, -1.0]),
        ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
        ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
        ([2, 3, 7, 6], [0.0, 1.0, 0.0]),
        ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
        ([1, 2, 6, 5], [1.0, 0.0, 0.0]),
    ];

    for (indices, normal) in faces {
        let base = mesh.num_vertices() as u32;
        for corner_index in indices {
            push_vertex(&mut mesh, corners[corner_index], normal);
        }
        push_triangle(&mut mesh, base, base + 1, base + 2);
        push_triangle(&mut mesh, base, base + 2, base + 3);
    }

    mesh
}

fn tessellate_cylinder(radius: f64, z_min: f64, z_max: f64, segments: u32) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();
    let n = segments.max(3) as usize;

    let mut side_bottom = Vec::with_capacity(n);
    let mut side_top = Vec::with_capacity(n);

    for i in 0..n {
        let angle = (i as f64 / n as f64) * (2.0 * PI);
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        let radial_normal = normalize([x, y, 0.0]);
        side_bottom.push(push_vertex(&mut mesh, [x, y, z_min], radial_normal));
        side_top.push(push_vertex(&mut mesh, [x, y, z_max], radial_normal));
    }

    for i in 0..n {
        let next = (i + 1) % n;
        let b0 = side_bottom[i];
        let t0 = side_top[i];
        let b1 = side_bottom[next];
        let t1 = side_top[next];
        push_triangle(&mut mesh, b0, t0, t1);
        push_triangle(&mut mesh, b0, t1, b1);
    }

    let bottom_center = push_vertex(&mut mesh, [0.0, 0.0, z_min], [0.0, 0.0, -1.0]);
    let mut bottom_ring = Vec::with_capacity(n);
    for i in 0..n {
        let angle = (i as f64 / n as f64) * (2.0 * PI);
        bottom_ring.push(push_vertex(
            &mut mesh,
            [radius * angle.cos(), radius * angle.sin(), z_min],
            [0.0, 0.0, -1.0],
        ));
    }
    for i in 0..n {
        let next = (i + 1) % n;
        push_triangle(&mut mesh, bottom_center, bottom_ring[next], bottom_ring[i]);
    }

    let top_center = push_vertex(&mut mesh, [0.0, 0.0, z_max], [0.0, 0.0, 1.0]);
    let mut top_ring = Vec::with_capacity(n);
    for i in 0..n {
        let angle = (i as f64 / n as f64) * (2.0 * PI);
        top_ring.push(push_vertex(
            &mut mesh,
            [radius * angle.cos(), radius * angle.sin(), z_max],
            [0.0, 0.0, 1.0],
        ));
    }
    for i in 0..n {
        let next = (i + 1) % n;
        push_triangle(&mut mesh, top_center, top_ring[i], top_ring[next]);
    }

    mesh
}

fn tessellate_sphere(
    center: [f64; 3],
    radius: f64,
    circle_segments: u32,
    latitude_segments: u32,
) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();
    let n_lon = circle_segments.max(3) as usize;
    let n_lat = latitude_segments.max(4) as usize;

    let mut grid = vec![vec![0_u32; n_lon + 1]; n_lat + 1];
    for (lat, row) in grid.iter_mut().enumerate().take(n_lat + 1) {
        let theta = (lat as f64 / n_lat as f64) * PI;
        let ring_radius = radius * theta.sin();
        let z = center[2] + radius * theta.cos();

        for (lon, slot) in row.iter_mut().enumerate().take(n_lon + 1) {
            let phi = (lon as f64 / n_lon as f64) * (2.0 * PI);
            let x = center[0] + ring_radius * phi.cos();
            let y = center[1] + ring_radius * phi.sin();
            let normal = normalize([x - center[0], y - center[1], z - center[2]]);
            *slot = push_vertex(&mut mesh, [x, y, z], normal);
        }
    }

    for lat in 0..n_lat {
        for lon in 0..n_lon {
            let a = grid[lat][lon];
            let b = grid[lat + 1][lon];
            let c = grid[lat + 1][lon + 1];
            let d = grid[lat][lon + 1];
            push_triangle(&mut mesh, a, b, c);
            push_triangle(&mut mesh, a, c, d);
        }
    }

    mesh
}

fn tessellate_cone(
    radius_bottom: f64,
    radius_top: f64,
    z_min: f64,
    z_max: f64,
    circle_segments: u32,
    height_segments: u32,
) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();
    let n = circle_segments.max(3) as usize;
    let height = (z_max - z_min).max(EPSILON);
    let slope = (radius_bottom - radius_top) / height;
    let side_normal_z = slope;

    if radius_top <= EPSILON {
        let mut bottom_ring = Vec::with_capacity(n);
        for i in 0..n {
            let angle = (i as f64 / n as f64) * (2.0 * PI);
            let x = radius_bottom * angle.cos();
            let y = radius_bottom * angle.sin();
            let normal = normalize([x, y, side_normal_z]);
            bottom_ring.push(push_vertex(&mut mesh, [x, y, z_min], normal));
        }
        let apex = push_vertex(&mut mesh, [0.0, 0.0, z_max], [0.0, 0.0, 1.0]);
        for i in 0..n {
            let next = (i + 1) % n;
            push_triangle(&mut mesh, bottom_ring[i], bottom_ring[next], apex);
        }

        let bottom_center = push_vertex(&mut mesh, [0.0, 0.0, z_min], [0.0, 0.0, -1.0]);
        let mut bottom_cap_ring = Vec::with_capacity(n);
        for i in 0..n {
            let angle = (i as f64 / n as f64) * (2.0 * PI);
            bottom_cap_ring.push(push_vertex(
                &mut mesh,
                [
                    radius_bottom * angle.cos(),
                    radius_bottom * angle.sin(),
                    z_min,
                ],
                [0.0, 0.0, -1.0],
            ));
        }
        for i in 0..n {
            let next = (i + 1) % n;
            push_triangle(
                &mut mesh,
                bottom_center,
                bottom_cap_ring[next],
                bottom_cap_ring[i],
            );
        }
        return mesh;
    }

    let h_segments = height_segments.max(1) as usize;
    let mut rings = vec![vec![0_u32; n]; h_segments + 1];
    for (h, ring) in rings.iter_mut().enumerate().take(h_segments + 1) {
        let t = h as f64 / h_segments as f64;
        let z = z_min + t * (z_max - z_min);
        let radius = radius_bottom + t * (radius_top - radius_bottom);
        for (i, slot) in ring.iter_mut().enumerate().take(n) {
            let angle = (i as f64 / n as f64) * (2.0 * PI);
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            let normal = normalize([x, y, side_normal_z]);
            *slot = push_vertex(&mut mesh, [x, y, z], normal);
        }
    }

    for h in 0..h_segments {
        for i in 0..n {
            let next = (i + 1) % n;
            let b0 = rings[h][i];
            let b1 = rings[h][next];
            let t0 = rings[h + 1][i];
            let t1 = rings[h + 1][next];
            push_triangle(&mut mesh, b0, t0, t1);
            push_triangle(&mut mesh, b0, t1, b1);
        }
    }

    let bottom_center = push_vertex(&mut mesh, [0.0, 0.0, z_min], [0.0, 0.0, -1.0]);
    let mut bottom_ring = Vec::with_capacity(n);
    for i in 0..n {
        let angle = (i as f64 / n as f64) * (2.0 * PI);
        bottom_ring.push(push_vertex(
            &mut mesh,
            [
                radius_bottom * angle.cos(),
                radius_bottom * angle.sin(),
                z_min,
            ],
            [0.0, 0.0, -1.0],
        ));
    }
    for i in 0..n {
        let next = (i + 1) % n;
        push_triangle(&mut mesh, bottom_center, bottom_ring[next], bottom_ring[i]);
    }

    let top_center = push_vertex(&mut mesh, [0.0, 0.0, z_max], [0.0, 0.0, 1.0]);
    let mut top_ring = Vec::with_capacity(n);
    for i in 0..n {
        let angle = (i as f64 / n as f64) * (2.0 * PI);
        top_ring.push(push_vertex(
            &mut mesh,
            [radius_top * angle.cos(), radius_top * angle.sin(), z_max],
            [0.0, 0.0, 1.0],
        ));
    }
    for i in 0..n {
        let next = (i + 1) % n;
        push_triangle(&mut mesh, top_center, top_ring[i], top_ring[next]);
    }

    mesh
}

fn push_vertex(mesh: &mut TriangleMesh, position: [f64; 3], normal: [f64; 3]) -> u32 {
    let normal = normalize(normal);
    let index = mesh.num_vertices() as u32;
    mesh.vertices.push(position[0] as f32);
    mesh.vertices.push(position[1] as f32);
    mesh.vertices.push(position[2] as f32);
    mesh.normals.push(normal[0] as f32);
    mesh.normals.push(normal[1] as f32);
    mesh.normals.push(normal[2] as f32);
    index
}

fn push_triangle(mesh: &mut TriangleMesh, a: u32, b: u32, c: u32) {
    mesh.indices.push(a);
    mesh.indices.push(b);
    mesh.indices.push(c);
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq <= EPSILON {
        return [0.0, 0.0, 1.0];
    }
    let inv = len_sq.sqrt().recip();
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

fn bounds(vertices: &[Point3]) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in vertices {
        min[0] = min[0].min(point.x);
        min[1] = min[1].min(point.y);
        min[2] = min[2].min(point.z);
        max[0] = max[0].max(point.x);
        max[1] = max[1].max(point.y);
        max[2] = max[2].max(point.z);
    }
    (min, max)
}

fn max_radial(vertices: &[Point3]) -> f64 {
    vertices
        .iter()
        .map(|point| point.x.hypot(point.y))
        .fold(0.0_f64, f64::max)
}

fn radial_extrema(vertices: &[Point3], z_min: f64, z_max: f64) -> (f64, f64) {
    let mut bottom = 0.0_f64;
    let mut top = 0.0_f64;
    for point in vertices {
        let radial = point.x.hypot(point.y);
        if (point.z - z_min).abs() <= 1e-6 {
            bottom = bottom.max(radial);
        }
        if (point.z - z_max).abs() <= 1e-6 {
            top = top.max(radial);
        }
    }
    (bottom, top)
}

#[cfg(test)]
mod tests {
    use super::{TessellationParams, tessellate_brep, tessellate_solid};
    use crate::kernel_primitives::{make_cone, make_cube, make_cylinder, make_sphere};

    fn assert_mesh_is_consistent(mesh: &super::TriangleMesh) {
        assert_eq!(mesh.vertices.len() % 3, 0);
        assert_eq!(mesh.normals.len(), mesh.vertices.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        let vertex_count = mesh.num_vertices() as u32;
        assert!(
            mesh.indices.iter().all(|index| *index < vertex_count),
            "mesh indices must reference existing vertices"
        );
    }

    #[test]
    fn cube_tessellation_contract_is_stable() {
        let cube = make_cube(10.0, 20.0, 30.0).expect("cube");
        let mesh = tessellate_brep(&cube, 32).expect("cube tessellation");
        assert_mesh_is_consistent(&mesh);
        assert_eq!(mesh.num_triangles(), 12);
        assert_eq!(mesh.num_vertices(), 24);
    }

    #[test]
    fn cylinder_tessellation_contract_is_stable() {
        let cylinder = make_cylinder(5.0, 10.0, 16).expect("cylinder");
        let mesh = tessellate_brep(&cylinder, 16).expect("cylinder tessellation");
        assert_mesh_is_consistent(&mesh);
        assert_eq!(mesh.num_triangles(), 64);
    }

    #[test]
    fn sphere_tessellation_contract_is_stable() {
        let sphere = make_sphere(8.0, 16).expect("sphere");
        let mesh = tessellate_brep(&sphere, 16).expect("sphere tessellation");
        assert_mesh_is_consistent(&mesh);
        assert_eq!(mesh.num_triangles(), 256);
    }

    #[test]
    fn cone_tessellation_supports_pointed_and_frustum_variants() {
        let pointed = make_cone(5.0, 0.0, 12.0, 16).expect("pointed cone");
        let pointed_mesh = tessellate_brep(&pointed, 16).expect("pointed cone tessellation");
        assert_mesh_is_consistent(&pointed_mesh);
        assert_eq!(pointed_mesh.num_triangles(), 32);

        let frustum = make_cone(6.0, 2.0, 12.0, 16).expect("frustum cone");
        let frustum_mesh = tessellate_brep(&frustum, 16).expect("frustum cone tessellation");
        assert_mesh_is_consistent(&frustum_mesh);
        assert_eq!(frustum_mesh.num_triangles(), 64);
    }

    #[test]
    fn tessellation_is_deterministic_for_identical_inputs() {
        let sphere = make_sphere(10.0, 32).expect("sphere");
        let params = TessellationParams::from_segments(24);
        let mesh_a = tessellate_solid(&sphere, &params).expect("first tessellation");
        let mesh_b = tessellate_solid(&sphere, &params).expect("second tessellation");
        assert_eq!(mesh_a, mesh_b);
    }

    #[test]
    fn tessellation_rejects_unclassified_topology() {
        let mut cube = make_cube(1.0, 1.0, 1.0).expect("cube");
        cube.geometry.surfaces.clear();
        let error = tessellate_brep(&cube, 16).expect_err("classification should fail");
        assert_eq!(
            error.to_string(),
            "invalid feature graph: kernel tessellation requires at least one surface"
        );
    }
}
