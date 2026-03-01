use serde::{Deserialize, Serialize};

use crate::hash::stable_hex_digest;
use crate::mesh::CadMeshPayload;
use crate::{CadError, CadResult};

/// Deterministic STEP schema used for Wave 1 demo exports.
pub const STEP_FILE_SCHEMA: &str = "AUTOMOTIVE_DESIGN_CC2";

/// Stable receipt emitted for deterministic STEP exports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepExportReceipt {
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub mesh_id: String,
    pub file_name: String,
    pub triangle_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
}

/// Deterministic STEP export artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepExportArtifact {
    pub receipt: CadStepExportReceipt,
    pub bytes: Vec<u8>,
}

impl CadStepExportArtifact {
    /// Return the STEP payload as UTF-8 text.
    pub fn text(&self) -> CadResult<&str> {
        std::str::from_utf8(&self.bytes).map_err(|error| CadError::ExportFailed {
            format: "step".to_string(),
            reason: format!("step payload is not valid utf-8: {error}"),
        })
    }
}

/// Export the active mesh as deterministic STEP text (solid-only, no assembly/PMI/colors).
pub fn export_step_from_mesh(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    mesh: &CadMeshPayload,
) -> CadResult<CadStepExportArtifact> {
    if document_id.trim().is_empty() {
        return Err(export_failed("document id must not be empty"));
    }
    if variant_id.trim().is_empty() {
        return Err(export_failed("variant id must not be empty"));
    }
    mesh.validate_contract()
        .map_err(|error| export_failed(format!("mesh payload is invalid: {error}")))?;
    if mesh.variant_id != variant_id {
        return Err(export_failed(format!(
            "mesh variant_id mismatch: payload={} requested={variant_id}",
            mesh.variant_id
        )));
    }
    let triangle_count = mesh.triangle_indices.len() / 3;
    if triangle_count == 0 {
        return Err(export_failed("mesh has zero triangles"));
    }

    let mut writer = StepEntityWriter::default();

    let application_context = writer.push("APPLICATION_CONTEXT('automotive_design')".to_string());
    let _application_protocol = writer.push(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{application_context})"
    ));
    let design_context = writer.push(format!(
        "DESIGN_CONTEXT('',#{application_context},'design')"
    ));
    let mechanical_context = writer.push(format!(
        "MECHANICAL_CONTEXT('',#{application_context},'mechanical')"
    ));

    let product_identifier = sanitize_step_segment(document_id);
    let product_name = format!(
        "{} {}",
        sanitize_step_segment(document_id),
        sanitize_step_segment(variant_id)
    );
    let product = writer.push(format!(
        "PRODUCT('{}','{}','',(#{mechanical_context}))",
        escape_step_string(&product_identifier),
        escape_step_string(&product_name)
    ));
    let product_definition_formation =
        writer.push(format!("PRODUCT_DEFINITION_FORMATION('1','',#{product})"));
    let product_definition = writer.push(format!(
        "PRODUCT_DEFINITION('design','',#{product_definition_formation},#{design_context})"
    ));
    let product_definition_shape = writer.push(format!(
        "PRODUCT_DEFINITION_SHAPE('','',#{product_definition})"
    ));

    let mut face_ids = Vec::<u64>::with_capacity(triangle_count);
    for triangle in mesh.triangle_indices.chunks_exact(3) {
        let a = triangle_vertex(mesh, triangle[0])?;
        let b = triangle_vertex(mesh, triangle[1])?;
        let c = triangle_vertex(mesh, triangle[2])?;
        if is_degenerate_triangle(a, b, c) {
            return Err(export_failed(format!(
                "triangle is degenerate at indices [{}, {}, {}]",
                triangle[0], triangle[1], triangle[2]
            )));
        }
        let point_a = writer.push(cartesian_point_line(a)?);
        let point_b = writer.push(cartesian_point_line(b)?);
        let point_c = writer.push(cartesian_point_line(c)?);
        let poly_loop = writer.push(format!("POLY_LOOP((#{point_a},#{point_b},#{point_c}))"));
        let face_outer_bound = writer.push(format!("FACE_OUTER_BOUND('',#{poly_loop},.T.)"));
        let face = writer.push(format!("FACE((#{face_outer_bound}))"));
        face_ids.push(face);
    }

    let closed_shell = writer.push(format!(
        "CLOSED_SHELL('',({}))",
        join_entity_refs(&face_ids)
    ));
    let faceted_brep = writer.push(format!(
        "FACETED_BREP('{}',#{closed_shell})",
        escape_step_string(&mesh.mesh_id)
    ));
    let length_unit =
        writer.push("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))".to_string());
    let angle_unit =
        writer.push("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))".to_string());
    let solid_angle_unit =
        writer.push("(NAMED_UNIT(*)SOLID_ANGLE_UNIT()SI_UNIT($,.STERADIAN.))".to_string());
    let uncertainty = writer.push(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{length_unit},'distance_accuracy_value','confusion accuracy')"
    ));
    let representation_context = writer.push(format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length_unit},#{angle_unit},#{solid_angle_unit}))REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY'))"
    ));
    let shape_representation = writer.push(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{faceted_brep}),#{representation_context})"
    ));
    let _shape_definition_representation = writer.push(format!(
        "SHAPE_DEFINITION_REPRESENTATION(#{product_definition_shape},#{shape_representation})"
    ));
    let _category = writer.push(format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part','',(#{}))",
        product
    ));

    let file_name = build_step_file_name(document_id, variant_id, document_revision);
    let mut text = String::new();
    text.push_str("ISO-10303-21;\n");
    text.push_str("HEADER;\n");
    text.push_str("FILE_DESCRIPTION(('OpenAgents CAD STEP export'),'2;1');\n");
    text.push_str(&format!(
        "FILE_NAME('{}','1970-01-01T00:00:00',('OpenAgents'),('OpenAgents'),'openagents-cad','openagents-cad','deterministic');\n",
        escape_step_string(&file_name)
    ));
    text.push_str(&format!("FILE_SCHEMA(('{}'));\n", STEP_FILE_SCHEMA));
    text.push_str("ENDSEC;\n");
    text.push_str("DATA;\n");
    for line in writer.into_lines() {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str("ENDSEC;\n");
    text.push_str("END-ISO-10303-21;\n");

    let bytes = text.into_bytes();
    let deterministic_hash = stable_hex_digest(&bytes);
    let receipt = CadStepExportReceipt {
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        mesh_id: mesh.mesh_id.clone(),
        file_name,
        triangle_count,
        byte_count: bytes.len(),
        deterministic_hash,
    };
    Ok(CadStepExportArtifact { receipt, bytes })
}

