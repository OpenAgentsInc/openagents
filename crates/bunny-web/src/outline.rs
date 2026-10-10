//! The `npr.outline-gray.v1` presentation in WebGL2
//! (`docs/verse/games/grow-little-bunny.md`, Art direction).
//!
//! The scene draws into an offscreen target with three parts: the flat
//! fill, each pixel's normal and object id, and depth. A full-screen pass
//! then draws the lines: where the object changes, where normals turn more
//! than 50 degrees, and where depth jumps, in ink that fades toward a light
//! gray with distance instead of fog. Things drawn with their own
//! inverted-hull outline (the bunny, its food, the farmer's net in a swing)
//! write a reserved id so the pass leaves those lines alone.
//!
//! Object ids: 0 is the ground and what lies flat on it (and the sky), 1 to
//! 126 gray things, 128 to 254 coloured (`chroma`) things, 255 a hull line.

use web_sys::{
    WebGl2RenderingContext as Gl, WebGlFramebuffer, WebGlProgram, WebGlTexture,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::look::Tier;

const VERTEX: &str = r"#version 300 es
out vec2 v_uv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  v_uv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
";

const FRAGMENT: &str = r"#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_colour;
uniform sampler2D u_normal;
uniform sampler2D u_depth;
uniform vec2 u_texel;
uniform float u_width;
uniform float u_near;
uniform float u_far;
uniform float u_contrast;
uniform vec3 u_ink;
uniform vec3 u_far_ink;
out vec4 o_colour;
float linear_depth(vec2 uv) {
  float z = texture(u_depth, uv).r * 2.0 - 1.0;
  return 2.0 * u_near * u_far / (u_far + u_near - z * (u_far - u_near));
}
void main() {
  vec4 colour = texture(u_colour, v_uv);
  vec4 n0 = texture(u_normal, v_uv);
  if (n0.a > 0.998) {
    o_colour = vec4(u_contrast > 0.5 ? vec3(0.0) : colour.rgb, 1.0);
    return;
  }
  float z0 = linear_depth(v_uv);
  vec3 v0 = n0.xyz * 2.0 - 1.0;
  float edge = 0.0;
  // Coloured things carry their own outline; inside them only another
  // object's edge draws. Far away, lines thin so small things stay readable.
  bool chroma0 = n0.a > 0.499;
  float reach = u_width * mix(1.0, 0.5, smoothstep(10.0, 40.0, z0));
  vec2 steps[4] = vec2[](vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.7, 0.7), vec2(0.7, -0.7));
  for (int i = 0; i < 4; i++) {
    vec2 o = steps[i] * u_texel * reach;
    vec4 na = texture(u_normal, v_uv + o);
    vec4 nb = texture(u_normal, v_uv - o);
    if (na.a > 0.998 || nb.a > 0.998) continue;
    if (abs(na.a - n0.a) > 0.002 || abs(nb.a - n0.a) > 0.002) edge = 1.0;
    vec3 va = na.xyz * 2.0 - 1.0;
    vec3 vb = nb.xyz * 2.0 - 1.0;
    // The sky has no normal; only the depth and id tests see its edge.
    bool solid = dot(v0, v0) > 0.25 && dot(va, va) > 0.25 && dot(vb, vb) > 0.25;
    if (chroma0) continue;
    if (solid && (dot(v0, va) < 0.643 || dot(v0, vb) < 0.643)) edge = 1.0;
    float ia = 1.0 / linear_depth(v_uv + o);
    float ib = 1.0 / linear_depth(v_uv - o);
    float i0 = 1.0 / z0;
    if (abs(ia + ib - 2.0 * i0) > 0.03 * i0) edge = 1.0;
  }
  vec3 fill = colour.rgb;
  vec3 ink = mix(u_ink, u_far_ink, smoothstep(12.0, 75.0, z0));
  if (u_contrast > 0.5) {
    ink = vec3(0.0);
    bool chroma = n0.a > 0.499;
    if (!chroma) {
      float light = dot(fill, vec3(0.299, 0.587, 0.114));
      fill = light > 0.82 ? vec3(1.0) : vec3(0.72);
    }
  }
  o_colour = vec4(mix(fill, ink, edge), 1.0);
}
";

/// The offscreen target and the line pass.
pub struct Outline {
    framebuffer: WebGlFramebuffer,
    colour: WebGlTexture,
    normal: WebGlTexture,
    depth: WebGlTexture,
    program: WebGlProgram,
    vao: WebGlVertexArrayObject,
    u_texel: Option<WebGlUniformLocation>,
    u_width: Option<WebGlUniformLocation>,
    u_near: Option<WebGlUniformLocation>,
    u_far: Option<WebGlUniformLocation>,
    u_contrast: Option<WebGlUniformLocation>,
    u_ink: Option<WebGlUniformLocation>,
    u_far_ink: Option<WebGlUniformLocation>,
    size: (i32, i32),
}

