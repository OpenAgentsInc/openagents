use serde::Serialize;

use crate::hash::stable_hex_digest;
use crate::mesh::CadMeshPayload;
use crate::{CadError, CadResult};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_VERSION: u32 = 2;
const CHUNK_TYPE_JSON: u32 = 0x4E4F534A;
const CHUNK_TYPE_BIN: u32 = 0x004E4942;

/// Stable receipt emitted for deterministic GLB exports.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CadGlbExportReceipt {
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub mesh_id: String,
    pub vertex_count: usize,
    pub index_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
}

/// Deterministic GLB export artifact.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CadGlbExportArtifact {
    pub receipt: CadGlbExportReceipt,
    pub bytes: Vec<u8>,
}

/// Export a CAD mesh payload to binary GLB bytes.
pub fn export_glb_from_mesh(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    mesh: &CadMeshPayload,
) -> CadResult<CadGlbExportArtifact> {
    validate_glb_identity(document_id, variant_id)?;
    mesh.validate_contract()
        .map_err(|error| glb_export_failed(format!("mesh payload is invalid: {error}")))?;
    if mesh.variant_id != variant_id {
        return Err(glb_export_failed(format!(
            "mesh variant_id mismatch: payload={} requested={variant_id}",
            mesh.variant_id
        )));
    }

    let vertex_count = mesh.vertices.len();
    let index_count = mesh.triangle_indices.len();
    if vertex_count == 0 || index_count == 0 {
        return Err(glb_export_failed("mesh has no geometry"));
    }

    let mut bin = Vec::<u8>::with_capacity(vertex_count * 3 * 4 + index_count * 4);
    for vertex in &mesh.vertices {
        for value in vertex.position_mm {
            bin.extend_from_slice(&value.to_le_bytes());
        }
    }
    for index in &mesh.triangle_indices {
        bin.extend_from_slice(&index.to_le_bytes());
    }

    let positions_byte_len = vertex_count * 3 * 4;
    let indices_byte_len = index_count * 4;
    let json = build_gltf_json(
        positions_byte_len,
        indices_byte_len,
        vertex_count,
        index_count,
        mesh.bounds.min_mm,
        mesh.bounds.max_mm,
    )?;

    let json_padding = (4 - (json.len() % 4)) % 4;
    let mut json_chunk = json;
    json_chunk.extend(std::iter::repeat_n(0x20, json_padding));

    let bin_padding = (4 - (bin.len() % 4)) % 4;
    bin.extend(std::iter::repeat_n(0u8, bin_padding));

    let total_length = 12 + 8 + json_chunk.len() + 8 + bin.len();
    let mut bytes = Vec::<u8>::with_capacity(total_length);

    // GLB header.
    bytes.extend_from_slice(GLB_MAGIC);
    bytes.extend_from_slice(&GLB_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(total_length as u32).to_le_bytes());

    // JSON chunk.
    bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
    bytes.extend_from_slice(&json_chunk);

    // BIN chunk.
    bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
    bytes.extend_from_slice(&bin);

    let receipt = CadGlbExportReceipt {
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        mesh_id: mesh.mesh_id.clone(),
        vertex_count,
        index_count,
        byte_count: bytes.len(),
        deterministic_hash: stable_hex_digest(&bytes),
    };

    Ok(CadGlbExportArtifact { receipt, bytes })
}

#[derive(Serialize)]
struct GltfAsset<'a> {
    version: &'a str,
    generator: &'a str,
}

#[derive(Serialize)]
struct GltfScene {
    nodes: [u32; 1],
}

#[derive(Serialize)]
struct GltfNode {
    mesh: u32,
}

#[derive(Serialize)]
struct GltfPrimitive {
    attributes: GltfAttributes,
    indices: u32,
}

#[derive(Serialize)]
struct GltfAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
}

#[derive(Serialize)]
struct GltfMesh {
    primitives: [GltfPrimitive; 1],
}

#[derive(Serialize)]
struct GltfAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    accessor_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<[f32; 3]>,
}

#[derive(Serialize)]
struct GltfBufferView {
    buffer: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    target: u32,
}

#[derive(Serialize)]
struct GltfBuffer {
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Serialize)]
struct GltfDocument<'a> {
    asset: GltfAsset<'a>,
    scene: u32,
    scenes: [GltfScene; 1],
    nodes: [GltfNode; 1],
    meshes: [GltfMesh; 1],
    accessors: [GltfAccessor; 2],
    #[serde(rename = "bufferViews")]
    buffer_views: [GltfBufferView; 2],
    buffers: [GltfBuffer; 1],
}

