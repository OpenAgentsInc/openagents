use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::hash::stable_hex_digest;
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::{CadError, CadResult};

/// vcad parity header label used for binary STL export.
pub const STL_BINARY_HEADER_LABEL: &str = "vcad binary STL export";

/// Stable receipt emitted for deterministic STL exports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStlExportReceipt {
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub mesh_id: String,
    pub triangle_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
}

/// Deterministic STL export artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStlExportArtifact {
    pub receipt: CadStlExportReceipt,
    pub bytes: Vec<u8>,
}

/// Parsed STL source format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadStlSourceFormat {
    Binary,
    Ascii,
}

impl CadStlSourceFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Ascii => "ascii",
        }
    }
}

/// STL import result converted to CAD mesh contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStlImportResult {
    pub source_format: CadStlSourceFormat,
    pub triangle_count: usize,
    pub unique_vertex_count: usize,
    pub import_hash: String,
    pub mesh: CadMeshPayload,
}

/// Export a CAD mesh payload to binary STL bytes.
pub fn export_stl_from_mesh(
    document_id: &str,
    document_revision: u64,
    variant_id: &str,
    mesh: &CadMeshPayload,
) -> CadResult<CadStlExportArtifact> {
    validate_stl_identity(document_id, variant_id)?;
    mesh.validate_contract()
        .map_err(|error| stl_export_failed(format!("mesh payload is invalid: {error}")))?;
    if mesh.variant_id != variant_id {
        return Err(stl_export_failed(format!(
            "mesh variant_id mismatch: payload={} requested={variant_id}",
            mesh.variant_id
        )));
    }

    let triangle_count = mesh.triangle_indices.len() / 3;
    if triangle_count == 0 {
        return Err(stl_export_failed("mesh has zero triangles"));
    }

    let mut bytes = Vec::with_capacity(84 + triangle_count * 50);
    let mut header = [0u8; 80];
    let label_bytes = STL_BINARY_HEADER_LABEL.as_bytes();
    let copy_len = label_bytes.len().min(80);
    header[..copy_len].copy_from_slice(&label_bytes[..copy_len]);
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&(triangle_count as u32).to_le_bytes());

    for triangle in mesh.triangle_indices.chunks_exact(3) {
        let a = triangle_vertex(mesh, triangle[0])?;
        let b = triangle_vertex(mesh, triangle[1])?;
        let c = triangle_vertex(mesh, triangle[2])?;

        let normal = face_normal(a, b, c);
        for value in normal {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for vertex in [a, b, c] {
            for value in vertex {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }

    let receipt = CadStlExportReceipt {
        document_id: document_id.to_string(),
        document_revision,
        variant_id: variant_id.to_string(),
        mesh_id: mesh.mesh_id.clone(),
        triangle_count,
        byte_count: bytes.len(),
        deterministic_hash: stable_hex_digest(&bytes),
    };
    Ok(CadStlExportArtifact { receipt, bytes })
}

/// Import STL bytes (binary or ASCII) into CAD mesh contract payload.
pub fn import_stl_to_mesh(
    document_revision: u64,
    variant_id: &str,
    stl_bytes: &[u8],
) -> CadResult<CadStlImportResult> {
    if variant_id.trim().is_empty() {
        return Err(stl_parse_failed("variant id must not be empty"));
    }
    if stl_bytes.is_empty() {
        return Err(stl_parse_failed("stl payload is empty"));
    }

    let source_format = if is_ascii_stl(stl_bytes) {
        CadStlSourceFormat::Ascii
    } else {
        CadStlSourceFormat::Binary
    };

    let raw = match source_format {
        CadStlSourceFormat::Binary => parse_binary_stl(stl_bytes)?,
        CadStlSourceFormat::Ascii => parse_ascii_stl(stl_bytes)?,
    };

    let import_hash = stable_hex_digest(stl_bytes);
    let mesh = raw_to_mesh(document_revision, variant_id, &import_hash, raw)?;
    let triangle_count = mesh.triangle_indices.len() / 3;
    let unique_vertex_count = mesh.vertices.len();

    Ok(CadStlImportResult {
        source_format,
        triangle_count,
        unique_vertex_count,
        import_hash,
        mesh,
    })
}

#[derive(Default)]
struct StlRawMesh {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn parse_binary_stl(bytes: &[u8]) -> CadResult<StlRawMesh> {
    if bytes.len() < 84 {
        return Err(stl_parse_failed(format!(
            "Invalid STL: expected at least 84 bytes, got {}",
            bytes.len()
        )));
    }

    let triangle_count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected_size = 84usize
        .checked_add(triangle_count.checked_mul(50).ok_or_else(|| {
            stl_parse_failed("Invalid STL: triangle count overflows binary payload size")
        })?)
        .ok_or_else(|| stl_parse_failed("Invalid STL: payload size overflow"))?;

    if bytes.len() < expected_size {
        return Err(stl_parse_failed(format!(
            "Invalid STL: expected {expected_size} bytes, got {}",
            bytes.len()
        )));
    }

    let mut vertex_map = HashMap::<(u32, u32, u32), u32>::new();
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::with_capacity(triangle_count * 3);
    let mut offset = 84usize;

    for _ in 0..triangle_count {
        offset += 12; // skip normal
        for _ in 0..3 {
            let x = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                bytes[offset + 8],
                bytes[offset + 9],
                bytes[offset + 10],
                bytes[offset + 11],
            ]);
            offset += 12;

            let key = (x.to_bits(), y.to_bits(), z.to_bits());
            let index = if let Some(index) = vertex_map.get(&key) {
                *index
            } else {
                let index = positions.len() as u32;
                positions.push([x, y, z]);
                vertex_map.insert(key, index);
                index
            };
            indices.push(index);
        }
        offset += 2; // attribute bytes
    }

    Ok(StlRawMesh { positions, indices })
}

fn parse_ascii_stl(bytes: &[u8]) -> CadResult<StlRawMesh> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| stl_parse_failed(format!("invalid ascii stl utf-8: {error}")))?;

    let mut vertex_map = HashMap::<(u32, u32, u32), u32>::new();
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("vertex") {
            continue;
        }

        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 {
            return Err(stl_parse_failed(format!(
                "Invalid vertex coordinates: {line}"
            )));
        }

        let x = parts[1]
            .parse::<f32>()
            .map_err(|_| stl_parse_failed(format!("Invalid vertex coordinates: {line}")))?;
        let y = parts[2]
            .parse::<f32>()
            .map_err(|_| stl_parse_failed(format!("Invalid vertex coordinates: {line}")))?;
        let z = parts[3]
            .parse::<f32>()
            .map_err(|_| stl_parse_failed(format!("Invalid vertex coordinates: {line}")))?;

        let key = (x.to_bits(), y.to_bits(), z.to_bits());
        let index = if let Some(index) = vertex_map.get(&key) {
            *index
        } else {
            let index = positions.len() as u32;
            positions.push([x, y, z]);
            vertex_map.insert(key, index);
            index
        };
        indices.push(index);
    }

    if indices.is_empty() {
        return Err(stl_parse_failed("No vertices found in ASCII STL"));
    }
    if !indices.len().is_multiple_of(3) {
        return Err(stl_parse_failed(format!(
            "Invalid STL: vertex count {} is not divisible by 3",
            indices.len()
        )));
    }

    Ok(StlRawMesh { positions, indices })
}

