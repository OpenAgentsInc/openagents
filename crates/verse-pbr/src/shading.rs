//! Shared local-light and coverage semantics before backend shader translation.

/// Expands `// VERSE_SHARED_SHADING` into the shared shading and fire
/// functions, and `// VERSE_WATER` into the water pass (`pbr/water.wgsl`),
/// as the renderer does before compiling a shader. Public so the
/// `verse` GLES translation test expands shaders exactly as the renderer does.
pub fn source(source: &str) -> String {
    source
        .replace(
            "// VERSE_SHARED_SHADING",
            &format!(
                "{}\n{}",
                include_str!("shading.wgsl"),
                include_str!("fx/fire.wgsl")
            ),
        )
        .replace("// VERSE_WATER", include_str!("pbr/water.wgsl"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn fire_shaders_validate() {
        for shader in [
            include_str!("pbr/photo.wgsl"),
            include_str!("imported/scene.wgsl"),
        ] {
            let shared = super::source(shader);
            for gles in [false, true] {
                let source = crate::gles::wgsl(&shared, gles);
                let module = naga::front::wgsl::parse_str(&source).unwrap();
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap();
            }
        }
    }
}
