use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::analysis::analyze_body_properties;
use crate::export::export_step_from_mesh;
use crate::glb::export_glb_from_mesh;
use crate::hash::stable_hex_digest;
use crate::kernel_primitives::{make_cone, make_cube, make_cylinder, make_sphere};
use crate::kernel_tessellate::{TriangleMesh, tessellate_brep};
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::stl::export_stl_from_mesh;
use crate::{CadError, CadResult};

pub const MCP_CAD_CREATE_TOOL: &str = "create_cad_document";
pub const MCP_CAD_EXPORT_TOOL: &str = "export_cad";
pub const MCP_CAD_INSPECT_TOOL: &str = "inspect_cad";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpToolContent {
    pub r#type: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpToolResponse {
    pub content: Vec<CadMcpToolContent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for CadMcpVec3 {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl CadMcpVec3 {
    fn ones() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadMcpPrimitiveType {
    Cube,
    Cylinder,
    Sphere,
    Cone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpPrimitive {
    #[serde(rename = "type")]
    pub primitive_type: CadMcpPrimitiveType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<CadMcpVec3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    #[serde(rename = "radius_bottom", skip_serializing_if = "Option::is_none")]
    pub radius_bottom: Option<f64>,
    #[serde(rename = "radius_top", skip_serializing_if = "Option::is_none")]
    pub radius_top: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CadMcpCoordinateValue {
    Number(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpPositionObject {
    pub x: CadMcpCoordinateValue,
    pub y: CadMcpCoordinateValue,
    #[serde(default)]
    pub z: Option<CadMcpCoordinateValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CadMcpPositionSpec {
    Named(String),
    Object(CadMcpPositionObject),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CadMcpOperation {
    Union {
        primitive: CadMcpPrimitive,
        #[serde(default)]
        at: Option<CadMcpPositionSpec>,
    },
    Difference {
        primitive: CadMcpPrimitive,
        #[serde(default)]
        at: Option<CadMcpPositionSpec>,
    },
    Intersection {
        primitive: CadMcpPrimitive,
        #[serde(default)]
        at: Option<CadMcpPositionSpec>,
    },
    Translate {
        #[serde(default)]
        offset: CadMcpVec3,
    },
    Rotate {
        #[serde(default)]
        angles: CadMcpVec3,
    },
    Scale {
        #[serde(default = "CadMcpVec3::ones")]
        factor: CadMcpVec3,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpPartInput {
    pub name: String,
    pub primitive: CadMcpPrimitive,
    #[serde(default)]
    pub operations: Vec<CadMcpOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CadMcpCreateFormat {
    Json,
    Compact,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpCreateInput {
    pub parts: Vec<CadMcpPartInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<CadMcpCreateFormat>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpMaterial {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpNode {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub op: CadMcpNodeOp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CadMcpNodeOp {
    Cube {
        size: CadMcpVec3,
    },
    Cylinder {
        radius: f64,
        height: f64,
        segments: u32,
    },
    Sphere {
        radius: f64,
        segments: u32,
    },
    Cone {
        radius_bottom: f64,
        radius_top: f64,
        height: f64,
        segments: u32,
    },
    Translate {
        child: u64,
        offset: CadMcpVec3,
    },
    Rotate {
        child: u64,
        angles: CadMcpVec3,
    },
    Scale {
        child: u64,
        factor: CadMcpVec3,
    },
    Union {
        left: u64,
        right: u64,
    },
    Difference {
        left: u64,
        right: u64,
    },
    Intersection {
        left: u64,
        right: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpRoot {
    pub root: u64,
    pub material: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpDocument {
    pub version: String,
    pub nodes: BTreeMap<String, CadMcpNode>,
    pub materials: BTreeMap<String, CadMcpMaterial>,
    pub roots: Vec<CadMcpRoot>,
    pub part_materials: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpExportInput {
    pub ir: CadMcpDocument,
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpInspectInput {
    pub ir: CadMcpDocument,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpPartMassInfo {
    pub name: String,
    pub volume_mm3: f64,
    pub material: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_kg_m3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_g: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpBoundingBox {
    pub min: CadMcpVec3,
    pub max: CadMcpVec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpInspectResult {
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub bounding_box: CadMcpBoundingBox,
    pub center_of_mass: CadMcpVec3,
    pub triangles: usize,
    pub parts: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_g: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_masses: Option<Vec<CadMcpPartMassInfo>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMcpPartMesh {
    pub name: String,
    pub material: String,
    pub mesh: CadMeshPayload,
}

pub fn create_cad_document_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "parts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "primitive": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["cube", "cylinder", "sphere", "cone"] },
                                "size": { "type": "object" },
                                "radius": { "type": "number" },
                                "height": { "type": "number" },
                                "segments": { "type": "number" },
                                "radius_bottom": { "type": "number" },
                                "radius_top": { "type": "number" }
                            },
                            "required": ["type"]
                        },
                        "operations": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": {
                                        "type": "string",
                                        "enum": ["union", "difference", "intersection", "translate", "rotate", "scale"]
                                    }
                                },
                                "required": ["type"]
                            }
                        },
                        "material": { "type": "string" }
                    },
                    "required": ["name", "primitive"]
                }
            },
            "format": {
                "type": "string",
                "enum": ["json", "compact"]
            }
        },
        "required": ["parts"]
    })
}

pub fn export_cad_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ir": {
                "type": "object",
                "description": "IR document from create_cad_document"
            },
            "filename": {
                "type": "string",
                "description": "Output filename with extension (.stl or .glb)"
            }
        },
        "required": ["ir", "filename"]
    })
}

pub fn inspect_cad_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ir": {
                "type": "object",
                "description": "IR document from create_cad_document"
            }
        },
        "required": ["ir"]
    })
}

pub fn create_cad_document(input: CadMcpCreateInput) -> CadResult<CadMcpToolResponse> {
    if input.parts.is_empty() {
        return Err(CadError::ParseFailed {
            reason: "create_cad_document requires at least one part".to_string(),
        });
    }

    let mut nodes = BTreeMap::<String, CadMcpNode>::new();
    let mut roots = Vec::<CadMcpRoot>::new();
    let mut part_materials = BTreeMap::<String, String>::new();
    let mut next_id = 1u64;

    for part in &input.parts {
        let name = part.name.trim();
        if name.is_empty() {
            return Err(CadError::ParseFailed {
                reason: "part name must not be empty".to_string(),
            });
        }

        let base_id = next_id;
        next_id = next_id.saturating_add(1);

        let base_node = CadMcpNode {
            id: base_id,
            name: None,
            op: primitive_to_node_op(&part.primitive)?,
        };
        let _ = nodes.insert(base_id.to_string(), base_node);

        let mut current = base_id;
        for operation in &part.operations {
            let op_id = next_id;
            next_id = next_id.saturating_add(1);
            let op = operation_to_node_op(
                operation,
                current,
                &part.primitive,
                &mut nodes,
                &mut next_id,
            )?;
            let _ = nodes.insert(
                op_id.to_string(),
                CadMcpNode {
                    id: op_id,
                    name: Some(name.to_string()),
                    op,
                },
            );
            current = op_id;
        }

        if let Some(final_node) = nodes.get_mut(&current.to_string()) {
            final_node.name = Some(name.to_string());
        }

        let material = normalize_material_id(part.material.as_deref().unwrap_or("default"));
        roots.push(CadMcpRoot {
            root: current,
            material: material.clone(),
        });
        let _ = part_materials.insert(name.to_string(), material);
    }

    let mut materials = default_materials();
    for root in &roots {
        materials
            .entry(root.material.clone())
            .or_insert_with(|| CadMcpMaterial { density: None });
    }

    let document = CadMcpDocument {
        version: "0.1".to_string(),
        nodes,
        materials,
        roots,
        part_materials,
    };

    let format = input.format.unwrap_or(CadMcpCreateFormat::Compact);
    let text = match format {
        CadMcpCreateFormat::Json => {
            serde_json::to_string_pretty(&document).map_err(|error| CadError::Serialization {
                reason: format!("failed to serialize create_cad_document payload: {error}"),
            })?
        }
        CadMcpCreateFormat::Compact => {
            serde_json::to_string(&document).map_err(|error| CadError::Serialization {
                reason: format!("failed to serialize create_cad_document payload: {error}"),
            })?
        }
    };

    Ok(tool_response(text))
}

pub fn export_cad(input: CadMcpExportInput) -> CadResult<CadMcpToolResponse> {
    if input.filename.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: "filename must not be empty".to_string(),
        });
    }

    let part_meshes = evaluate_document_part_meshes(&input.ir)?;
    if part_meshes.is_empty() {
        return Err(CadError::ParseFailed {
            reason: "Document has no parts to export".to_string(),
        });
    }

    let merged_mesh = merge_part_meshes(&part_meshes, "mesh.mcp.export", "variant.mcp.export")?;
    let extension = file_extension(&input.filename);
    let bytes = match extension.as_str() {
        "stl" => {
            export_stl_from_mesh(
                "cad-mcp.export",
                merged_mesh.document_revision,
                &merged_mesh.variant_id,
                &merged_mesh,
            )
            .map_err(|error| CadError::ExportFailed {
                format: "stl".to_string(),
                reason: error.to_string(),
            })?
            .bytes
        }
        "glb" => {
            export_glb_from_mesh(
                "cad-mcp.export",
                merged_mesh.document_revision,
                &merged_mesh.variant_id,
                &merged_mesh,
            )
            .map_err(|error| CadError::ExportFailed {
                format: "glb".to_string(),
                reason: error.to_string(),
            })?
            .bytes
        }
        _ => {
            return Err(CadError::ExportFailed {
                format: extension.clone(),
                reason: format!("Unsupported format: .{extension}. Use .stl or .glb"),
            });
        }
    };

    let path = resolve_output_path(&input.filename)?;
    fs::write(&path, &bytes).map_err(|error| CadError::ExportFailed {
        format: extension.clone(),
        reason: format!("failed writing {}: {error}", path.display()),
    })?;

    let result = json!({
        "path": path.to_string_lossy(),
        "bytes": bytes.len(),
        "format": extension,
        "parts": part_meshes.len(),
    });

    Ok(tool_response(result.to_string()))
}