fn is_ascii_stl(bytes: &[u8]) -> bool {
    let header_len = bytes.len().min(80);
    let header = String::from_utf8_lossy(&bytes[..header_len]);
    if !header
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("solid")
    {
        return false;
    }

    let sample_len = bytes.len().min(1024);
    let sample = String::from_utf8_lossy(&bytes[..sample_len]).to_ascii_lowercase();
    sample.contains("facet")
}

fn raw_to_mesh(
    document_revision: u64,
    variant_id: &str,
    import_hash: &str,
    raw: StlRawMesh,
) -> CadResult<CadMeshPayload> {
    if raw.positions.is_empty() || raw.indices.is_empty() {
        return Err(stl_parse_failed("stl mesh produced no triangles"));
    }

    let bounds = compute_bounds(&raw.positions)?;
    let normals = compute_vertex_normals(&raw.positions, &raw.indices);
    let vertices = raw
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| CadMeshVertex {
            position_mm: *position,
            normal: normals[index],
            uv: [0.0, 0.0],
            material_slot: 0,
            flags: 0,
        })
        .collect::<Vec<_>>();

    let mesh = CadMeshPayload {
        mesh_id: format!("mesh.stl.{}", &import_hash[..16]),
        document_revision,
        variant_id: variant_id.to_string(),
        topology: CadMeshTopology::Triangles,
        vertices,
        triangle_indices: raw.indices,
        edges: Vec::new(),
        material_slots: vec![CadMeshMaterialSlot::default()],
        bounds,
    };
    mesh.validate_contract().map_err(|error| {
        stl_parse_failed(format!(
            "stl import generated invalid mesh payload: {error}"
        ))
    })?;
    Ok(mesh)
}

