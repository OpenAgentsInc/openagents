//! GPUI's Scene rendered with WebGL2 instanced attributes. No storage
//! buffers or wgpu. Reuses the first-frame painter's GLSL shader math.
//!
//! Backdrop blur has a defined fallback: keep the unblurred backdrop and
//! draw the view's ordinary translucent fill above it. Native video
//! surfaces aren't produced by the web image adapter.
#[path = "webgl/atlas.rs"]
mod atlas;
#[path = "webgl/shaders.rs"]
mod shaders;

use gpui::{
    AtlasTextureKind, Background, Bounds, Corners, DevicePixels, EdgeFadeParams, GpuSpecs, Hsla,
    PlatformAtlas, PrimitiveBatch, ScaledPixels, Scene, Size,
};
use openagents_first_frame_paint::shaders as first;
use std::{
    borrow::Cow,
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlFramebuffer, WebGlProgram,
    WebGlRenderbuffer, WebGlTexture, WebGlUniformLocation,
};

fn fail(why: &str) -> JsValue {
    JsValue::from_str(why)
}
fn rect(b: &Bounds<ScaledPixels>) -> [f32; 4] {
    [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0]
}
fn corners(c: &Corners<ScaledPixels>) -> [f32; 4] {
    [
        c.top_left.0,
        c.top_right.0,
        c.bottom_right.0,
        c.bottom_left.0,
    ]
}
fn color(c: Hsla) -> [f32; 4] {
    let c: gpui::Rgba = c.into();
    [c.r, c.g, c.b, c.a]
}
fn bytes(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding, and all bit patterns are valid u8 values.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}
const UNIFORMS: &[&str] = &[
    "u_vp",
    "u_tex",
    "u_atlas",
    "u_gamma",
    "u_contrast",
    "u_subpixel_contrast",
    "u_channel",
    "u_fade_y",
    "u_fade_x",
    "u_clip",
    "u_clip_radius",
    "u_bg0",
    "u_bg1",
    "u_bg_meta",
    "u_bg_solid",
    "u_stops",
    "u_dashed",
    "u_mask_bounds",
    "u_mask_params",
    "u_bottom_feather",
    "u_transform",
    "u_translation",
    "u_path_bounds",
    "u_blur",
    "u_tile",
];
struct Program {
    gl: GL,
    program: WebGlProgram,
    uniforms: HashMap<&'static str, Option<WebGlUniformLocation>>,
}
impl Program {
    fn new(gl: &GL, vs: &str, fs: &str) -> Result<Self, JsValue> {
        let program = gl
            .create_program()
            .ok_or_else(|| fail("no WebGL program"))?;
        for (kind, source) in [
            (GL::VERTEX_SHADER, first::vs(vs)),
            (GL::FRAGMENT_SHADER, first::fs(fs)),
        ] {
            let shader = gl
                .create_shader(kind)
                .ok_or_else(|| fail("no WebGL shader"))?;
            gl.shader_source(&shader, &source);
            gl.compile_shader(&shader);
            if !gl
                .get_shader_parameter(&shader, GL::COMPILE_STATUS)
                .as_bool()
                .unwrap_or(false)
            {
                let error = gl.get_shader_info_log(&shader).unwrap_or_default();
                gl.delete_shader(Some(&shader));
                gl.delete_program(Some(&program));
                return Err(fail(&error));
            }
            gl.attach_shader(&program, &shader);
            gl.delete_shader(Some(&shader));
        }
        for i in 0..7 {
            gl.bind_attrib_location(&program, i, &format!("a{i}"));
        }
        gl.link_program(&program);
        if !gl
            .get_program_parameter(&program, GL::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            let error = gl.get_program_info_log(&program).unwrap_or_default();
            gl.delete_program(Some(&program));
            return Err(fail(&error));
        }
        let uniforms = UNIFORMS
            .iter()
            .map(|&n| (n, gl.get_uniform_location(&program, n)))
            .collect();
        Ok(Self {
            gl: gl.clone(),
            program,
            uniforms,
        })
    }
    fn u(&self, n: &str) -> Option<&WebGlUniformLocation> {
        self.uniforms.get(n).and_then(Option::as_ref)
    }
    fn bind(&self, w: i32, h: i32) {
        self.gl.use_program(Some(&self.program));
        self.gl.uniform2f(self.u("u_vp"), w as f32, h as f32);
    }
    fn v4(&self, n: &str, v: &[f32]) {
        self.gl.uniform4fv_with_f32_array(self.u(n), v);
    }
    fn f(&self, n: &str, v: f32) {
        self.gl.uniform1f(self.u(n), v);
    }
    fn fade(&self, f: &EdgeFadeParams) {
        self.v4(
            "u_fade_y",
            &[f.top_y, f.bottom_y, f.band_top, f.band_bottom],
        );
        self.v4(
            "u_fade_x",
            &[f.left_x, f.right_x, f.band_left, f.band_right],
        );
        self.v4(
            "u_clip",
            &[f.clip_left, f.clip_top, f.clip_right, f.clip_bottom],
        );
        self.f("u_clip_radius", f.clip_radius);
    }
    fn background(&self, b: &Background) {
        if b.as_solid().is_some() {
            self.v4("u_bg_meta", &[0.0; 4]);
            return;
        }
        // Background's fields are private. Its public serde representation
        // exposes gradients and patterns without depending on its ABI.
        let value = serde_json::to_value(b).expect("GPUI background serializes");
        let tag = match value["tag"].as_str() {
            Some("LinearGradient") => 1.0,
            Some("PatternSlash") => 2.0,
            Some("Checkerboard") => 3.0,
            _ => 0.0,
        };
        self.v4(
            "u_bg_meta",
            &[
                tag,
                value["gradient_angle_or_pattern_height"]
                    .as_f64()
                    .unwrap_or(0.0) as f32,
                if value["color_space"] == "Oklab" {
                    1.0
                } else {
                    0.0
                },
                0.0,
            ],
        );
        let c = |v: &serde_json::Value| -> Hsla {
            serde_json::from_value(v.clone()).unwrap_or_default()
        };
        self.v4("u_bg_solid", &color(c(&value["solid"])));
        self.v4("u_bg0", &color(c(&value["colors"][0]["color"])));
        self.v4("u_bg1", &color(c(&value["colors"][1]["color"])));
        self.gl.uniform2f(
            self.u("u_stops"),
            value["colors"][0]["percentage"].as_f64().unwrap_or(0.0) as f32,
            value["colors"][1]["percentage"].as_f64().unwrap_or(1.0) as f32,
        );
    }
}
impl Drop for Program {
    fn drop(&mut self) {
        self.gl.delete_program(Some(&self.program));
    }
}

