use serde::{Deserialize, Serialize};

use crate::drafting::{ProjectedView, Visibility};
use crate::hash::stable_hex_digest;
use crate::kernel_booleans::BooleanPipelineOutcome;
use crate::kernel_primitives::BRepSolid;
use crate::kernel_step;
use crate::mesh::CadMeshPayload;
use crate::stl::export_stl_from_mesh;
use crate::{CadError, CadResult};

/// Deterministic STEP schema used for Wave 1 demo exports.
pub const STEP_FILE_SCHEMA: &str = "AUTOMOTIVE_DESIGN_CC2";
/// vcad parity contract for mesh-only solids after boolean operations.
pub const STEP_EXPORT_NOT_BREP_REASON: &str = "cannot export to STEP: solid has been converted to mesh (B-rep data lost after boolean operations)";
/// vcad parity contract for empty solids.
pub const STEP_EXPORT_EMPTY_REASON: &str = "cannot export to STEP: solid is empty";
/// vcad baseline parity contract for drafting PDF export.
pub const PDF_EXPORT_PARITY_REASON: &str =
    "vcad baseline has no native drawing PDF exporter; use desktop/browser print pipeline";
/// Stable schema ID for machine-readable hand assembly BOM payloads.
pub const HAND_ASSEMBLY_BOM_SCHEMA: &str = "openagents.cad.hand_assembly_bom.v1";

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

/// Deterministic machine-readable BOM line for hand assembly exports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyBomLineItem {
    pub part_id: String,
    pub part_name: String,
    pub category: String,
    pub quantity: u32,
    pub source: String,
    pub material_id: Option<String>,
    pub notes: String,
}

/// Deterministic print metadata attached to hand assembly BOM exports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyPrintMetadata {
    pub orientation_hints: Vec<String>,
    pub print_fit_mm: String,
    pub print_clearance_mm: String,
    pub tolerance_note: String,
}

/// Deterministic machine-readable BOM contract for hand assembly exports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyBomDocument {
    pub schema: String,
    pub assembly_id: String,
    pub assembly_name: String,
    pub design_profile: String,
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub items: Vec<CadHandAssemblyBomLineItem>,
    pub print_metadata: CadHandAssemblyPrintMetadata,
}

/// Input options for deterministic hand assembly export packaging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyPackageOptions {
    pub assembly_name: String,
    pub design_profile: String,
    pub finger_count: u8,
    pub servo_motor_count: u8,
    pub force_sensor_count: u8,
    pub proximity_sensor_count: u8,
    pub include_control_board_mount: bool,
    pub print_fit_mm: f64,
    pub print_clearance_mm: f64,
    pub material_id: String,
}

impl Default for CadHandAssemblyPackageOptions {
    fn default() -> Self {
        Self {
            assembly_name: "humanoid_hand_v1".to_string(),
            design_profile: "humanoid_hand_v1".to_string(),
            finger_count: 5,
            servo_motor_count: 5,
            force_sensor_count: 5,
            proximity_sensor_count: 5,
            include_control_board_mount: true,
            print_fit_mm: 0.15,
            print_clearance_mm: 0.35,
            material_id: "pla".to_string(),
        }
    }
}

/// Stable receipt emitted for deterministic hand assembly export packages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyExportReceipt {
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub assembly_name: String,
    pub design_profile: String,
    pub step_file_name: String,
    pub stl_file_name: String,
    pub bom_file_name: String,
    pub step_hash: String,
    pub stl_hash: String,
    pub bom_hash: String,
    pub package_hash: String,
    pub total_byte_count: usize,
    pub bom_item_count: usize,
}

/// Deterministic hand assembly export package artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadHandAssemblyExportArtifact {
    pub receipt: CadHandAssemblyExportReceipt,
    pub step: CadStepExportArtifact,
    pub stl_bytes: Vec<u8>,
    pub bom_bytes: Vec<u8>,
}

impl CadHandAssemblyExportArtifact {
    /// Return machine-readable BOM payload as UTF-8 text.
    pub fn bom_text(&self) -> CadResult<&str> {
        std::str::from_utf8(&self.bom_bytes).map_err(|error| CadError::ExportFailed {
            format: "bom".to_string(),
            reason: format!("bom payload is not valid utf-8: {error}"),
        })
    }
}

/// Stable receipt emitted for deterministic BRep-backed STEP exports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepBrepExportReceipt {
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub file_name: String,
    pub solid_count: usize,
    pub shell_count: usize,
    pub face_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
}

/// Deterministic BRep-backed STEP export artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepBrepExportArtifact {
    pub receipt: CadStepBrepExportReceipt,
    pub bytes: Vec<u8>,
}

impl CadStepBrepExportArtifact {
    /// Return the STEP payload as UTF-8 text.
    pub fn text(&self) -> CadResult<&str> {
        std::str::from_utf8(&self.bytes).map_err(|error| CadError::ExportFailed {
            format: "step".to_string(),
            reason: format!("step payload is not valid utf-8: {error}"),
        })
    }
}