pub fn inspect_cad(input: CadMcpInspectInput) -> CadResult<CadMcpToolResponse> {
    let part_meshes = evaluate_document_part_meshes(&input.ir)?;
    if part_meshes.is_empty() {
        return Err(CadError::ParseFailed {
            reason: "Document has no parts to inspect".to_string(),
        });
    }

    let mut total_volume_mm3 = 0.0f64;
    let mut total_surface_area_mm2 = 0.0f64;
    let mut total_triangles = 0usize;
    let mut weighted_cog = CadMcpVec3::default();

    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];

    let mut has_mass = false;
    let mut total_mass_g = 0.0f64;
    let mut part_masses = Vec::<CadMcpPartMassInfo>::new();

    for part in &part_meshes {
        let properties =
            analyze_body_properties(&part.mesh, 1.0).map_err(|error| CadError::EvalFailed {
                reason: format!("failed to analyze part '{}': {}", part.name, error.message),
            })?;

        total_volume_mm3 += properties.properties.volume_mm3;
        total_surface_area_mm2 += properties.properties.surface_area_mm2;
        total_triangles = total_triangles.saturating_add(properties.triangle_count);

        weighted_cog.x +=
            properties.properties.center_of_gravity_mm[0] * properties.properties.volume_mm3;
        weighted_cog.y +=
            properties.properties.center_of_gravity_mm[1] * properties.properties.volume_mm3;
        weighted_cog.z +=
            properties.properties.center_of_gravity_mm[2] * properties.properties.volume_mm3;

        min[0] = min[0].min(f64::from(part.mesh.bounds.min_mm[0]));
        min[1] = min[1].min(f64::from(part.mesh.bounds.min_mm[1]));
        min[2] = min[2].min(f64::from(part.mesh.bounds.min_mm[2]));
        max[0] = max[0].max(f64::from(part.mesh.bounds.max_mm[0]));
        max[1] = max[1].max(f64::from(part.mesh.bounds.max_mm[1]));
        max[2] = max[2].max(f64::from(part.mesh.bounds.max_mm[2]));

        let mut part_mass = CadMcpPartMassInfo {
            name: part.name.clone(),
            volume_mm3: round3(properties.properties.volume_mm3),
            material: part.material.clone(),
            density_kg_m3: None,
            mass_g: None,
        };

        if let Some(density) = input
            .ir
            .materials
            .get(&part.material)
            .and_then(|material| material.density)
            .filter(|density| density.is_finite() && *density > 0.0)
        {
            let mass = analyze_body_properties(&part.mesh, density).map_err(|error| {
                CadError::EvalFailed {
                    reason: format!(
                        "failed to estimate mass for part '{}': {}",
                        part.name, error.message
                    ),
                }
            })?;
            let mass_g = mass.properties.mass_kg * 1000.0;
            part_mass.density_kg_m3 = Some(round3(density));
            part_mass.mass_g = Some(round3(mass_g));
            total_mass_g += mass_g;
            has_mass = true;
        }

        part_masses.push(part_mass);
    }

    let center_of_mass = if total_volume_mm3 > 1e-9 {
        CadMcpVec3 {
            x: round3(weighted_cog.x / total_volume_mm3),
            y: round3(weighted_cog.y / total_volume_mm3),
            z: round3(weighted_cog.z / total_volume_mm3),
        }
    } else {
        CadMcpVec3::default()
    };

    let result = CadMcpInspectResult {
        volume_mm3: round3(total_volume_mm3),
        surface_area_mm2: round3(total_surface_area_mm2),
        bounding_box: CadMcpBoundingBox {
            min: CadMcpVec3 {
                x: round3(min[0]),
                y: round3(min[1]),
                z: round3(min[2]),
            },
            max: CadMcpVec3 {
                x: round3(max[0]),
                y: round3(max[1]),
                z: round3(max[2]),
            },
        },
        center_of_mass,
        triangles: total_triangles,
        parts: part_meshes.len(),
        mass_g: has_mass.then(|| round3(total_mass_g)),
        part_masses: has_mass.then_some(part_masses),
    };

    let text = serde_json::to_string_pretty(&result).map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize inspect_cad payload: {error}"),
    })?;
    Ok(tool_response(text))
}

