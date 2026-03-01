use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Binary contract revision for CAD mesh payloads consumed by render surfaces.
pub const CAD_MESH_BINARY_CONTRACT_VERSION: u16 = 1;

/// Primitive topology emitted by CAD mesh payloads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadMeshTopology {
    #[default]
    Triangles,
}

/// Axis-aligned mesh bounds in canonical units (millimeters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadMeshBounds {
    pub min_mm: [f32; 3],
    pub max_mm: [f32; 3],
}

/// Renderer-facing packed vertex contract.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadMeshVertex {
    pub position_mm: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub material_slot: u16,
    pub flags: u16,
}

impl CadMeshVertex {
    pub const BINARY_SIZE: usize = 36;

    fn append_le_bytes(&self, out: &mut Vec<u8>) {
        for value in self.position_mm {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.normal {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.uv {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.material_slot.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
    }
}

/// Renderer-facing explicit edge segment for edge/silhouette overlays.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadMeshEdgeSegment {
    pub start_vertex: u32,
    pub end_vertex: u32,
    pub flags: u32,
}

impl CadMeshEdgeSegment {
    pub const BINARY_SIZE: usize = 12;

    fn append_le_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.start_vertex.to_le_bytes());
        out.extend_from_slice(&self.end_vertex.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
    }
}

/// Renderer-facing material slot metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMeshMaterialSlot {
    pub slot: u16,
    pub reserved: u16,
    pub base_color_rgba: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for CadMeshMaterialSlot {
    fn default() -> Self {
        Self {
            slot: 0,
            reserved: 0,
            base_color_rgba: [0.78, 0.78, 0.80, 1.0],
            roughness: 0.5,
            metallic: 0.0,
        }
    }
}

impl CadMeshMaterialSlot {
    pub const BINARY_SIZE: usize = 28;

    fn append_le_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.slot.to_le_bytes());
        out.extend_from_slice(&self.reserved.to_le_bytes());
        for value in self.base_color_rgba {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.roughness.to_le_bytes());
        out.extend_from_slice(&self.metallic.to_le_bytes());
    }
}

/// CAD mesh contract shared between deterministic eval/tessellation and renderer integration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CadMeshPayload {
    pub mesh_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub topology: CadMeshTopology,
    pub vertices: Vec<CadMeshVertex>,
    pub triangle_indices: Vec<u32>,
    pub edges: Vec<CadMeshEdgeSegment>,
    pub material_slots: Vec<CadMeshMaterialSlot>,
    pub bounds: CadMeshBounds,
}

/// Little-endian binary sections consumed by render backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadMeshBinaryPayload {
    pub contract_version: u16,
    pub vertex_bytes: Vec<u8>,
    pub index_bytes: Vec<u8>,
    pub edge_bytes: Vec<u8>,
    pub material_bytes: Vec<u8>,
    pub deterministic_hash: String,
}

