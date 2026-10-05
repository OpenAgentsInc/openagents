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
pub fn is_gles(backend: wgpu::Backend) -> bool {
    backend == wgpu::Backend::Gl
}

/// `source` with the GLES or the default side of each `//#if GLES` block.
///
/// # Panics
///
/// Panics on an unbalanced or nested block. Shaders are compiled into the
/// binary and the tests preprocess each one, so this is a programming error.
#[must_use]
pub fn wgsl(source: &str, gles: bool) -> Cow<'_, str> {
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