pub fn create_cad_document_from_value(input: Value) -> CadResult<CadMcpToolResponse> {
    let parsed: CadMcpCreateInput =
        serde_json::from_value(input).map_err(|error| CadError::ParseFailed {
            reason: format!("invalid create_cad_document payload: {error}"),
        })?;
    create_cad_document(parsed)
}

pub fn export_cad_from_value(input: Value) -> CadResult<CadMcpToolResponse> {
    let parsed: CadMcpExportInput =
        serde_json::from_value(input).map_err(|error| CadError::ParseFailed {
            reason: format!("invalid export_cad payload: {error}"),
        })?;
    export_cad(parsed)
}

pub fn inspect_cad_from_value(input: Value) -> CadResult<CadMcpToolResponse> {
    let parsed: CadMcpInspectInput =
        serde_json::from_value(input).map_err(|error| CadError::ParseFailed {
            reason: format!("invalid inspect_cad payload: {error}"),
        })?;
    inspect_cad(parsed)
}

fn tool_response(text: String) -> CadMcpToolResponse {
    CadMcpToolResponse {
        content: vec![CadMcpToolContent {
            r#type: "text".to_string(),
            text,
        }],
    }
}

fn primitive_to_node_op(primitive: &CadMcpPrimitive) -> CadResult<CadMcpNodeOp> {
    match primitive.primitive_type {
        CadMcpPrimitiveType::Cube => {
            let size = primitive.size.clone().unwrap_or(CadMcpVec3 {
                x: 10.0,
                y: 10.0,
                z: 10.0,
            });
            if !is_positive_vec3(&size) {
                return Err(CadError::InvalidPrimitive {
                    reason: "cube size must be finite and > 0".to_string(),
                });
            }
            Ok(CadMcpNodeOp::Cube { size })
        }
        CadMcpPrimitiveType::Cylinder => {
            let radius = primitive.radius.unwrap_or(5.0);
            let height = primitive.height.unwrap_or(10.0);
            let segments = primitive.segments.unwrap_or(32).max(3);
            validate_positive_number("cylinder radius", radius)?;
            validate_positive_number("cylinder height", height)?;
            Ok(CadMcpNodeOp::Cylinder {
                radius,
                height,
                segments,
            })
        }
        CadMcpPrimitiveType::Sphere => {
            let radius = primitive.radius.unwrap_or(5.0);
            let segments = primitive.segments.unwrap_or(32).max(8);
            validate_positive_number("sphere radius", radius)?;
            Ok(CadMcpNodeOp::Sphere { radius, segments })
        }
        CadMcpPrimitiveType::Cone => {
            let radius_bottom = primitive.radius_bottom.or(primitive.radius).unwrap_or(5.0);
            let radius_top = primitive.radius_top.unwrap_or(0.0);
            let height = primitive.height.unwrap_or(10.0);
            let segments = primitive.segments.unwrap_or(32).max(3);
            validate_positive_number("cone radius_bottom", radius_bottom)?;
            if !radius_top.is_finite() || radius_top < 0.0 {
                return Err(CadError::InvalidPrimitive {
                    reason: "cone radius_top must be finite and >= 0".to_string(),
                });
            }
            validate_positive_number("cone height", height)?;
            Ok(CadMcpNodeOp::Cone {
                radius_bottom,
                radius_top,
                height,
                segments,
            })
        }
    }
}

