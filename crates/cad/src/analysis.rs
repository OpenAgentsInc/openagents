use crate::mesh::CadMeshPayload;

pub const DENSITY_ALUMINUM_6061_KG_M3: f64 = 2_700.0;

/// Deterministic body property estimate derived from mesh payload data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadBodyProperties {
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub mass_kg: f64,
    pub center_of_gravity_mm: [f64; 3],
    pub bounds_min_mm: [f64; 3],
    pub bounds_max_mm: [f64; 3],
    pub bounds_size_mm: [f64; 3],
}

/// Estimate body properties from a tessellated mesh and material density.
pub fn estimate_body_properties(
    payload: &CadMeshPayload,
    density_kg_m3: f64,
) -> Option<CadBodyProperties> {
    if payload.vertices.is_empty() || payload.triangle_indices.len() < 3 || !density_kg_m3.is_finite() {
        return None;
    }
    if density_kg_m3 <= 0.0 {
        return None;
    }

    let bounds_min_mm = [
        f64::from(payload.bounds.min_mm[0]),
        f64::from(payload.bounds.min_mm[1]),
        f64::from(payload.bounds.min_mm[2]),
    ];
    let bounds_max_mm = [
        f64::from(payload.bounds.max_mm[0]),
        f64::from(payload.bounds.max_mm[1]),
        f64::from(payload.bounds.max_mm[2]),
    ];
    let bounds_size_mm = [
        (bounds_max_mm[0] - bounds_min_mm[0]).max(0.0),
        (bounds_max_mm[1] - bounds_min_mm[1]).max(0.0),
        (bounds_max_mm[2] - bounds_min_mm[2]).max(0.0),
    ];
    let bounds_center_mm = [
        (bounds_min_mm[0] + bounds_max_mm[0]) * 0.5,
        (bounds_min_mm[1] + bounds_max_mm[1]) * 0.5,
        (bounds_min_mm[2] + bounds_max_mm[2]) * 0.5,
    ];

    let mut surface_area_mm2 = 0.0f64;
    let mut signed_volume_mm3 = 0.0f64;
    let mut centroid_accum = [0.0f64; 3];

    for triangle in payload.triangle_indices.chunks_exact(3) {
        let p0 = vertex3(payload, triangle[0] as usize)?;
        let p1 = vertex3(payload, triangle[1] as usize)?;
        let p2 = vertex3(payload, triangle[2] as usize)?;

        let edge1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let edge2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let cross = cross3(edge1, edge2);
        let tri_area = 0.5 * length3(cross);
        surface_area_mm2 += tri_area;

        let tetra_volume = dot3(p0, cross3(p1, p2)) / 6.0;
        signed_volume_mm3 += tetra_volume;
        let tetra_centroid = [
            (p0[0] + p1[0] + p2[0]) * 0.25,
            (p0[1] + p1[1] + p2[1]) * 0.25,
            (p0[2] + p1[2] + p2[2]) * 0.25,
        ];
        centroid_accum[0] += tetra_centroid[0] * tetra_volume;
        centroid_accum[1] += tetra_centroid[1] * tetra_volume;
        centroid_accum[2] += tetra_centroid[2] * tetra_volume;
    }

    let volume_mm3 = signed_volume_mm3.abs();
    let center_of_gravity_mm = if signed_volume_mm3.abs() > 1e-9 {
        [
            centroid_accum[0] / signed_volume_mm3,
            centroid_accum[1] / signed_volume_mm3,
            centroid_accum[2] / signed_volume_mm3,
        ]
    } else {
        bounds_center_mm
    };
    let volume_m3 = volume_mm3 * 1e-9;
    let mass_kg = density_kg_m3 * volume_m3;

    Some(CadBodyProperties {
        volume_mm3,
        surface_area_mm2,
        mass_kg,
        center_of_gravity_mm,
        bounds_min_mm,
        bounds_max_mm,
        bounds_size_mm,
    })
}

fn vertex3(payload: &CadMeshPayload, index: usize) -> Option<[f64; 3]> {
    let vertex = payload.vertices.get(index)?;
    Some([
        f64::from(vertex.position_mm[0]),
        f64::from(vertex.position_mm[1]),
        f64::from(vertex.position_mm[2]),
    ])
}

fn cross3(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [
        lhs[1] * rhs[2] - lhs[2] * rhs[1],
        lhs[2] * rhs[0] - lhs[0] * rhs[2],
        lhs[0] * rhs[1] - lhs[1] * rhs[0],
    ]
}

fn dot3(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    lhs[0] * rhs[0] + lhs[1] * rhs[1] + lhs[2] * rhs[2]
}

fn length3(vector: [f64; 3]) -> f64 {
    dot3(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{DENSITY_ALUMINUM_6061_KG_M3, estimate_body_properties};
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn tetra_mesh_payload() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.tetra".to_string(),
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
                    position_mm: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![
                0, 2, 1, //
                0, 1, 3, //
                0, 3, 2, //
                1, 2, 3, //
            ],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [1.0, 1.0, 1.0],
            },
        }
    }

    #[test]
    fn body_properties_estimate_is_deterministic_for_closed_tetrahedron() {
        let payload = tetra_mesh_payload();
        let first = estimate_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("tetra estimate should succeed");
        let second = estimate_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("tetra estimate should stay deterministic");

        assert_eq!(first, second);
        assert!((first.volume_mm3 - (1.0 / 6.0)).abs() < 1e-9);
        assert!((first.center_of_gravity_mm[0] - 0.25).abs() < 1e-9);
        assert!((first.center_of_gravity_mm[1] - 0.25).abs() < 1e-9);
        assert!((first.center_of_gravity_mm[2] - 0.25).abs() < 1e-9);
        assert!((first.mass_kg - 4.5e-7).abs() < 1e-10);
    }

    #[test]
    fn invalid_density_or_payload_returns_none() {
        let payload = tetra_mesh_payload();
        assert!(estimate_body_properties(&payload, 0.0).is_none());

        let mut empty = payload;
        empty.vertices.clear();
        assert!(estimate_body_properties(&empty, DENSITY_ALUMINUM_6061_KG_M3).is_none());
    }
}
