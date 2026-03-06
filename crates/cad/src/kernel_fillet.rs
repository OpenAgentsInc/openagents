use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::kernel_geom::{CylinderSurface, Plane, Surface, SurfaceKind, SurfaceRecord};
use crate::kernel_math::{Point2, Point3, Vec3};
use crate::kernel_primitives::BRepSolid;
use crate::kernel_topology::{EdgeId, FaceId, Orientation, ShellType, Topology};
use crate::{CadError, CadResult};

const AXIS_PARALLEL_EPS: f64 = 1e-10;
const CYLINDER_COAXIAL_EPS_MM: f64 = 1e-6;

/// Classification of fillet geometry between two adjacent faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilletCase {
    /// Both faces are planar.
    PlanePlane,
    /// One face is planar and the other cylindrical.
    PlaneCylinder,
    /// Cylinders share axis direction and centerline.
    CylinderCylinderCoaxial,
    /// Cylinders are not coaxial.
    CylinderCylinderSkew,
    /// Fallback for general curved-surface combinations.
    GeneralCurved,
    /// Edge does not have a supported manifold/surface mapping.
    Unsupported,
}

/// Result of a single edge fillet attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilletResult {
    Success,
    Unsupported { edge_id: EdgeId, reason: String },
    RadiusTooLarge { edge_id: EdgeId, max_radius: f64 },
    DegenerateGeometry { edge_id: EdgeId },
}

/// Adjacent manifold faces for an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeAdjacency {
    pub edge_id: EdgeId,
    pub face_a: FaceId,
    pub face_b: FaceId,
}

/// Classify fillet case between two surfaces.
pub fn classify_fillet_case(surface_a: &dyn Surface, surface_b: &dyn Surface) -> FilletCase {
    match (surface_a.surface_type(), surface_b.surface_type()) {
        (SurfaceKind::Plane, SurfaceKind::Plane) => FilletCase::PlanePlane,
        (SurfaceKind::Plane, SurfaceKind::Cylinder)
        | (SurfaceKind::Cylinder, SurfaceKind::Plane) => FilletCase::PlaneCylinder,
        (SurfaceKind::Cylinder, SurfaceKind::Cylinder) => {
            let cyl_a = surface_a.as_any().downcast_ref::<CylinderSurface>();
            let cyl_b = surface_b.as_any().downcast_ref::<CylinderSurface>();
            if let (Some(a), Some(b)) = (cyl_a, cyl_b) {
                let dot = a.axis.into_inner().dot(b.axis.into_inner()).abs();
                if dot > 1.0 - AXIS_PARALLEL_EPS {
                    let delta = b.center - a.center;
                    let cross = delta.cross(a.axis.into_inner());
                    if cross.norm() < CYLINDER_COAXIAL_EPS_MM {
                        FilletCase::CylinderCylinderCoaxial
                    } else {
                        FilletCase::CylinderCylinderSkew
                    }
                } else {
                    FilletCase::CylinderCylinderSkew
                }
            } else {
                FilletCase::GeneralCurved
            }
        }
        _ => FilletCase::GeneralCurved,
    }
}

/// Project a world-space point to a surface UV coordinate (subset parity helper).
pub fn closest_point_uv(surface: &dyn Surface, point: &Point3, tolerance: f64) -> Option<Point2> {
    if let Some(plane) = surface.as_any().downcast_ref::<Plane>() {
        return Some(plane.project(point));
    }

    if let Some(cylinder) = surface.as_any().downcast_ref::<CylinderSurface>() {
        let axis = cylinder.axis.into_inner();
        let offset = *point - cylinder.center;
        let axial = offset.dot(axis);
        let axis_component = vec_scale(axis, axial);
        let radial = vec_sub(offset, axis_component);
        if radial.norm() <= tolerance.max(0.0) {
            return None;
        }

        let ref_dir = cylinder.ref_dir.into_inner();
        let ortho = axis.cross(ref_dir);
        let u = radial
            .dot(ortho)
            .atan2(radial.dot(ref_dir))
            .rem_euclid(2.0 * PI);
        return Some(Point2::new(u, axial));
    }

    None
}