#[derive(Default)]
struct StepEntityWriter {
    next_id: u64,
    lines: Vec<String>,
}

impl StepEntityWriter {
    fn push(&mut self, payload: String) -> u64 {
        let id = if self.next_id == 0 { 1 } else { self.next_id };
        self.next_id = id.saturating_add(1);
        self.lines.push(format!("#{id}={payload};"));
        id
    }

    fn into_lines(self) -> Vec<String> {
        self.lines
    }
}

fn triangle_vertex(mesh: &CadMeshPayload, index: u32) -> CadResult<[f32; 3]> {
    let vertex = mesh
        .vertices
        .get(index as usize)
        .ok_or_else(|| export_failed(format!("triangle index out of range: {index}")))?;
    Ok(vertex.position_mm)
}

fn cartesian_point_line(position_mm: [f32; 3]) -> CadResult<String> {
    if !position_mm.iter().all(|value| value.is_finite()) {
        return Err(export_failed(
            "triangle vertex contains non-finite coordinate",
        ));
    }
    Ok(format!(
        "CARTESIAN_POINT('',({},{},{}))",
        format_step_real(position_mm[0]),
        format_step_real(position_mm[1]),
        format_step_real(position_mm[2])
    ))
}

fn format_step_real(value: f32) -> String {
    let mut rounded = ((value as f64) * 1_000_000.0).round() / 1_000_000.0;
    if rounded.abs() < 0.000_000_5 {
        rounded = 0.0;
    }
    format!("{rounded:.6}")
}