fn build_gltf_json(
    positions_byte_len: usize,
    indices_byte_len: usize,
    vertex_count: usize,
    index_count: usize,
    min: [f32; 3],
    max: [f32; 3],
) -> CadResult<Vec<u8>> {
    let document = GltfDocument {
        asset: GltfAsset {
            version: "2.0",
            generator: "vcad",
        },
        scene: 0,
        scenes: [GltfScene { nodes: [0] }],
        nodes: [GltfNode { mesh: 0 }],
        meshes: [GltfMesh {
            primitives: [GltfPrimitive {
                attributes: GltfAttributes { position: 0 },
                indices: 1,
            }],
        }],
        accessors: [
            GltfAccessor {
                buffer_view: 0,
                component_type: 5126,
                count: vertex_count,
                accessor_type: "VEC3",
                min: Some(min),
                max: Some(max),
            },
            GltfAccessor {
                buffer_view: 1,
                component_type: 5125,
                count: index_count,
                accessor_type: "SCALAR",
                min: None,
                max: None,
            },
        ],
        buffer_views: [
            GltfBufferView {
                buffer: 0,
                byte_offset: 0,
                byte_length: positions_byte_len,
                target: 34962,
            },
            GltfBufferView {
                buffer: 0,
                byte_offset: positions_byte_len,
                byte_length: indices_byte_len,
                target: 34963,
            },
        ],
        buffers: [GltfBuffer {
            byte_length: positions_byte_len + indices_byte_len,
        }],
    };

    serde_json::to_vec(&document)
        .map_err(|error| glb_export_failed(format!("failed to serialize glb json chunk: {error}")))
}

fn validate_glb_identity(document_id: &str, variant_id: &str) -> CadResult<()> {
    if document_id.trim().is_empty() {
        return Err(glb_export_failed("document id must not be empty"));
    }
    if variant_id.trim().is_empty() {
        return Err(glb_export_failed("variant id must not be empty"));
    }
    Ok(())
}

fn glb_export_failed(reason: impl Into<String>) -> CadError {
    CadError::ExportFailed {
        format: "glb".to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::export_glb_from_mesh;
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn sample_tetra_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.glb.sample".to_string(),
            document_revision: 33,
            variant_id: "variant.glb".to_string(),
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
                    position_mm: [20.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 20.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 20.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 0.5],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3, 0, 2, 3],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [20.0, 20.0, 20.0],
            },
        }
    }

    #[test]
    fn glb_export_is_deterministic_and_has_valid_header() {
        let mesh = sample_tetra_mesh();
        let first =
            export_glb_from_mesh("doc.glb", mesh.document_revision, &mesh.variant_id, &mesh)
                .expect("first glb export");
        let second =
            export_glb_from_mesh("doc.glb", mesh.document_revision, &mesh.variant_id, &mesh)
                .expect("second glb export");

        assert_eq!(first.bytes, second.bytes);
        assert_eq!(
            first.receipt.deterministic_hash,
            second.receipt.deterministic_hash
        );
        assert_eq!(&first.bytes[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(first.bytes[4..8].try_into().unwrap()), 2);
        assert_eq!(first.receipt.vertex_count, 4);
        assert_eq!(first.receipt.index_count, 12);
    }

    #[test]
    fn glb_export_rejects_variant_mismatch() {
        let mesh = sample_tetra_mesh();
        let error = export_glb_from_mesh("doc.glb", mesh.document_revision, "variant.other", &mesh)
            .expect_err("variant mismatch should fail");
        assert!(
            error
                .to_string()
                .contains("mesh variant_id mismatch: payload=variant.glb requested=variant.other")
        );
    }

    #[test]
    fn glb_export_rejects_invalid_mesh_contract() {
        let mut mesh = sample_tetra_mesh();
        mesh.triangle_indices.clear();

        let error =
            export_glb_from_mesh("doc.glb", mesh.document_revision, &mesh.variant_id, &mesh)
                .expect_err("invalid mesh should fail");
        assert!(error.to_string().contains(
            "mesh payload is invalid: invalid primitive: mesh payload must include triangle indices"
        ));
    }
}