/// Stable receipt emitted for deterministic drafting DXF exports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadDraftingDxfExportReceipt {
    pub edge_count: usize,
    pub visible_edge_count: usize,
    pub hidden_edge_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
}

/// Deterministic drafting DXF export artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadDraftingDxfExportArtifact {
    pub receipt: CadDraftingDxfExportReceipt,
    pub bytes: Vec<u8>,
}

impl CadDraftingDxfExportArtifact {
    /// Return the DXF payload as UTF-8 text.
    pub fn text(&self) -> CadResult<&str> {
        std::str::from_utf8(&self.bytes).map_err(|error| CadError::ExportFailed {
            format: "dxf".to_string(),
            reason: format!("dxf payload is not valid utf-8: {error}"),
        })
    }
}

/// Export a projected drafting view to deterministic DXF R12 bytes.
///
/// The output intentionally matches vcad's drafting DXF contract:
/// - Header: `$ACADVER=AC1009`, `$INSUNITS=4`
/// - Linetypes: `CONTINUOUS`, `HIDDEN`
/// - Layers: `VISIBLE`, `HIDDEN`
/// - Entities: one `LINE` per projected edge with visibility-mapped layer/linetype.
pub fn export_projected_view_to_dxf(
    view: &ProjectedView,
) -> CadResult<CadDraftingDxfExportArtifact> {
    for (index, edge) in view.edges.iter().enumerate() {
        if !edge.start.x.is_finite()
            || !edge.start.y.is_finite()
            || !edge.end.x.is_finite()
            || !edge.end.y.is_finite()
        {
            return Err(dxf_export_failed(format!(
                "edge {index} contains non-finite coordinate"
            )));
        }
    }

    let mut text = String::new();

    // Header.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "SECTION");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "HEADER");
    push_dxf_line(&mut text, "9");
    push_dxf_line(&mut text, "$ACADVER");
    push_dxf_line(&mut text, "1");
    push_dxf_line(&mut text, "AC1009");
    push_dxf_line(&mut text, "9");
    push_dxf_line(&mut text, "$INSUNITS");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "4");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "ENDSEC");

    // Tables.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "SECTION");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "TABLES");

    // Linetype table.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "TABLE");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "LTYPE");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "2");

    // CONTINUOUS linetype.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "LTYPE");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "CONTINUOUS");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "3");
    push_dxf_line(&mut text, "Solid line");
    push_dxf_line(&mut text, "72");
    push_dxf_line(&mut text, "65");
    push_dxf_line(&mut text, "73");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "40");
    push_dxf_line(&mut text, "0.0");

    // HIDDEN linetype.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "LTYPE");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "HIDDEN");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "3");
    push_dxf_line(&mut text, "Hidden line");
    push_dxf_line(&mut text, "72");
    push_dxf_line(&mut text, "65");
    push_dxf_line(&mut text, "73");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "40");
    push_dxf_line(&mut text, "9.525");
    push_dxf_line(&mut text, "49");
    push_dxf_line(&mut text, "6.35");
    push_dxf_line(&mut text, "49");
    push_dxf_line(&mut text, "-3.175");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "ENDTAB");

    // Layer table.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "TABLE");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "LAYER");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "2");

    // VISIBLE layer.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "LAYER");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "VISIBLE");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "62");
    push_dxf_line(&mut text, "7");
    push_dxf_line(&mut text, "6");
    push_dxf_line(&mut text, "CONTINUOUS");

    // HIDDEN layer.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "LAYER");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "HIDDEN");
    push_dxf_line(&mut text, "70");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "62");
    push_dxf_line(&mut text, "8");
    push_dxf_line(&mut text, "6");
    push_dxf_line(&mut text, "HIDDEN");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "ENDTAB");

    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "ENDSEC");

    // Entities.
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "SECTION");
    push_dxf_line(&mut text, "2");
    push_dxf_line(&mut text, "ENTITIES");

    let mut visible_edge_count = 0usize;
    let mut hidden_edge_count = 0usize;

    for edge in &view.edges {
        let (layer, linetype) = match edge.visibility {
            Visibility::Visible => {
                visible_edge_count = visible_edge_count.saturating_add(1);
                ("VISIBLE", "CONTINUOUS")
            }
            Visibility::Hidden => {
                hidden_edge_count = hidden_edge_count.saturating_add(1);
                ("HIDDEN", "HIDDEN")
            }
        };
        push_dxf_line(&mut text, "0");
        push_dxf_line(&mut text, "LINE");
        push_dxf_line(&mut text, "8");
        push_dxf_line(&mut text, layer);
        push_dxf_line(&mut text, "6");
        push_dxf_line(&mut text, linetype);
        push_dxf_line(&mut text, "10");
        push_dxf_line(&mut text, &format_dxf_real(edge.start.x));
        push_dxf_line(&mut text, "20");
        push_dxf_line(&mut text, &format_dxf_real(edge.start.y));
        push_dxf_line(&mut text, "11");
        push_dxf_line(&mut text, &format_dxf_real(edge.end.x));
        push_dxf_line(&mut text, "21");
        push_dxf_line(&mut text, &format_dxf_real(edge.end.y));
    }

    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "ENDSEC");
    push_dxf_line(&mut text, "0");
    push_dxf_line(&mut text, "EOF");

    let bytes = text.into_bytes();
    let receipt = CadDraftingDxfExportReceipt {
        edge_count: view.edges.len(),
        visible_edge_count,
        hidden_edge_count,
        byte_count: bytes.len(),
        deterministic_hash: stable_hex_digest(&bytes),
    };
    Ok(CadDraftingDxfExportArtifact { receipt, bytes })
}