fn compute_bounds(positions: &[[f32; 3]]) -> CadResult<CadMeshBounds> {
    let mut iter = positions.iter();
    let Some(first) = iter.next() else {
        return Err(stl_parse_failed("stl mesh has no positions"));
    };
    let mut min = *first;
    let mut max = *first;

    for point in iter {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }

    Ok(CadMeshBounds {
        min_mm: min,
        max_mm: max,
    })
}

fn compute_vertex_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32, 0.0f32, 0.0f32]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let a = positions[triangle[0] as usize];
        let b = positions[triangle[1] as usize];
        let c = positions[triangle[2] as usize];
        let face = face_normal(a, b, c);
        for index in triangle {
            let slot = &mut normals[*index as usize];
            slot[0] += face[0];
            slot[1] += face[1];
            slot[2] += face[2];
        }
    }

    normals.into_iter().map(normalize_or_default).collect()
}

fn normalize_or_default(normal: [f32; 3]) -> [f32; 3] {
    let length =
        ((normal[0] as f64).powi(2) + (normal[1] as f64).powi(2) + (normal[2] as f64).powi(2))
            .sqrt();
    if length <= 1.0e-12 {
        return [0.0, 0.0, 1.0];
    }

    [
        (normal[0] as f64 / length) as f32,
        (normal[1] as f64 / length) as f32,
        (normal[2] as f64 / length) as f32,
    ]
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ux = (b[0] - a[0]) as f64;
    let uy = (b[1] - a[1]) as f64;
    let uz = (b[2] - a[2]) as f64;
    let vx = (c[0] - a[0]) as f64;
    let vy = (c[1] - a[1]) as f64;
    let vz = (c[2] - a[2]) as f64;

    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    let length = (nx * nx + ny * ny + nz * nz).sqrt();
    if length <= 1.0e-12 {
        return [0.0, 0.0, 0.0];
    }

    [
        (nx / length) as f32,
        (ny / length) as f32,
        (nz / length) as f32,
    ]
}

fn triangle_vertex(mesh: &CadMeshPayload, index: u32) -> CadResult<[f32; 3]> {
    let vertex = mesh
        .vertices
        .get(index as usize)
        .ok_or_else(|| stl_export_failed(format!("triangle index out of range: {index}")))?;
    if !vertex.position_mm.iter().all(|value| value.is_finite()) {
        return Err(stl_export_failed(format!(
            "triangle vertex contains non-finite coordinate at index {index}"
        )));
    }
    Ok(vertex.position_mm)
}

