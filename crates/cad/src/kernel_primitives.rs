use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::kernel_geom::{
    ConeSurface, CylinderSurface, GeometryStore, Plane, SphereSurface, SurfaceRecord,
};
use crate::kernel_math::{Point3, Vec3};
use crate::kernel_topology::{HalfEdgeId, Orientation, ShellType, SolidId, Topology, VertexId};
use crate::policy::{CANONICAL_UNIT, MIN_POSITIVE_DIMENSION_MM};
use crate::{CadError, CadResult};

const RADIAL_EQ_EPSILON: f64 = 1e-12;

/// Result of constructing a B-rep primitive: topology + geometry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BRepSolid {
    /// The topological structure.
    pub topology: Topology,
    /// The geometric data (surfaces only in this parity lane).
    pub geometry: GeometryStore,
    /// The owning solid entity.
    pub solid_id: SolidId,
}

/// Build a B-rep box (cuboid) with corner at origin and dimensions `(sx, sy, sz)`.
pub fn make_cube(sx: f64, sy: f64, sz: f64) -> CadResult<BRepSolid> {
    validate_positive_dimension("cube sx", sx)?;
    validate_positive_dimension("cube sy", sy)?;
    validate_positive_dimension("cube sz", sz)?;

    let mut topo = Topology::new();
    let mut geom = GeometryStore::default();

    let v0 = topo.add_vertex(Point3::new(0.0, 0.0, 0.0));
    let v1 = topo.add_vertex(Point3::new(sx, 0.0, 0.0));
    let v2 = topo.add_vertex(Point3::new(sx, sy, 0.0));
    let v3 = topo.add_vertex(Point3::new(0.0, sy, 0.0));
    let v4 = topo.add_vertex(Point3::new(0.0, 0.0, sz));
    let v5 = topo.add_vertex(Point3::new(sx, 0.0, sz));
    let v6 = topo.add_vertex(Point3::new(sx, sy, sz));
    let v7 = topo.add_vertex(Point3::new(0.0, sy, sz));

    // Vertex order is CCW when viewed from outside each face.
    let face_defs: [([VertexId; 4], Point3, Vec3, Vec3); 6] = [
        (
            [v0, v3, v2, v1],
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            [v4, v5, v6, v7],
            Point3::new(0.0, 0.0, sz),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [v0, v1, v5, v4],
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            [v2, v3, v7, v6],
            Point3::new(0.0, sy, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            [v0, v4, v7, v3],
            Point3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [v1, v2, v6, v5],
            Point3::new(sx, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
    ];

    let mut face_ids = Vec::with_capacity(face_defs.len());
    let mut half_edges = BTreeMap::<(VertexId, VertexId), HalfEdgeId>::new();

    for (verts, plane_origin, x_dir, y_dir) in face_defs {
        let surface_index =
            geom.add_surface(SurfaceRecord::Plane(Plane::new(plane_origin, x_dir, y_dir)));

        let mut loop_half_edges = Vec::with_capacity(4);
        for index in 0..4 {
            let he = topo.add_half_edge(verts[index])?;
            loop_half_edges.push(he);
            half_edges.insert((verts[index], verts[(index + 1) % 4]), he);
        }

        let loop_id = topo.add_loop(&loop_half_edges)?;
        let face_id = topo.add_face(loop_id, surface_index, Orientation::Forward)?;
        face_ids.push(face_id);
    }

    let mut paired = BTreeSet::<(VertexId, VertexId)>::new();
    for ((from, to), he_ab) in &half_edges {
        if paired.contains(&(*to, *from)) {
            continue;
        }
        if let Some(he_ba) = half_edges.get(&(*to, *from)) {
            topo.add_edge(*he_ab, *he_ba)?;
            paired.insert((*from, *to));
        }
    }

    let shell_id = topo.add_shell(face_ids, ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

/// Build a B-rep cylinder with the given radius and height (axis along +Z).
///
/// `segments` is accepted for vcad constructor parity, but does not affect
/// topological structure in this substrate lane.
pub fn make_cylinder(radius: f64, height: f64, _segments: u32) -> CadResult<BRepSolid> {
    validate_positive_dimension("cylinder radius", radius)?;
    validate_positive_dimension("cylinder height", height)?;

    let mut topo = Topology::new();
    let mut geom = GeometryStore::default();

    let v_bot = topo.add_vertex(Point3::new(radius, 0.0, 0.0));
    let v_top = topo.add_vertex(Point3::new(radius, 0.0, height));

    let cyl_idx = geom.add_surface(SurfaceRecord::Cylinder(CylinderSurface::new(radius)));
    let bot_idx = geom.add_surface(SurfaceRecord::Plane(Plane::new(
        Point3::origin(),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
    )));
    let top_idx = geom.add_surface(SurfaceRecord::Plane(Plane::new(
        Point3::new(0.0, 0.0, height),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    )));

    let he_bot_lat = topo.add_half_edge(v_bot)?;
    let he_seam_up = topo.add_half_edge(v_bot)?;
    let he_top_lat = topo.add_half_edge(v_top)?;
    let he_seam_down = topo.add_half_edge(v_top)?;
    let lat_loop = topo.add_loop(&[he_bot_lat, he_seam_up, he_top_lat, he_seam_down])?;
    let lat_face = topo.add_face(lat_loop, cyl_idx, Orientation::Forward)?;

    let he_bot_cap = topo.add_half_edge(v_bot)?;
    let bot_loop = topo.add_loop(&[he_bot_cap])?;
    let bot_face = topo.add_face(bot_loop, bot_idx, Orientation::Forward)?;

    let he_top_cap = topo.add_half_edge(v_top)?;
    let top_loop = topo.add_loop(&[he_top_cap])?;
    let top_face = topo.add_face(top_loop, top_idx, Orientation::Forward)?;

    topo.add_edge(he_bot_lat, he_bot_cap)?;
    topo.add_edge(he_top_lat, he_top_cap)?;
    topo.add_edge(he_seam_up, he_seam_down)?;

    let shell_id = topo.add_shell(vec![lat_face, bot_face, top_face], ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

/// Build a B-rep sphere with the given radius, centered at origin.
///
/// `segments` is accepted for vcad constructor parity, but does not affect
/// topological structure in this substrate lane.
pub fn make_sphere(radius: f64, _segments: u32) -> CadResult<BRepSolid> {
    validate_positive_dimension("sphere radius", radius)?;

    let mut topo = Topology::new();
    let mut geom = GeometryStore::default();

    let surf_idx = geom.add_surface(SurfaceRecord::Sphere(SphereSurface::new(
        Point3::origin(),
        radius,
    )));

    let v_north = topo.add_vertex(Point3::new(0.0, 0.0, radius));
    let v_south = topo.add_vertex(Point3::new(0.0, 0.0, -radius));
    let _v_seam = topo.add_vertex(Point3::new(radius, 0.0, 0.0));

    let he_seam_up = topo.add_half_edge(v_south)?;
    let he_north_degen = topo.add_half_edge(v_north)?;
    let he_seam_down = topo.add_half_edge(v_north)?;
    let he_south_degen = topo.add_half_edge(v_south)?;

    let sphere_loop = topo.add_loop(&[he_south_degen, he_seam_up, he_north_degen, he_seam_down])?;
    let sphere_face = topo.add_face(sphere_loop, surf_idx, Orientation::Forward)?;

    topo.add_edge(he_seam_up, he_seam_down)?;
    topo.add_edge(he_north_degen, he_south_degen)?;

    let shell_id = topo.add_shell(vec![sphere_face], ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

/// Build a B-rep cone (or frustum) with bottom radius, top radius, and height.
///
/// If radii are equal (within epsilon), this routes to [`make_cylinder`].
/// If `radius_top` is zero (within epsilon), this constructs a pointed cone.
pub fn make_cone(
    radius_bottom: f64,
    radius_top: f64,
    height: f64,
    segments: u32,
) -> CadResult<BRepSolid> {
    validate_positive_dimension("cone radius_bottom", radius_bottom)?;
    validate_non_negative_dimension("cone radius_top", radius_top)?;
    validate_positive_dimension("cone height", height)?;

    if (radius_bottom - radius_top).abs() < RADIAL_EQ_EPSILON {
        return make_cylinder(radius_bottom, height, segments);
    }

    let mut topo = Topology::new();
    let mut geom = GeometryStore::default();

    let cone_idx = geom.add_surface(SurfaceRecord::Cone(cone_surface_from_frustum(
        radius_bottom,
        radius_top,
        height,
    )));
    let bot_idx = geom.add_surface(SurfaceRecord::Plane(Plane::new(
        Point3::origin(),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
    )));

    let is_pointed = radius_top < RADIAL_EQ_EPSILON;

    if is_pointed {
        let v_bot = topo.add_vertex(Point3::new(radius_bottom, 0.0, 0.0));
        let v_apex = topo.add_vertex(Point3::new(0.0, 0.0, height));

        let he_bot_lat = topo.add_half_edge(v_bot)?;
        let he_seam_up = topo.add_half_edge(v_bot)?;
        let he_seam_down = topo.add_half_edge(v_apex)?;
        let lat_loop = topo.add_loop(&[he_bot_lat, he_seam_up, he_seam_down])?;
        let lat_face = topo.add_face(lat_loop, cone_idx, Orientation::Forward)?;

        let he_bot_cap = topo.add_half_edge(v_bot)?;
        let bot_loop = topo.add_loop(&[he_bot_cap])?;
        let bot_face = topo.add_face(bot_loop, bot_idx, Orientation::Forward)?;

        topo.add_edge(he_bot_lat, he_bot_cap)?;
        topo.add_edge(he_seam_up, he_seam_down)?;

        let shell_id = topo.add_shell(vec![lat_face, bot_face], ShellType::Outer)?;
        let solid_id = topo.add_solid(shell_id)?;
        return Ok(BRepSolid {
            topology: topo,
            geometry: geom,
            solid_id,
        });
    }

    let top_idx = geom.add_surface(SurfaceRecord::Plane(Plane::new(
        Point3::new(0.0, 0.0, height),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    )));

    let v_bot = topo.add_vertex(Point3::new(radius_bottom, 0.0, 0.0));
    let v_top = topo.add_vertex(Point3::new(radius_top, 0.0, height));

    let he_bot_lat = topo.add_half_edge(v_bot)?;
    let he_seam_up = topo.add_half_edge(v_bot)?;
    let he_top_lat = topo.add_half_edge(v_top)?;
    let he_seam_down = topo.add_half_edge(v_top)?;
    let lat_loop = topo.add_loop(&[he_bot_lat, he_seam_up, he_top_lat, he_seam_down])?;
    let lat_face = topo.add_face(lat_loop, cone_idx, Orientation::Forward)?;

    let he_bot_cap = topo.add_half_edge(v_bot)?;
    let bot_loop = topo.add_loop(&[he_bot_cap])?;
    let bot_face = topo.add_face(bot_loop, bot_idx, Orientation::Forward)?;

    let he_top_cap = topo.add_half_edge(v_top)?;
    let top_loop = topo.add_loop(&[he_top_cap])?;
    let top_face = topo.add_face(top_loop, top_idx, Orientation::Forward)?;

    topo.add_edge(he_bot_lat, he_bot_cap)?;
    topo.add_edge(he_top_lat, he_top_cap)?;
    topo.add_edge(he_seam_up, he_seam_down)?;

    let shell_id = topo.add_shell(vec![lat_face, bot_face, top_face], ShellType::Outer)?;
    let solid_id = topo.add_solid(shell_id)?;

    Ok(BRepSolid {
        topology: topo,
        geometry: geom,
        solid_id,
    })
}

fn cone_surface_from_frustum(radius_bottom: f64, radius_top: f64, height: f64) -> ConeSurface {
    let delta = radius_top - radius_bottom;
    let half_angle_rad = (delta.abs() / height).atan();

    if delta < 0.0 {
        // Radius shrinks with +Z: apex above top, axis points downward.
        let apex_z = height + (radius_top * height / (radius_bottom - radius_top));
        ConeSurface::new(
            Point3::new(0.0, 0.0, apex_z),
            Vec3::new(0.0, 0.0, -1.0),
            half_angle_rad,
        )
    } else {
        // Radius grows with +Z: apex below bottom, axis points upward.
        let apex_z = -(radius_bottom * height / (radius_top - radius_bottom));
        ConeSurface::new(Point3::new(0.0, 0.0, apex_z), Vec3::z(), half_angle_rad)
    }
}

fn validate_positive_dimension(name: &str, value: f64) -> CadResult<()> {
    if !value.is_finite() {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{name} must be finite"),
        });
    }
    if value <= MIN_POSITIVE_DIMENSION_MM {
        return Err(CadError::InvalidPrimitive {
            reason: format!(
                "{name} must be greater than tolerance {} {}",
                MIN_POSITIVE_DIMENSION_MM, CANONICAL_UNIT
            ),
        });
    }
    Ok(())
}

fn validate_non_negative_dimension(name: &str, value: f64) -> CadResult<()> {
    if !value.is_finite() {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{name} must be finite"),
        });
    }
    if value < 0.0 {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{name} must be non-negative {}", CANONICAL_UNIT),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{make_cone, make_cube, make_cylinder, make_sphere};
    use crate::CadError;

    #[test]
    fn cube_topology_and_geometry_counts_match_vcad_shape_contract() {
        let solid = make_cube(10.0, 20.0, 30.0).expect("cube should build");
        let counts = solid.topology.counts();
        assert_eq!(counts.vertex_count, 8);
        assert_eq!(counts.half_edge_count, 24);
        assert_eq!(counts.edge_count, 12);
        assert_eq!(counts.loop_count, 6);
        assert_eq!(counts.face_count, 6);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
        assert_eq!(solid.geometry.surfaces.len(), 6);
    }

    #[test]
    fn cylinder_topology_and_geometry_counts_match_vcad_shape_contract() {
        let solid = make_cylinder(5.0, 10.0, 32).expect("cylinder should build");
        let counts = solid.topology.counts();
        assert_eq!(counts.vertex_count, 2);
        assert_eq!(counts.half_edge_count, 6);
        assert_eq!(counts.edge_count, 3);
        assert_eq!(counts.loop_count, 3);
        assert_eq!(counts.face_count, 3);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
        assert_eq!(solid.geometry.surfaces.len(), 3);
    }

    #[test]
    fn sphere_topology_and_geometry_counts_match_vcad_shape_contract() {
        let solid = make_sphere(10.0, 32).expect("sphere should build");
        let counts = solid.topology.counts();
        assert_eq!(counts.vertex_count, 3);
        assert_eq!(counts.half_edge_count, 4);
        assert_eq!(counts.edge_count, 2);
        assert_eq!(counts.loop_count, 1);
        assert_eq!(counts.face_count, 1);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
        assert_eq!(solid.geometry.surfaces.len(), 1);
    }

    #[test]
    fn pointed_cone_topology_counts_match_vcad_shape_contract() {
        let solid = make_cone(5.0, 0.0, 10.0, 32).expect("pointed cone should build");
        let counts = solid.topology.counts();
        assert_eq!(counts.vertex_count, 2);
        assert_eq!(counts.half_edge_count, 4);
        assert_eq!(counts.edge_count, 2);
        assert_eq!(counts.loop_count, 2);
        assert_eq!(counts.face_count, 2);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
        assert_eq!(solid.geometry.surfaces.len(), 2);
    }

    #[test]
    fn frustum_cone_topology_counts_match_vcad_shape_contract() {
        let solid = make_cone(5.0, 3.0, 10.0, 32).expect("frustum cone should build");
        let counts = solid.topology.counts();
        assert_eq!(counts.vertex_count, 2);
        assert_eq!(counts.half_edge_count, 6);
        assert_eq!(counts.edge_count, 3);
        assert_eq!(counts.loop_count, 3);
        assert_eq!(counts.face_count, 3);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
        assert_eq!(solid.geometry.surfaces.len(), 3);
    }

    #[test]
    fn equal_radii_cone_routes_to_cylinder_contract() {
        let cone = make_cone(5.0, 5.0, 10.0, 32).expect("cone should route to cylinder");
        let cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder should build");
        assert_eq!(cone.topology.counts(), cylinder.topology.counts());
        assert_eq!(
            cone.geometry.surfaces.len(),
            cylinder.geometry.surfaces.len()
        );
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        let invalid_cube = make_cube(0.0, 2.0, 3.0).expect_err("cube with zero width must fail");
        assert!(matches!(invalid_cube, CadError::InvalidPrimitive { .. }));

        let invalid_cylinder =
            make_cylinder(1.0, 0.0, 8).expect_err("cylinder with zero height must fail");
        assert!(matches!(
            invalid_cylinder,
            CadError::InvalidPrimitive { .. }
        ));

        let invalid_cone =
            make_cone(1.0, -1.0, 5.0, 16).expect_err("cone with negative top radius must fail");
        assert!(matches!(invalid_cone, CadError::InvalidPrimitive { .. }));
    }
}