impl CadMeshPayload {
    /// Validate mesh contract invariants before renderer ingestion.
    pub fn validate_contract(&self) -> CadResult<()> {
        if self.mesh_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "mesh payload requires non-empty mesh_id".to_string(),
            });
        }
        if self.variant_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "mesh payload requires non-empty variant_id".to_string(),
            });
        }
        if self.vertices.is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "mesh payload must include at least one vertex".to_string(),
            });
        }
        if self.triangle_indices.is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "mesh payload must include triangle indices".to_string(),
            });
        }
        if !self.triangle_indices.len().is_multiple_of(3) {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "triangle index count {} must be divisible by 3",
                    self.triangle_indices.len()
                ),
            });
        }
        if self.material_slots.is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "mesh payload requires at least one material slot".to_string(),
            });
        }

        validate_bounds(self.bounds)?;

        let mut material_slots = BTreeSet::<u16>::new();
        for material in &self.material_slots {
            if !material
                .base_color_rgba
                .iter()
                .all(|value| value.is_finite())
                || !material.roughness.is_finite()
                || !material.metallic.is_finite()
            {
                return Err(CadError::InvalidPrimitive {
                    reason: format!("material slot {} contains non-finite values", material.slot),
                });
            }
            if !material_slots.insert(material.slot) {
                return Err(CadError::InvalidPrimitive {
                    reason: format!("duplicate material slot {}", material.slot),
                });
            }
        }

        for (index, vertex) in self.vertices.iter().enumerate() {
            if !vertex.position_mm.iter().all(|value| value.is_finite())
                || !vertex.normal.iter().all(|value| value.is_finite())
                || !vertex.uv.iter().all(|value| value.is_finite())
            {
                return Err(CadError::InvalidPrimitive {
                    reason: format!("vertex {index} contains non-finite values"),
                });
            }
            if !material_slots.contains(&vertex.material_slot) {
                return Err(CadError::InvalidPrimitive {
                    reason: format!(
                        "vertex {index} references missing material slot {}",
                        vertex.material_slot
                    ),
                });
            }
        }

        let vertex_count = self.vertices.len() as u32;
        for (index, triangle_index) in self.triangle_indices.iter().enumerate() {
            if *triangle_index >= vertex_count {
                return Err(CadError::InvalidPrimitive {
                    reason: format!(
                        "triangle index {index} out of range: {triangle_index} >= {vertex_count}"
                    ),
                });
            }
        }

        for (index, edge) in self.edges.iter().enumerate() {
            if edge.start_vertex >= vertex_count || edge.end_vertex >= vertex_count {
                return Err(CadError::InvalidPrimitive {
                    reason: format!(
                        "edge {index} index out of range: {}->{}, vertex_count={vertex_count}",
                        edge.start_vertex, edge.end_vertex
                    ),
                });
            }
            if edge.start_vertex == edge.end_vertex {
                return Err(CadError::InvalidPrimitive {
                    reason: format!("edge {index} start and end vertices must differ"),
                });
            }
        }

        Ok(())
    }

    /// Encode deterministic little-endian binary sections for renderer consumption.
    pub fn to_binary_payload(&self) -> CadResult<CadMeshBinaryPayload> {
        self.validate_contract()?;
        let mut vertex_bytes = Vec::with_capacity(self.vertices.len() * CadMeshVertex::BINARY_SIZE);
        for vertex in &self.vertices {
            vertex.append_le_bytes(&mut vertex_bytes);
        }

        let mut index_bytes = Vec::with_capacity(self.triangle_indices.len() * 4);
        for index in &self.triangle_indices {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        }

        let mut edge_bytes = Vec::with_capacity(self.edges.len() * CadMeshEdgeSegment::BINARY_SIZE);
        for edge in &self.edges {
            edge.append_le_bytes(&mut edge_bytes);
        }

        let mut material_bytes =
            Vec::with_capacity(self.material_slots.len() * CadMeshMaterialSlot::BINARY_SIZE);
        for material in &self.material_slots {
            material.append_le_bytes(&mut material_bytes);
        }

        let deterministic_hash = deterministic_mesh_hash(
            self,
            &vertex_bytes,
            &index_bytes,
            &edge_bytes,
            &material_bytes,
        );
        Ok(CadMeshBinaryPayload {
            contract_version: CAD_MESH_BINARY_CONTRACT_VERSION,
            vertex_bytes,
            index_bytes,
            edge_bytes,
            material_bytes,
            deterministic_hash,
        })
    }
}

fn validate_bounds(bounds: CadMeshBounds) -> CadResult<()> {
    if !bounds
        .min_mm
        .iter()
        .chain(bounds.max_mm.iter())
        .all(|value| value.is_finite())
    {
        return Err(CadError::InvalidPrimitive {
            reason: "mesh bounds must be finite".to_string(),
        });
    }
    for axis in 0..3 {
        if bounds.min_mm[axis] > bounds.max_mm[axis] {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "mesh bounds min/max invalid on axis {axis}: {} > {}",
                    bounds.min_mm[axis], bounds.max_mm[axis]
                ),
            });
        }
    }
    Ok(())
}

