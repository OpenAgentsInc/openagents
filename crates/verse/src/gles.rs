//! OpenGL ES support: WGSL variants for wgpu's GLES backend.
//!
//! wgpu translates WGSL to GLSL ES 3.00 on its GLES backend, which Android
//! uses where Vulkan is unavailable (including the emulator). GLSL ES lacks a
//! few things the physical shaders use on Metal, Vulkan, and DirectX: the
//! `noperspective` qualifier, reading the values of a depth texture that is
//! also sampled with comparison, and `textureNumLevels`. A shader marks the
//! lines that differ with `//#if GLES`, `//#else`, and `//#endif` comment
//! lines; [`wgsl`] keeps one side. Every other backend compiles the `#else`
//! side, which is the original shader, so its output does not change.
//!
//! Presentation also differs: the renderer draws into an sRGB texture and
//! encodes it into a linear surface itself (`render::Present`), because some
//! EGL drivers ignore an sRGB window colorspace.
//!
//! The tests translate every Verse shader, in both variants and at every
//! pipeline-constant setting the renderer uses, through naga's GLSL ES 3.00
//! writer with the options wgpu's GLES backend passes. A shader construct
//! that GLSL ES cannot express fails there, on any development machine.

use std::borrow::Cow;

/// Whether `backend` compiles shaders to GLSL ES.
#[must_use]
pub(crate) fn is_gles(backend: wgpu::Backend) -> bool {
    backend == wgpu::Backend::Gl
}