fn operation_to_node_op(
    operation: &CadMcpOperation,
    current: u64,
    base_primitive: &CadMcpPrimitive,
    nodes: &mut BTreeMap<String, CadMcpNode>,
    next_id: &mut u64,
) -> CadResult<CadMcpNodeOp> {
    match operation {
        CadMcpOperation::Translate { offset } => {
            if !is_finite_vec3(offset) {
                return Err(CadError::InvalidParameter {
                    name: "offset".to_string(),
                    reason: "translate offset must be finite".to_string(),
                });
            }
            Ok(CadMcpNodeOp::Translate {
                child: current,
                offset: offset.clone(),
            })
        }
        CadMcpOperation::Rotate { angles } => {
            if !is_finite_vec3(angles) {
                return Err(CadError::InvalidParameter {
                    name: "angles".to_string(),
                    reason: "rotate angles must be finite".to_string(),
                });
            }
            Ok(CadMcpNodeOp::Rotate {
                child: current,
                angles: angles.clone(),
            })
        }
        CadMcpOperation::Scale { factor } => {
            if !is_finite_vec3(factor)
                || factor.x.abs() < 1e-9
                || factor.y.abs() < 1e-9
                || factor.z.abs() < 1e-9
            {
                return Err(CadError::InvalidParameter {
                    name: "factor".to_string(),
                    reason: "scale factor components must be finite and non-zero".to_string(),
                });
            }
            Ok(CadMcpNodeOp::Scale {
                child: current,
                factor: factor.clone(),
            })
        }
        CadMcpOperation::Union { primitive, at } => {
            let tool = append_tool_primitive_node(primitive, at, base_primitive, nodes, next_id)?;
            Ok(CadMcpNodeOp::Union {
                left: current,
                right: tool,
            })
        }
        CadMcpOperation::Difference { primitive, at } => {
            let tool = append_tool_primitive_node(primitive, at, base_primitive, nodes, next_id)?;
            Ok(CadMcpNodeOp::Difference {
                left: current,
                right: tool,
            })
        }
        CadMcpOperation::Intersection { primitive, at } => {
            let tool = append_tool_primitive_node(primitive, at, base_primitive, nodes, next_id)?;
            Ok(CadMcpNodeOp::Intersection {
                left: current,
                right: tool,
            })
        }
    }
}

fn append_tool_primitive_node(
    primitive: &CadMcpPrimitive,
    at: &Option<CadMcpPositionSpec>,
    base_primitive: &CadMcpPrimitive,
    nodes: &mut BTreeMap<String, CadMcpNode>,
    next_id: &mut u64,
) -> CadResult<u64> {
    let primitive_id = *next_id;
    *next_id = next_id.saturating_add(1);
    let _ = nodes.insert(
        primitive_id.to_string(),
        CadMcpNode {
            id: primitive_id,
            name: None,
            op: primitive_to_node_op(primitive)?,
        },
    );

    if let Some(position) = at {
        let resolved = resolve_position(position, base_primitive)?;
        let translate_id = *next_id;
        *next_id = next_id.saturating_add(1);
        let _ = nodes.insert(
            translate_id.to_string(),
            CadMcpNode {
                id: translate_id,
                name: None,
                op: CadMcpNodeOp::Translate {
                    child: primitive_id,
                    offset: resolved,
                },
            },
        );
        return Ok(translate_id);
    }

    Ok(primitive_id)
}

fn evaluate_document_part_meshes(document: &CadMcpDocument) -> CadResult<Vec<CadMcpPartMesh>> {
    if document.roots.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = BTreeSet::<u64>::new();
    let mut cache = BTreeMap::<u64, CadMeshPayload>::new();
    let mut parts = Vec::with_capacity(document.roots.len());

    for (index, root) in document.roots.iter().enumerate() {
        if !seen.insert(root.root) {
            continue;
        }

        let mesh = evaluate_node_mesh(root.root, document, &mut cache)?;
        if mesh.vertices.is_empty() || mesh.triangle_indices.is_empty() {
            continue;
        }

        let part_name = document
            .nodes
            .get(&root.root.to_string())
            .and_then(|node| node.name.clone())
            .unwrap_or_else(|| format!("part_{}", index + 1));

        parts.push(CadMcpPartMesh {
            name: part_name,
            material: normalize_material_id(&root.material),
            mesh,
        });
    }

    Ok(parts)
}