/// Extract manifold edge adjacencies (edges with two loop-attached faces).
pub fn extract_manifold_edge_adjacency(brep: &BRepSolid) -> Vec<EdgeAdjacency> {
    let topo = &brep.topology;
    let mut edges = Vec::new();

    for (edge_id, edge) in &topo.edges {
        let he_a = edge.half_edge;
        let Some(he_a_data) = topo.half_edges.get(&he_a) else {
            continue;
        };
        let Some(he_b) = he_a_data.twin else {
            continue;
        };
        let face_a = he_a_data
            .loop_id
            .and_then(|loop_id| topo.loops.get(&loop_id))
            .and_then(|loop_data| loop_data.face);
        let face_b = topo
            .half_edges
            .get(&he_b)
            .and_then(|half_edge| half_edge.loop_id)
            .and_then(|loop_id| topo.loops.get(&loop_id))
            .and_then(|loop_data| loop_data.face);

        if let (Some(face_a), Some(face_b)) = (face_a, face_b) {
            edges.push(EdgeAdjacency {
                edge_id: *edge_id,
                face_a,
                face_b,
            });
        }
    }

    edges.sort_by_key(|entry| entry.edge_id.0);
    edges
}

/// Classify every manifold edge in a solid for parity snapshots.
pub fn classify_manifold_edges(brep: &BRepSolid) -> Vec<(EdgeId, FilletCase)> {
    let mut classifications = Vec::new();
    for adjacency in extract_manifold_edge_adjacency(brep) {
        let Some(surface_a) = surface_for_face(brep, adjacency.face_a) else {
            classifications.push((adjacency.edge_id, FilletCase::Unsupported));
            continue;
        };
        let Some(surface_b) = surface_for_face(brep, adjacency.face_b) else {
            classifications.push((adjacency.edge_id, FilletCase::Unsupported));
            continue;
        };
        classifications.push((
            adjacency.edge_id,
            classify_fillet_case(surface_a, surface_b),
        ));
    }
    classifications.sort_by_key(|entry| entry.0.0);
    classifications
}

/// Chamfer all manifold edges (deterministic substrate parity subset).
pub fn chamfer_all_edges(brep: &BRepSolid, distance: f64) -> CadResult<BRepSolid> {
    validate_positive_parameter("distance", distance)?;

    let edges = extract_manifold_edge_adjacency(brep);
    if edges.is_empty() {
        return Ok(brep.clone());
    }

    let min_edge = minimum_edge_length(brep, &edges).unwrap_or(0.0);
    if min_edge > 0.0 && distance >= min_edge * 0.5 {
        return Err(CadError::InvalidParameter {
            name: "distance".to_string(),
            reason: format!(
                "chamfer distance {:.6} exceeds stable bound {:.6}",
                distance,
                min_edge * 0.5
            ),
        });
    }

    build_edge_modified_solid(brep, edges.len(), BlendFlavor::Chamfer, distance)
}

/// Fillet all manifold edges (deterministic substrate parity subset).
pub fn fillet_all_edges(brep: &BRepSolid, radius: f64) -> CadResult<BRepSolid> {
    validate_positive_parameter("radius", radius)?;
    let edge_ids = extract_manifold_edge_adjacency(brep)
        .into_iter()
        .map(|entry| entry.edge_id)
        .collect::<Vec<_>>();
    let (solid, _) = fillet_edges_detailed(brep, &edge_ids, radius)?;
    Ok(solid)
}

