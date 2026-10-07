//! Shared local-light and coverage semantics before backend shader translation.

/// Expands `// VERSE_SHARED_SHADING` into the shared shading and fire
/// functions, `// VERSE_WATER` into the shared water shader and the
/// physical renderer's water pass (`water/water.wgsl` and
/// `water/photo.wgsl`), and `// VERSE_WATER_IMPORTED` into the shared water
/// shader and the imported renderer's pass (`water/imported.wgsl`), as the
/// renderers do before compiling a shader. Public so the `verse` GLES
/// translation test expands shaders exactly as the renderer does.
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
        .replace(
            "// VERSE_WATER_IMPORTED",
            &format!("{}\n{}", crate::water::SHARED, crate::water::IMPORTED),
        )
        .replace(
            "// VERSE_WATER",
            &format!("{}\n{}", crate::water::SHARED, crate::water::PHOTO),
        )
}

#[cfg(test)]
mod tests {
    /// Every shader the shared fire and water code is spliced into parses
    /// and validates in its default and GLES variants.
    #[test]
    fn shared_shaders_validate() {
        for (name, shader) in [
            ("pbr/photo.wgsl", include_str!("pbr/photo.wgsl")),
            ("imported/scene.wgsl", include_str!("imported/scene.wgsl")),
        ] {
            let shared = super::source(shader);
            assert!(
                shared.contains("fn water_gerstner("),
                "{name} carries the shared water shader"
            );
            for gles in [false, true] {
                let source = crate::gles::wgsl(&shared, gles);
                let module = naga::front::wgsl::parse_str(&source)
                    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
                for entry in ["vs_water", "fs_water", "fs_water_transmit"] {
                    assert!(
                        module.entry_points.iter().any(|e| e.name == entry),
                        "{name} has {entry}"
                    );
                }
            }
        }
    }
}