fn evaluate_node_mesh(
    node_id: u64,
    document: &CadMcpDocument,
    cache: &mut BTreeMap<u64, CadMeshPayload>,
) -> CadResult<CadMeshPayload> {
    if let Some(cached) = cache.get(&node_id) {
        return Ok(cached.clone());
    }

    let node =
        document
            .nodes
            .get(&node_id.to_string())
            .ok_or_else(|| CadError::InvalidFeatureGraph {
                reason: format!("missing node id {node_id}"),
            })?;

    let result = match &node.op {
        CadMcpNodeOp::Cube { size } => {
            primitive_mesh("cube", make_cube(size.x, size.y, size.z)?, node.id, 32)?
        }
        CadMcpNodeOp::Cylinder {
            radius,
            height,
            segments,
        } => primitive_mesh(
            "cylinder",
            make_cylinder(*radius, *height, *segments)?,
            node.id,
            *segments,
        )?,
        CadMcpNodeOp::Sphere { radius, segments } => primitive_mesh(
            "sphere",
            make_sphere(*radius, *segments)?,
            node.id,
            *segments,
        )?,
        CadMcpNodeOp::Cone {
            radius_bottom,
            radius_top,
            height,
            segments,
        } => primitive_mesh(
            "cone",
            make_cone(*radius_bottom, *radius_top, *height, *segments)?,
            node.id,
            *segments,
        )?,
        CadMcpNodeOp::Translate { child, offset } => {
            let child_mesh = evaluate_node_mesh(*child, document, cache)?;
            transform_mesh_translate(&child_mesh, offset)
        }
        CadMcpNodeOp::Rotate { child, angles } => {
            let child_mesh = evaluate_node_mesh(*child, document, cache)?;
            transform_mesh_rotate(&child_mesh, angles)
        }
        CadMcpNodeOp::Scale { child, factor } => {
            let child_mesh = evaluate_node_mesh(*child, document, cache)?;
            transform_mesh_scale(&child_mesh, factor)?
        }
        CadMcpNodeOp::Union { left, right } => {
            let left_mesh = evaluate_node_mesh(*left, document, cache)?;
            let right_mesh = evaluate_node_mesh(*right, document, cache)?;
            merge_two_meshes("union", &left_mesh, &right_mesh)?
        }
        CadMcpNodeOp::Difference { left, .. } => {
            // vcad parity lane currently models difference as left-preserving in substrate BRep fallback.
            evaluate_node_mesh(*left, document, cache)?
        }
        CadMcpNodeOp::Intersection { left, right } => {
            let left_mesh = evaluate_node_mesh(*left, document, cache)?;
            let right_mesh = evaluate_node_mesh(*right, document, cache)?;
            intersection_bbox_mesh("intersection", &left_mesh, &right_mesh)?
        }
    };

    let _ = cache.insert(node_id, result.clone());
    Ok(result)
}

fn primitive_mesh(
    primitive: &str,
    brep: crate::kernel_primitives::BRepSolid,
    node_id: u64,
    segments: u32,
) -> CadResult<CadMeshPayload> {
    let mesh = tessellate_brep(&brep, segments)?;
    triangle_mesh_to_payload(
        &mesh,
        &format!("mesh.mcp.{primitive}.{node_id}"),
        &format!("variant.mcp.{primitive}.{node_id}"),
        node_id,
    )
}

fn triangle_mesh_to_payload(
    triangle_mesh: &TriangleMesh,
    mesh_id: &str,
    variant_id: &str,
    document_revision: u64,
) -> CadResult<CadMeshPayload> {
    if !triangle_mesh.vertices.len().is_multiple_of(3) {
        return Err(CadError::Serialization {
            reason: "triangle mesh vertex buffer must be divisible by 3".to_string(),
        });
    }

    let vertex_count = triangle_mesh.vertices.len() / 3;
    let normal_count = triangle_mesh.normals.len() / 3;

    let mut vertices = Vec::<CadMeshVertex>::with_capacity(vertex_count);
    for index in 0..vertex_count {
        let pos = [
            triangle_mesh.vertices[index * 3],
            triangle_mesh.vertices[(index * 3) + 1],
            triangle_mesh.vertices[(index * 3) + 2],
        ];

        let normal = if index < normal_count {
            [
                triangle_mesh.normals[index * 3],
                triangle_mesh.normals[(index * 3) + 1],
                triangle_mesh.normals[(index * 3) + 2],
            ]
        } else {
            [0.0, 0.0, 1.0]
        };

        vertices.push(CadMeshVertex {
            position_mm: pos,
            normal,
            uv: [0.0, 0.0],
            material_slot: 0,
            flags: 0,
        });
    }

    let triangle_indices = triangle_mesh.indices.clone();
    let bounds = compute_bounds(&vertices);

    let payload = CadMeshPayload {
        mesh_id: mesh_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        topology: CadMeshTopology::Triangles,
        vertices,
        triangle_indices,
        edges: Vec::new(),
        material_slots: vec![CadMeshMaterialSlot::default()],
        bounds,
    };

    payload.validate_contract()?;
    Ok(payload)
}

fn compute_bounds(vertices: &[CadMeshVertex]) -> CadMeshBounds {
    if vertices.is_empty() {
        return CadMeshBounds {
            min_mm: [0.0, 0.0, 0.0],
            max_mm: [0.0, 0.0, 0.0],
        };
    }

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for vertex in vertices {
        min[0] = min[0].min(vertex.position_mm[0]);
        min[1] = min[1].min(vertex.position_mm[1]);
        min[2] = min[2].min(vertex.position_mm[2]);

        max[0] = max[0].max(vertex.position_mm[0]);
        max[1] = max[1].max(vertex.position_mm[1]);
        max[2] = max[2].max(vertex.position_mm[2]);
    }

    CadMeshBounds {
        min_mm: min,
        max_mm: max,
    }
}

fn transform_mesh_translate(mesh: &CadMeshPayload, offset: &CadMcpVec3) -> CadMeshPayload {
    let mut transformed = mesh.clone();
    for vertex in &mut transformed.vertices {
        vertex.position_mm[0] = (f64::from(vertex.position_mm[0]) + offset.x) as f32;
        vertex.position_mm[1] = (f64::from(vertex.position_mm[1]) + offset.y) as f32;
        vertex.position_mm[2] = (f64::from(vertex.position_mm[2]) + offset.z) as f32;
    }
    transformed.bounds = compute_bounds(&transformed.vertices);
    transformed.mesh_id = format!("{}+translate", mesh.mesh_id);
    transformed
}