/// `source` with the GLES or the default side of each `//#if GLES` block.
///
/// # Panics
///
/// Panics on an unbalanced or nested block. Shaders are compiled into the
/// binary and the tests preprocess each one, so this is a programming error.
#[must_use]
pub(crate) fn wgsl(source: &str, gles: bool) -> Cow<'_, str> {
    if !source.contains("//#if") {
        return Cow::Borrowed(source);
    }
    // Outside a block, None; inside, whether the current side is kept.
    let mut keep: Option<bool> = None;
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        match line.trim() {
            "//#if GLES" => {
                assert!(keep.is_none(), "nested //#if GLES");
                keep = Some(gles);
            }
            "//#else" => {
                assert!(keep.is_some(), "//#else outside //#if GLES");
                keep = Some(!gles);
            }
            "//#endif" => {
                assert!(keep.is_some(), "//#endif outside //#if GLES");
                keep = None;
            }
            directive if directive.starts_with("//#") => {
                panic!("unknown shader directive {directive}")
            }
            _ if keep.unwrap_or(true) => out.push_str(line),
            _ => {}
        }
        // Dropped lines stay as empty lines, so compiler errors report the
        // line numbers of the file.
        out.push('\n');
    }
    assert!(keep.is_none(), "unterminated //#if GLES");
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use naga::back::glsl;

    /// A pipeline constant and every value the renderer passes for it.
    type Constant = (&'static str, &'static [f64]);

    /// Each shader the renderer compiles, with its pipeline constants.
    const SHADERS: [(&str, &str, &[Constant]); 5] = [
        ("shader.wgsl", include_str!("shader.wgsl"), &[]),
        ("ui.wgsl", include_str!("ui.wgsl"), &[]),
        (
            "pbr/photo.wgsl",
            include_str!("pbr/photo.wgsl"),
            &[
                ("DIRECT", &[0.0, 1.0]),
                ("DEBUG", &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            ],
        ),
        ("pbr/post.wgsl", include_str!("pbr/post.wgsl"), &[]),
        ("present.wgsl", include_str!("present.wgsl"), &[]),
    ];

    fn parse(name: &str, source: &str) -> (naga::Module, naga::valid::ModuleInfo) {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        (module, info)
    }

    /// Every combination of the shader's pipeline constants.
    fn constant_sets(constants: &[Constant]) -> Vec<naga::back::PipelineConstants> {
        let mut sets = vec![naga::back::PipelineConstants::default()];
        for (name, values) in constants {
            sets = sets
                .iter()
                .flat_map(|set| {
                    values.iter().map(move |value| {
                        let mut next = set.clone();
                        next.insert((*name).to_owned(), *value);
                        next
                    })
                })
                .collect();
        }
        sets
    }

    /// Resource slots as wgpu's GLES pipeline layout numbers them: one
    /// counter per resource class, in group and binding order.
    fn binding_map(module: &naga::Module) -> glsl::BindingMap {
        let mut bindings: Vec<_> = module
            .global_variables
            .iter()
            .filter_map(|(_, var)| var.binding.map(|b| (b, &module.types[var.ty].inner)))
            .collect();
        bindings.sort_by_key(|(b, _)| (b.group, b.binding));
        let mut counters = [0u8; 3];
        let mut map = glsl::BindingMap::default();
        for (binding, inner) in bindings {
            let class = match inner {
                naga::TypeInner::Sampler { .. } => 0,
                naga::TypeInner::Image { .. } => 1,
                _ => 2,
            };
            map.insert(binding, counters[class]);
            counters[class] += 1;
        }
        map
    }

    /// Writes one entry point as wgpu's GLES backend does, returning the
    /// GLSL and the (texture, sampler) name pairs it samples.
    fn write_gles(
        module: &naga::Module,
        info: &naga::valid::ModuleInfo,
        stage: naga::ShaderStage,
        entry: &str,
        constants: &naga::back::PipelineConstants,
    ) -> Result<(String, Vec<(String, String)>), String> {
        let (module, info) = naga::back::pipeline_constants::process_overrides(
            module,
            info,
            Some((stage, entry)),
            constants,
        )
        .map_err(|e| e.to_string())?;
        let options = glsl::Options {
            // The GLES 3.0 floor: every Android GLES device offers at least this.
            version: glsl::Version::Embedded {
                version: 300,
                is_webgl: false,
            },
            // wgpu sets these without the texture-shadow-LOD extension and
            // without base-instance support, the minimum a device can have.
            writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE
                | glsl::WriterFlags::FORCE_POINT_SIZE,
            binding_map: binding_map(&module),
            zero_initialize_workgroup_memory: true,
        };
        let pipeline = glsl::PipelineOptions {
            shader_stage: stage,
            entry_point: entry.to_owned(),
            multiview: None,
        };
        let policies = naga::proc::BoundsCheckPolicies {
            index: naga::proc::BoundsCheckPolicy::Unchecked,
            buffer: naga::proc::BoundsCheckPolicy::Unchecked,
            image_load: naga::proc::BoundsCheckPolicy::Unchecked,
            binding_array: naga::proc::BoundsCheckPolicy::Unchecked,
        };
        let mut out = String::new();
        let mut writer = glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, policies)
            .map_err(|e| e.to_string())?;
        let reflection = writer.write().map_err(|e| e.to_string())?;
        let name = |handle: naga::Handle<naga::GlobalVariable>| {
            module.global_variables[handle]
                .name
                .clone()
                .unwrap_or_default()
        };
        let pairs = reflection
            .texture_mapping
            .values()
            .filter_map(|m| Some((name(m.texture), name(m.sampler?))))
            .collect();
        Ok((out, pairs))
    }

    /// Translates every entry point of every shader to GLSL ES 3.00, the
    /// language wgpu's GLES backend compiles, at every pipeline constant.
    #[test]
    fn every_shader_translates_to_glsl_es_300() {
        let mut failures = Vec::new();
        let mut entries = 0;
        for (name, source, constants) in SHADERS {
            let source = wgsl(source, true);
            let (module, info) = parse(name, &source);
            for set in constant_sets(constants) {
                // GLES binds each texture unit to one sampler for the whole
                // program, so a texture must not meet two samplers in any
                // pair of stages that can share a pipeline.
                let mut samplers: std::collections::HashMap<String, String> = Default::default();
                for entry in &module.entry_points {
                    entries += 1;
                    match write_gles(&module, &info, entry.stage, &entry.name, &set) {
                        Ok((glsl, pairs)) => {
                            for forbidden in ["noperspective", "textureQueryLevels"] {
                                if glsl.contains(forbidden) {
                                    failures.push(format!(
                                        "{name} {}: GLSL uses {forbidden}",
                                        entry.name
                                    ));
                                }
                            }
                            if glsl.contains("#extension") {
                                failures.push(format!(
                                    "{name} {}: GLSL needs an extension:\n{}",
                                    entry.name,
                                    glsl.lines()
                                        .filter(|l| l.starts_with("#extension"))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                ));
                            }
                            for (texture, sampler) in pairs {
                                if let Some(other) =
                                    samplers.insert(texture.clone(), sampler.clone())
                                    && other != sampler
                                {
                                    failures.push(format!(
                                        "{name}: {texture} is sampled with both {other} and {sampler}"
                                    ));
                                }
                            }
                        }
                        Err(error) => {
                            failures.push(format!("{name} {} {set:?}: {error}", entry.name))
                        }
                    }
                }
            }
        }
        assert!(entries > 20, "only {entries} entry points were translated");
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The default side is the shader every other backend compiles; it must
    /// stay valid WGSL, and the Metal writer must accept it.
    #[test]
    fn default_variants_are_valid_and_translate_to_metal() {
        for (name, source, constants) in SHADERS {
            let source = wgsl(source, false);
            let (module, info) = parse(name, &source);
            for set in constant_sets(constants) {
                let (module, info) =
                    naga::back::pipeline_constants::process_overrides(&module, &info, None, &set)
                        .unwrap_or_else(|e| panic!("{name}: {e}"));
                naga::back::msl::write_string(
                    &module,
                    &info,
                    &naga::back::msl::Options {
                        // iOS 14 and macOS 11, the oldest wgpu's Metal backend runs on.
                        lang_version: (2, 3),
                        ..Default::default()
                    },
                    &naga::back::msl::PipelineOptions::default(),
                )
                .unwrap_or_else(|e| panic!("{name} {set:?}: {e}"));
            }
        }
    }

    /// The GLES side replaces exactly the constructs GLSL ES lacks: the
    /// default side still uses them, so the preprocessor did not drop the
    /// wrong branch.
    #[test]
    fn variants_differ_only_where_gles_needs_them() {
        let photo = include_str!("pbr/photo.wgsl");
        let default = wgsl(photo, false);
        let gles = wgsl(photo, true);
        assert!(default.contains("@interpolate(linear)"));
        assert!(!gles.contains("@interpolate(linear)"));
        assert!(default.contains("textureLoad(shadow_map"));
        assert!(!gles.contains("textureLoad(shadow_map"));
        // Blank lines stand in for the other side, so line numbers match.
        assert_eq!(default.lines().count(), photo.lines().count());
        assert_eq!(gles.lines().count(), photo.lines().count());
        // Shaders without blocks pass through unchanged.
        let plain = include_str!("shader.wgsl");
        assert!(matches!(wgsl(plain, true), Cow::Borrowed(s) if s == plain));
    }

    /// The GLES variants exist because the default side does not translate:
    /// this keeps the test above honest about what it detects.
    #[test]
    fn the_default_photo_shader_does_not_translate_to_glsl_es() {
        let source = wgsl(include_str!("pbr/photo.wgsl"), false);
        let (module, info) = parse("photo", &source);
        let set = naga::back::PipelineConstants::default();
        let error = |stage, entry| {
            write_gles(&module, &info, stage, entry, &set)
                .err()
                .unwrap_or_default()
        };
        // Reading the depth texture's values in the blocker search.
        assert!(!error(naga::ShaderStage::Fragment, "fs_lit").is_empty());
        // The noperspective qualifier on screen-space varyings.
        for (stage, entry) in [
            (naga::ShaderStage::Vertex, "vs_body"),
            (naga::ShaderStage::Fragment, "fs_body"),
            (naga::ShaderStage::Vertex, "vs_wide"),
            (naga::ShaderStage::Fragment, "fs_wide"),
        ] {
            let error = error(stage, entry);
            assert!(error.contains("NOPERSPECTIVE"), "{entry}: {error}");
        }
    }

    #[test]
    #[should_panic(expected = "unterminated")]
    fn an_unterminated_block_is_refused() {
        let _ = wgsl("//#if GLES\nfn a() {}\n", true);
    }
}
