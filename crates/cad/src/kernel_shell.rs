use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::kernel_geom::{BilinearSurface, Plane, SurfaceKind, SurfaceRecord};
use crate::kernel_math::{Point3, Vec3};
use crate::kernel_primitives::BRepSolid;
use crate::kernel_tessellate::{TriangleMesh, tessellate_brep};
use crate::kernel_topology::{
    FaceId, HalfEdgeId, LoopId, Orientation, ShellType, Topology, VertexId,
};
use crate::{CadError, CadResult};

/// Errors emitted by analytical shelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellError {
    SurfaceCollapse(FaceId, String),
    VertexCollision(VertexId),
    SelfIntersection(FaceId, FaceId),
    UnsupportedSurface(SurfaceKind),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SurfaceCollapse(_, reason) => write!(f, "surface collapsed: {reason}"),
            Self::VertexCollision(vertex_id) => {
                write!(
                    f,
                    "inner vertex crossed outer shell at vertex {}",
                    vertex_id.0
                )
            }
            Self::SelfIntersection(face_a, face_b) => write!(
                f,
                "offset produced self-intersection between faces {} and {}",
                face_a.0, face_b.0
            ),
            Self::UnsupportedSurface(kind) => {
                write!(f, "unsupported surface type for analytical shell: {kind:?}")
            }
        }
    }
}

impl std::error::Error for ShellError {}

/// Create a shell (hollow body) from a B-rep solid.
///
/// This first attempts analytical shelling and falls back to mesh shelling when
/// analytical offset is unavailable.
pub fn shell_brep(brep: &BRepSolid, thickness: f64) -> CadResult<BRepSolid> {
    validate_positive_thickness(thickness)?;

    match shell_brep_analytical(brep, thickness, &[]) {
        Ok(result) => Ok(result),
        Err(_error) => {
            let outer_mesh = tessellate_brep(brep, 32)?;
            let shell_mesh = shell_mesh(&outer_mesh, thickness)?;
            mesh_to_brep(&shell_mesh)
        }
    }
}