fn texture(gl: &Gl, unit: u32) -> Result<WebGlTexture, String> {
    let texture = gl.create_texture().ok_or("no texture")?;
    gl.active_texture(Gl::TEXTURE0 + unit);
    gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
    for (name, value) in [
        (Gl::TEXTURE_MIN_FILTER, Gl::NEAREST),
        (Gl::TEXTURE_MAG_FILTER, Gl::NEAREST),
        (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE),
        (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameteri(Gl::TEXTURE_2D, name, value as i32);
    }
    Ok(texture)
}

impl Outline {
    pub fn new(gl: &Gl) -> Result<Self, String> {
        let program = crate::app::program(gl, VERTEX, FRAGMENT)?;
        let at = |name: &str| gl.get_uniform_location(&program, name);
        gl.use_program(Some(&program));
        for (index, name) in ["u_colour", "u_normal", "u_depth"].iter().enumerate() {
            gl.uniform1i(at(name).as_ref(), index as i32);
        }
        let outline = Self {
            framebuffer: gl.create_framebuffer().ok_or("no framebuffer")?,
            colour: texture(gl, 0)?,
            normal: texture(gl, 1)?,
            depth: texture(gl, 2)?,
            vao: gl.create_vertex_array().ok_or("no vertex array")?,
            u_texel: at("u_texel"),
            u_width: at("u_width"),
            u_near: at("u_near"),
            u_far: at("u_far"),
            u_contrast: at("u_contrast"),
            u_ink: at("u_ink"),
            u_far_ink: at("u_far_ink"),
            program,
            size: (0, 0),
        };
        outline.resize(gl, 4, 4)?;
        Ok(outline)
    }

    fn resize(&self, gl: &Gl, width: i32, height: i32) -> Result<(), String> {
        let image = |texture: &WebGlTexture, unit: u32, internal: u32, format: u32, kind: u32| {
            gl.active_texture(Gl::TEXTURE0 + unit);
            gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
            gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                0,
                internal as i32,
                width,
                height,
                0,
                format,
                kind,
                None,
            )
        };
        image(&self.colour, 0, Gl::RGBA8, Gl::RGBA, Gl::UNSIGNED_BYTE)
            .map_err(|_| "colour target")?;
        image(&self.normal, 1, Gl::RGBA8, Gl::RGBA, Gl::UNSIGNED_BYTE)
            .map_err(|_| "normal target")?;
        image(
            &self.depth,
            2,
            Gl::DEPTH_COMPONENT24,
            Gl::DEPTH_COMPONENT,
            Gl::UNSIGNED_INT,
        )
        .map_err(|_| "depth target")?;
        gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&self.framebuffer));
        for (attachment, texture) in [
            (Gl::COLOR_ATTACHMENT0, &self.colour),
            (Gl::COLOR_ATTACHMENT1, &self.normal),
            (Gl::DEPTH_ATTACHMENT, &self.depth),
        ] {
            gl.framebuffer_texture_2d(
                Gl::FRAMEBUFFER,
                attachment,
                Gl::TEXTURE_2D,
                Some(texture),
                0,
            );
        }
        let complete = gl.check_framebuffer_status(Gl::FRAMEBUFFER) == Gl::FRAMEBUFFER_COMPLETE;
        gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        if complete {
            Ok(())
        } else {
            Err("the offscreen target is incomplete".into())
        }
    }

    /// Binds the offscreen target at `width` by `height` and clears it to
    /// `sky`.
    pub fn begin(&mut self, gl: &Gl, width: i32, height: i32, sky: [f32; 3]) -> Result<(), String> {
        if self.size != (width, height) {
            self.resize(gl, width, height)?;
            self.size = (width, height);
        }
        gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&self.framebuffer));
        let buffers =
            js_sys::Array::of2(&Gl::COLOR_ATTACHMENT0.into(), &Gl::COLOR_ATTACHMENT1.into());
        gl.draw_buffers(&buffers);
        gl.viewport(0, 0, width, height);
        gl.clear_bufferfv_with_f32_array(Gl::COLOR, 0, &[sky[0], sky[1], sky[2], 1.0]);
        gl.clear_bufferfv_with_f32_array(Gl::COLOR, 1, &[0.5, 0.5, 0.5, 0.0]);
        gl.clear(Gl::DEPTH_BUFFER_BIT);
        Ok(())
    }

    /// Draws the target to the canvas with its lines.
    #[allow(clippy::too_many_arguments)]
    pub fn finish(
        &self,
        gl: &Gl,
        canvas: (i32, i32),
        near: f32,
        far: f32,
        tier: Tier,
        contrast: bool,
        ink: [f32; 3],
        far_ink: [f32; 3],
    ) {
        gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        gl.viewport(0, 0, canvas.0, canvas.1);
        gl.use_program(Some(&self.program));
        gl.bind_vertex_array(Some(&self.vao));
        for (unit, texture) in [(0, &self.colour), (1, &self.normal), (2, &self.depth)] {
            gl.active_texture(Gl::TEXTURE0 + unit);
            gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        }
        let filter = if self.size == canvas {
            Gl::NEAREST
        } else {
            Gl::LINEAR
        };
        gl.active_texture(Gl::TEXTURE0);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, filter as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, filter as i32);
        gl.uniform2f(
            self.u_texel.as_ref(),
            1.0 / self.size.0 as f32,
            1.0 / self.size.1 as f32,
        );
        gl.uniform1f(self.u_width.as_ref(), tier.line_radius(self.size.1));
        gl.uniform1f(self.u_near.as_ref(), near);
        gl.uniform1f(self.u_far.as_ref(), far);
        gl.uniform1f(self.u_contrast.as_ref(), if contrast { 1.0 } else { 0.0 });
        gl.uniform3f(self.u_ink.as_ref(), ink[0], ink[1], ink[2]);
        gl.uniform3f(self.u_far_ink.as_ref(), far_ink[0], far_ink[1], far_ink[2]);
        gl.disable(Gl::DEPTH_TEST);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        gl.enable(Gl::DEPTH_TEST);
        gl.bind_vertex_array(None);
    }
}
