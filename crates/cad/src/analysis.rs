use std::collections::BTreeMap;

use crate::mesh::CadMeshPayload;

pub const DENSITY_ALUMINUM_6061_KG_M3: f64 = 2_700.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadBodyAnalysisErrorCode {
    EmptyVertices,
    EmptyTriangles,
    MalformedTriangles,
    InvalidDensity,
    MissingVertex,
    NonFiniteVertex,
}

impl CadBodyAnalysisErrorCode {
    pub fn stable_code(self) -> &'static str {
        match self {
            Self::EmptyVertices => "CAD-ANALYSIS-EMPTY-VERTICES",
            Self::EmptyTriangles => "CAD-ANALYSIS-EMPTY-TRIANGLES",
            Self::MalformedTriangles => "CAD-ANALYSIS-MALFORMED-TRIANGLES",
            Self::InvalidDensity => "CAD-ANALYSIS-INVALID-DENSITY",
            Self::MissingVertex => "CAD-ANALYSIS-MISSING-VERTEX",
            Self::NonFiniteVertex => "CAD-ANALYSIS-NONFINITE-VERTEX",
        }
    }

    pub fn remediation_hint(self) -> &'static str {
        match self {
            Self::EmptyVertices => "Rebuild geometry before requesting physical analysis.",
            Self::EmptyTriangles => "Ensure tessellation produced triangle geometry.",
            Self::MalformedTriangles => "Re-run tessellation and verify index buffer grouping.",
            Self::InvalidDensity => "Use a finite material density greater than zero.",
            Self::MissingVertex => {
                "Rebuild mesh and verify triangle indices reference valid vertices."
            }
            Self::NonFiniteVertex => "Repair geometry so all vertex positions are finite values.",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadBodyAnalysisError {
    pub code: CadBodyAnalysisErrorCode,
    pub message: String,
}

impl CadBodyAnalysisError {
    fn new(code: CadBodyAnalysisErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn remediation_hint(&self) -> &'static str {
        self.code.remediation_hint()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadCenterOfGravitySource {
    MeshVolume,
    BoundsCenterFallback,
}

impl CadCenterOfGravitySource {
    pub fn label(self) -> &'static str {
        match self {
            Self::MeshVolume => "mesh_volume",
            Self::BoundsCenterFallback => "bounds_center_fallback",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadBodyAnalysisReceipt {
    pub properties: CadBodyProperties,
    pub center_of_gravity_source: CadCenterOfGravitySource,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

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

pub const CAD_DEFLECTION_HEURISTIC_MODEL_ID: &str = "cad.deflection.wave1.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadDeflectionConfidence {
    Low,
    Medium,
}

impl CadDeflectionConfidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadDeflectionHeuristicInput {
    pub span_mm: f64,
    pub width_mm: f64,
    pub thickness_mm: f64,
    pub load_kg: f64,
    pub youngs_modulus_gpa: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadDeflectionHeuristicErrorCode {
    InvalidSpan,
    InvalidWidth,
    InvalidThickness,
    InvalidLoad,
    InvalidElasticModulus,
}

impl CadDeflectionHeuristicErrorCode {
    pub fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidSpan => "CAD-DEFLECTION-INVALID-SPAN",
            Self::InvalidWidth => "CAD-DEFLECTION-INVALID-WIDTH",
            Self::InvalidThickness => "CAD-DEFLECTION-INVALID-THICKNESS",
            Self::InvalidLoad => "CAD-DEFLECTION-INVALID-LOAD",
            Self::InvalidElasticModulus => "CAD-DEFLECTION-INVALID-MODULUS",
        }
    }

    pub fn remediation_hint(self) -> &'static str {
        match self {
            Self::InvalidSpan => "Provide a finite positive span for beam deflection estimation.",
            Self::InvalidWidth => {
                "Provide a finite positive beam width for area moment calculation."
            }
            Self::InvalidThickness => {
                "Provide a finite positive thickness; near-zero thickness invalidates stiffness estimate."
            }
            Self::InvalidLoad => "Provide a finite non-negative load in kilograms.",
            Self::InvalidElasticModulus => {
                "Provide a finite positive Young's modulus for the selected material."
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadDeflectionHeuristicError {
    pub code: CadDeflectionHeuristicErrorCode,
    pub message: String,
}

impl CadDeflectionHeuristicError {
    fn new(code: CadDeflectionHeuristicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn remediation_hint(&self) -> &'static str {
        self.code.remediation_hint()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CadDeflectionHeuristicEstimate {
    pub max_deflection_mm: f64,
    pub confidence: CadDeflectionConfidence,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadEdgeType {
    Segment,
    Tagged,
}

impl CadEdgeType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Segment => "segment",
            Self::Tagged => "tagged",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadFaceProperties {
    pub face_index: usize,
    pub area_mm2: f64,
    pub normal: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadEdgeProperties {
    pub edge_index: usize,
    pub length_mm: f64,
    pub edge_type: CadEdgeType,
}

/// Estimate body properties from a tessellated mesh and material density.
pub fn estimate_body_properties(
    payload: &CadMeshPayload,
    density_kg_m3: f64,
) -> Option<CadBodyProperties> {
    analyze_body_properties(payload, density_kg_m3)
        .ok()
        .map(|receipt| receipt.properties)
}

/// Deterministically compute volume/mass/CoG with explicit failure classification.
pub fn analyze_body_properties(
    payload: &CadMeshPayload,
    density_kg_m3: f64,
) -> Result<CadBodyAnalysisReceipt, CadBodyAnalysisError> {
    if payload.vertices.is_empty() {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::EmptyVertices,
            "analysis requires at least one mesh vertex",
        ));
    }
    if payload.triangle_indices.len() < 3 {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::EmptyTriangles,
            "analysis requires at least one triangle",
        ));
    }
    if !payload.triangle_indices.len().is_multiple_of(3) {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::MalformedTriangles,
            format!(
                "triangle index buffer length must be divisible by 3, got {}",
                payload.triangle_indices.len()
            ),
        ));
    }
    if !density_kg_m3.is_finite() || density_kg_m3 <= 0.0 {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::InvalidDensity,
            format!("density must be finite and > 0, got {density_kg_m3}"),
        ));
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

    for (triangle_index, triangle) in payload.triangle_indices.chunks_exact(3).enumerate() {
        let p0 = checked_vertex3(payload, triangle[0] as usize, triangle_index)?;
        let p1 = checked_vertex3(payload, triangle[1] as usize, triangle_index)?;
        let p2 = checked_vertex3(payload, triangle[2] as usize, triangle_index)?;

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
    let (center_of_gravity_mm, center_of_gravity_source) = if signed_volume_mm3.abs() > 1e-9 {
        (
            [
                centroid_accum[0] / signed_volume_mm3,
                centroid_accum[1] / signed_volume_mm3,
                centroid_accum[2] / signed_volume_mm3,
            ],
            CadCenterOfGravitySource::MeshVolume,
        )
    } else {
        (
            bounds_center_mm,
            CadCenterOfGravitySource::BoundsCenterFallback,
        )
    };
    let volume_m3 = volume_mm3 * 1e-9;
    let mass_kg = density_kg_m3 * volume_m3;

    Ok(CadBodyAnalysisReceipt {
        properties: CadBodyProperties {
            volume_mm3,
            surface_area_mm2,
            mass_kg,
            center_of_gravity_mm,
            bounds_min_mm,
            bounds_max_mm,
            bounds_size_mm,
        },
        center_of_gravity_source,
        vertex_count: payload.vertices.len(),
        triangle_count: payload.triangle_indices.len() / 3,
    })
}

/// Beam-style deflection approximation for rack-like parts.
///
/// This is a heuristic for quick engineering feedback, not a certified FEA result.
pub fn estimate_beam_deflection_heuristic(
    input: CadDeflectionHeuristicInput,
) -> Result<CadDeflectionHeuristicEstimate, CadDeflectionHeuristicError> {
    if !input.span_mm.is_finite() || input.span_mm <= 0.0 {
        return Err(CadDeflectionHeuristicError::new(
            CadDeflectionHeuristicErrorCode::InvalidSpan,
            format!("span_mm must be finite and > 0, got {}", input.span_mm),
        ));
    }
    if !input.width_mm.is_finite() || input.width_mm <= 0.0 {
        return Err(CadDeflectionHeuristicError::new(
            CadDeflectionHeuristicErrorCode::InvalidWidth,
            format!("width_mm must be finite and > 0, got {}", input.width_mm),
        ));
    }
    if !input.thickness_mm.is_finite() || input.thickness_mm <= 0.0 {
        return Err(CadDeflectionHeuristicError::new(
            CadDeflectionHeuristicErrorCode::InvalidThickness,
            format!(
                "thickness_mm must be finite and > 0, got {}",
                input.thickness_mm
            ),
        ));
    }
    if !input.load_kg.is_finite() || input.load_kg < 0.0 {
        return Err(CadDeflectionHeuristicError::new(
            CadDeflectionHeuristicErrorCode::InvalidLoad,
            format!("load_kg must be finite and >= 0, got {}", input.load_kg),
        ));
    }
    if !input.youngs_modulus_gpa.is_finite() || input.youngs_modulus_gpa <= 0.0 {
        return Err(CadDeflectionHeuristicError::new(
            CadDeflectionHeuristicErrorCode::InvalidElasticModulus,
            format!(
                "youngs_modulus_gpa must be finite and > 0, got {}",
                input.youngs_modulus_gpa
            ),
        ));
    }

    const GRAVITY_M_S2: f64 = 9.80665;
    // Simply-supported beam with center point load.
    const NUMERATOR_FACTOR: f64 = 1.0;
    const DENOMINATOR_FACTOR: f64 = 48.0;

    let force_n = input.load_kg * GRAVITY_M_S2;
    let span_m = input.span_mm * 1e-3;
    let width_m = input.width_mm * 1e-3;
    let thickness_m = input.thickness_mm * 1e-3;
    let youngs_modulus_pa = input.youngs_modulus_gpa * 1e9;
    let second_moment_m4 = width_m * thickness_m.powi(3) / 12.0;

    let max_deflection_m = (NUMERATOR_FACTOR * force_n * span_m.powi(3))
        / (DENOMINATOR_FACTOR * youngs_modulus_pa * second_moment_m4);
    let max_deflection_mm = max_deflection_m * 1e3;

    let slenderness_ratio = input.span_mm / input.thickness_mm;
    let confidence = if (10.0..=60.0).contains(&slenderness_ratio) && input.load_kg <= 20.0 {
        CadDeflectionConfidence::Medium
    } else {
        CadDeflectionConfidence::Low
    };

    let metadata = BTreeMap::from([
        (
            "model_id".to_string(),
            CAD_DEFLECTION_HEURISTIC_MODEL_ID.to_string(),
        ),
        ("confidence".to_string(), confidence.label().to_string()),
        (
            "limit.1".to_string(),
            "assumes simply-supported beam with center point load".to_string(),
        ),
        (
            "limit.2".to_string(),
            "ignores local cutouts/vents and bracket fastener compliance".to_string(),
        ),
        (
            "limit.3".to_string(),
            "uses axis-aligned rectangular section approximation from bounds".to_string(),
        ),
        ("input.span_mm".to_string(), format6(input.span_mm)),
        ("input.width_mm".to_string(), format6(input.width_mm)),
        (
            "input.thickness_mm".to_string(),
            format6(input.thickness_mm),
        ),
        ("input.load_kg".to_string(), format6(input.load_kg)),
        (
            "input.youngs_modulus_gpa".to_string(),
            format6(input.youngs_modulus_gpa),
        ),
        (
            "derived.slenderness_ratio".to_string(),
            format6(slenderness_ratio),
        ),
        (
            "derived.second_moment_m4".to_string(),
            format!("{second_moment_m4:.12e}"),
        ),
        (
            "result.max_deflection_mm".to_string(),
            format6(max_deflection_mm),
        ),
    ]);

    Ok(CadDeflectionHeuristicEstimate {
        max_deflection_mm,
        confidence,
        metadata,
    })
}

pub fn face_properties(payload: &CadMeshPayload, face_index: usize) -> Option<CadFaceProperties> {
    let triangle_offset = face_index.checked_mul(3)?;
    let i0 = *payload.triangle_indices.get(triangle_offset)? as usize;
    let i1 = *payload.triangle_indices.get(triangle_offset + 1)? as usize;
    let i2 = *payload.triangle_indices.get(triangle_offset + 2)? as usize;
    let p0 = vertex3(payload, i0)?;
    let p1 = vertex3(payload, i1)?;
    let p2 = vertex3(payload, i2)?;
    let edge1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let edge2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross = cross3(edge1, edge2);
    let cross_length = length3(cross);
    let area_mm2 = 0.5 * cross_length;
    let normal = if cross_length > 1e-12 {
        [
            cross[0] / cross_length,
            cross[1] / cross_length,
            cross[2] / cross_length,
        ]
    } else {
        [0.0, 0.0, 0.0]
    };
    Some(CadFaceProperties {
        face_index,
        area_mm2,
        normal,
    })
}

pub fn edge_properties(payload: &CadMeshPayload, edge_index: usize) -> Option<CadEdgeProperties> {
    let edge = payload.edges.get(edge_index)?;
    let start = vertex3(payload, edge.start_vertex as usize)?;
    let end = vertex3(payload, edge.end_vertex as usize)?;
    let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length_mm = length3(delta);
    let edge_type = if edge.flags == 0 {
        CadEdgeType::Segment
    } else {
        CadEdgeType::Tagged
    };
    Some(CadEdgeProperties {
        edge_index,
        length_mm,
        edge_type,
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

fn checked_vertex3(
    payload: &CadMeshPayload,
    index: usize,
    triangle_index: usize,
) -> Result<[f64; 3], CadBodyAnalysisError> {
    let Some(vertex) = payload.vertices.get(index) else {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::MissingVertex,
            format!("triangle[{triangle_index}] references missing vertex index {index}"),
        ));
    };
    let coordinates = [
        f64::from(vertex.position_mm[0]),
        f64::from(vertex.position_mm[1]),
        f64::from(vertex.position_mm[2]),
    ];
    if coordinates.iter().any(|value| !value.is_finite()) {
        return Err(CadBodyAnalysisError::new(
            CadBodyAnalysisErrorCode::NonFiniteVertex,
            format!("triangle[{triangle_index}] contains non-finite vertex position"),
        ));
    }
    Ok(coordinates)
}

fn format6(value: f64) -> String {
    format!("{value:.6}")
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
    use super::{
        CAD_DEFLECTION_HEURISTIC_MODEL_ID, CadBodyAnalysisErrorCode, CadCenterOfGravitySource,
        CadDeflectionConfidence, CadDeflectionHeuristicErrorCode, CadDeflectionHeuristicInput,
        DENSITY_ALUMINUM_6061_KG_M3, analyze_body_properties, edge_properties,
        estimate_beam_deflection_heuristic, estimate_body_properties, face_properties,
    };
    use crate::mesh::{
        CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology,
        CadMeshVertex,
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

    #[test]
    fn core_analysis_receipt_is_deterministic_and_records_cog_source() {
        let payload = tetra_mesh_payload();
        let first = analyze_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("analysis receipt should resolve");
        let second = analyze_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("analysis receipt should stay deterministic");

        assert_eq!(first, second);
        assert_eq!(first.vertex_count, payload.vertices.len());
        assert_eq!(first.triangle_count, payload.triangle_indices.len() / 3);
        assert_eq!(
            first.center_of_gravity_source,
            CadCenterOfGravitySource::MeshVolume
        );
    }

    #[test]
    fn degenerate_volume_falls_back_to_bounds_center_for_cog() {
        let payload = CadMeshPayload {
            mesh_id: "mesh.planar".to_string(),
            document_revision: 2,
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
                    position_mm: [10.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 10.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [10.0, 10.0, 0.0],
            },
        };

        let receipt = analyze_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect("planar mesh should still analyze");
        assert_eq!(
            receipt.center_of_gravity_source,
            CadCenterOfGravitySource::BoundsCenterFallback
        );
        assert_eq!(receipt.properties.center_of_gravity_mm, [5.0, 5.0, 0.0]);
        assert_eq!(receipt.properties.volume_mm3, 0.0);
    }

    #[test]
    fn missing_vertex_indices_are_classified_with_stable_error_code() {
        let mut payload = tetra_mesh_payload();
        payload.triangle_indices[0] = 42;
        let error = analyze_body_properties(&payload, DENSITY_ALUMINUM_6061_KG_M3)
            .expect_err("analysis should fail when triangle references missing vertex");
        assert_eq!(error.code, CadBodyAnalysisErrorCode::MissingVertex);
        assert_eq!(error.code.stable_code(), "CAD-ANALYSIS-MISSING-VERTEX");
        assert!(!error.remediation_hint().is_empty());
    }

    #[test]
    fn face_properties_return_area_and_normal() {
        let payload = tetra_mesh_payload();
        let face = face_properties(&payload, 0).expect("face properties should resolve");
        assert_eq!(face.face_index, 0);
        assert!((face.area_mm2 - 0.5).abs() < 1e-9);
        assert!((face.normal[2] + 1.0).abs() < 1e-9);
    }

    #[test]
    fn edge_properties_return_length_and_type() {
        let mut payload = tetra_mesh_payload();
        payload.edges = vec![
            CadMeshEdgeSegment {
                start_vertex: 0,
                end_vertex: 1,
                flags: 0,
            },
            CadMeshEdgeSegment {
                start_vertex: 1,
                end_vertex: 2,
                flags: 7,
            },
        ];

        let first = edge_properties(&payload, 0).expect("edge 0 should resolve");
        assert!((first.length_mm - 1.0).abs() < 1e-9);
        assert_eq!(first.edge_type.label(), "segment");

        let second = edge_properties(&payload, 1).expect("edge 1 should resolve");
        assert!((second.length_mm - (2.0f64).sqrt()).abs() < 1e-9);
        assert_eq!(second.edge_type.label(), "tagged");
    }

    #[test]
    fn deflection_heuristic_is_deterministic_with_confidence_and_limits() {
        let input = CadDeflectionHeuristicInput {
            span_mm: 320.0,
            width_mm: 160.0,
            thickness_mm: 6.0,
            load_kg: 10.0,
            youngs_modulus_gpa: 69.0,
        };
        let first =
            estimate_beam_deflection_heuristic(input).expect("deflection estimate should succeed");
        let second = estimate_beam_deflection_heuristic(input)
            .expect("deflection estimate should remain deterministic");
        assert_eq!(first, second);
        assert!(first.max_deflection_mm > 0.0);
        assert_eq!(first.confidence, CadDeflectionConfidence::Medium);
        assert_eq!(
            first.metadata.get("model_id").map(String::as_str),
            Some(CAD_DEFLECTION_HEURISTIC_MODEL_ID)
        );
        assert_eq!(
            first.metadata.get("confidence").map(String::as_str),
            Some("medium")
        );
        assert!(first.metadata.contains_key("limit.1"));
    }

    #[test]
    fn deflection_heuristic_classifies_invalid_inputs() {
        let error = estimate_beam_deflection_heuristic(CadDeflectionHeuristicInput {
            span_mm: 200.0,
            width_mm: 120.0,
            thickness_mm: 0.0,
            load_kg: 10.0,
            youngs_modulus_gpa: 69.0,
        })
        .expect_err("zero thickness should fail");
        assert_eq!(
            error.code,
            CadDeflectionHeuristicErrorCode::InvalidThickness
        );
        assert_eq!(error.code.stable_code(), "CAD-DEFLECTION-INVALID-THICKNESS");
        assert!(!error.remediation_hint().is_empty());
    }
}