fn validate_stl_identity(document_id: &str, variant_id: &str) -> CadResult<()> {
    if document_id.trim().is_empty() {
        return Err(stl_export_failed("document id must not be empty"));
    }
    if variant_id.trim().is_empty() {
        return Err(stl_export_failed("variant id must not be empty"));
    }
    Ok(())
}

fn stl_export_failed(reason: impl Into<String>) -> CadError {
    CadError::ExportFailed {
        format: "stl".to_string(),
        reason: reason.into(),
    }
}

fn stl_parse_failed(reason: impl Into<String>) -> CadError {
    CadError::ParseFailed {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadStlSourceFormat, STL_BINARY_HEADER_LABEL, export_stl_from_mesh, import_stl_to_mesh,
    };
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn sample_tetra_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.stl.sample".to_string(),
            document_revision: 21,
            variant_id: "variant.stl".to_string(),
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
    fn binary_stl_export_is_deterministic_and_round_trips() {
        let mesh = sample_tetra_mesh();
        let first =
            export_stl_from_mesh("doc.stl", mesh.document_revision, &mesh.variant_id, &mesh)
                .expect("first stl export");
        let second =
            export_stl_from_mesh("doc.stl", mesh.document_revision, &mesh.variant_id, &mesh)
                .expect("second stl export");
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(
            first.receipt.deterministic_hash,
            second.receipt.deterministic_hash
        );
        assert_eq!(first.receipt.triangle_count, 4);

        let header = String::from_utf8_lossy(&first.bytes[..80]);
        assert!(header.starts_with(STL_BINARY_HEADER_LABEL));

        let imported = import_stl_to_mesh(mesh.document_revision, &mesh.variant_id, &first.bytes)
            .expect("import exported stl");
        assert_eq!(imported.source_format, CadStlSourceFormat::Binary);
        assert_eq!(imported.triangle_count, 4);
        assert_eq!(imported.unique_vertex_count, 4);
    }

    #[test]
    fn ascii_stl_import_matches_vcad_contract() {
        let ascii = "solid fixture\n\
  facet normal 0 0 1\n\
    outer loop\n\
      vertex 0 0 0\n\
      vertex 10 0 0\n\
      vertex 0 10 0\n\
    endloop\n\
  endfacet\n\
  facet normal 0 0 1\n\
    outer loop\n\
      vertex 10 0 0\n\
      vertex 10 10 0\n\
      vertex 0 10 0\n\
    endloop\n\
  endfacet\n\
endsolid fixture\n";

        let result = import_stl_to_mesh(5, "variant.ascii", ascii.as_bytes())
            .expect("ascii stl should parse");
        assert_eq!(result.source_format, CadStlSourceFormat::Ascii);
        assert_eq!(result.triangle_count, 2);
        assert_eq!(result.unique_vertex_count, 4);
        assert_eq!(result.mesh.triangle_indices.len(), 6);
    }

    #[test]
    fn binary_stl_import_rejects_truncated_payload() {
        let error = import_stl_to_mesh(5, "variant.stl", &[0u8; 12])
            .expect_err("truncated stl should fail");
        assert!(
            error
                .to_string()
                .contains("parse failed: Invalid STL: expected at least 84 bytes, got 12")
        );
    }

    #[test]
    fn ascii_stl_import_rejects_missing_vertices() {
        let payload =
            b"solid missing\nfacet normal 0 0 1\nouter loop\nendloop\nendfacet\nendsolid missing\n";
        let error = import_stl_to_mesh(5, "variant.stl", payload)
            .expect_err("ascii without vertices should fail");
        assert!(
            error
                .to_string()
                .contains("parse failed: No vertices found in ASCII STL")
        );
    }

    #[test]
    fn stl_export_rejects_variant_mismatch() {
        let mesh = sample_tetra_mesh();
        let error = export_stl_from_mesh("doc.stl", mesh.document_revision, "variant.other", &mesh)
            .expect_err("variant mismatch should fail");
        assert!(
            error
                .to_string()
                .contains("mesh variant_id mismatch: payload=variant.stl requested=variant.other")
        );
    }
}
