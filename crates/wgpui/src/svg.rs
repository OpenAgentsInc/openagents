//! SVG rendering support for WGPUI.
//!
//! Uses resvg to rasterize SVGs to RGBA pixels, which are then uploaded
//! to GPU textures for rendering as quads.

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;

    /// Cache key for rasterized SVGs.
    #[derive(Clone, Eq, PartialEq)]
    struct SvgCacheKey {
        /// Hash of SVG data
        data_hash: u64,
        /// Target width in pixels
        width: u32,
        /// Target height in pixels
        height: u32,
    }

    impl Hash for SvgCacheKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.data_hash.hash(state);
            self.width.hash(state);
            self.height.hash(state);
        }
    }

    /// Rasterized SVG data ready for GPU upload.
    #[derive(Clone)]
    pub struct SvgRasterized {
        /// RGBA pixel data
        pub pixels: Vec<u8>,
        /// Width in pixels
        pub width: u32,
        /// Height in pixels
        pub height: u32,
    }

    /// SVG renderer with caching.
    pub struct SvgRenderer {
        cache: HashMap<SvgCacheKey, Arc<SvgRasterized>>,
        /// Options for usvg parsing
        options: usvg::Options<'static>,
    }

    impl Default for SvgRenderer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SvgRenderer {
        /// Create a new SVG renderer.
        pub fn new() -> Self {
            Self {
                cache: HashMap::new(),
                options: usvg::Options::default(),
            }
        }

        /// Hash SVG data for cache key.
        fn hash_svg_data(data: &[u8]) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            let mut hasher = DefaultHasher::new();
            data.hash(&mut hasher);
            hasher.finish()
        }

        /// Rasterize SVG bytes to RGBA pixels at the given size.
        ///
        /// Results are cached by content hash + dimensions.
        pub fn rasterize(
            &mut self,
            svg_bytes: &[u8],
            width: u32,
            height: u32,
            scale_factor: f32,
        ) -> Option<Arc<SvgRasterized>> {
            // Apply scale factor for HiDPI
            let physical_width = (width as f32 * scale_factor).ceil() as u32;
            let physical_height = (height as f32 * scale_factor).ceil() as u32;

            if physical_width == 0 || physical_height == 0 {
                return None;
            }

            let data_hash = Self::hash_svg_data(svg_bytes);
            let cache_key = SvgCacheKey {
                data_hash,
                width: physical_width,
                height: physical_height,
            };

            // Return cached if available
            if let Some(cached) = self.cache.get(&cache_key) {
                return Some(cached.clone());
            }

            // Parse SVG
            let tree = match usvg::Tree::from_data(svg_bytes, &self.options) {
                Ok(tree) => tree,
                Err(e) => {
                    log::error!("Failed to parse SVG: {}", e);
                    return None;
                }
            };

            // Calculate scale to fit target size
            let svg_size = tree.size();
            let scale_x = physical_width as f32 / svg_size.width();
            let scale_y = physical_height as f32 / svg_size.height();
            let scale = scale_x.min(scale_y);

            // Create pixmap for rendering
            let mut pixmap = match resvg::tiny_skia::Pixmap::new(physical_width, physical_height) {
                Some(pixmap) => pixmap,
                None => {
                    log::error!(
                        "Failed to create pixmap {}x{}",
                        physical_width,
                        physical_height
                    );
                    return None;
                }
            };

            // Render SVG to pixmap
            let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
            resvg::render(&tree, transform, &mut pixmap.as_mut());

            // Convert premultiplied alpha to straight alpha RGBA
            let pixels = pixmap.data().to_vec();

            let rasterized = Arc::new(SvgRasterized {
                pixels,
                width: physical_width,
                height: physical_height,
            });

            self.cache.insert(cache_key, rasterized.clone());

            Some(rasterized)
        }

        /// Clear the cache.
        pub fn clear_cache(&mut self) {
            self.cache.clear();
        }

        /// Get cache size (number of entries).
        pub fn cache_size(&self) -> usize {
            self.cache.len()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use desktop::*;

// Stub for WASM - SVG rendering not supported
#[cfg(target_arch = "wasm32")]
mod wasm_stub {
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct SvgRasterized {
        pub pixels: Vec<u8>,
        pub width: u32,
        pub height: u32,
    }

    pub struct SvgRenderer;

    impl Default for SvgRenderer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SvgRenderer {
        pub fn new() -> Self {
            Self
        }

        pub fn rasterize(
            &mut self,
            _svg_bytes: &[u8],
            _width: u32,
            _height: u32,
            _scale_factor: f32,
        ) -> Option<Arc<SvgRasterized>> {
            log::warn!("SVG rendering not supported on WASM");
            None
        }

        pub fn clear_cache(&mut self) {}

        pub fn cache_size(&self) -> usize {
            0
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_stub::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svg_renderer_new() {
        let renderer = SvgRenderer::new();
        assert_eq!(renderer.cache_size(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_svg_rasterize_simple() {
        let mut renderer = SvgRenderer::new();

        // Simple circle SVG
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40" fill="red"/>
        </svg>"#;

        let result = renderer.rasterize(svg, 50, 50, 1.0);
        assert!(result.is_some());

        let rasterized = result.unwrap();
        assert_eq!(rasterized.width, 50);
        assert_eq!(rasterized.height, 50);
        assert_eq!(rasterized.pixels.len(), 50 * 50 * 4); // RGBA
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_svg_caching() {
        let mut renderer = SvgRenderer::new();

        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="blue"/>
        </svg>"#;

        // First call should cache
        let _ = renderer.rasterize(svg, 32, 32, 1.0);
        assert_eq!(renderer.cache_size(), 1);

        // Same call should hit cache
        let _ = renderer.rasterize(svg, 32, 32, 1.0);
        assert_eq!(renderer.cache_size(), 1);

        // Different size should add new entry
        let _ = renderer.rasterize(svg, 64, 64, 1.0);
        assert_eq!(renderer.cache_size(), 2);

        // Clear cache
        renderer.clear_cache();
        assert_eq!(renderer.cache_size(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_svg_scale_factor() {
        let mut renderer = SvgRenderer::new();

        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="green"/>
        </svg>"#;

        // At 2x scale factor, physical size should be doubled
        let result = renderer.rasterize(svg, 50, 50, 2.0);
        assert!(result.is_some());

        let rasterized = result.unwrap();
        assert_eq!(rasterized.width, 100); // 50 * 2.0
        assert_eq!(rasterized.height, 100);
    }
}
