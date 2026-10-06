//! The GLES translation test: every shader the renderer compiles, preprocessed
//! by [`crate::gles::wgsl`] and translated with the naga release wgpu uses.
//! It lives here rather than in `verse-gfx` because most of those shaders
//! belong to this crate.

use crate::gles::*;
use naga::back::glsl;
use std::borrow::Cow;

/// A pipeline constant and every value the renderer passes for it.
type Constant = (&'static str, &'static [f64]);

/// Each shader the renderer compiles, with its pipeline constants.
const SHADERS: [(&str, &str, &[Constant]); 5] = [
    ("shader.wgsl", include_str!("shader.wgsl"), &[]),
    ("ui.wgsl", crate::ui::SHADER, &[]),
    (
        "pbr/photo.wgsl",
        include_str!("../../verse-pbr/src/pbr/photo.wgsl"),
        &[
            ("DIRECT", &[0.0, 1.0]),
            ("DEBUG", &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            ("PCSS", &[0.0, 1.0]),
            ("DETAIL", &[0.0, 1.0]),
            ("SCREEN", &[0.0, 1.0]),
        ],
    ),
    (
        "pbr/post.wgsl",
        include_str!("../../verse-pbr/src/pbr/post.wgsl"),
        &[],
    ),
    ("present.wgsl", include_str!("present.wgsl"), &[]),
];

fn parse(name: &str, source: &str) -> (naga::Module, naga::valid::ModuleInfo) {
    // Expand the same shared contract the renderer inserts before compilation.
    let expanded = source.replace(
        "// VERSE_SHARED_SHADING",
        include_str!("../../verse-pbr/src/shading.wgsl"),
    );
    let source = expanded.as_str();
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
                                failures
                                    .push(format!("{name} {}: GLSL uses {forbidden}", entry.name));
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
                            if let Some(other) = samplers.insert(texture.clone(), sampler.clone())
                                && other != sampler
                            {
                                failures.push(format!(
                                    "{name}: {texture} is sampled with both {other} and {sampler}"
                                ));
                            }
                        }
                    }
                    Err(error) => failures.push(format!("{name} {} {set:?}: {error}", entry.name)),
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

/// The daylight sky is a plain full-screen fragment entry over the frame
/// uniform: it validates, translates to GLSL ES 3.00 and Metal at both
/// `DIRECT` settings, and binds no texture, so WebGL2 draws it as
/// desktops do.
#[test]
fn daylight_sky_validates_without_textures() {
    for gles in [false, true] {
        let source = wgsl(include_str!("../../verse-pbr/src/pbr/photo.wgsl"), gles);
        let (module, info) = parse("pbr/photo.wgsl", &source);
        let entry = module
            .entry_points
            .iter()
            .find(|e| e.name == "fs_daylight")
            .expect("photo.wgsl has fs_daylight");
        assert_eq!(entry.stage, naga::ShaderStage::Fragment);
        for set in constant_sets(&[("DIRECT", &[0.0, 1.0])]) {
            let (glsl, pairs) = write_gles(
                &module,
                &info,
                naga::ShaderStage::Fragment,
                "fs_daylight",
                &set,
            )
            .unwrap();
            assert!(pairs.is_empty(), "the sky samples {pairs:?}");
            assert!(!glsl.contains("texture("), "the sky samples a texture");
        }
    }
}

/// Lit and textured surfaces read the daylight sky's reflection cube with
/// an explicit level of detail, which GLSL ES 3.00 has without an
/// extension, and through the one sampler the probes also use. The
/// height fog and the sky's irradiance are uniform arithmetic.
/// `default_variants_are_valid_and_translate_to_metal` covers Metal.
#[test]
fn the_sky_light_and_height_fog_translate_for_webgl2() {
    let source = wgsl(include_str!("../../verse-pbr/src/pbr/photo.wgsl"), true);
    let (module, info) = parse("photo", &source);
    for set in constant_sets(&[("DIRECT", &[0.0, 1.0])]) {
        for entry in [
            "fs_lit",
            "fs_textured",
            "fs_textured_masked",
            "fs_textured_blend",
        ] {
            let (glsl, pairs) =
                write_gles(&module, &info, naga::ShaderStage::Fragment, entry, &set)
                    .unwrap_or_else(|e| panic!("{entry}: {e}"));
            assert!(glsl.contains("samplerCube"), "{entry} has no cube");
            assert!(
                glsl.contains("textureLod("),
                "{entry} reads no explicit level"
            );
            assert!(!glsl.contains("#extension"), "{entry} needs an extension");
            assert!(
                pairs
                    .iter()
                    .any(|(texture, sampler)| texture == "sky_cube" && sampler == "linear_clamp"),
                "{entry}: {pairs:?}"
            );
        }
        // Fog reaches the lines and faces drawn on the stage too.
        for (stage, entry) in [
            (naga::ShaderStage::Fragment, "fs_legacy"),
            (naga::ShaderStage::Fragment, "fs_wide"),
        ] {
            let (glsl, _) = write_gles(&module, &info, stage, entry, &set)
                .unwrap_or_else(|e| panic!("{entry}: {e}"));
            assert!(glsl.contains("height_fog"), "{entry} has no height fog");
        }
    }
}