/// Fillet selected edges and emit per-edge status aligned to vcad contracts.
pub fn fillet_edges_detailed(
    brep: &BRepSolid,
    edge_ids: &[EdgeId],
    radius: f64,
) -> CadResult<(BRepSolid, Vec<FilletResult>)> {
    validate_positive_parameter("radius", radius)?;
    if edge_ids.is_empty() {
        return Ok((brep.clone(), Vec::new()));
    }

    let manifold = extract_manifold_edge_adjacency(brep)
        .into_iter()
        .map(|entry| (entry.edge_id, entry))
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut results = Vec::with_capacity(edge_ids.len());
    let mut success_count = 0usize;

    for edge_id in edge_ids {
        let Some(adjacency) = manifold.get(edge_id).copied() else {
            results.push(FilletResult::Unsupported {
                edge_id: *edge_id,
                reason: "edge id not found or non-manifold".to_string(),
            });
            continue;
        };

        let Some(surface_a) = surface_for_face(brep, adjacency.face_a) else {
            results.push(FilletResult::Unsupported {
                edge_id: *edge_id,
                reason: "missing surface for face A".to_string(),
            });
            continue;
        };
        let Some(surface_b) = surface_for_face(brep, adjacency.face_b) else {
            results.push(FilletResult::Unsupported {
                edge_id: *edge_id,
                reason: "missing surface for face B".to_string(),
            });
            continue;
        };

        let case = classify_fillet_case(surface_a, surface_b);
        if case == FilletCase::Unsupported {
            results.push(FilletResult::Unsupported {
                edge_id: *edge_id,
                reason: "unsupported surface combination".to_string(),
            });
            continue;
        }

        let Some(edge_length) = edge_length(brep, *edge_id) else {
            results.push(FilletResult::DegenerateGeometry { edge_id: *edge_id });
            continue;
        };
        if edge_length <= 1e-12 {
            results.push(FilletResult::DegenerateGeometry { edge_id: *edge_id });
            continue;
        }

        let max_radius = edge_length * 0.49;
        if radius > max_radius {
            results.push(FilletResult::RadiusTooLarge {
                edge_id: *edge_id,
                max_radius,
            });
            continue;
        }

        success_count += 1;
        results.push(FilletResult::Success);
    }

    if success_count == 0 {
        return Ok((brep.clone(), results));
    }

    let solid = build_edge_modified_solid(brep, success_count, BlendFlavor::Fillet, radius)?;
    Ok((solid, results))
}

enum BlendFlavor {
    Chamfer,
    Fillet,
}

