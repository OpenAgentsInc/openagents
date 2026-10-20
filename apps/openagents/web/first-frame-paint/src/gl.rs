//! The WebGL2 half: compiles the five programs, uploads the instance
//! arrays and the glyph texture, and replays the scene's commands.

use openagents_first_frame::Command;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlTexture,
};

use crate::{LAYER_FLOATS, PATH_FLOATS, QUAD_FLOATS, SHADOW_FLOATS, SPRITE_FLOATS, shaders};

fn fail(why: &'static str) -> JsValue {
    JsValue::from_str(why)
}

const ATTRIBUTES: [&str; 7] = ["a0", "a1", "a2", "a3", "a4", "a5", "a6"];

fn program(gl: &GL, vs: &str, fs: &str, w: f32, h: f32) -> Result<WebGlProgram, JsValue> {
    let p = gl.create_program().ok_or_else(|| fail("no program"))?;
    for (kind, source) in [
        (GL::VERTEX_SHADER, shaders::vs(vs)),
        (GL::FRAGMENT_SHADER, shaders::fs(fs)),
    ] {
        let s = gl.create_shader(kind).ok_or_else(|| fail("no shader"))?;
        gl.shader_source(&s, &source);
        gl.compile_shader(&s);
        if !gl
            .get_shader_parameter(&s, GL::COMPILE_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return Err(JsValue::from_str(
                &gl.get_shader_info_log(&s).unwrap_or_default(),
            ));
        }
        gl.attach_shader(&p, &s);
    }
    for (at, name) in ATTRIBUTES.iter().enumerate() {
        gl.bind_attrib_location(&p, at as u32, name);
    }
    gl.link_program(&p);
    if !gl
        .get_program_parameter(&p, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        return Err(JsValue::from_str(
            &gl.get_program_info_log(&p).unwrap_or_default(),
        ));
    }
    gl.use_program(Some(&p));
    gl.uniform2f(gl.get_uniform_location(&p, "u_vp").as_ref(), w, h);
    Ok(p)
}

fn bytes(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and any bit pattern is a valid u8.
    unsafe { core::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

fn buffer(gl: &GL, data: &[f32]) -> Result<WebGlBuffer, JsValue> {
    let b = gl.create_buffer().ok_or_else(|| fail("no buffer"))?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&b));
    gl.buffer_data_with_u8_array(GL::ARRAY_BUFFER, bytes(data), GL::STATIC_DRAW);
    Ok(b)
}

/// Points attributes `0..vec4s` at `buffer`, from record `first`.
fn attributes(gl: &GL, buffer: &WebGlBuffer, vec4s: u32, floats: usize, first: u32, divisor: u32) {
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(buffer));
    for at in 0..ATTRIBUTES.len() as u32 {
        if at < vec4s {
            gl.enable_vertex_attrib_array(at);
            gl.vertex_attrib_pointer_with_i32(
                at,
                4,
                GL::FLOAT,
                false,
                (floats * 4) as i32,
                ((first as usize * floats + at as usize * 4) * 4) as i32,
            );
            gl.vertex_attrib_divisor(at, divisor);
        } else {
            gl.disable_vertex_attrib_array(at);
        }
    }
}