struct Layer {
    gl: GL,
    texture: WebGlTexture,
    resolved: WebGlFramebuffer,
    msaa: WebGlFramebuffer,
    rb: WebGlRenderbuffer,
}
impl Layer {
    fn new(gl: &GL, w: i32, h: i32) -> Result<Self, JsValue> {
        let texture = texture(gl, GL::NEAREST)?;
        gl.tex_storage_2d(GL::TEXTURE_2D, 1, GL::RGBA8, w, h);
        let resolved = gl
            .create_framebuffer()
            .ok_or_else(|| fail("no resolve framebuffer"))?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&resolved));
        gl.framebuffer_texture_2d(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::TEXTURE_2D,
            Some(&texture),
            0,
        );
        let rb = gl
            .create_renderbuffer()
            .ok_or_else(|| fail("no path renderbuffer"))?;
        gl.bind_renderbuffer(GL::RENDERBUFFER, Some(&rb));
        let samples = gl
            .get_parameter(GL::MAX_SAMPLES)?
            .as_f64()
            .unwrap_or(1.0)
            .min(4.0) as i32;
        gl.renderbuffer_storage_multisample(GL::RENDERBUFFER, samples, GL::RGBA8, w, h);
        let msaa = gl
            .create_framebuffer()
            .ok_or_else(|| fail("no path framebuffer"))?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&msaa));
        gl.framebuffer_renderbuffer(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::RENDERBUFFER,
            Some(&rb),
        );
        if gl.check_framebuffer_status(GL::FRAMEBUFFER) != GL::FRAMEBUFFER_COMPLETE {
            return Err(fail("incomplete path framebuffer"));
        }
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        Ok(Self {
            gl: gl.clone(),
            texture,
            resolved,
            msaa,
            rb,
        })
    }
}
impl Drop for Layer {
    fn drop(&mut self) {
        self.gl.delete_texture(Some(&self.texture));
        self.gl.delete_framebuffer(Some(&self.resolved));
        self.gl.delete_framebuffer(Some(&self.msaa));
        self.gl.delete_renderbuffer(Some(&self.rb));
    }
}
fn texture(gl: &GL, filter: u32) -> Result<WebGlTexture, JsValue> {
    let t = gl.create_texture().ok_or_else(|| fail("no texture"))?;
    gl.bind_texture(GL::TEXTURE_2D, Some(&t));
    for (n, v) in [
        (GL::TEXTURE_MIN_FILTER, filter),
        (GL::TEXTURE_MAG_FILTER, filter),
        (GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE),
        (GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameteri(GL::TEXTURE_2D, n, v as i32);
    }
    Ok(t)
}
fn attributes(gl: &GL, buffer: &WebGlBuffer, floats: usize, first: usize, divisor: u32) {
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(buffer));
    for i in 0..7 {
        if i < floats / 4 {
            gl.enable_vertex_attrib_array(i as u32);
            gl.vertex_attrib_pointer_with_i32(
                i as u32,
                4,
                GL::FLOAT,
                false,
                (floats * 4) as i32,
                ((first * floats + i * 4) * 4) as i32,
            );
            gl.vertex_attrib_divisor(i as u32, divisor);
        } else {
            gl.disable_vertex_attrib_array(i as u32);
        }
    }
}
fn upload(gl: &GL, buffer: &WebGlBuffer, data: &[f32]) {
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(buffer));
    gl.buffer_data_with_u8_array(GL::ARRAY_BUFFER, bytes(data), GL::STREAM_DRAW);
}
fn blend(gl: &GL, premultiplied: bool) {
    gl.blend_func_separate(
        if premultiplied {
            GL::ONE
        } else {
            GL::SRC_ALPHA
        },
        GL::ONE_MINUS_SRC_ALPHA,
        GL::ONE,
        GL::ONE_MINUS_SRC_ALPHA,
    );
}