/// The GLES side replaces exactly the constructs GLSL ES lacks: the
/// default side still uses them, so the preprocessor did not drop the
/// wrong branch.
#[test]
fn variants_differ_only_where_gles_needs_them() {
    let photo = include_str!("../../verse-pbr/src/pbr/photo.wgsl");
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

/// The sun's shadow cascades are layers of one depth array. GLSL ES 3.00
/// has no `textureLod` for `sampler2DArrayShadow`, so its comparison
/// samples must come out as `textureGrad` with zero gradients, which it
/// has, with or without the blocker search constant, and the lit and
/// textured entries that read the cascades need no extension.
#[test]
fn sun_cascades_sample_a_depth_array_on_glsl_es() {
    let source = wgsl(include_str!("../../verse-pbr/src/pbr/photo.wgsl"), true);
    let (module, info) = parse("photo", &source);
    for pcss in [0.0, 1.0] {
        let mut set = naga::back::PipelineConstants::default();
        set.insert("PCSS".to_owned(), pcss);
        for entry in ["fs_lit", "fs_textured", "fs_textured_masked"] {
            let (glsl, _) = write_gles(&module, &info, naga::ShaderStage::Fragment, entry, &set)
                .unwrap_or_else(|e| panic!("{entry}: {e}"));
            assert!(glsl.contains("sampler2DArrayShadow"), "{entry}");
            assert!(glsl.contains("textureGrad("), "{entry}");
            assert!(!glsl.contains("#extension"), "{entry}");
        }
    }
}

/// The engine renderer's scene shader, which draws the Grid, as GLES and
/// WebGL2 compile it: the pose block cut to the 254 bones a 16 KiB uniform
/// block holds (`verse_pbr::imported` admission), every entry point
/// translated without an extension, and local shadows read from a 2D depth
/// array rather than a cube array.
#[test]
fn the_engine_scene_shader_translates_to_glsl_es_300() {
    let source = include_str!("../../verse-pbr/src/imported/scene.wgsl").replace(
        "bones:array<mat4x4<f32>,256>",
        "bones:array<mat4x4<f32>,254>",
    );
    let (module, info) = parse("imported/scene.wgsl", &source);
    let set = naga::back::PipelineConstants::default();
    for (stage, entry) in [
        (naga::ShaderStage::Vertex, "vs"),
        (naga::ShaderStage::Fragment, "fs"),
        (naga::ShaderStage::Fragment, "shadow_fs"),
    ] {
        let (glsl, _) = write_gles(&module, &info, stage, entry, &set)
            .unwrap_or_else(|e| panic!("{entry}: {e}"));
        assert!(!glsl.contains("#extension"), "{entry}");
        assert!(!glsl.contains("CubeArray"), "{entry}");
        if entry != "shadow_fs" {
            assert!(glsl.contains("[254]"), "{entry}: the pose block is cut");
        }
        if entry == "fs" {
            assert!(glsl.contains("sampler2DArrayShadow"), "{entry}");
        }
    }
}

/// The GLES variants exist because the default side does not translate:
/// this keeps the test above honest about what it detects.
#[test]
fn the_default_photo_shader_does_not_translate_to_glsl_es() {
    let source = wgsl(include_str!("../../verse-pbr/src/pbr/photo.wgsl"), false);
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

/// The textured mesh entries translate to GLSL ES, and only the masked
/// ones discard, so opaque and blended draws keep early depth rejection.
#[test]
fn textured_entries_translate_and_only_masked_entries_discard() {
    let source = wgsl(include_str!("../../verse-pbr/src/pbr/photo.wgsl"), true);
    let (module, info) = parse("photo", &source);
    let set = naga::back::PipelineConstants::default();
    for (stage, entry) in [
        (naga::ShaderStage::Vertex, "vs_textured"),
        (naga::ShaderStage::Fragment, "fs_textured"),
        (naga::ShaderStage::Fragment, "fs_textured_masked"),
        (naga::ShaderStage::Fragment, "fs_textured_blend"),
        (naga::ShaderStage::Vertex, "vs_shadow_textured"),
        (naga::ShaderStage::Fragment, "fs_shadow_masked"),
    ] {
        let (glsl, _) = write_gles(&module, &info, stage, entry, &set)
            .unwrap_or_else(|e| panic!("{entry}: {e}"));
        assert_eq!(
            glsl.contains("discard"),
            entry.ends_with("masked"),
            "{entry}"
        );
    }
}

#[test]
#[should_panic(expected = "unterminated")]
fn an_unterminated_block_is_refused() {
    let _ = wgsl("//#if GLES\nfn a() {}\n", true);
}