/// Export a projected drafting view to deterministic DXF bytes.
pub fn export_projected_view_to_dxf_buffer(view: &ProjectedView) -> CadResult<Vec<u8>> {
    Ok(export_projected_view_to_dxf(view)?.bytes)
}

/// Export a projected drafting view to PDF bytes.
///
/// The pinned vcad baseline has no native drawing-PDF exporter in CAD core
/// (`export/dxf.rs` exists, `export/pdf.rs` is absent). PDF output is currently
/// handled via UI/system print flow, so the parity contract is a deterministic
/// unsupported-export error for this API.
pub fn export_projected_view_to_pdf(_view: &ProjectedView) -> CadResult<Vec<u8>> {
    Err(CadError::ExportFailed {
        format: "pdf".to_string(),
        reason: PDF_EXPORT_PARITY_REASON.to_string(),
    })
}

/// Export the active mesh as deterministic STEP text (solid-only, no assembly/PMI/colors).
pub fn export_step_from_mesh(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    mesh: &CadMeshPayload,
) -> CadResult<CadStepExportArtifact> {
    validate_step_export_identity(document_id, variant_id)?;
    mesh.validate_contract()
        .map_err(|error| step_export_failed(format!("mesh payload is invalid: {error}")))?;
    if mesh.variant_id != variant_id {
        return Err(step_export_failed(format!(
            "mesh variant_id mismatch: payload={} requested={variant_id}",
            mesh.variant_id
        )));
    }
    let triangle_count = mesh.triangle_indices.len() / 3;
    if triangle_count == 0 {
        return Err(step_export_failed("mesh has zero triangles"));
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
            return Err(step_export_failed(format!(
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

/// Export a deterministic hand assembly package from the active mesh:
/// - STEP artifact for CAD interchange
/// - STL artifact for print-oriented pipelines
/// - Machine-readable BOM JSON with print metadata
pub fn export_hand_assembly_package_from_mesh(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    mesh: &CadMeshPayload,
    options: &CadHandAssemblyPackageOptions,
) -> CadResult<CadHandAssemblyExportArtifact> {
    validate_step_export_identity(document_id, variant_id)?;
    validate_hand_assembly_package_options(options)?;

    let step = export_step_from_mesh(document_id, document_revision, variant_id, mesh)?;
    let stl = export_stl_from_mesh(document_id, document_revision, variant_id, mesh)?;
    let bom = build_hand_assembly_bom_document(document_id, document_revision, variant_id, options);
    let bom_bytes = serde_json::to_vec_pretty(&bom).map_err(|error| CadError::ExportFailed {
        format: "bom".to_string(),
        reason: format!("failed to serialize hand assembly bom json: {error}"),
    })?;
    let bom_hash = stable_hex_digest(&bom_bytes);
    let step_file_name = step.receipt.file_name.clone();
    let stl_file_name = build_stl_file_name(document_id, variant_id, document_revision);
    let bom_file_name = build_bom_file_name(document_id, variant_id, document_revision);
    let package_descriptor = format!(
        "assembly={}#profile={}#step={}:{}#stl={}:{}#bom={}:{}",
        sanitize_step_segment(options.assembly_name.as_str()),
        sanitize_step_segment(options.design_profile.as_str()),
        step_file_name,
        step.receipt.deterministic_hash,
        stl_file_name,
        stl.receipt.deterministic_hash,
        bom_file_name,
        bom_hash
    );
    let package_hash = stable_hex_digest(package_descriptor.as_bytes());
    let total_byte_count = step
        .bytes
        .len()
        .saturating_add(stl.bytes.len())
        .saturating_add(bom_bytes.len());
    let receipt = CadHandAssemblyExportReceipt {
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        assembly_name: options.assembly_name.clone(),
        design_profile: options.design_profile.clone(),
        step_file_name,
        stl_file_name,
        bom_file_name,
        step_hash: step.receipt.deterministic_hash.clone(),
        stl_hash: stl.receipt.deterministic_hash.clone(),
        bom_hash,
        package_hash,
        total_byte_count,
        bom_item_count: bom.items.len(),
    };
    Ok(CadHandAssemblyExportArtifact {
        receipt,
        step,
        stl_bytes: stl.bytes,
        bom_bytes,
    })
}

/// Return whether a post-boolean result can be exported to STEP.
///
/// This mirrors vcad's `can_export_step` contract:
/// - `true` when a BRep result exists
/// - `false` when the result is mesh-only or empty.
pub fn can_export_post_boolean_step(brep_result: Option<&BRepSolid>) -> bool {
    brep_result.is_some()
}

/// Export a post-boolean result to STEP bytes using the BRep path.
///
/// If no BRep result exists:
/// - `BooleanPipelineOutcome::EmptyResult` maps to `STEP_EXPORT_EMPTY_REASON`
/// - all other outcomes map to `STEP_EXPORT_NOT_BREP_REASON`.
pub fn export_step_from_post_boolean_brep(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    brep_result: Option<&BRepSolid>,
    outcome: BooleanPipelineOutcome,
) -> CadResult<CadStepBrepExportArtifact> {
    match brep_result {
        Some(brep) => export_step_from_brep(document_id, document_revision, variant_id, brep),
        None if outcome == BooleanPipelineOutcome::EmptyResult => {
            Err(step_export_failed(STEP_EXPORT_EMPTY_REASON))
        }
        None => Err(step_export_failed(STEP_EXPORT_NOT_BREP_REASON)),
    }
}

/// Export a BRep solid as deterministic STEP bytes via the kernel STEP adapter path.
pub fn export_step_from_brep(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    brep: &BRepSolid,
) -> CadResult<CadStepBrepExportArtifact> {
    validate_step_export_identity(document_id, variant_id)?;
    let counts = brep.topology.counts();
    if counts.solid_count == 0 || counts.shell_count == 0 || counts.face_count == 0 {
        return Err(step_export_failed(STEP_EXPORT_EMPTY_REASON));
    }

    let bytes = kernel_step::write_step_to_buffer(brep).map_err(|error| {
        step_export_failed(format!("kernel step adapter write failed: {error}"))
    })?;
    let file_name = build_step_file_name(document_id, variant_id, document_revision);
    let receipt = CadStepBrepExportReceipt {
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        file_name,
        solid_count: counts.solid_count,
        shell_count: counts.shell_count,
        face_count: counts.face_count,
        byte_count: bytes.len(),
        deterministic_hash: stable_hex_digest(&bytes),
    };
    Ok(CadStepBrepExportArtifact { receipt, bytes })
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
        .ok_or_else(|| step_export_failed(format!("triangle index out of range: {index}")))?;
    Ok(vertex.position_mm)
}

fn cartesian_point_line(position_mm: [f32; 3]) -> CadResult<String> {
    if !position_mm.iter().all(|value| value.is_finite()) {
        return Err(step_export_failed(
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
        "{}.step",
        build_export_file_stem(document_id, variant_id, document_revision)
    )
}

fn build_stl_file_name(document_id: &str, variant_id: &str, document_revision: u64) -> String {
    format!(
        "{}.stl",
        build_export_file_stem(document_id, variant_id, document_revision)
    )
}

fn build_bom_file_name(document_id: &str, variant_id: &str, document_revision: u64) -> String {
    format!(
        "{}.bom.json",
        build_export_file_stem(document_id, variant_id, document_revision)
    )
}

fn build_export_file_stem(document_id: &str, variant_id: &str, document_revision: u64) -> String {
    format!(
        "{}-{}-r{:06}",
        sanitize_step_segment(document_id),
        sanitize_step_segment(variant_id),
        document_revision
    )
}

fn escape_step_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn validate_step_export_identity(document_id: &str, variant_id: &str) -> CadResult<()> {
    if document_id.trim().is_empty() {
        return Err(step_export_failed("document id must not be empty"));
    }
    if variant_id.trim().is_empty() {
        return Err(step_export_failed("variant id must not be empty"));
    }
    Ok(())
}

fn validate_hand_assembly_package_options(
    options: &CadHandAssemblyPackageOptions,
) -> CadResult<()> {
    if options.assembly_name.trim().is_empty() {
        return Err(CadError::ExportFailed {
            format: "assembly-package".to_string(),
            reason: "assembly name must not be empty".to_string(),
        });
    }
    if options.design_profile.trim().is_empty() {
        return Err(CadError::ExportFailed {
            format: "assembly-package".to_string(),
            reason: "design profile must not be empty".to_string(),
        });
    }
    if options.finger_count < 2 {
        return Err(CadError::ExportFailed {
            format: "assembly-package".to_string(),
            reason: "finger count must be at least 2".to_string(),
        });
    }
    if !options.print_fit_mm.is_finite() || !options.print_clearance_mm.is_finite() {
        return Err(CadError::ExportFailed {
            format: "assembly-package".to_string(),
            reason: "print fit and print clearance must be finite".to_string(),
        });
    }
    if options.print_fit_mm <= 0.0 || options.print_clearance_mm <= 0.0 {
        return Err(CadError::ExportFailed {
            format: "assembly-package".to_string(),
            reason: "print fit and print clearance must be positive".to_string(),
        });
    }
    Ok(())
}

fn build_hand_assembly_bom_document(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    options: &CadHandAssemblyPackageOptions,
) -> CadHandAssemblyBomDocument {
    let assembly_id = format!(
        "assembly.{}.{}",
        sanitize_step_segment(options.design_profile.as_str()),
        build_export_file_stem(document_id, variant_id, document_revision)
    );
    let mut items = Vec::new();
    let printed_material = Some(options.material_id.clone());
    items.push(CadHandAssemblyBomLineItem {
        part_id: "printed.palm_chassis".to_string(),
        part_name: "Palm Chassis".to_string(),
        category: "printed_part".to_string(),
        quantity: 1,
        source: "printed".to_string(),
        material_id: printed_material.clone(),
        notes: "Primary palm body; print with flat palm surface on build plate.".to_string(),
    });
    for (digit_index, digit_name) in hand_digit_names(options.finger_count).iter().enumerate() {
        items.push(CadHandAssemblyBomLineItem {
            part_id: format!(
                "printed.digit.{}",
                sanitize_step_segment(digit_name.as_str())
            ),
            part_name: digit_name.clone(),
            category: "printed_part".to_string(),
            quantity: 1,
            source: "printed".to_string(),
            material_id: printed_material.clone(),
            notes: format!(
                "Digit {} shell with integrated tendon channel and joint seats.",
                digit_index.saturating_add(1)
            ),
        });
    }
    items.push(CadHandAssemblyBomLineItem {
        part_id: "printed.arm_interface_mount".to_string(),
        part_name: "Arm Interface Mount".to_string(),
        category: "printed_part".to_string(),
        quantity: 1,
        source: "printed".to_string(),
        material_id: printed_material.clone(),
        notes: "Mount flange for wrist/arm attachment and bench fixturing.".to_string(),
    });
    if options.include_control_board_mount {
        items.push(CadHandAssemblyBomLineItem {
            part_id: "printed.control_board_tray".to_string(),
            part_name: "Control Board Tray".to_string(),
            category: "printed_part".to_string(),
            quantity: 1,
            source: "printed".to_string(),
            material_id: printed_material,
            notes: "Printed tray for controller board and harness anchor points.".to_string(),
        });
    }

    if options.servo_motor_count > 0 {
        items.push(CadHandAssemblyBomLineItem {
            part_id: "hardware.servo.sg90".to_string(),
            part_name: "Micro Servo (SG90 class)".to_string(),
            category: "motor".to_string(),
            quantity: options.servo_motor_count as u32,
            source: "off_the_shelf".to_string(),
            material_id: None,
            notes: "Actuator count scales with digit/joint drive topology.".to_string(),
        });
    }
    let fastener_quantity = (options.servo_motor_count as u32).saturating_mul(4).max(12);
    items.push(CadHandAssemblyBomLineItem {
        part_id: "hardware.fastener.m2x8_shcs".to_string(),
        part_name: "M2x8 Socket Head Screw".to_string(),
        category: "fastener".to_string(),
        quantity: fastener_quantity,
        source: "off_the_shelf".to_string(),
        material_id: None,
        notes: "Primary structural and servo-mount fastener.".to_string(),
    });
    items.push(CadHandAssemblyBomLineItem {
        part_id: "hardware.fastener.m2_hex_nut".to_string(),
        part_name: "M2 Hex Nut".to_string(),
        category: "fastener".to_string(),
        quantity: fastener_quantity,
        source: "off_the_shelf".to_string(),
        material_id: None,
        notes: "Matching captive nuts for M2 fastener stack.".to_string(),
    });
    items.push(CadHandAssemblyBomLineItem {
        part_id: "hardware.tendon.cable_1mm".to_string(),
        part_name: "1.0 mm Tendon Cable".to_string(),
        category: "actuation".to_string(),
        quantity: options.finger_count as u32,
        source: "off_the_shelf".to_string(),
        material_id: None,
        notes: "One routed tendon per digit channel by default.".to_string(),
    });

    if options.force_sensor_count > 0 {
        items.push(CadHandAssemblyBomLineItem {
            part_id: "sensor.force.fingertip_pad".to_string(),
            part_name: "Fingertip Force Sensor".to_string(),
            category: "sensor".to_string(),
            quantity: options.force_sensor_count as u32,
            source: "off_the_shelf".to_string(),
            material_id: None,
            notes: "Fingertip force sensing pads for grasp feedback.".to_string(),
        });
    }
    if options.proximity_sensor_count > 0 {
        items.push(CadHandAssemblyBomLineItem {
            part_id: "sensor.proximity.short_range".to_string(),
            part_name: "Proximity Sensor Module".to_string(),
            category: "sensor".to_string(),
            quantity: options.proximity_sensor_count as u32,
            source: "off_the_shelf".to_string(),
            material_id: None,
            notes: "Short-range proximity modules for pre-contact detection.".to_string(),
        });
    }
    items.push(CadHandAssemblyBomLineItem {
        part_id: "electronics.control_board.micro".to_string(),
        part_name: "Micro Control Board".to_string(),
        category: "electronics".to_string(),
        quantity: 1,
        source: "off_the_shelf".to_string(),
        material_id: None,
        notes: "Controller board with PWM outputs and sensor input channels.".to_string(),
    });

    let print_metadata = CadHandAssemblyPrintMetadata {
        orientation_hints: vec![
            "Print palm chassis flat with mount surface down.".to_string(),
            "Print digits upright to preserve tendon channel roundness.".to_string(),
            "Print arm interface mount with flange on bed for concentric holes.".to_string(),
        ],
        print_fit_mm: canonical_print_decimal(options.print_fit_mm),
        print_clearance_mm: canonical_print_decimal(options.print_clearance_mm),
        tolerance_note:
            "Verify printer-specific fit with test coupons before final full assembly run."
                .to_string(),
    };

    CadHandAssemblyBomDocument {
        schema: HAND_ASSEMBLY_BOM_SCHEMA.to_string(),
        assembly_id,
        assembly_name: options.assembly_name.clone(),
        design_profile: options.design_profile.clone(),
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        items,
        print_metadata,
    }
}

fn hand_digit_names(finger_count: u8) -> Vec<String> {
    const CANONICAL: [&str; 5] = [
        "Index Finger",
        "Middle Finger",
        "Ring Finger",
        "Pinky Finger",
        "Thumb",
    ];
    let mut names = Vec::with_capacity(finger_count as usize);
    for index in 0..finger_count {
        if let Some(name) = CANONICAL.get(index as usize) {
            names.push((*name).to_string());
        } else {
            names.push(format!("Aux Finger {}", index.saturating_add(1)));
        }
    }
    names
}

fn canonical_print_decimal(value: f64) -> String {
    let rounded = (value * 1_000.0).round() / 1_000.0;
    format!("{rounded:.3}")
}

fn push_dxf_line(text: &mut String, line: &str) {
    text.push_str(line);
    text.push('\n');
}

fn format_dxf_real(value: f64) -> String {
    format!("{value:.6}")
}

fn step_export_failed(reason: impl Into<String>) -> CadError {
    CadError::ExportFailed {
        format: "step".to_string(),
        reason: reason.into(),
    }
}

fn dxf_export_failed(reason: impl Into<String>) -> CadError {
    CadError::ExportFailed {
        format: "dxf".to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadHandAssemblyPackageOptions, PDF_EXPORT_PARITY_REASON, STEP_EXPORT_EMPTY_REASON,
        STEP_EXPORT_NOT_BREP_REASON, can_export_post_boolean_step,
        export_hand_assembly_package_from_mesh, export_projected_view_to_dxf,
        export_projected_view_to_pdf, export_step_from_mesh, export_step_from_post_boolean_brep,
        sanitize_step_segment,
    };
    use crate::drafting::{
        EdgeType, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility,
    };
    use crate::kernel_booleans::{
        BooleanPipelineConfig, BooleanPipelineOutcome, KernelBooleanOp, run_staged_boolean_pipeline,
    };
    use crate::kernel_primitives::make_cube;
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn sample_projected_view() -> ProjectedView {
        let mut view = ProjectedView::new(ViewDirection::Front);
        view.add_edge(ProjectedEdge::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(40.0, 0.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(40.0, 0.0),
            Point2D::new(40.0, 25.0),
            Visibility::Hidden,
            EdgeType::Boundary,
            0.0,
        ));
        view.add_edge(ProjectedEdge::new(
            Point2D::new(40.0, 25.0),
            Point2D::new(0.0, 25.0),
            Visibility::Visible,
            EdgeType::Silhouette,
            0.0,
        ));
        view
    }

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

    fn translated_cube(dx: f64, dy: f64, dz: f64) -> crate::kernel_primitives::BRepSolid {
        let mut cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        for vertex in cube.topology.vertices.values_mut() {
            vertex.point.x += dx;
            vertex.point.y += dy;
            vertex.point.z += dz;
        }
        cube
    }

    #[test]
    fn dxf_export_is_deterministic_and_byte_stable() {
        let view = sample_projected_view();
        let first = export_projected_view_to_dxf(&view).expect("dxf export should succeed");
        let second =
            export_projected_view_to_dxf(&view).expect("dxf export should be deterministic");
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(
            first.receipt.deterministic_hash,
            second.receipt.deterministic_hash
        );
        assert_eq!(first.receipt.edge_count, 3);
        assert_eq!(first.receipt.visible_edge_count, 2);
        assert_eq!(first.receipt.hidden_edge_count, 1);
    }

    #[test]
    fn dxf_export_matches_vcad_r12_contract_tokens() {
        let view = sample_projected_view();
        let artifact = export_projected_view_to_dxf(&view).expect("dxf export should succeed");
        let text = artifact.text().expect("dxf payload should be utf-8");
        assert!(text.contains("$ACADVER\n1\nAC1009\n"));
        assert!(text.contains("$INSUNITS\n70\n4\n"));
        assert!(text.contains("TABLE\n2\nLTYPE\n70\n2\n"));
        assert!(text.contains("TABLE\n2\nLAYER\n70\n2\n"));
        assert!(text.contains("SECTION\n2\nENTITIES\n"));
        assert!(text.contains("\n8\nVISIBLE\n6\nCONTINUOUS\n"));
        assert!(text.contains("\n8\nHIDDEN\n6\nHIDDEN\n"));
        assert!(text.ends_with("0\nEOF\n"));
    }

    #[test]
    fn dxf_export_hash_matches_golden_fingerprint() {
        let view = sample_projected_view();
        let artifact = export_projected_view_to_dxf(&view).expect("dxf export should succeed");
        assert_eq!(artifact.receipt.deterministic_hash, "f995b4f675c0711c");
    }

    #[test]
    fn dxf_export_rejects_non_finite_coordinates() {
        let mut view = ProjectedView::new(ViewDirection::Front);
        view.add_edge(ProjectedEdge::new(
            Point2D::new(f64::NAN, 0.0),
            Point2D::new(10.0, 0.0),
            Visibility::Visible,
            EdgeType::Sharp,
            0.0,
        ));
        let error =
            export_projected_view_to_dxf(&view).expect_err("non-finite coordinates should fail");
        assert!(
            error
                .to_string()
                .contains("edge 0 contains non-finite coordinate")
        );
    }

    #[test]
    fn pdf_export_matches_vcad_unimplemented_contract() {
        let view = sample_projected_view();
        let error = export_projected_view_to_pdf(&view)
            .expect_err("pdf export should be unsupported in parity baseline");
        assert!(
            error
                .to_string()
                .contains("export failed (pdf): vcad baseline has no native drawing PDF exporter")
        );
        assert_eq!(
            error,
            crate::CadError::ExportFailed {
                format: "pdf".to_string(),
                reason: PDF_EXPORT_PARITY_REASON.to_string()
            }
        );
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
    fn step_post_boolean_brep_export_succeeds_for_brep_result() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(5.0, 0.0, 0.0);
        let result = run_staged_boolean_pipeline(
            &left,
            &right,
            KernelBooleanOp::Difference,
            BooleanPipelineConfig::default(),
        )
        .expect("boolean pipeline should succeed");
        assert!(result.brep_result.is_some());
        assert!(can_export_post_boolean_step(result.brep_result.as_ref()));

        let first = export_step_from_post_boolean_brep(
            "doc.step.post-boolean",
            12,
            "variant.boolean",
            result.brep_result.as_ref(),
            result.outcome,
        )
        .expect("BRep result should export");
        let second = export_step_from_post_boolean_brep(
            "doc.step.post-boolean",
            12,
            "variant.boolean",
            result.brep_result.as_ref(),
            result.outcome,
        )
        .expect("repeated export should succeed");
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(
            first.receipt.deterministic_hash,
            second.receipt.deterministic_hash
        );
        assert!(first.receipt.face_count > 0);
        let text = first.text().expect("step payload should decode");
        assert!(text.contains("MANIFOLD_SOLID_BREP"));
        assert!(text.contains("OPENAGENTS_KERNEL_SUMMARY"));
    }

    #[test]
    fn step_post_boolean_brep_export_reports_not_brep_for_mesh_fallback() {
        assert!(!can_export_post_boolean_step(None));
        let error = export_step_from_post_boolean_brep(
            "doc.step.post-boolean",
            12,
            "variant.boolean",
            None,
            BooleanPipelineOutcome::MeshFallback,
        )
        .expect_err("mesh-only post-boolean state should fail");
        assert_eq!(
            error,
            crate::CadError::ExportFailed {
                format: "step".to_string(),
                reason: STEP_EXPORT_NOT_BREP_REASON.to_string(),
            }
        );
    }

    #[test]
    fn step_post_boolean_brep_export_reports_empty_for_empty_result() {
        assert!(!can_export_post_boolean_step(None));
        let error = export_step_from_post_boolean_brep(
            "doc.step.post-boolean",
            12,
            "variant.boolean",
            None,
            BooleanPipelineOutcome::EmptyResult,
        )
        .expect_err("empty post-boolean state should fail");
        assert_eq!(
            error,
            crate::CadError::ExportFailed {
                format: "step".to_string(),
                reason: STEP_EXPORT_EMPTY_REASON.to_string(),
            }
        );
    }

    #[test]
    fn sanitize_step_segment_collapses_symbols() {
        assert_eq!(sanitize_step_segment("Rack V1/2026"), "rack-v1-2026");
        assert_eq!(sanitize_step_segment("___"), "cad");
    }

    #[test]
    fn hand_assembly_export_package_is_deterministic_and_machine_readable() {
        let mesh = sample_tetra_mesh();
        let options = CadHandAssemblyPackageOptions::default();
        let first = export_hand_assembly_package_from_mesh(
            "doc.hand.demo",
            11,
            "variant.baseline",
            &mesh,
            &options,
        )
        .expect("first hand package export should succeed");
        let second = export_hand_assembly_package_from_mesh(
            "doc.hand.demo",
            11,
            "variant.baseline",
            &mesh,
            &options,
        )
        .expect("second hand package export should succeed");

        assert_eq!(first.receipt, second.receipt);
        assert_eq!(first.step.bytes, second.step.bytes);
        assert_eq!(first.stl_bytes, second.stl_bytes);
        assert_eq!(first.bom_bytes, second.bom_bytes);
        assert_eq!(
            first.receipt.step_hash, first.step.receipt.deterministic_hash,
            "package should include step hash"
        );
        assert_eq!(first.receipt.step_file_name, first.step.receipt.file_name);
        assert_eq!(
            first.receipt.stl_file_name,
            "doc-hand-demo-variant-baseline-r000011.stl"
        );
        assert_eq!(
            first.receipt.bom_file_name,
            "doc-hand-demo-variant-baseline-r000011.bom.json"
        );

        let bom_json: serde_json::Value =
            serde_json::from_slice(&first.bom_bytes).expect("bom should be valid json");
        assert_eq!(
            bom_json.get("schema").and_then(|value| value.as_str()),
            Some("openagents.cad.hand_assembly_bom.v1")
        );
        assert_eq!(
            bom_json
                .get("assembly_name")
                .and_then(|value| value.as_str()),
            Some("humanoid_hand_v1")
        );
        assert!(
            bom_json
                .get("items")
                .and_then(|value| value.as_array())
                .is_some_and(|items| items.len() >= 10)
        );
    }

    #[test]
    fn hand_assembly_export_package_part_names_are_stable() {
        let mesh = sample_tetra_mesh();
        let package = export_hand_assembly_package_from_mesh(
            "doc.hand.demo",
            3,
            "variant.baseline",
            &mesh,
            &CadHandAssemblyPackageOptions::default(),
        )
        .expect("hand package export should succeed");
        let bom_json: serde_json::Value =
            serde_json::from_slice(&package.bom_bytes).expect("bom should be valid json");
        let item_names = bom_json
            .get("items")
            .and_then(|value| value.as_array())
            .expect("items should be present")
            .iter()
            .map(|item| {
                item.get("part_name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            item_names,
            vec![
                "Palm Chassis",
                "Index Finger",
                "Middle Finger",
                "Ring Finger",
                "Pinky Finger",
                "Thumb",
                "Arm Interface Mount",
                "Control Board Tray",
                "Micro Servo (SG90 class)",
                "M2x8 Socket Head Screw",
                "M2 Hex Nut",
                "1.0 mm Tendon Cable",
                "Fingertip Force Sensor",
                "Proximity Sensor Module",
                "Micro Control Board",
            ]
        );
    }

    #[test]
    fn hand_assembly_export_package_hash_changes_when_bom_changes() {
        let mesh = sample_tetra_mesh();
        let baseline = export_hand_assembly_package_from_mesh(
            "doc.hand.demo",
            4,
            "variant.baseline",
            &mesh,
            &CadHandAssemblyPackageOptions::default(),
        )
        .expect("baseline package export should succeed");

        let mut updated_options = CadHandAssemblyPackageOptions::default();
        updated_options.servo_motor_count = 6;
        let updated = export_hand_assembly_package_from_mesh(
            "doc.hand.demo",
            4,
            "variant.baseline",
            &mesh,
            &updated_options,
        )
        .expect("updated package export should succeed");

        assert_eq!(
            baseline.receipt.step_hash, updated.receipt.step_hash,
            "same mesh should keep deterministic step hash"
        );
        assert_ne!(
            baseline.receipt.bom_hash, updated.receipt.bom_hash,
            "bom hash must change when bom items change"
        );
        assert_ne!(
            baseline.receipt.package_hash, updated.receipt.package_hash,
            "package hash must change when bom hash changes"
        );
    }
}
