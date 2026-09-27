//! Mesh upload interface for chunk meshes.
//!
//! The renderer implements this trait to consume CPU mesh buffers and create
//! GPU VB/IB resources. Kept here to avoid a tight coupling with renderer internals.

/// CPU representation of a voxel chunk mesh (positions, normals, indices).
#[derive(Default, Debug, Clone, PartialEq)]
pub struct ChunkMeshEntry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Abstraction over uploading and removing per-chunk meshes on the client.
pub trait MeshUpload {
    fn upload_chunk_mesh(&mut self, did: u64, chunk: (u32, u32, u32), mesh: &ChunkMeshEntry);
    fn remove_chunk_mesh(&mut self, did: u64, chunk: (u32, u32, u32));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_default_is_empty() {
        let e = ChunkMeshEntry::default();
        assert!(e.positions.is_empty() && e.normals.is_empty() && e.indices.is_empty());
    }
}