fn transform_mesh_rotate(mesh: &CadMeshPayload, angles: &CadMcpVec3) -> CadMeshPayload {
    let radians = CadMcpVec3 {
        x: angles.x * PI / 180.0,
        y: angles.y * PI / 180.0,
        z: angles.z * PI / 180.0,
    };

    let mut transformed = mesh.clone();
    for vertex in &mut transformed.vertices {
        let rotated_position = rotate_xyz(
            [
                f64::from(vertex.position_mm[0]),
                f64::from(vertex.position_mm[1]),
                f64::from(vertex.position_mm[2]),
            ],
            &radians,
        );
        vertex.position_mm = [
            rotated_position[0] as f32,
            rotated_position[1] as f32,
            rotated_position[2] as f32,
        ];

        let rotated_normal = rotate_xyz(
            [
                f64::from(vertex.normal[0]),
                f64::from(vertex.normal[1]),
                f64::from(vertex.normal[2]),
            ],
            &radians,
        );
        let normalized = normalize3(rotated_normal);
        vertex.normal = [
            normalized[0] as f32,
            normalized[1] as f32,
            normalized[2] as f32,
        ];
    }

    transformed.bounds = compute_bounds(&transformed.vertices);
    transformed.mesh_id = format!("{}+rotate", mesh.mesh_id);
    transformed
}

fn transform_mesh_scale(mesh: &CadMeshPayload, factor: &CadMcpVec3) -> CadResult<CadMeshPayload> {
    if factor.x.abs() < 1e-9 || factor.y.abs() < 1e-9 || factor.z.abs() < 1e-9 {
        return Err(CadError::InvalidParameter {
            name: "factor".to_string(),
            reason: "scale factor components must be non-zero".to_string(),
        });
    }

    let mut transformed = mesh.clone();
    for vertex in &mut transformed.vertices {
        vertex.position_mm[0] = (f64::from(vertex.position_mm[0]) * factor.x) as f32;
        vertex.position_mm[1] = (f64::from(vertex.position_mm[1]) * factor.y) as f32;
        vertex.position_mm[2] = (f64::from(vertex.position_mm[2]) * factor.z) as f32;

        let scaled_normal = [
            f64::from(vertex.normal[0]) / factor.x,
            f64::from(vertex.normal[1]) / factor.y,
            f64::from(vertex.normal[2]) / factor.z,
        ];
        let normalized = normalize3(scaled_normal);
        vertex.normal = [
            normalized[0] as f32,
            normalized[1] as f32,
            normalized[2] as f32,
        ];
    }

    transformed.bounds = compute_bounds(&transformed.vertices);
    transformed.mesh_id = format!("{}+scale", mesh.mesh_id);
    Ok(transformed)
}

fn rotate_xyz(position: [f64; 3], radians: &CadMcpVec3) -> [f64; 3] {
    let (sx, cx) = radians.x.sin_cos();
    let (sy, cy) = radians.y.sin_cos();
    let (sz, cz) = radians.z.sin_cos();

    let mut v = position;

    // X
    let y = (v[1] * cx) - (v[2] * sx);
    let z = (v[1] * sx) + (v[2] * cx);
    v[1] = y;
    v[2] = z;

    // Y
    let x = (v[0] * cy) + (v[2] * sy);
    let z = (-v[0] * sy) + (v[2] * cy);
    v[0] = x;
    v[2] = z;

    // Z
    let x = (v[0] * cz) - (v[1] * sz);
    let y = (v[0] * sz) + (v[1] * cz);
    v[0] = x;
    v[1] = y;

    v
}

fn normalize3(value: [f64; 3]) -> [f64; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    [value[0] / length, value[1] / length, value[2] / length]
}

fn merge_two_meshes(
    label: &str,
    left: &CadMeshPayload,
    right: &CadMeshPayload,
) -> CadResult<CadMeshPayload> {
    let mut merged = left.clone();
    let vertex_offset = merged.vertices.len() as u32;

    merged.vertices.extend(right.vertices.iter().copied());
    merged.triangle_indices.extend(
        right
            .triangle_indices
            .iter()
            .map(|index| index + vertex_offset),
    );
    merged.bounds = compute_bounds(&merged.vertices);
    merged.mesh_id = format!("{}+{}", left.mesh_id, label);
    merged.validate_contract()?;
    Ok(merged)
}

fn intersection_bbox_mesh(
    label: &str,
    left: &CadMeshPayload,
    right: &CadMeshPayload,
) -> CadResult<CadMeshPayload> {
    let min = [
        f64::from(left.bounds.min_mm[0]).max(f64::from(right.bounds.min_mm[0])),
        f64::from(left.bounds.min_mm[1]).max(f64::from(right.bounds.min_mm[1])),
        f64::from(left.bounds.min_mm[2]).max(f64::from(right.bounds.min_mm[2])),
    ];
    let max = [
        f64::from(left.bounds.max_mm[0]).min(f64::from(right.bounds.max_mm[0])),
        f64::from(left.bounds.max_mm[1]).min(f64::from(right.bounds.max_mm[1])),
        f64::from(left.bounds.max_mm[2]).min(f64::from(right.bounds.max_mm[2])),
    ];

    if max[0] <= min[0] || max[1] <= min[1] || max[2] <= min[2] {
        return Ok(left.clone());
    }

    let mut solid = make_cube(max[0] - min[0], max[1] - min[1], max[2] - min[2])?;
    for vertex in solid.topology.vertices.values_mut() {
        vertex.point.x += min[0];
        vertex.point.y += min[1];
        vertex.point.z += min[2];
    }
    let mesh = tessellate_brep(&solid, 32)?;
    let mut payload = triangle_mesh_to_payload(
        &mesh,
        &format!("mesh.mcp.{label}"),
        &format!("variant.mcp.{label}"),
        left.document_revision.max(right.document_revision),
    )?;
    payload.mesh_id = format!("{}+{}", left.mesh_id, label);
    Ok(payload)
}

