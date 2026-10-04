//! Portable triangle-list overlays, admitted before native buffer writes.
use crate::residency::{Catalog, CatalogId};
use bytemuck::{Pod, Zeroable};

/// One overlay vertex in the caller's logical viewport coordinates.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// Position from the viewport's top-left. Offscreen clipping is permitted.
    pub pos: [f32; 2],
    /// Normalized atlas texture coordinates.
    pub uv: [f32; 2],
    /// Normalized linear RGBA, with unpremultiplied color.
    pub color: [f32; 4],
}

/// Borrowed immutable geometry using the submitting renderer's atlas catalog.
#[derive(Clone, Debug)]
pub struct ResolvedOverlay<'a> {
    catalog: CatalogId,
    vertices: &'a [Vertex],
}
impl<'a> ResolvedOverlay<'a> {
    pub const MAX_BYTES: usize = 4 * 1024 * 1024;

    pub fn extract(catalog: &Catalog, vertices: &'a [Vertex]) -> Result<Self, String> {
        if std::mem::size_of_val(vertices) > Self::MAX_BYTES {
            return Err("Overlay exceeds 4 MiB".into());
        }
        if !vertices.len().is_multiple_of(3) {
            return Err("Overlay has an incomplete triangle".into());
        }
        if vertices.iter().any(|v| {
            v.pos.iter().any(|x| !x.is_finite())
                || v.uv
                    .iter()
                    .chain(v.color.iter())
                    .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
        }) {
            return Err("Overlay contains invalid vertex values".into());
        }
        Ok(Self {
            catalog: catalog.id(),
            vertices,
        })
    }

    /// Fence empty overlays as well as geometry when the atlas is replaced.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        catalog.check(self.catalog)
    }
    pub fn vertices(&self) -> &'a [Vertex] {
        self.vertices
    }
    pub fn catalog(&self) -> CatalogId {
        self.catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        let pack = serde_json::from_value(serde_json::json!({
            "version":1,"source_revision":"test","textures":[],"models":{
                "room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[],"clips":[],"height":1,"attachments":[]}
            }
        }))
        .unwrap();
        Catalog::new(&pack).unwrap()
    }
    fn triangle() -> [Vertex; 3] {
        [Vertex {
            pos: [-20., 30.],
            uv: [0., 1.],
            color: [1., 0.5, 0., 0.75],
        }; 3]
    }
    #[test]
    fn extraction_borrows_geometry_and_fences_replaced_atlases() {
        let replacement = catalog();
        let catalog = catalog();
        let vertices = triangle();
        for source in [&vertices[..], &[][..]] {
            let overlay = ResolvedOverlay::extract(&catalog, source).unwrap();
            assert!(std::ptr::eq(overlay.vertices().as_ptr(), source.as_ptr()));
            assert!(overlay.validate(&catalog).is_ok());
            assert!(overlay.validate(&replacement).is_err());
        }
    }
    #[test]
    fn rejects_invalid_geometry_before_submission() {
        let catalog = catalog();
        let valid = triangle();
        assert!(ResolvedOverlay::extract(&catalog, &valid[..2]).is_err());
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut vertices = valid;
            vertices[1].pos[0] = bad;
            assert!(ResolvedOverlay::extract(&catalog, &vertices).is_err());
        }
        for bad in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
            for field in 0..6 {
                let mut vertices = valid;
                if field < 2 {
                    vertices[0].uv[field] = bad;
                } else {
                    vertices[0].color[field - 2] = bad;
                }
                assert!(ResolvedOverlay::extract(&catalog, &vertices).is_err());
            }
        }
    }
    #[test]
    fn enforces_the_existing_native_upload_budget() {
        let catalog = catalog();
        let count = ResolvedOverlay::MAX_BYTES / std::mem::size_of::<Vertex>();
        let count = count - count % 3;
        let mut vertices = vec![triangle()[0]; count];
        assert!(ResolvedOverlay::extract(&catalog, &vertices).is_ok());
        vertices.extend(triangle());
        assert!(ResolvedOverlay::extract(&catalog, &vertices).is_err());
    }
}