fn deterministic_mesh_hash(
    payload: &CadMeshPayload,
    vertex_bytes: &[u8],
    index_bytes: &[u8],
    edge_bytes: &[u8],
    material_bytes: &[u8],
) -> String {
    let mut bytes = Vec::<u8>::new();
    bytes.extend_from_slice(payload.mesh_id.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(payload.variant_id.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&payload.document_revision.to_le_bytes());
    bytes.push(match payload.topology {
        CadMeshTopology::Triangles => 1,
    });
    for value in payload.bounds.min_mm {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in payload.bounds.max_mm {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    append_section(&mut bytes, vertex_bytes);
    append_section(&mut bytes, index_bytes);
    append_section(&mut bytes, edge_bytes);
    append_section(&mut bytes, material_bytes);
    format!("{:016x}", fnv1a64(&bytes))
}

fn append_section(out: &mut Vec<u8>, section: &[u8]) {
    out.extend_from_slice(&(section.len() as u64).to_le_bytes());
    out.extend_from_slice(section);
}

fn fnv1a64(input: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use super::{
        CAD_MESH_BINARY_CONTRACT_VERSION, CadMeshBounds, CadMeshEdgeSegment, CadMeshMaterialSlot,
        CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };
    use crate::CadError;

    fn sample_payload() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.rack.variant-lightweight".to_string(),
            document_revision: 12,
            variant_id: "variant.lightweight".to_string(),
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
                    position_mm: [120.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 80.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2],
            edges: vec![
                CadMeshEdgeSegment {
                    start_vertex: 0,
                    end_vertex: 1,
                    flags: 1,
                },
                CadMeshEdgeSegment {
                    start_vertex: 1,
                    end_vertex: 2,
                    flags: 1,
                },
            ],
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [120.0, 80.0, 0.0],
            },
        }
    }

    #[test]
    fn binary_contract_layout_is_stable() {
        assert_eq!(CadMeshVertex::BINARY_SIZE, 36);
        assert_eq!(size_of::<CadMeshVertex>(), 36);
        assert_eq!(align_of::<CadMeshVertex>(), 4);

        assert_eq!(CadMeshEdgeSegment::BINARY_SIZE, 12);
        assert_eq!(size_of::<CadMeshEdgeSegment>(), 12);
        assert_eq!(align_of::<CadMeshEdgeSegment>(), 4);

        assert_eq!(CadMeshMaterialSlot::BINARY_SIZE, 28);
        assert_eq!(size_of::<CadMeshMaterialSlot>(), 28);
        assert_eq!(align_of::<CadMeshMaterialSlot>(), 4);

        assert_eq!(CAD_MESH_BINARY_CONTRACT_VERSION, 1);
    }

    #[test]
    fn mesh_payload_binary_encoding_is_deterministic() {
        let payload = sample_payload();
        let encoded_a = payload
            .to_binary_payload()
            .expect("sample payload should encode");
        let encoded_b = payload
            .to_binary_payload()
            .expect("sample payload should encode twice");

        assert_eq!(encoded_a, encoded_b);
        assert_eq!(encoded_a.vertex_bytes.len(), 3 * CadMeshVertex::BINARY_SIZE);
        assert_eq!(encoded_a.index_bytes.len(), 3 * 4);
        assert_eq!(
            encoded_a.edge_bytes.len(),
            2 * CadMeshEdgeSegment::BINARY_SIZE
        );
        assert_eq!(
            encoded_a.material_bytes.len(),
            CadMeshMaterialSlot::BINARY_SIZE
        );
        assert_eq!(encoded_a.deterministic_hash, "270bbec4933e4a5f");
    }

    #[test]
    fn mesh_payload_schema_is_stable() {
        let payload = sample_payload();
        let encoded = serde_json::to_string(&payload).expect("mesh payload should serialize");
        assert!(encoded.contains("mesh.rack.variant-lightweight"));
        assert!(encoded.contains("triangle_indices"));
        assert!(encoded.contains("material_slots"));
    }

    #[test]
    fn mesh_payload_rejects_invalid_indices_and_missing_material_slots() {
        let mut payload = sample_payload();
        payload.triangle_indices = vec![0, 1, 42];
        let error = payload
            .to_binary_payload()
            .expect_err("out-of-range index should fail");
        assert_eq!(
            error,
            CadError::InvalidPrimitive {
                reason: "triangle index 2 out of range: 42 >= 3".to_string(),
            }
        );

        let mut payload = sample_payload();
        payload.material_slots.clear();
        let error = payload
            .to_binary_payload()
            .expect_err("missing material slots should fail");
        assert_eq!(
            error,
            CadError::InvalidPrimitive {
                reason: "mesh payload requires at least one material slot".to_string(),
            }
        );
    }
}