pub(crate) struct Renderer {
    gl: GL,
    atlas: Arc<atlas::Atlas>,
    atlas_trait: Arc<dyn PlatformAtlas>,
    textures: Vec<Option<WebGlTexture>>,
    texture_generations: Vec<u64>,
    loss_listener: Option<(HtmlCanvasElement, Closure<dyn FnMut(web_sys::Event)>)>,
    quad: Program,
    shadow: Program,
    sprite: Program,
    image: Program,
    path: Program,
    copy: Program,
    underline: Program,
    buffers: Vec<WebGlBuffer>,
    layer: Option<Layer>,
    w: i32,
    h: i32,
    lost: Rc<Cell<bool>>,
}
impl Renderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let attrs = web_sys::WebGlContextAttributes::new();
        attrs.set_alpha(true);
        attrs.set_antialias(false);
        attrs.set_depth(false);
        attrs.set_stencil(false);
        attrs.set_premultiplied_alpha(true);
        let gl: GL = canvas
            .get_context_with_context_options("webgl2", &attrs)?
            .ok_or_else(|| fail("WebGL2 is unavailable"))?
            .dyn_into()?;
        // Retain the context across loss. On restore, draw rebuilds programs
        // and uploads the atlas's retained tile pixels.
        let needs_restore = Rc::new(Cell::new(false));
        let flag = needs_restore.clone();
        let lost = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            event.prevent_default();
            flag.set(true);
        });
        canvas
            .add_event_listener_with_callback("webglcontextlost", lost.as_ref().unchecked_ref())?;

        let max_size = gl
            .get_parameter(GL::MAX_TEXTURE_SIZE)?
            .as_f64()
            .unwrap_or(4096.0) as i32;
        let atlas = Arc::new(atlas::Atlas {
            state: Mutex::default(),
            max_size,
        });
        let mut renderer = Self::with_atlas(gl, atlas)?;
        renderer.loss_listener = Some((canvas.clone(), lost));
        renderer.lost = needs_restore;
        Ok(renderer)
    }
    fn with_atlas(gl: GL, atlas: Arc<atlas::Atlas>) -> Result<Self, JsValue> {
        let quad = Program::new(&gl, first::QUAD_VS, &shaders::quad())?;
        let shadow = Program::new(&gl, first::SHADOW_VS, first::SHADOW_FS)?;
        let sprite_vs = first::SPRITE_VS
            .replace(
                "void main()",
                "uniform float u_blur; uniform vec4 u_transform; uniform vec2 u_translation;\nvoid main()",
            )
            .replace(
                "to_device(a0.xy + u * a0.zw)",
                "to_device(mat2(u_transform.xy,u_transform.zw)*(a0.xy+u*a0.zw)+u_translation)",
            );
        let sprite_vs = sprite_vs.replace("v_uv = (a3.xy + u * a3.zw) / u_atlas;", "v_uv = (a3.xy + (u*a0.zw-3.0*u_blur)*(a3.zw/max(a0.zw-6.0*u_blur,vec2(0.000001))))/u_atlas;");
        let sprite = Program::new(&gl, &sprite_vs, &shaders::sprite())?;
        let image = Program::new(&gl, first::IMAGE_VS, &shaders::image())?;
        let path = Program::new(&gl, first::PATH_VS, &shaders::path())?;
        let copy = Program::new(&gl, first::LAYER_VS, first::LAYER_FS)?;
        let underline = Program::new(&gl, shaders::UNDERLINE_VS, shaders::UNDERLINE_FS)?;
        let mut buffers = Vec::new();
        for _ in 0..7 {
            buffers.push(
                gl.create_buffer()
                    .ok_or_else(|| fail("no instance buffer"))?,
            );
        }
        let atlas_trait = atlas.clone();
        Ok(Self {
            gl,
            atlas,
            atlas_trait,
            textures: Vec::new(),
            texture_generations: Vec::new(),
            loss_listener: None,
            quad,
            shadow,
            sprite,
            image,
            path,
            copy,
            underline,
            buffers,
            layer: None,
            w: 0,
            h: 0,
            lost: Rc::new(Cell::new(false)),
        })
    }
    pub fn max_texture_dimension(&self) -> u32 {
        self.atlas.max_size as u32
    }
    pub fn sprite_atlas(&self) -> &Arc<dyn PlatformAtlas> {
        &self.atlas_trait
    }
    pub fn supports_dual_source_blending(&self) -> bool {
        true
    }
    pub fn gpu_specs(&self) -> GpuSpecs {
        GpuSpecs {
            ..Default::default()
        }
    }
    pub fn update_transparency(&mut self, _transparent: bool) {}
    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        self.w = size.width.0;
        self.h = size.height.0;
        self.layer = None;
    }
    pub fn needs_redraw(&self) -> bool {
        self.lost.get() && !self.gl.is_context_lost()
    }
    pub fn draw(&mut self, scene: &Scene) -> bool {
        match self.paint(scene) {
            Ok(()) => !self.lost.get() && self.w > 0 && self.h > 0,
            Err(error) => {
                log::error!("WebGL2 frame failed: {error:?}");
                false
            }
        }
    }
    fn sync_atlas(&mut self) -> Result<(), JsValue> {
        let mut state = self.atlas.state.lock().unwrap();
        self.textures.resize_with(state.pages.len(), || None);
        self.texture_generations.resize(state.pages.len(), 0);
        self.gl.active_texture(GL::TEXTURE0);
        self.gl.pixel_storei(GL::UNPACK_ALIGNMENT, 1);
        for ((page, slot), generation) in state
            .pages
            .iter_mut()
            .zip(&mut self.textures)
            .zip(&mut self.texture_generations)
        {
            let Some(page) = page else {
                if let Some(t) = slot.take() {
                    self.gl.delete_texture(Some(&t));
                }
                continue;
            };
            if !page.dirty && slot.is_some() {
                continue;
            }
            if slot.is_none() || *generation != page.generation {
                if let Some(t) = slot.take() {
                    self.gl.delete_texture(Some(&t));
                }
                let t = texture(&self.gl, GL::LINEAR)?;
                let internal = if page.kind == AtlasTextureKind::Monochrome {
                    GL::R8
                } else {
                    GL::RGBA8
                };
                self.gl
                    .tex_storage_2d(GL::TEXTURE_2D, 1, internal, page.size, page.size);
                *slot = Some(t);
                *generation = page.generation;
            } else {
                self.gl.bind_texture(GL::TEXTURE_2D, slot.as_ref());
            }
            let format = if page.kind == AtlasTextureKind::Monochrome {
                GL::RED
            } else {
                GL::RGBA
            };
            for (tile, pixels) in page.tiles.values() {
                let converted: Cow<'_, [u8]> = if page.kind == AtlasTextureKind::Monochrome {
                    Cow::Borrowed(pixels)
                } else {
                    let mut p = pixels.clone();
                    for px in p.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                    Cow::Owned(p)
                };
                // Clear the guard on reused allocations as well as the tile.
                let channels = if page.kind == AtlasTextureKind::Monochrome {
                    1
                } else {
                    4
                };
                let (width, height) = (
                    tile.bounds.size.width.0 as usize,
                    tile.bounds.size.height.0 as usize,
                );
                let mut padded = vec![0u8; (width + 2) * (height + 2) * channels];
                for row in 0..height {
                    let start = ((row + 1) * (width + 2) + 1) * channels;
                    padded[start..start + width * channels].copy_from_slice(
                        &converted[row * width * channels..(row + 1) * width * channels],
                    );
                }
                self.gl
                    .tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
                        GL::TEXTURE_2D,
                        0,
                        tile.bounds.origin.x.0 - 1,
                        tile.bounds.origin.y.0 - 1,
                        tile.bounds.size.width.0 + 2,
                        tile.bounds.size.height.0 + 2,
                        format,
                        GL::UNSIGNED_BYTE,
                        Some(&padded),
                    )?;
            }
            page.dirty = false;
        }
        Ok(())
    }
    fn bind_atlas(&self, program: &Program, index: u32) {
        let state = self.atlas.state.lock().unwrap();
        let Some(page) = state.pages.get(index as usize).and_then(Option::as_ref) else {
            return;
        };
        self.gl.active_texture(GL::TEXTURE0);
        self.gl
            .bind_texture(GL::TEXTURE_2D, self.textures[index as usize].as_ref());
        self.gl.uniform1i(program.u("u_tex"), 0);
        self.gl
            .uniform2f(program.u("u_atlas"), page.size as f32, page.size as f32);
    }
    fn paint(&mut self, scene: &Scene) -> Result<(), JsValue> {
        if self.gl.is_context_lost() {
            self.lost.set(true);
            return Ok(());
        }
        if self.lost.get() {
            let (w, h) = (self.w, self.h);
            let mut replacement = Self::with_atlas(self.gl.clone(), self.atlas.clone())?;
            replacement.loss_listener = self.loss_listener.take();
            replacement.lost = self.lost.clone();
            replacement.lost.set(false);
            *self = replacement;
            self.w = w;
            self.h = h;
        }
        if self.w <= 0 || self.h <= 0 {
            return Ok(());
        }
        self.sync_atlas()?;
        let gl = &self.gl;
        let (w, h) = (self.w, self.h);
        gl.viewport(0, 0, w, h);
        gl.disable(GL::DEPTH_TEST);
        gl.enable(GL::BLEND);
        gl.color_mask(true, true, true, true);
        blend(gl, false);
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        gl.clear_color(0.0, 0.0, 0.0, 0.0);
        gl.clear(GL::COLOR_BUFFER_BIT);
        let mut quads = Vec::with_capacity(scene.quads.len() * 24);
        for q in &scene.quads {
            quads.extend(rect(&q.bounds));
            quads.extend(rect(&q.content_mask.bounds));
            quads.extend(color(q.background.as_solid().unwrap_or_default()));
            quads.extend(color(q.border_color));
            quads.extend(corners(&q.corner_radii));
            quads.extend([
                q.border_widths.top.0,
                q.border_widths.right.0,
                q.border_widths.bottom.0,
                q.border_widths.left.0,
            ]);
        }
        upload(gl, &self.buffers[0], &quads);
        let mut shadows = Vec::with_capacity(scene.shadows.len() * 28);
        for s in &scene.shadows {
            shadows.extend(rect(&s.bounds));
            shadows.extend(corners(&s.corner_radii));
            shadows.extend(rect(&s.content_mask.bounds));
            shadows.extend(color(s.color));
            shadows.extend(rect(&s.element_bounds));
            shadows.extend(corners(&s.element_corner_radii));
            shadows.extend([
                s.blur_radius.0,
                if s.inset != 0 { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ]);
        }
        upload(gl, &self.buffers[1], &shadows);
        let mut sprites = Vec::new();
        for s in &scene.monochrome_sprites {
            sprites.extend(rect(&s.bounds));
            sprites.extend(rect(&s.content_mask.bounds));
            sprites.extend(color(s.color));
            sprites.extend([
                s.tile.bounds.origin.x.0 as f32,
                s.tile.bounds.origin.y.0 as f32,
                s.tile.bounds.size.width.0 as f32,
                s.tile.bounds.size.height.0 as f32,
            ]);
        }
        let subpixel_base = scene.monochrome_sprites.len();
        for s in &scene.subpixel_sprites {
            sprites.extend(rect(&s.bounds));
            sprites.extend(rect(&s.content_mask.bounds));
            sprites.extend(color(s.color));
            sprites.extend([
                s.tile.bounds.origin.x.0 as f32,
                s.tile.bounds.origin.y.0 as f32,
                s.tile.bounds.size.width.0 as f32,
                s.tile.bounds.size.height.0 as f32,
            ]);
        }
        upload(gl, &self.buffers[2], &sprites);
        let mut images = Vec::new();
        for s in &scene.polychrome_sprites {
            images.extend(rect(&s.bounds));
            images.extend(rect(&s.content_mask.bounds));
            images.extend([
                s.tile.bounds.origin.x.0 as f32,
                s.tile.bounds.origin.y.0 as f32,
                s.tile.bounds.size.width.0 as f32,
                s.tile.bounds.size.height.0 as f32,
            ]);
            images.extend([
                s.opacity,
                if s.grayscale == true.into() { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ]);
            images.extend(corners(&s.corner_radii));
        }
        upload(gl, &self.buffers[3], &images);
        let mut underlines = Vec::new();
        for u in &scene.underlines {
            underlines.extend(rect(&u.bounds));
            underlines.extend(rect(&u.content_mask.bounds));
            underlines.extend(color(u.color));
            underlines.extend([
                u.thickness.0,
                if u.wavy == true.into() { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ]);
        }
        upload(gl, &self.buffers[4], &underlines);
        let mut vertices = Vec::new();
        let mut offsets = Vec::new();
        for p in &scene.paths {
            offsets.push(vertices.len() / 12);
            for v in &p.vertices {
                vertices.extend([
                    v.xy_position.x.0,
                    v.xy_position.y.0,
                    v.st_position.x,
                    v.st_position.y,
                ]);
                vertices.extend(color(p.color.as_solid().unwrap_or_default()));
                vertices.extend(rect(&p.clipped_bounds()));
            }
        }
        offsets.push(vertices.len() / 12);
        upload(gl, &self.buffers[5], &vertices);
        if !scene.paths.is_empty() && self.layer.is_none() {
            gl.active_texture(GL::TEXTURE1);
            self.layer = Some(Layer::new(gl, w, h)?);
        }
        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Quads(range) => {
                    self.quad.bind(w, h);
                    let mut at = range.start;
                    while at < range.end {
                        let q = &scene.quads[at];
                        let mut end = at + 1;
                        if q.background.as_solid().is_some() {
                            while end < range.end {
                                let n = &scene.quads[end];
                                if n.background.as_solid().is_none()
                                    || q.fade != n.fade
                                    || q.border_style != n.border_style
                                {
                                    break;
                                }
                                end += 1;
                            }
                        }
                        self.quad.fade(&q.fade);
                        self.quad.background(&q.background);
                        self.quad.f(
                            "u_dashed",
                            if q.border_style == gpui::BorderStyle::Solid {
                                0.0
                            } else {
                                1.0
                            },
                        );
                        attributes(gl, &self.buffers[0], 24, at, 1);
                        gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (end - at) as i32);
                        at = end;
                    }
                }
                PrimitiveBatch::Shadows(r) => {
                    self.shadow.bind(w, h);
                    attributes(gl, &self.buffers[1], 28, r.start, 1);
                    gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, r.len() as i32);
                }
                PrimitiveBatch::MonochromeSprites { texture_id, range } => {
                    self.sprite.bind(w, h);
                    self.bind_atlas(&self.sprite, texture_id.index);
                    self.sprite
                        .v4("u_gamma", &gpui::get_gamma_correction_ratios(1.8));
                    self.sprite.f("u_contrast", 1.0);
                    gl.uniform1i(self.sprite.u("u_channel"), -1);
                    let mut i = range.start;
                    while i < range.end {
                        let s = &scene.monochrome_sprites[i];
                        let mut end = i + 1;
                        if s.blur == 0.0 {
                            while end < range.end {
                                let n = &scene.monochrome_sprites[end];
                                if n.fade != s.fade
                                    || n.transformation != s.transformation
                                    || n.blur != 0.0
                                {
                                    break;
                                }
                                end += 1;
                            }
                        }
                        self.sprite.fade(&s.fade);
                        self.sprite.f("u_blur", s.blur);
                        self.sprite.v4(
                            "u_tile",
                            &[
                                s.tile.bounds.origin.x.0 as f32,
                                s.tile.bounds.origin.y.0 as f32,
                                s.tile.bounds.size.width.0 as f32,
                                s.tile.bounds.size.height.0 as f32,
                            ],
                        );
                        let m = s.transformation.rotation_scale;
                        self.sprite
                            .v4("u_transform", &[m[0][0], m[1][0], m[0][1], m[1][1]]);
                        gl.uniform2fv_with_f32_array(
                            self.sprite.u("u_translation"),
                            &s.transformation.translation,
                        );
                        attributes(gl, &self.buffers[2], 16, i, 1);
                        gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (end - i) as i32);
                        i = end;
                    }
                }
                PrimitiveBatch::SubpixelSprites { texture_id, range } => {
                    self.sprite.bind(w, h);
                    self.bind_atlas(&self.sprite, texture_id.index);
                    self.sprite
                        .v4("u_gamma", &gpui::get_gamma_correction_ratios(1.8));
                    self.sprite.f("u_subpixel_contrast", 0.5);
                    self.sprite.f("u_blur", 0.0);
                    for i in range {
                        let s = &scene.subpixel_sprites[i];
                        self.sprite.fade(&s.fade);
                        let m = s.transformation.rotation_scale;
                        self.sprite
                            .v4("u_transform", &[m[0][0], m[1][0], m[0][1], m[1][1]]);
                        gl.uniform2fv_with_f32_array(
                            self.sprite.u("u_translation"),
                            &s.transformation.translation,
                        );
                        attributes(gl, &self.buffers[2], 16, subpixel_base + i, 1);
                        for c in 0..3 {
                            gl.color_mask(c == 0, c == 1, c == 2, false);
                            gl.uniform1i(self.sprite.u("u_channel"), c);
                            gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, 1);
                        }
                        gl.color_mask(true, true, true, true);
                    }
                }
                PrimitiveBatch::PolychromeSprites { texture_id, range } => {
                    self.image.bind(w, h);
                    self.bind_atlas(&self.image, texture_id.index);
                    for i in range {
                        let s = &scene.polychrome_sprites[i];
                        self.image.fade(&s.fade);
                        self.image.v4("u_mask_bounds", &rect(&s.alpha_mask.bounds));
                        self.image.v4(
                            "u_mask_params",
                            &[
                                s.alpha_mask.radius,
                                s.alpha_mask.feather,
                                s.alpha_mask.clearance,
                                s.alpha_mask.bottom_y,
                            ],
                        );
                        self.image
                            .f("u_bottom_feather", s.alpha_mask.bottom_feather);
                        attributes(gl, &self.buffers[3], 20, i, 1);
                        gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, 1);
                    }
                }
                PrimitiveBatch::Underlines(r) => {
                    self.underline.bind(w, h);
                    attributes(gl, &self.buffers[4], 16, r.start, 1);
                    gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, r.len() as i32);
                }
                PrimitiveBatch::Paths(r) => {
                    let layer = self.layer.as_ref().unwrap();
                    gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&layer.msaa));
                    gl.clear(GL::COLOR_BUFFER_BIT);
                    blend(gl, true);
                    self.path.bind(w, h);
                    for i in r.clone() {
                        let p = &scene.paths[i];
                        self.path.background(&p.color);
                        self.path.v4("u_path_bounds", &rect(&p.bounds));
                        attributes(gl, &self.buffers[5], 12, offsets[i], 0);
                        gl.draw_arrays(GL::TRIANGLES, 0, (offsets[i + 1] - offsets[i]) as i32);
                    }
                    gl.bind_framebuffer(GL::READ_FRAMEBUFFER, Some(&layer.msaa));
                    gl.bind_framebuffer(GL::DRAW_FRAMEBUFFER, Some(&layer.resolved));
                    gl.blit_framebuffer(0, 0, w, h, 0, 0, w, h, GL::COLOR_BUFFER_BIT, GL::NEAREST);
                    gl.bind_framebuffer(GL::FRAMEBUFFER, None);
                    self.copy.bind(w, h);
                    gl.active_texture(GL::TEXTURE1);
                    gl.bind_texture(GL::TEXTURE_2D, Some(&layer.texture));
                    gl.uniform1i(self.copy.u("u_tex"), 1);
                    let paths = &scene.paths[r];
                    let mut rects = Vec::new();
                    if paths.first().map(|p| p.order) == paths.last().map(|p| p.order) {
                        for p in paths {
                            rects.extend(rect(&p.clipped_bounds()));
                        }
                    } else if let Some(first) = paths.first() {
                        let mut b = first.clipped_bounds();
                        for p in &paths[1..] {
                            b = b.union(&p.clipped_bounds());
                        }
                        rects.extend(rect(&b));
                    }
                    upload(gl, &self.buffers[6], &rects);
                    attributes(gl, &self.buffers[6], 4, 0, 1);
                    gl.draw_arrays_instanced(GL::TRIANGLE_STRIP, 0, 4, (rects.len() / 4) as i32);
                    blend(gl, false);
                }
                PrimitiveBatch::Surfaces(_) => {}
            }
        }
        Ok(())
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some((canvas, listener)) = &self.loss_listener {
            let _ = canvas.remove_event_listener_with_callback(
                "webglcontextlost",
                listener.as_ref().unchecked_ref(),
            );
        }
        for b in &self.buffers {
            self.gl.delete_buffer(Some(b));
        }
        for t in self.textures.iter().flatten() {
            self.gl.delete_texture(Some(t));
        }
    }
}