fn join_entity_refs(ids: &[u64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn is_degenerate_triangle(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> bool {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let area_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];
    area_sq <= 1.0e-10
}

fn sanitize_step_segment(value: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_hyphen = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            previous_hyphen = false;
        } else if !previous_hyphen {
            sanitized.push('-');
            previous_hyphen = true;
        }
    }
    let trimmed = sanitized.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "cad".to_string()
    } else {
        trimmed
    }
}

fn build_step_file_name(document_id: &str, variant_id: &str, document_revision: u64) -> String {
    format!(
        "{}-{}-r{:06}.step",
        sanitize_step_segment(document_id),
        sanitize_step_segment(variant_id),
        document_revision
    )
}

fn escape_step_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn export_failed(reason: impl Into<String>) -> CadError {
    CadError::ExportFailed {
        format: "step".to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{export_step_from_mesh, sanitize_step_segment};
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn sample_tetra_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.variant-baseline.abc123".to_string(),
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
                    position_mm: [40.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 40.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 40.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 0.5],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![
                0, 1, 2, //
                0, 1, 3, //
                1, 2, 3, //
                0, 2, 3, //
            ],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [40.0, 40.0, 40.0],
            },
        }
    }

    #[test]
    fn step_export_is_deterministic_and_byte_stable() {
        let mesh = sample_tetra_mesh();
        let first = export_step_from_mesh("doc.step-demo", 7, "variant.baseline", &mesh)
            .expect("step export should succeed");
        let second = export_step_from_mesh("doc.step-demo", 7, "variant.baseline", &mesh)
            .expect("step export should be deterministic");

        assert_eq!(first.bytes, second.bytes);
        assert_eq!(
            first.receipt.deterministic_hash,
            second.receipt.deterministic_hash
        );
        assert_eq!(
            first.receipt.file_name,
            "doc-step-demo-variant-baseline-r000007.step"
        );
        assert_eq!(first.receipt.triangle_count, 4);
    }

    #[test]
    fn step_export_hash_matches_golden_fingerprint() {
        let mesh = sample_tetra_mesh();
        let artifact = export_step_from_mesh("doc.step-demo", 7, "variant.baseline", &mesh)
            .expect("step export should succeed");
        assert_eq!(artifact.receipt.deterministic_hash, "9763353cf44df213");
    }

    #[test]
    fn step_export_rejects_variant_mismatch() {
        let mesh = sample_tetra_mesh();
        let error = export_step_from_mesh("doc.step-demo", 7, "variant.low-cost", &mesh)
            .expect_err("variant mismatch should fail");
        assert!(error.to_string().contains(
            "mesh variant_id mismatch: payload=variant.baseline requested=variant.low-cost"
        ));
    }

    #[test]
    fn step_export_rejects_degenerate_triangles() {
        let mut mesh = sample_tetra_mesh();
        mesh.triangle_indices = vec![0, 0, 1];
        let error = export_step_from_mesh("doc.step-demo", 7, "variant.baseline", &mesh)
            .expect_err("degenerate triangles should fail");
        assert!(
            error
                .to_string()
                .contains("triangle is degenerate at indices [0, 0, 1]")
        );
    }

    #[test]
    fn sanitize_step_segment_collapses_symbols() {
        assert_eq!(sanitize_step_segment("Rack V1/2026"), "rack-v1-2026");
        assert_eq!(sanitize_step_segment("___"), "cad");
    }
}