/// Analytical shell for planar-face solids.
///
/// `open_face_ids` removes selected outer faces and creates side walls around
/// those openings.
pub fn shell_brep_analytical(
    brep: &BRepSolid,
    thickness: f64,
    open_face_ids: &[FaceId],
) -> Result<BRepSolid, ShellError> {
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(ShellError::SurfaceCollapse(
            FaceId(0),
            "thickness must be finite and positive".to_string(),
        ));
    }

    let topo = &brep.topology;
    let geom = &brep.geometry;

    let solid = topo.solids.get(&brep.solid_id).ok_or_else(|| {
        ShellError::SurfaceCollapse(FaceId(0), "missing source solid".to_string())
    })?;
    let shell = topo.shells.get(&solid.outer_shell).ok_or_else(|| {
        ShellError::SurfaceCollapse(FaceId(0), "missing source outer shell".to_string())
    })?;

    if shell.faces.is_empty() {
        return Ok(brep.clone());
    }

    let mut open_face_set = BTreeSet::new();
    for face_id in open_face_ids {
        if shell.faces.contains(face_id) {
            open_face_set.insert(*face_id);
        }
    }

    let (min_corner, max_corner) = topology_bounds(topo);
    let min_span = (max_corner.x - min_corner.x)
        .min(max_corner.y - min_corner.y)
        .min(max_corner.z - min_corner.z);
    if min_span.is_finite() && thickness * 2.0 >= min_span - 1e-9 {
        let face_id = shell.faces[0];
        return Err(ShellError::SurfaceCollapse(
            face_id,
            format!(
                "offset {:.4} collapses minimum body span {:.4}",
                thickness, min_span
            ),
        ));
    }

    for face_id in &shell.faces {
        if open_face_set.contains(face_id) {
            continue;
        }
        let face = topo.faces.get(face_id).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source face".to_string())
        })?;
        let surface = geom.surfaces.get(face.surface_index).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source surface".to_string())
        })?;
        if surface.kind() != SurfaceKind::Plane {
            return Err(ShellError::UnsupportedSurface(surface.kind()));
        }
    }

    let mut vertex_normals: BTreeMap<VertexId, Vec<Vec3>> = BTreeMap::new();
    for face_id in &shell.faces {
        if open_face_set.contains(face_id) {
            continue;
        }
        let face = topo.faces.get(face_id).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source face".to_string())
        })?;
        let surface = geom.surfaces.get(face.surface_index).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source surface".to_string())
        })?;

        let plane = match surface {
            SurfaceRecord::Plane(plane) => plane,
            _ => return Err(ShellError::UnsupportedSurface(surface.kind())),
        };

        let face_vertices = loop_vertices(topo, face.outer_loop)?;
        let mut outward = plane.normal_dir.into_inner();
        if face.orientation == Orientation::Reversed {
            outward = vec_scale(outward, -1.0);
        }
        for vertex_id in face_vertices {
            vertex_normals.entry(vertex_id).or_default().push(outward);
        }
    }

    let mut new_topo = Topology::new();
    let mut new_geom = crate::kernel_geom::GeometryStore::default();

    let mut outer_vertex_map = BTreeMap::<VertexId, VertexId>::new();
    let mut inner_vertex_map = BTreeMap::<VertexId, VertexId>::new();

    for (old_vertex_id, vertex) in &topo.vertices {
        let new_vertex_id = new_topo.add_vertex(vertex.point);
        outer_vertex_map.insert(*old_vertex_id, new_vertex_id);
    }

    for (old_vertex_id, vertex) in &topo.vertices {
        let normals = vertex_normals
            .get(old_vertex_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let inner_point = compute_offset_vertex(vertex.point, normals, thickness);
        let new_vertex_id = new_topo.add_vertex(inner_point);
        inner_vertex_map.insert(*old_vertex_id, new_vertex_id);
    }

    let mut face_ids = Vec::new();

    for face_id in &shell.faces {
        let source_face = topo.faces.get(face_id).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source face".to_string())
        })?;
        let source_surface = geom
            .surfaces
            .get(source_face.surface_index)
            .ok_or_else(|| {
                ShellError::SurfaceCollapse(*face_id, "missing source surface".to_string())
            })?;
        let surface_index = new_geom.add_surface(source_surface.clone());

        let source_loop = loop_vertices(topo, source_face.outer_loop)?;
        let outer_vertices = source_loop
            .iter()
            .map(|vertex_id| {
                outer_vertex_map.get(vertex_id).copied().ok_or_else(|| {
                    ShellError::SurfaceCollapse(
                        *face_id,
                        format!("missing outer vertex map for {}", vertex_id.0),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let new_face_id = add_face(
            &mut new_topo,
            &outer_vertices,
            surface_index,
            source_face.orientation,
        )?;
        face_ids.push(new_face_id);
    }

    for face_id in &shell.faces {
        if open_face_set.contains(face_id) {
            continue;
        }

        let source_face = topo.faces.get(face_id).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source face".to_string())
        })?;
        let source_surface = geom
            .surfaces
            .get(source_face.surface_index)
            .ok_or_else(|| {
                ShellError::SurfaceCollapse(*face_id, "missing source surface".to_string())
            })?;

        let plane = match source_surface {
            SurfaceRecord::Plane(plane) => plane,
            _ => return Err(ShellError::UnsupportedSurface(source_surface.kind())),
        };
        let inner_plane = offset_plane(plane, source_face.orientation, thickness);
        let surface_index = new_geom.add_surface(SurfaceRecord::Plane(inner_plane));

        let source_loop = loop_vertices(topo, source_face.outer_loop)?;
        let inner_vertices = source_loop
            .iter()
            .rev()
            .map(|vertex_id| {
                inner_vertex_map.get(vertex_id).copied().ok_or_else(|| {
                    ShellError::SurfaceCollapse(
                        *face_id,
                        format!("missing inner vertex map for {}", vertex_id.0),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let orientation = match source_face.orientation {
            Orientation::Forward => Orientation::Reversed,
            Orientation::Reversed => Orientation::Forward,
        };
        let new_face_id = add_face(&mut new_topo, &inner_vertices, surface_index, orientation)?;
        face_ids.push(new_face_id);
    }

    for face_id in &open_face_set {
        let source_face = topo.faces.get(face_id).ok_or_else(|| {
            ShellError::SurfaceCollapse(*face_id, "missing source face".to_string())
        })?;
        let source_loop = loop_vertices(topo, source_face.outer_loop)?;
        if source_loop.len() < 2 {
            continue;
        }

        for index in 0..source_loop.len() {
            let next = (index + 1) % source_loop.len();
            let source_a = source_loop[index];
            let source_b = source_loop[next];

            let outer_a = outer_vertex_map.get(&source_a).copied().ok_or_else(|| {
                ShellError::SurfaceCollapse(
                    *face_id,
                    format!("missing outer vertex map for {}", source_a.0),
                )
            })?;
            let outer_b = outer_vertex_map.get(&source_b).copied().ok_or_else(|| {
                ShellError::SurfaceCollapse(
                    *face_id,
                    format!("missing outer vertex map for {}", source_b.0),
                )
            })?;
            let inner_a = inner_vertex_map.get(&source_a).copied().ok_or_else(|| {
                ShellError::SurfaceCollapse(
                    *face_id,
                    format!("missing inner vertex map for {}", source_a.0),
                )
            })?;
            let inner_b = inner_vertex_map.get(&source_b).copied().ok_or_else(|| {
                ShellError::SurfaceCollapse(
                    *face_id,
                    format!("missing inner vertex map for {}", source_b.0),
                )
            })?;

            let pa = point_for_vertex(&new_topo, outer_a, *face_id)?;
            let pb = point_for_vertex(&new_topo, outer_b, *face_id)?;
            let pc = point_for_vertex(&new_topo, inner_b, *face_id)?;
            let pd = point_for_vertex(&new_topo, inner_a, *face_id)?;

            let surface_index = new_geom.add_surface(SurfaceRecord::Bilinear(
                BilinearSurface::new(pa, pb, pd, pc),
            ));

            let wall_vertices = vec![outer_a, outer_b, inner_b, inner_a];
            let wall_face = add_face(
                &mut new_topo,
                &wall_vertices,
                surface_index,
                Orientation::Forward,
            )?;
            face_ids.push(wall_face);
        }
    }

    pair_twin_half_edges(&mut new_topo).map_err(|error| {
        ShellError::SurfaceCollapse(FaceId(0), format!("failed pairing half-edges: {error}"))
    })?;

    let shell_id = new_topo
        .add_shell(face_ids, ShellType::Outer)
        .map_err(|error| {
            ShellError::SurfaceCollapse(FaceId(0), format!("failed creating shell: {error}"))
        })?;
    let solid_id = new_topo.add_solid(shell_id).map_err(|error| {
        ShellError::SurfaceCollapse(FaceId(0), format!("failed creating solid: {error}"))
    })?;

    Ok(BRepSolid {
        topology: new_topo,
        geometry: new_geom,
        solid_id,
    })
}

/// Shell a triangle mesh by offsetting vertices along averaged vertex normals.
pub fn shell_mesh(mesh: &TriangleMesh, thickness: f64) -> CadResult<TriangleMesh> {
    validate_positive_thickness(thickness)?;

    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Ok(mesh.clone());
    }

    if !mesh.vertices.len().is_multiple_of(3) || !mesh.indices.len().is_multiple_of(3) {
        return Err(CadError::InvalidFeatureGraph {
            reason: "shell mesh requires packed xyz vertices and triangle indices".to_string(),
        });
    }

    let normals = compute_vertex_normals(mesh);
    let vertex_count = mesh.vertices.len() / 3;

    let mut inner_vertices = Vec::with_capacity(mesh.vertices.len());
    for i in 0..vertex_count {
        let vx = mesh.vertices[i * 3] as f64;
        let vy = mesh.vertices[i * 3 + 1] as f64;
        let vz = mesh.vertices[i * 3 + 2] as f64;

        let nx = normals[i * 3];
        let ny = normals[i * 3 + 1];
        let nz = normals[i * 3 + 2];

        inner_vertices.push((vx - thickness * nx) as f32);
        inner_vertices.push((vy - thickness * ny) as f32);
        inner_vertices.push((vz - thickness * nz) as f32);
    }

    let mut vertices = mesh.vertices.clone();
    vertices.extend(inner_vertices);

    let mut indices = mesh.indices.clone();
    let offset = vertex_count as u32;
    for tri in mesh.indices.chunks(3) {
        indices.push(tri[0] + offset);
        indices.push(tri[2] + offset);
        indices.push(tri[1] + offset);
    }

    Ok(TriangleMesh {
        vertices,
        indices,
        normals: Vec::new(),
    })
}

fn mesh_to_brep(mesh: &TriangleMesh) -> CadResult<BRepSolid> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "mesh_to_brep requires non-empty vertices and indices".to_string(),
        });
    }
    if !mesh.vertices.len().is_multiple_of(3) || !mesh.indices.len().is_multiple_of(3) {
        return Err(CadError::InvalidFeatureGraph {
            reason: "mesh_to_brep requires packed xyz vertices and triangle indices".to_string(),
        });
    }

    let mut topo = Topology::new();
    let mut geom = crate::kernel_geom::GeometryStore::default();
    let mut vertex_cache = BTreeMap::<[i64; 3], VertexId>::new();
    let mut face_ids = Vec::new();

    for triangle in mesh.indices.chunks(3) {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;

        let p0 = mesh_point(mesh, i0)?;
        let p1 = mesh_point(mesh, i1)?;
        let p2 = mesh_point(mesh, i2)?;

        let x_dir = p1 - p0;
        let y_dir = p2 - p0;
        if x_dir.norm() <= 1e-12 || y_dir.norm() <= 1e-12 {
            continue;
        }

        let v0 = get_or_create_vertex(&mut topo, &mut vertex_cache, p0);
        let v1 = get_or_create_vertex(&mut topo, &mut vertex_cache, p1);
        let v2 = get_or_create_vertex(&mut topo, &mut vertex_cache, p2);

        let surface_index = geom.add_surface(SurfaceRecord::Plane(Plane::new(p0, x_dir, y_dir)));
        let face_id = add_face(
            &mut topo,
            &[v0, v1, v2],
            surface_index,
            Orientation::Forward,
        )
        .map_err(|error| CadError::InvalidFeatureGraph {
            reason: format!("mesh_to_brep failed creating face: {error}"),
        })?;
        face_ids.push(face_id);
    }

    if face_ids.is_empty() {
        return Err(CadError::InvalidFeatureGraph {
            reason: "mesh_to_brep produced zero valid faces".to_string(),
        });
    }

    pair_twin_half_edges(&mut topo)?;
    let shell_id = topo.add_shell(face_ids, ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

fn mesh_point(mesh: &TriangleMesh, index: usize) -> CadResult<Point3> {
    let base = index
        .checked_mul(3)
        .ok_or_else(|| CadError::InvalidFeatureGraph {
            reason: "triangle vertex index overflow".to_string(),
        })?;
    if base + 2 >= mesh.vertices.len() {
        return Err(CadError::InvalidFeatureGraph {
            reason: format!("triangle vertex index out of range: {index}"),
        });
    }
    Ok(Point3::new(
        mesh.vertices[base] as f64,
        mesh.vertices[base + 1] as f64,
        mesh.vertices[base + 2] as f64,
    ))
}

fn get_or_create_vertex(
    topo: &mut Topology,
    cache: &mut BTreeMap<[i64; 3], VertexId>,
    point: Point3,
) -> VertexId {
    let key = [
        (point.x * 1e6).round() as i64,
        (point.y * 1e6).round() as i64,
        (point.z * 1e6).round() as i64,
    ];
    if let Some(vertex_id) = cache.get(&key) {
        return *vertex_id;
    }
    let vertex_id = topo.add_vertex(point);
    cache.insert(key, vertex_id);
    vertex_id
}

fn compute_vertex_normals(mesh: &TriangleMesh) -> Vec<f64> {
    let vertex_count = mesh.vertices.len() / 3;
    let mut normals = vec![0.0_f64; vertex_count * 3];

    for triangle in mesh.indices.chunks(3) {
        if triangle.len() != 3 {
            continue;
        }
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;
        if i0 >= vertex_count || i1 >= vertex_count || i2 >= vertex_count {
            continue;
        }

        let v0 = point_from_f32(mesh, i0);
        let v1 = point_from_f32(mesh, i1);
        let v2 = point_from_f32(mesh, i2);

        let e1 = vec_sub(v1, v0);
        let e2 = vec_sub(v2, v0);
        let normal = e1.cross(e2);

        accumulate_normal(&mut normals, i0, normal);
        accumulate_normal(&mut normals, i1, normal);
        accumulate_normal(&mut normals, i2, normal);
    }

    for i in 0..vertex_count {
        let nx = normals[i * 3];
        let ny = normals[i * 3 + 1];
        let nz = normals[i * 3 + 2];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 1e-12 {
            normals[i * 3] = nx / len;
            normals[i * 3 + 1] = ny / len;
            normals[i * 3 + 2] = nz / len;
        } else {
            normals[i * 3] = 0.0;
            normals[i * 3 + 1] = 0.0;
            normals[i * 3 + 2] = 1.0;
        }
    }

    normals
}

fn accumulate_normal(normals: &mut [f64], index: usize, normal: Vec3) {
    normals[index * 3] += normal.x;
    normals[index * 3 + 1] += normal.y;
    normals[index * 3 + 2] += normal.z;
}

fn point_from_f32(mesh: &TriangleMesh, index: usize) -> Vec3 {
    Vec3::new(
        mesh.vertices[index * 3] as f64,
        mesh.vertices[index * 3 + 1] as f64,
        mesh.vertices[index * 3 + 2] as f64,
    )
}

fn compute_offset_vertex(position: Point3, outward_normals: &[Vec3], thickness: f64) -> Point3 {
    if outward_normals.is_empty() {
        return position;
    }

    let mut average = Vec3::new(0.0, 0.0, 0.0);
    for normal in outward_normals {
        average = vec_add(average, *normal);
    }
    let count = outward_normals.len() as f64;
    average = vec_scale(average, 1.0 / count);

    let avg_len = average.norm();
    if avg_len <= 1e-12 {
        return position;
    }

    let direction = vec_scale(average, 1.0 / avg_len);
    let offset = vec_scale(direction, thickness / avg_len);
    position + vec_scale(offset, -1.0)
}

fn offset_plane(plane: &Plane, orientation: Orientation, thickness: f64) -> Plane {
    let mut outward = plane.normal_dir.into_inner();
    if orientation == Orientation::Reversed {
        outward = vec_scale(outward, -1.0);
    }
    let inner_origin = plane.origin + vec_scale(outward, -thickness);
    Plane::new(
        inner_origin,
        plane.x_dir.into_inner(),
        plane.y_dir.into_inner(),
    )
}

fn add_face(
    topo: &mut Topology,
    vertices: &[VertexId],
    surface_index: usize,
    orientation: Orientation,
) -> Result<FaceId, ShellError> {
    if vertices.len() < 3 {
        return Err(ShellError::SurfaceCollapse(
            FaceId(0),
            "face requires at least 3 vertices".to_string(),
        ));
    }

    let mut half_edges = Vec::with_capacity(vertices.len());
    for vertex_id in vertices {
        let half_edge = topo.add_half_edge(*vertex_id).map_err(|error| {
            ShellError::SurfaceCollapse(FaceId(0), format!("failed adding half-edge: {error}"))
        })?;
        half_edges.push(half_edge);
    }

    let loop_id = topo.add_loop(&half_edges).map_err(|error| {
        ShellError::SurfaceCollapse(FaceId(0), format!("failed creating loop: {error}"))
    })?;
    topo.add_face(loop_id, surface_index, orientation)
        .map_err(|error| {
            ShellError::SurfaceCollapse(FaceId(0), format!("failed creating face: {error}"))
        })
}

fn loop_vertices(topo: &Topology, loop_id: LoopId) -> Result<Vec<VertexId>, ShellError> {
    let loop_data = topo.loops.get(&loop_id).ok_or_else(|| {
        ShellError::SurfaceCollapse(FaceId(0), format!("missing loop {}", loop_id.0))
    })?;

    let start = loop_data.half_edge;
    let mut vertices = Vec::new();
    let mut current = start;

    for _ in 0..=topo.half_edges.len().max(1) {
        let half_edge = topo.half_edges.get(&current).ok_or_else(|| {
            ShellError::SurfaceCollapse(FaceId(0), format!("missing half-edge {}", current.0))
        })?;
        vertices.push(half_edge.origin);

        let next = half_edge.next.ok_or_else(|| {
            ShellError::SurfaceCollapse(
                FaceId(0),
                format!("half-edge {} missing next link", current.0),
            )
        })?;
        if next == start {
            return Ok(vertices);
        }
        current = next;
    }

    Err(ShellError::SurfaceCollapse(
        FaceId(0),
        format!("loop {} traversal exceeded topology bound", loop_id.0),
    ))
}

fn pair_twin_half_edges(topo: &mut Topology) -> CadResult<()> {
    let mut directed = BTreeMap::<(VertexId, VertexId), HalfEdgeId>::new();
    let half_edge_ids = topo.half_edges.keys().copied().collect::<Vec<_>>();

    for half_edge_id in half_edge_ids {
        let Some(half_edge) = topo.half_edges.get(&half_edge_id).cloned() else {
            continue;
        };
        let Some(next_id) = half_edge.next else {
            continue;
        };
        let Some(next_half_edge) = topo.half_edges.get(&next_id) else {
            continue;
        };

        let origin = half_edge.origin;
        let destination = next_half_edge.origin;

        if let Some(twin_id) = directed.remove(&(destination, origin)) {
            let can_pair = topo
                .half_edges
                .get(&half_edge_id)
                .and_then(|entry| entry.twin)
                .is_none()
                && topo
                    .half_edges
                    .get(&twin_id)
                    .and_then(|entry| entry.twin)
                    .is_none();
            if can_pair {
                topo.add_edge(half_edge_id, twin_id)?;
            }
        } else {
            directed.insert((origin, destination), half_edge_id);
        }
    }

    Ok(())
}

fn point_for_vertex(
    topo: &Topology,
    vertex_id: VertexId,
    face_id: FaceId,
) -> Result<Point3, ShellError> {
    topo.vertices
        .get(&vertex_id)
        .map(|vertex| vertex.point)
        .ok_or_else(|| {
            ShellError::SurfaceCollapse(
                face_id,
                format!("missing vertex {} in rebuilt shell", vertex_id.0),
            )
        })
}

fn topology_bounds(topo: &Topology) -> (Point3, Point3) {
    let mut min_corner = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max_corner = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    for vertex in topo.vertices.values() {
        let p = vertex.point;
        min_corner.x = min_corner.x.min(p.x);
        min_corner.y = min_corner.y.min(p.y);
        min_corner.z = min_corner.z.min(p.z);
        max_corner.x = max_corner.x.max(p.x);
        max_corner.y = max_corner.y.max(p.y);
        max_corner.z = max_corner.z.max(p.z);
    }

    if !min_corner.x.is_finite() {
        return (Point3::origin(), Point3::origin());
    }
    (min_corner, max_corner)
}

fn validate_positive_thickness(thickness: f64) -> CadResult<()> {
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: "thickness".to_string(),
            reason: "shell thickness must be finite and positive".to_string(),
        });
    }
    Ok(())
}