fn merge_part_meshes(
    part_meshes: &[CadMcpPartMesh],
    mesh_id: &str,
    variant_id: &str,
) -> CadResult<CadMeshPayload> {
    let mut merged = CadMeshPayload {
        mesh_id: mesh_id.to_string(),
        document_revision: 1,
        variant_id: variant_id.to_string(),
        topology: CadMeshTopology::Triangles,
        vertices: Vec::new(),
        triangle_indices: Vec::new(),
        edges: Vec::new(),
        material_slots: vec![CadMeshMaterialSlot::default()],
        bounds: CadMeshBounds {
            min_mm: [0.0, 0.0, 0.0],
            max_mm: [0.0, 0.0, 0.0],
        },
    };

    for part in part_meshes {
        let offset = merged.vertices.len() as u32;
        merged.vertices.extend(part.mesh.vertices.iter().copied());
        merged.triangle_indices.extend(
            part.mesh
                .triangle_indices
                .iter()
                .map(|index| index + offset),
        );
    }

    if merged.vertices.is_empty() || merged.triangle_indices.is_empty() {
        return Err(CadError::ExportFailed {
            format: "mesh".to_string(),
            reason: "document evaluated to empty mesh".to_string(),
        });
    }

    merged.bounds = compute_bounds(&merged.vertices);
    merged.validate_contract()?;
    Ok(merged)
}

fn resolve_output_path(filename: &str) -> CadResult<PathBuf> {
    let current_dir = std::env::current_dir().map_err(|error| CadError::ExportFailed {
        format: "path".to_string(),
        reason: format!("failed to resolve current directory: {error}"),
    })?;
    Ok(current_dir.join(filename))
}

fn file_extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(String::new, |extension| extension.to_ascii_lowercase())
}

fn normalize_material_id(material_id: &str) -> String {
    material_id.trim().to_ascii_lowercase()
}

fn default_materials() -> BTreeMap<String, CadMcpMaterial> {
    BTreeMap::from([
        ("default".to_string(), CadMcpMaterial { density: None }),
        (
            "aluminum".to_string(),
            CadMcpMaterial {
                density: Some(2700.0),
            },
        ),
        (
            "steel".to_string(),
            CadMcpMaterial {
                density: Some(7870.0),
            },
        ),
    ])
}

fn validate_positive_number(label: &str, value: f64) -> CadResult<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{label} must be finite and > 0"),
        });
    }
    Ok(())
}

fn is_finite_vec3(value: &CadMcpVec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn is_positive_vec3(value: &CadMcpVec3) -> bool {
    is_finite_vec3(value) && value.x > 0.0 && value.y > 0.0 && value.z > 0.0
}

fn resolve_position(
    position: &CadMcpPositionSpec,
    base_primitive: &CadMcpPrimitive,
) -> CadResult<CadMcpVec3> {
    let bbox = primitive_bbox(base_primitive)?;

    match position {
        CadMcpPositionSpec::Named(name) => {
            let center = CadMcpVec3 {
                x: (bbox.min.x + bbox.max.x) * 0.5,
                y: (bbox.min.y + bbox.max.y) * 0.5,
                z: (bbox.min.z + bbox.max.z) * 0.5,
            };
            match name.as_str() {
                "center" => Ok(center),
                "top-center" => Ok(CadMcpVec3 {
                    x: center.x,
                    y: center.y,
                    z: bbox.max.z,
                }),
                "bottom-center" => Ok(CadMcpVec3 {
                    x: center.x,
                    y: center.y,
                    z: bbox.min.z,
                }),
                _ => Err(CadError::InvalidParameter {
                    name: "at".to_string(),
                    reason: format!("unsupported named position: {name}"),
                }),
            }
        }
        CadMcpPositionSpec::Object(object) => Ok(CadMcpVec3 {
            x: resolve_coordinate(&object.x, bbox.min.x, bbox.max.x)?,
            y: resolve_coordinate(&object.y, bbox.min.y, bbox.max.y)?,
            z: resolve_coordinate(
                object
                    .z
                    .as_ref()
                    .unwrap_or(&CadMcpCoordinateValue::Number(0.0)),
                bbox.min.z,
                bbox.max.z,
            )?,
        }),
    }
}

fn resolve_coordinate(value: &CadMcpCoordinateValue, min: f64, max: f64) -> CadResult<f64> {
    match value {
        CadMcpCoordinateValue::Number(number) => {
            if !number.is_finite() {
                return Err(CadError::InvalidParameter {
                    name: "coordinate".to_string(),
                    reason: "coordinate must be finite".to_string(),
                });
            }
            Ok(*number)
        }
        CadMcpCoordinateValue::String(raw) => {
            let trimmed = raw.trim();
            let Some(number_text) = trimmed.strip_suffix('%') else {
                return Err(CadError::InvalidParameter {
                    name: "coordinate".to_string(),
                    reason: format!("invalid coordinate value: {raw}"),
                });
            };
            let pct: f64 = number_text
                .parse()
                .map_err(|error| CadError::InvalidParameter {
                    name: "coordinate".to_string(),
                    reason: format!("invalid percentage coordinate '{raw}': {error}"),
                })?;
            Ok(min + ((max - min) * (pct / 100.0)))
        }
    }
}

#[derive(Clone, Copy)]
struct PrimitiveBounds {
    min: CadMcpVec3,
    max: CadMcpVec3,
}

fn primitive_bbox(primitive: &CadMcpPrimitive) -> CadResult<PrimitiveBounds> {
    match primitive.primitive_type {
        CadMcpPrimitiveType::Cube => {
            let size = primitive.size.clone().unwrap_or(CadMcpVec3 {
                x: 10.0,
                y: 10.0,
                z: 10.0,
            });
            validate_positive_number("cube size.x", size.x)?;
            validate_positive_number("cube size.y", size.y)?;
            validate_positive_number("cube size.z", size.z)?;
            Ok(PrimitiveBounds {
                min: CadMcpVec3::default(),
                max: size,
            })
        }
        CadMcpPrimitiveType::Cylinder => {
            let radius = primitive.radius.unwrap_or(5.0);
            let height = primitive.height.unwrap_or(10.0);
            validate_positive_number("cylinder radius", radius)?;
            validate_positive_number("cylinder height", height)?;
            Ok(PrimitiveBounds {
                min: CadMcpVec3 {
                    x: -radius,
                    y: -radius,
                    z: 0.0,
                },
                max: CadMcpVec3 {
                    x: radius,
                    y: radius,
                    z: height,
                },
            })
        }
        CadMcpPrimitiveType::Sphere => {
            let radius = primitive.radius.unwrap_or(5.0);
            validate_positive_number("sphere radius", radius)?;
            Ok(PrimitiveBounds {
                min: CadMcpVec3 {
                    x: -radius,
                    y: -radius,
                    z: -radius,
                },
                max: CadMcpVec3 {
                    x: radius,
                    y: radius,
                    z: radius,
                },
            })
        }
        CadMcpPrimitiveType::Cone => {
            let radius_bottom = primitive.radius_bottom.or(primitive.radius).unwrap_or(5.0);
            let radius_top = primitive.radius_top.unwrap_or(0.0);
            let height = primitive.height.unwrap_or(10.0);
            validate_positive_number("cone radius_bottom", radius_bottom)?;
            if !radius_top.is_finite() || radius_top < 0.0 {
                return Err(CadError::InvalidPrimitive {
                    reason: "cone radius_top must be finite and >= 0".to_string(),
                });
            }
            validate_positive_number("cone height", height)?;
            let max_radius = radius_bottom.max(radius_top);
            Ok(PrimitiveBounds {
                min: CadMcpVec3 {
                    x: -max_radius,
                    y: -max_radius,
                    z: 0.0,
                },
                max: CadMcpVec3 {
                    x: max_radius,
                    y: max_radius,
                    z: height,
                },
            })
        }
    }
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

pub fn mcp_document_hash(document: &CadMcpDocument) -> CadResult<String> {
    let serialized = serde_json::to_vec(document).map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize mcp document for hashing: {error}"),
    })?;
    Ok(stable_hex_digest(&serialized))
}