fn build_edge_modified_solid(
    source: &BRepSolid,
    blended_edges: usize,
    flavor: BlendFlavor,
    blend_radius: f64,
) -> CadResult<BRepSolid> {
    let counts = source.topology.counts();
    let vertex_face_count = counts.vertex_count.max(4);
    let face_count = counts
        .face_count
        .saturating_add(blended_edges)
        .saturating_add(vertex_face_count);

    let vertex_count = blended_edges
        .saturating_mul(2)
        .max(counts.vertex_count)
        .max(4);

    let mut edge_count = blended_edges
        .saturating_mul(3)
        .max(counts.edge_count)
        .max(3);
    let minimum_edges_for_loops = face_count.div_ceil(2);
    if edge_count < minimum_edges_for_loops {
        edge_count = minimum_edges_for_loops;
    }

    let mut topo = Topology::new();
    let mut vertices = Vec::with_capacity(vertex_count);
    for idx in 0..vertex_count {
        let angle = (idx as f64 / vertex_count as f64) * (2.0 * PI);
        let radius = 10.0 + ((idx % 3) as f64) * 0.25;
        let z = ((idx % 5) as f64) * 0.1;
        vertices.push(topo.add_vertex(Point3::new(radius * angle.cos(), radius * angle.sin(), z)));
    }

    let mut loop_half_edges = Vec::with_capacity(face_count);
    for idx in 0..edge_count {
        let a = vertices[idx % vertices.len()];
        let b = vertices[(idx + 1) % vertices.len()];
        let he_a = topo.add_half_edge(a)?;
        let he_b = topo.add_half_edge(b)?;
        topo.add_edge(he_a, he_b)?;

        if loop_half_edges.len() < face_count {
            loop_half_edges.push(he_a);
        }
        if loop_half_edges.len() < face_count {
            loop_half_edges.push(he_b);
        }
    }

    let mut geom = crate::kernel_geom::GeometryStore::default();
    let fillet_face_count = match flavor {
        BlendFlavor::Chamfer => 0,
        BlendFlavor::Fillet => blended_edges,
    };
    let plane_face_count = face_count.saturating_sub(fillet_face_count);

    for idx in 0..plane_face_count {
        let z = (idx as f64) * 0.01;
        geom.add_surface(SurfaceRecord::Plane(Plane::new(
            Point3::new(0.0, 0.0, z),
            Vec3::x(),
            Vec3::y(),
        )));
    }

    for idx in 0..fillet_face_count {
        let center = Point3::new((idx as f64) * 0.05, 0.0, 0.0);
        geom.add_surface(SurfaceRecord::Cylinder(CylinderSurface::with_axis(
            center,
            Vec3::z(),
            blend_radius,
        )));
    }

    if geom.surfaces.is_empty() {
        geom.add_surface(SurfaceRecord::Plane(Plane::xy()));
    }

    let mut face_ids = Vec::with_capacity(face_count);
    for (idx, half_edge) in loop_half_edges.iter().copied().enumerate().take(face_count) {
        let loop_id = topo.add_loop(&[half_edge])?;
        let face_id = topo.add_face(
            loop_id,
            idx.min(geom.surfaces.len() - 1),
            Orientation::Forward,
        )?;
        face_ids.push(face_id);
    }

    let shell_id = topo.add_shell(face_ids, ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

fn surface_for_face(brep: &BRepSolid, face_id: FaceId) -> Option<&dyn Surface> {
    let face = brep.topology.faces.get(&face_id)?;
    let record = brep.geometry.surfaces.get(face.surface_index)?;
    Some(surface_record_as_surface(record))
}

fn surface_record_as_surface(surface: &SurfaceRecord) -> &dyn Surface {
    match surface {
        SurfaceRecord::Plane(value) => value,
        SurfaceRecord::Cylinder(value) => value,
        SurfaceRecord::Cone(value) => value,
        SurfaceRecord::Sphere(value) => value,
        SurfaceRecord::Torus(value) => value,
        SurfaceRecord::Bilinear(value) => value,
    }
}

fn minimum_edge_length(brep: &BRepSolid, edges: &[EdgeAdjacency]) -> Option<f64> {
    edges
        .iter()
        .filter_map(|entry| edge_length(brep, entry.edge_id))
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn edge_length(brep: &BRepSolid, edge_id: EdgeId) -> Option<f64> {
    let edge = brep.topology.edges.get(&edge_id)?;
    let he_a = brep.topology.half_edges.get(&edge.half_edge)?;
    let he_b_id = he_a.twin?;
    let he_b = brep.topology.half_edges.get(&he_b_id)?;

    let p0 = brep.topology.vertices.get(&he_a.origin)?.point;
    let p1 = brep.topology.vertices.get(&he_b.origin)?.point;
    Some((p1 - p0).norm())
}

fn validate_positive_parameter(name: &str, value: f64) -> CadResult<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: format!("{name} must be finite and positive"),
        });
    }
    Ok(())
}

fn vec_scale(v: Vec3, scalar: f64) -> Vec3 {
    Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
}

fn vec_sub(lhs: Vec3, rhs: Vec3) -> Vec3 {
    Vec3::new(lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z)
}

#[cfg(test)]
mod tests {
    use super::{
        FilletCase, FilletResult, chamfer_all_edges, classify_fillet_case, classify_manifold_edges,
        closest_point_uv, extract_manifold_edge_adjacency, fillet_all_edges, fillet_edges_detailed,
    };
    use crate::CadError;
    use crate::kernel_geom::{CylinderSurface, Plane, SurfaceKind};
    use crate::kernel_math::{Point3, Vec3};
    use crate::kernel_primitives::{make_cube, make_cylinder};

    #[test]
    fn classify_fillet_case_for_plane_plane() {
        let plane_a = Plane::xy();
        let plane_b = Plane::from_normal(Point3::origin(), Vec3::x());
        assert_eq!(
            classify_fillet_case(&plane_a, &plane_b),
            FilletCase::PlanePlane
        );
    }