fn vec_scale(v: Vec3, scalar: f64) -> Vec3 {
    Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
}

fn vec_add(lhs: Vec3, rhs: Vec3) -> Vec3 {
    Vec3::new(lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z)
}

fn vec_sub(lhs: Vec3, rhs: Vec3) -> Vec3 {
    Vec3::new(lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z)
}

#[cfg(test)]
mod tests {
    use super::{ShellError, shell_brep, shell_brep_analytical, shell_mesh};
    use crate::CadError;
    use crate::kernel_primitives::{make_cube, make_cylinder, make_sphere};
    use crate::kernel_tessellate::tessellate_brep;

    #[test]
    fn shell_mesh_basic_contract() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let mesh = tessellate_brep(&cube, 16).expect("mesh");
        let shell = shell_mesh(&mesh, 1.0).expect("shell");

        assert_eq!(shell.vertices.len() / 3, (mesh.vertices.len() / 3) * 2);
        assert_eq!(shell.indices.len() / 3, (mesh.indices.len() / 3) * 2);
    }

    #[test]
    fn shell_cube_analytical_closed_contract() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let shell = shell_brep_analytical(&cube, 1.0, &[]).expect("analytical shell");

        assert_eq!(shell.topology.counts().face_count, 12);
        assert_eq!(shell.geometry.surfaces.len(), 12);

        let mut has_inner_vertex = false;
        for vertex in shell.topology.vertices.values() {
            let p = vertex.point;
            if p.x > 0.5 && p.x < 9.5 && p.y > 0.5 && p.y < 9.5 && p.z > 0.5 && p.z < 9.5 {
                has_inner_vertex = true;
            }
        }
        assert!(has_inner_vertex);
    }

    #[test]
    fn shell_cube_with_open_face_contract() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let solid = cube
            .topology
            .solids
            .get(&cube.solid_id)
            .expect("source solid should exist");
        let shell = cube
            .topology
            .shells
            .get(&solid.outer_shell)
            .expect("source shell should exist");
        let open_face = shell.faces[0];

        let shelled = shell_brep_analytical(&cube, 1.0, &[open_face]).expect("open-face shell");
        assert_eq!(shelled.topology.counts().face_count, 15);
    }

    #[test]
    fn shell_brep_falls_back_for_non_planar_solids() {
        let cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder");
        let shelled = shell_brep(&cylinder, 1.0).expect("fallback shell should succeed");
        assert!(shelled.topology.counts().face_count > 0);

        let sphere = make_sphere(10.0, 32).expect("sphere");
        let shelled_sphere = shell_brep(&sphere, 2.0).expect("fallback sphere shell");
        assert!(shelled_sphere.topology.counts().face_count > 0);
    }

    #[test]
    fn shell_analytical_detects_collapse() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let result = shell_brep_analytical(&cube, 6.0, &[]);
        assert!(result.is_err());
        assert!(matches!(result, Err(ShellError::SurfaceCollapse(_, _))));
    }

    #[test]
    fn invalid_thickness_maps_to_error_model() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let error = shell_brep(&cube, 0.0).expect_err("invalid thickness");
        assert!(matches!(error, CadError::InvalidParameter { .. }));
    }
}