fn texture(gl: &GL, filter: u32) -> Result<WebGlTexture, JsValue> {
    let t = gl.create_texture().ok_or_else(|| fail("no texture"))?;
    gl.bind_texture(GL::TEXTURE_2D, Some(&t));
    for (name, value) in [
        (GL::TEXTURE_MIN_FILTER, filter),
        (GL::TEXTURE_MAG_FILTER, filter),
        (GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE),
        (GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameteri(GL::TEXTURE_2D, name, value as i32);
    }
    Ok(t)
}

/// GPUI's blending for an opaque surface, for path layers, and for
/// copying a layer back.
fn blend(gl: &GL, kind: u8) {
    match kind {
        0 => gl.blend_func_separate(
            GL::SRC_ALPHA,
            GL::ONE_MINUS_SRC_ALPHA,
            GL::ONE,
            GL::ONE_MINUS_SRC_ALPHA,
        ),
        1 => gl.blend_func_separate(
            GL::ONE,
            GL::ONE_MINUS_SRC_ALPHA,
            GL::ONE,
            GL::ONE_MINUS_SRC_ALPHA,
        ),
        _ => gl.blend_func_separate(GL::ONE, GL::ONE_MINUS_SRC_ALPHA, GL::ONE, GL::ONE),
    }
}

/// Draws the encoded scene `bytes` into `canvas`, sizing the canvas to the
/// frame. Returns the number of draw commands, or why it couldn't.
#[wasm_bindgen]
pub fn paint(canvas: &HtmlCanvasElement, scene: &[u8]) -> Result<u32, JsValue> {
    let scene = crate::read(scene).map_err(|e| fail(e.0))?;
    let (w, h) = (scene.width, scene.height);
    canvas.set_width(w);
    canvas.set_height(h);
    let attributes_ = web_sys::WebGlContextAttributes::new();
    attributes_.set_alpha(false);
    attributes_.set_antialias(false);
    attributes_.set_depth(false);
    attributes_.set_stencil(false);
    // The frame is drawn once and stays until the app replaces it.
    attributes_.set_preserve_drawing_buffer(true);
    let gl: GL = canvas
        .get_context_with_context_options("webgl2", &attributes_)?
        .ok_or_else(|| fail("no WebGL2"))?
        .unchecked_into();
    let (fw, fh) = (w as f32, h as f32);
    let i = crate::instances(&scene);
    let offsets = crate::path_offsets(&scene);

    let quad = program(&gl, shaders::QUAD_VS, shaders::QUAD_FS, fw, fh)?;
    let shadow = program(&gl, shaders::SHADOW_VS, shaders::SHADOW_FS, fw, fh)?;
    let sprite = program(&gl, shaders::SPRITE_VS, shaders::SPRITE_FS, fw, fh)?;
    let atlas = &scene.atlas;
    gl.uniform2f(
        gl.get_uniform_location(&sprite, "u_atlas").as_ref(),
        atlas.width.max(1) as f32,
        atlas.height.max(1) as f32,
    );
    gl.uniform1i(gl.get_uniform_location(&sprite, "u_tex").as_ref(), 0);
    gl.uniform4fv_with_f32_array(
        gl.get_uniform_location(&sprite, "u_gamma").as_ref(),
        &scene.gamma_ratios,
    );
    gl.uniform1f(
        gl.get_uniform_location(&sprite, "u_contrast").as_ref(),
        scene.contrast,
    );
    gl.uniform1f(
        gl.get_uniform_location(&sprite, "u_subpixel_contrast")
            .as_ref(),
        scene.subpixel_contrast,
    );
    let channel = gl.get_uniform_location(&sprite, "u_channel");

    let quads = buffer(&gl, &i.quads)?;
    let shadows = buffer(&gl, &i.shadows)?;
    let sprites = buffer(&gl, &i.sprites)?;

    gl.active_texture(GL::TEXTURE0);
    texture(&gl, GL::LINEAR)?;
    gl.pixel_storei(GL::UNPACK_ALIGNMENT, 1);
    let (aw, ah) = (atlas.width.max(1) as i32, atlas.height.max(1) as i32);
    let rgb = atlas.channels == 3;
    let blank = vec![0u8; atlas.channels as usize];
    let pixels: &[u8] = if atlas.pixels.is_empty() {
        &blank
    } else {
        &atlas.pixels
    };
    let (internal, format) = if rgb {
        (GL::RGB8, GL::RGB)
    } else {
        (GL::R8, GL::RED)
    };
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        GL::TEXTURE_2D,
        0,
        internal as i32,
        aw,
        ah,
        0,
        format,
        GL::UNSIGNED_BYTE,
        Some(pixels),
    )?;

    // Paths draw into a multisampled layer, resolved into a texture.
    let has_paths = scene
        .commands
        .iter()
        .any(|c| matches!(c, Command::Paths { .. }));
    let mut layer = None;
    if has_paths {
        let path = program(&gl, shaders::PATH_VS, shaders::PATH_FS, fw, fh)?;
        let copy = program(&gl, shaders::LAYER_VS, shaders::LAYER_FS, fw, fh)?;
        gl.uniform1i(gl.get_uniform_location(&copy, "u_tex").as_ref(), 1);
        gl.active_texture(GL::TEXTURE1);
        let resolved = texture(&gl, GL::NEAREST)?;
        gl.tex_storage_2d(GL::TEXTURE_2D, 1, GL::RGBA8, w as i32, h as i32);
        let resolved_fb = gl
            .create_framebuffer()
            .ok_or_else(|| fail("no framebuffer"))?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&resolved_fb));
        gl.framebuffer_texture_2d(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::TEXTURE_2D,
            Some(&resolved),
            0,
        );
        let samples = gl
            .get_parameter(GL::MAX_SAMPLES)?
            .as_f64()
            .unwrap_or(1.0)
            .min(4.0) as i32;
        let rb = gl
            .create_renderbuffer()
            .ok_or_else(|| fail("no renderbuffer"))?;
        gl.bind_renderbuffer(GL::RENDERBUFFER, Some(&rb));
        gl.renderbuffer_storage_multisample(
            GL::RENDERBUFFER,
            samples,
            GL::RGBA8,
            w as i32,
            h as i32,
        );
        let msaa_fb = gl
            .create_framebuffer()
            .ok_or_else(|| fail("no framebuffer"))?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&msaa_fb));
        gl.framebuffer_renderbuffer(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::RENDERBUFFER,
            Some(&rb),
        );
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        let vertices = buffer(&gl, &i.path_vertices)?;
        let rects = buffer(&gl, &i.layers)?;
        layer = Some((path, copy, msaa_fb, resolved_fb, vertices, rects));
    }

    gl.viewport(0, 0, w as i32, h as i32);
    gl.disable(GL::DEPTH_TEST);
    gl.enable(GL::BLEND);
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(GL::COLOR_BUFFER_BIT);
    blend(&gl, 0);
    for command in &scene.commands {
        match command {
            Command::Quads(r)
            | Command::Shadows(r)
            | Command::Sprites(r)
            | Command::SubpixelSprites(r)
                if r.is_empty() => {}
            Command::Quads(r) => {
                gl.use_program(Some(&quad));
                attributes(&gl, &quads, 6, QUAD_FLOATS, r.start, 1);
                gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (r.end - r.start) as i32);
            }
            Command::Shadows(r) => {
                gl.use_program(Some(&shadow));
                attributes(&gl, &shadows, 7, SHADOW_FLOATS, r.start, 1);
                gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (r.end - r.start) as i32);
            }
            Command::Sprites(r) => {
                gl.use_program(Some(&sprite));
                gl.uniform1i(channel.as_ref(), -1);
                attributes(&gl, &sprites, 4, SPRITE_FLOATS, r.start, 1);
                gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (r.end - r.start) as i32);
            }
            Command::SubpixelSprites(r) => {
                gl.use_program(Some(&sprite));
                attributes(&gl, &sprites, 4, SPRITE_FLOATS, r.start, 1);
                for c in 0..3 {
                    gl.color_mask(c == 0, c == 1, c == 2, false);
                    gl.uniform1i(channel.as_ref(), c);
                    gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (r.end - r.start) as i32);
                }
                gl.color_mask(true, true, true, true);
            }
            Command::Paths {
                paths,
                sprites: rects_range,
            } => {
                let Some((path, copy, msaa_fb, resolved_fb, vertices, rects)) = &layer else {
                    continue;
                };
                let first = offsets[paths.start as usize];
                let count = offsets[paths.end as usize] - first;
                if count == 0 || rects_range.is_empty() {
                    continue;
                }
                gl.bind_framebuffer(GL::FRAMEBUFFER, Some(msaa_fb));
                gl.clear(GL::COLOR_BUFFER_BIT);
                gl.use_program(Some(path));
                blend(&gl, 1);
                attributes(&gl, vertices, 3, PATH_FLOATS, first, 0);
                gl.draw_arrays(GL::TRIANGLES, 0, count as i32);
                gl.bind_framebuffer(GL::READ_FRAMEBUFFER, Some(msaa_fb));
                gl.bind_framebuffer(GL::DRAW_FRAMEBUFFER, Some(resolved_fb));
                let (iw, ih) = (w as i32, h as i32);
                gl.blit_framebuffer(
                    0,
                    0,
                    iw,
                    ih,
                    0,
                    0,
                    iw,
                    ih,
                    GL::COLOR_BUFFER_BIT,
                    GL::NEAREST,
                );
                gl.bind_framebuffer(GL::FRAMEBUFFER, None);
                gl.use_program(Some(copy));
                blend(&gl, 2);
                attributes(&gl, rects, 1, LAYER_FLOATS, rects_range.start, 1);
                gl.draw_arrays_instanced(
                    GL::TRIANGLE_STRIP,
                    0,
                    4,
                    (rects_range.end - rects_range.start) as i32,
                );
                blend(&gl, 0);
            }
        }
    }
    Ok(scene.commands.len() as u32)
}