pub fn mcp_export_step_preview(document: &CadMcpDocument) -> CadResult<String> {
    let part_meshes = evaluate_document_part_meshes(document)?;
    if part_meshes.is_empty() {
        return Err(CadError::ExportFailed {
            format: "step".to_string(),
            reason: "document has no parts to preview STEP export".to_string(),
        });
    }

    let merged = merge_part_meshes(
        &part_meshes,
        "mesh.mcp.step_preview",
        "variant.mcp.step_preview",
    )?;
    let artifact = export_step_from_mesh(
        "cad-mcp.export.preview",
        merged.document_revision,
        &merged.variant_id,
        &merged,
    )?;
    String::from_utf8(artifact.bytes).map_err(|error| CadError::Serialization {
        reason: format!("failed to decode step preview bytes: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CadMcpCreateFormat, CadMcpCreateInput, CadMcpOperation, CadMcpPartInput, CadMcpPrimitive,
        CadMcpPrimitiveType, CadMcpVec3, create_cad_document, mcp_document_hash,
    };

    fn cube_part() -> CadMcpPartInput {
        CadMcpPartInput {
            name: "cube".to_string(),
            primitive: CadMcpPrimitive {
                primitive_type: CadMcpPrimitiveType::Cube,
                size: Some(CadMcpVec3 {
                    x: 10.0,
                    y: 10.0,
                    z: 10.0,
                }),
                radius: None,
                height: None,
                segments: None,
                radius_bottom: None,
                radius_top: None,
            },
            operations: vec![CadMcpOperation::Translate {
                offset: CadMcpVec3 {
                    x: 5.0,
                    y: 0.0,
                    z: 0.0,
                },
            }],
            material: Some("aluminum".to_string()),
        }
    }

    #[test]
    fn create_document_returns_compact_by_default() {
        let result = create_cad_document(CadMcpCreateInput {
            parts: vec![cube_part()],
            format: None,
        })
        .expect("create document");
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].r#type, "text");
        assert!(!result.content[0].text.contains('\n'));
    }

    #[test]
    fn create_document_json_format_is_pretty() {
        let result = create_cad_document(CadMcpCreateInput {
            parts: vec![cube_part()],
            format: Some(CadMcpCreateFormat::Json),
        })
        .expect("create document");
        assert!(result.content[0].text.contains('\n'));
    }

    #[test]
    fn document_hash_is_deterministic() {
        let first = create_cad_document(CadMcpCreateInput {
            parts: vec![cube_part()],
            format: None,
        })
        .expect("first")
        .content
        .first()
        .expect("first text")
        .text
        .clone();
        let second = create_cad_document(CadMcpCreateInput {
            parts: vec![cube_part()],
            format: None,
        })
        .expect("second")
        .content
        .first()
        .expect("second text")
        .text
        .clone();

        let first_doc: super::CadMcpDocument =
            serde_json::from_str(&first).expect("parse first document");
        let second_doc: super::CadMcpDocument =
            serde_json::from_str(&second).expect("parse second document");

        let first_hash = mcp_document_hash(&first_doc).expect("first hash");
        let second_hash = mcp_document_hash(&second_doc).expect("second hash");
        assert_eq!(first_hash, second_hash);
    }
}