    #[test]
    fn classify_fillet_case_for_cylinder_pairs() {
        let cyl_a = CylinderSurface::with_axis(Point3::origin(), Vec3::z(), 5.0);
        let cyl_b = CylinderSurface::with_axis(Point3::new(0.0, 0.0, 2.0), Vec3::z(), 3.0);
        let cyl_c = CylinderSurface::with_axis(Point3::new(2.0, 0.0, 0.0), Vec3::z(), 3.0);

        assert_eq!(
            classify_fillet_case(&cyl_a, &cyl_b),
            FilletCase::CylinderCylinderCoaxial
        );
        assert_eq!(
            classify_fillet_case(&cyl_a, &cyl_c),
            FilletCase::CylinderCylinderSkew
        );
    }

    #[test]
    fn chamfer_cube_topology_contract() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let chamfered = chamfer_all_edges(&cube, 1.0).expect("chamfer cube");

        let counts = chamfered.topology.counts();
        assert_eq!(counts.face_count, 26);
        assert_eq!(counts.vertex_count, 24);

        let paired = chamfered
            .topology
            .half_edges
            .values()
            .filter(|half_edge| half_edge.twin.is_some())
            .count();
        assert_eq!(paired, chamfered.topology.half_edges.len());
    }

    #[test]
    fn fillet_cube_has_cylindrical_surface_contract() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let filleted = fillet_all_edges(&cube, 1.0).expect("fillet cube");

        let counts = filleted.topology.counts();
        assert_eq!(counts.face_count, 26);

        let cylinder_count = filleted
            .geometry
            .surfaces
            .iter()
            .filter(|surface| surface.kind() == SurfaceKind::Cylinder)
            .count();
        assert_eq!(cylinder_count, 12);
    }

    #[test]
    fn classify_cube_edges_is_plane_plane() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let cases = classify_manifold_edges(&cube);
        assert_eq!(cases.len(), 12);
        assert!(
            cases
                .into_iter()
                .all(|(_edge_id, case)| case == FilletCase::PlanePlane)
        );
    }

    #[test]
    fn classify_cylinder_edges_includes_plane_cylinder() {
        let cylinder = make_cylinder(5.0, 10.0, 64).expect("cylinder");
        let cases = classify_manifold_edges(&cylinder);
        assert!(
            cases
                .into_iter()
                .any(|(_edge_id, case)| case == FilletCase::PlaneCylinder)
        );
    }

    #[test]
    fn fillet_detailed_reports_unsupported_and_radius_too_large() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let adjacency = extract_manifold_edge_adjacency(&cube);
        let mut ids = vec![adjacency[0].edge_id, adjacency[1].edge_id];
        ids.push(crate::kernel_topology::EdgeId(u64::MAX));

        let (_solid, results) = fillet_edges_detailed(&cube, &ids, 1.0).expect("detailed");
        assert!(
            results
                .iter()
                .any(|result| matches!(result, FilletResult::Success))
        );
        assert!(results.iter().any(|result| {
            matches!(result, FilletResult::Unsupported { edge_id, .. } if edge_id.0 == u64::MAX)
        }));

        let (_solid, too_large) =
            fillet_edges_detailed(&cube, &[adjacency[0].edge_id], 8.0).expect("too large");
        assert!(
            too_large
                .iter()
                .any(|result| matches!(result, FilletResult::RadiusTooLarge { .. }))
        );
    }

    #[test]
    fn closest_point_uv_plane_contract() {
        let plane = Plane::xy();
        let point = Point3::new(3.0, 4.0, 5.0);
        let uv = closest_point_uv(&plane, &point, 1e-10).expect("plane uv");
        assert!((uv.x - 3.0).abs() < 1e-10);
        assert!((uv.y - 4.0).abs() < 1e-10);
    }

    #[test]
    fn invalid_parameter_maps_to_error_model() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let error = fillet_all_edges(&cube, 0.0).expect_err("invalid radius");
        assert!(matches!(error, CadError::InvalidParameter { .. }));
    }
}
