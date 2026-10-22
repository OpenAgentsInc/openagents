//! WebGL2: the shaders and the draw calls.
//!
//! Everything is lit by the same toon light: an area's ambient preset, its
//! point lights, the moon outdoors, a lightning flash, and the player's
//! lantern, a warm cone that lights only where they look. Light falls into
//! two bands, and what it doesn't reach falls to plum, never grey. The
//! architecture's surfaces are patterns made in the shader from a pattern
//! id and two palette colours. A last pass darkens the edges of the view to
//! plum (the vignette) and lays a paper grain over it.
//!
//! The house's frame (+x east, +z north, y up) is left-handed, so the view
//! mirrors z and the front faces wind clockwise.

use std::cell::RefCell;
use std::collections::HashMap;

use glam::{Mat4, Vec3};
use web_sys::{
    WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlTexture, WebGlUniformLocation,
    WebGlVertexArrayObject,
};

/// The lighting every lit shader shares.
const LIGHT: &str = r"
uniform vec3 u_ambient;
uniform vec4 u_lights[8];
uniform vec3 u_light_col[8];
uniform int u_light_count;
uniform vec3 u_eye;
uniform vec3 u_look;
uniform float u_lantern;
uniform float u_lantern_cos;
uniform vec3 u_moon_dir;
uniform vec3 u_moon_col;
uniform float u_flash;
uniform vec3 u_fog;
uniform vec2 u_fog_range;
uniform vec3 u_shadow;
const vec3 LICORICE = vec3(0.106, 0.090, 0.078);
const vec3 LANTERN = vec3(1.0, 0.77, 0.42);

vec3 light_at(vec3 p, vec3 n) {
  vec3 L = u_ambient;
  L += u_moon_col * (max(dot(n, -u_moon_dir), 0.0) * 0.7 + 0.3);
  for (int i = 0; i < 8; i++) {
    if (i >= u_light_count) break;
    vec3 d = u_lights[i].xyz - p;
    float dist = length(d);
    float att = clamp(1.0 - dist / u_lights[i].w, 0.0, 1.0);
    att *= att;
    float nd = max(dot(n, d / max(dist, 1e-3)), 0.0) * 0.6 + 0.4;
    L += u_light_col[i] * att * nd;
  }
  vec3 to_p = p - u_eye;
  float dist = length(to_p);
  vec3 dir = to_p / max(dist, 1e-3);
  float cone = smoothstep(u_lantern_cos, u_lantern_cos + 0.1, dot(dir, u_look));
  float fall = clamp(1.0 - dist / 12.0, 0.0, 1.0);
  float facing = max(dot(n, -dir), 0.0) * 0.7 + 0.3;
  L += LANTERN * u_lantern * (cone * fall * fall * facing * 1.35 + 0.22 * clamp(1.0 - dist / 3.5, 0.0, 1.0));
  L += vec3(u_flash);
  return L;
}

vec3 toon(vec3 base, vec3 L, vec3 p) {
  float i = dot(L, vec3(0.3, 0.59, 0.11));
  vec3 hue = L / max(i, 0.25);
  vec3 lit = base * hue * clamp(i, 0.75, 1.3);
  vec3 shade = base * u_shadow;
  vec3 c = mix(shade, lit, smoothstep(0.26, 0.32, i));
  c = mix(u_fog, c, smoothstep(0.015, 0.14, i));
  float d = length(p - u_eye);
  return mix(c, u_fog, smoothstep(u_fog_range.x, u_fog_range.y, d));
}
";

const ARCH_VS: &str = r"#version 300 es
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec3 a_a;
layout(location = 4) in vec3 a_b;
layout(location = 5) in float a_pattern;
uniform mat4 u_view_projection;
out vec3 v_world;
out vec3 v_normal;
out vec2 v_uv;
out vec3 v_a;
out vec3 v_b;
out float v_pattern;
void main() {
  v_world = a_position;
  v_normal = a_normal;
  v_uv = a_uv;
  v_a = a_a;
  v_b = a_b;
  v_pattern = a_pattern;
  gl_Position = u_view_projection * vec4(a_position, 1.0);
}
";

const ARCH_FS: &str = r"
in vec3 v_world;
in vec3 v_normal;
in vec2 v_uv;
in vec3 v_a;
in vec3 v_b;
in float v_pattern;
uniform float u_time;
out vec4 o_colour;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
vec2 hash2(vec2 p) {
  return fract(sin(vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)))) * 43758.5453);
}

vec3 surface(int id, vec2 uv, vec3 a, vec3 b) {
  if (id == 1) {
    vec2 g = uv * 1.6;
    vec2 ip = floor(g);
    vec2 fp = fract(g);
    float d1 = 8.0;
    float d2 = 8.0;
    vec2 cell = ip;
    for (int j = -1; j <= 1; j++) {
      for (int i = -1; i <= 1; i++) {
        vec2 o = vec2(float(i), float(j));
        vec2 r = o + 0.15 + hash2(ip + o) * 0.7 - fp;
        float d = dot(r, r);
        if (d < d1) { d2 = d1; d1 = d; cell = ip + o; } else if (d < d2) { d2 = d; }
      }
    }
    float edge = sqrt(d2) - sqrt(d1);
    vec3 stone = mix(a, mix(a, b, 0.45), hash(cell) * 0.8);
    stone = mix(stone, a * 1.15, smoothstep(0.35, 0.6, edge) * 0.35);
    return mix(mix(LICORICE, b, 0.25), stone, smoothstep(0.05, 0.11, edge));
  }
  if (id == 2) {
    float rows = 4.0;
    float row = floor(uv.y * rows);
    float u = uv.x + hash(vec2(row, 3.0)) * 5.0;
    float across = fract(uv.y * rows);
    float seam = 1.0 - smoothstep(0.0, 0.06, across) * (1.0 - smoothstep(0.94, 1.0, across));
    float butt = smoothstep(0.985, 1.0, fract(u / 2.7));
    float swirl = sin(u * 2.3 + sin(u * 0.6 + row * 1.7) * 2.2 + across * 7.0);
    float grain = smoothstep(0.9, 0.97, swirl);
    vec3 c = mix(a, a * 0.8, hash(vec2(row, 7.0)) * 0.7);
    c = mix(c, b, grain * 0.55);
    float knot = smoothstep(0.12, 0.08, length(vec2(fract(u / 1.9) - 0.5, across - 0.5) * vec2(2.0, 1.0)));
    c = mix(c, b, knot * 0.7);
    return mix(c, LICORICE, max(seam, butt));
  }
  if (id == 3) {
    float s = fract(uv.x * 1.6);
    vec3 c = s < 0.3 ? b : a;
    float edge = smoothstep(0.02, 0.0, abs(s - 0.3)) + smoothstep(0.02, 0.0, s);
    vec2 q = fract(uv * vec2(1.6, 1.4)) - vec2(0.65, 0.5);
    float flower = smoothstep(0.08, 0.06, length(q)) * step(0.3, s);
    c = mix(c, mix(a, b, 0.6), flower);
    return mix(c, LICORICE, clamp(edge, 0.0, 1.0) * 0.6);
  }
  if (id == 4) {
    vec2 q = fract(uv * 5.0) - 0.5;
    float size = 0.18 + 0.12 * sin(uv.x * 0.4 + uv.y * 0.3);
    return mix(b, a, smoothstep(size, size + 0.05, length(q)));
  }
  if (id == 5) {
    float h = 0.24;
    float row = floor(uv.y / h);
    float u = uv.x / 0.52 + mod(row, 2.0) * 0.5;
    float mortar = max(smoothstep(0.92, 0.97, fract(uv.y / h)), smoothstep(0.95, 0.99, fract(u)));
    vec3 c = mix(a, a * 0.78, hash(vec2(floor(u), row)) * 0.8);
    return mix(c, b, mortar);
  }
  if (id == 6) {
    vec2 t = floor(uv * 2.0);
    float check = mod(t.x + t.y, 2.0);
    vec2 f = fract(uv * 2.0);
    float line = 1.0 - smoothstep(0.0, 0.04, min(min(f.x, f.y), min(1.0 - f.x, 1.0 - f.y)));
    return mix(mix(a, b, check), LICORICE, line * 0.7);
  }
  if (id == 7) {
    float n = hash(floor(uv * 2.5));
    float speck = hash(floor(uv * 13.0));
    vec3 c = mix(a, a * 0.8, n);
    return mix(c, b, step(0.9, speck) * 0.8);
  }
  return a;
}

void main() {
  int id = int(v_pattern + 0.5);
  if (id >= 100) {
    float flicker = 0.88 + 0.12 * sin(u_time * 9.0 + v_world.x * 3.0) * sin(u_time * 5.3);
    o_colour = vec4(v_a * flicker * 1.15, 1.0);
    return;
  }
  vec3 base = surface(id, v_uv, v_a, v_b);
  vec3 n = normalize(v_normal);
  o_colour = vec4(toon(base, light_at(v_world, n), v_world), 1.0);
}
";

const MESH_VS: &str = r"#version 300 es
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_hull;
layout(location = 3) in vec3 a_colour;
layout(location = 4) in float a_weight;
uniform mat4 u_view_projection;
uniform mat4 u_model;
uniform float u_outline;
uniform vec2 u_viewport;
out vec3 v_normal;
out vec3 v_colour;
out vec3 v_world;
out float v_line;
void main() {
  vec4 world = u_model * vec4(a_position, 1.0);
  vec4 clip = u_view_projection * world;
  v_line = 0.0;
  if (u_outline > 0.0) {
    if (a_weight <= 0.0) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
    vec4 push = u_view_projection * vec4(mat3(u_model) * a_hull, 0.0);
    vec2 dir = push.xy;
    float len = length(dir);
    if (len > 1e-6) { dir /= len; }
    float near = clamp(5.0 / clip.w, 0.4, 1.0);
    clip.xy += dir * u_outline * a_weight * near * 2.0 / u_viewport * clip.w;
    v_line = 1.0;
  }
  v_normal = mat3(u_model) * a_normal;
  v_colour = a_colour;
  v_world = world.xyz;
  gl_Position = clip;
}
";

const MESH_FS: &str = r"
in vec3 v_normal;
in vec3 v_colour;
in vec3 v_world;
in float v_line;
uniform float u_glow;
out vec4 o_colour;
void main() {
  if (v_line > 0.5) {
    float d = length(v_world - u_eye);
    o_colour = vec4(mix(LICORICE, u_fog, smoothstep(u_fog_range.x, u_fog_range.y, d)), 1.0);
    return;
  }
  // Candle and lantern colours glow on their own.
  float bright = step(0.95, v_colour.r) * step(0.7, v_colour.g) * step(v_colour.b, 0.5);
  if (u_glow > 0.0 || bright > 0.5) {
    o_colour = vec4(v_colour * 1.1, 1.0);
    return;
  }
  vec3 n = normalize(v_normal);
  o_colour = vec4(toon(v_colour, light_at(v_world, n), v_world), 1.0);
}
";

const CARD_VS: &str = r"#version 300 es
layout(location = 0) in vec2 a_corner;
uniform mat4 u_view_projection;
uniform vec3 u_base;
uniform vec3 u_right;
uniform vec2 u_size;
uniform float u_lift;
out vec2 v_uv;
out vec3 v_world;
void main() {
  vec3 world = u_base + u_right * (a_corner.x - 0.5) * u_size.x + vec3(0.0, a_corner.y * u_size.y + u_lift, 0.0);
  v_uv = vec2(a_corner.x, 1.0 - a_corner.y);
  v_world = world;
  gl_Position = u_view_projection * vec4(world, 1.0);
}
";

const CARD_FS: &str = r"
in vec2 v_uv;
in vec3 v_world;
uniform sampler2D u_image;
uniform vec3 u_card_normal;
out vec4 o_colour;
void main() {
  vec4 t = texture(u_image, v_uv);
  if (t.a < 0.5) discard;
  o_colour = vec4(toon(t.rgb, light_at(v_world, u_card_normal), v_world), 1.0);
}
";

const SKY_VS: &str = r"#version 300 es
const vec2 P[3] = vec2[3](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
out vec2 v_ndc;
void main() {
  v_ndc = P[gl_VertexID];
  gl_Position = vec4(P[gl_VertexID], 0.999, 1.0);
}
";

const SKY_FS: &str = r"#version 300 es
precision highp float;
in vec2 v_ndc;
uniform mat4 u_inverse;
uniform vec3 u_top;
uniform vec3 u_horizon;
uniform vec3 u_ground;
out vec4 o_colour;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
void main() {
  vec4 far = u_inverse * vec4(v_ndc, 1.0, 1.0);
  vec4 near = u_inverse * vec4(v_ndc, -1.0, 1.0);
  vec3 dir = normalize(far.xyz / far.w - near.xyz / near.w);
  float up = dir.y;
  vec3 c = up > 0.0 ? mix(u_horizon, u_top, pow(clamp(up * 1.6, 0.0, 1.0), 0.7)) : u_ground;
  // A few stars high up.
  vec2 cell = floor(vec2(atan(dir.x, dir.z) * 80.0, up * 160.0));
  float star = step(0.995, hash(cell)) * smoothstep(0.25, 0.6, up);
  c = mix(c, vec3(0.945, 0.894, 0.784), star * 0.8);
  o_colour = vec4(c, 1.0);
}
";

const POST_FS: &str = r"#version 300 es
precision highp float;
in vec2 v_ndc;
uniform vec2 u_res;
uniform float u_vignette;
uniform float u_grain;
uniform float u_time;
uniform vec3 u_edge;
uniform float u_fade;
out vec4 o_colour;
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
void main() {
  vec2 c = v_ndc * 0.5;
  c.x *= u_res.x / u_res.y;
  float r = length(c);
  float v = smoothstep(0.1, 0.62, r) * min(u_vignette * 1.2, 0.95);
  v = max(v, u_fade);
  float g = (hash(floor(gl_FragCoord.xy / 1.5) + floor(u_time * 24.0)) - 0.5) * u_grain;
  vec3 colour = u_edge;
  float a = v;
  if (abs(g) * 0.5 > v) {
    colour = g > 0.0 ? vec3(0.945, 0.894, 0.784) : vec3(0.106, 0.090, 0.078);
    a = abs(g) * 0.5;
  }
  o_colour = vec4(colour, clamp(a, 0.0, 1.0));
}
";

const UI_FS: &str = r"#version 300 es
precision mediump float;
in vec2 v_ndc;
uniform sampler2D u_image;
out vec4 o_colour;
void main() {
  vec2 uv = vec2(v_ndc.x * 0.5 + 0.5, 0.5 - v_ndc.y * 0.5);
  o_colour = texture(u_image, uv);
}
";

fn compile(gl: &Gl, kind: u32, source: &str) -> Result<web_sys::WebGlShader, String> {
    let shader = gl.create_shader(kind).ok_or("no shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default())
    }
}

/// A linked program with its uniforms looked up once each.
pub struct Program {
    pub program: WebGlProgram,
    uniforms: RefCell<HashMap<&'static str, Option<WebGlUniformLocation>>>,
}

impl Program {
    fn new(gl: &Gl, vertex: &str, fragment: &str) -> Result<Self, String> {
        let vs = compile(gl, Gl::VERTEX_SHADER, vertex)?;
        let fs = compile(gl, Gl::FRAGMENT_SHADER, fragment)?;
        let program = gl.create_program().ok_or("no program")?;
        gl.attach_shader(&program, &vs);
        gl.attach_shader(&program, &fs);
        gl.link_program(&program);
        if !gl
            .get_program_parameter(&program, Gl::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return Err(gl.get_program_info_log(&program).unwrap_or_default());
        }
        Ok(Self {
            program,
            uniforms: RefCell::new(HashMap::new()),
        })
    }

    pub fn at(&self, gl: &Gl, name: &'static str) -> Option<WebGlUniformLocation> {
        self.uniforms
            .borrow_mut()
            .entry(name)
            .or_insert_with(|| gl.get_uniform_location(&self.program, name))
            .clone()
    }
}

fn lit(fragment: &str) -> String {
    format!("#version 300 es\nprecision highp float;\n{LIGHT}\n{fragment}")
}

/// A vertex array and how many vertices it holds.
pub struct GpuMesh {
    vao: WebGlVertexArrayObject,
    buffer: WebGlBuffer,
    pub count: i32,
}

/// The lighting one draw is under.
#[derive(Clone, Debug, PartialEq)]
pub struct Lighting {
    pub ambient: [f32; 3],
    /// Up to eight: position and radius, and colour times intensity.
    pub lights: Vec<([f32; 4], [f32; 3])>,
    pub moon_dir: [f32; 3],
    pub moon: [f32; 3],
    pub flash: f32,
    /// The lantern's strength, 0 to 1, and its cone's cosine.
    pub lantern: f32,
    pub lantern_cos: f32,
    pub fog: [f32; 3],
    pub fog_range: [f32; 2],
    pub shadow: [f32; 3],
}

/// The per-frame camera.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub look: Vec3,
    pub width: f32,
    pub height: f32,
    pub time: f32,
}

pub struct Gpu {
    pub gl: Gl,
    arch: Program,
    mesh: Program,
    card: Program,
    sky: Program,
    post: Program,
    ui: Program,
    quad: GpuMesh,
    empty: WebGlVertexArrayObject,
    /// CSS pixels to drawing-buffer pixels.
    pub scale: f32,
}

impl Gpu {
    /// Compiles the shaders.
    ///
    /// # Errors
    /// A shader doesn't compile or link.
    pub fn new(gl: Gl) -> Result<Self, String> {
        let arch = Program::new(&gl, ARCH_VS, &lit(ARCH_FS))?;
        let mesh = Program::new(&gl, MESH_VS, &lit(MESH_FS))?;
        let card = Program::new(&gl, CARD_VS, &lit(CARD_FS))?;
        let sky = Program::new(&gl, SKY_VS, SKY_FS)?;
        let post = Program::new(&gl, SKY_VS, POST_FS)?;
        let ui = Program::new(&gl, SKY_VS, UI_FS)?;
        let empty = gl.create_vertex_array().ok_or("no vertex array")?;
        let quad = upload(
            &gl,
            &[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0],
            &[2],
        )?;
        gl.enable(Gl::DEPTH_TEST);
        gl.enable(Gl::CULL_FACE);
        // The mirrored view turns counter-clockwise faces clockwise.
        gl.front_face(Gl::CW);
        Ok(Self {
            gl,
            arch,
            mesh,
            card,
            sky,
            post,
            ui,
            quad,
            empty,
            scale: 1.0,
        })
    }

    /// Uploads architecture (15 floats a vertex).
    pub fn upload_arch(&self, data: &[f32]) -> Option<GpuMesh> {
        upload(&self.gl, data, &[3, 3, 2, 3, 3, 1]).ok()
    }

    /// Uploads a toon mesh (13 floats a vertex).
    pub fn upload_mesh(&self, data: &[f32]) -> Option<GpuMesh> {
        upload(&self.gl, data, &[3, 3, 3, 3, 1]).ok()
    }

    pub fn free(&self, mesh: GpuMesh) {
        self.gl.delete_vertex_array(Some(&mesh.vao));
        self.gl.delete_buffer(Some(&mesh.buffer));
    }

    /// An RGBA texture, nearest-sampled for crisp die-cut edges.
    pub fn texture(&self, width: u32, height: u32, rgba: &[u8]) -> Option<WebGlTexture> {
        let gl = &self.gl;
        let texture = gl.create_texture()?;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA as i32,
            width as i32,
            height as i32,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(rgba),
        )
        .ok()?;
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        Some(texture)
    }

    /// A texture from a canvas (the UI layer).
    pub fn canvas_texture(
        &self,
        texture: Option<&WebGlTexture>,
        canvas: &web_sys::HtmlCanvasElement,
    ) -> Option<WebGlTexture> {
        let gl = &self.gl;
        let texture = match texture {
            Some(t) => t.clone(),
            None => gl.create_texture()?,
        };
        gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
        gl.pixel_storei(Gl::UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0);
        gl.tex_image_2d_with_u32_and_u32_and_html_canvas_element(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA as i32,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            canvas,
        )
        .ok()?;
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        Some(texture)
    }

    fn set_lighting(&self, program: &Program, camera: &Camera, l: &Lighting) {
        let gl = &self.gl;
        let at = |n| program.at(gl, n);
        gl.uniform_matrix4fv_with_f32_array(
            at("u_view_projection").as_ref(),
            false,
            &camera.view_projection.to_cols_array(),
        );
        gl.uniform3f(
            at("u_eye").as_ref(),
            camera.eye.x,
            camera.eye.y,
            camera.eye.z,
        );
        gl.uniform3f(
            at("u_look").as_ref(),
            camera.look.x,
            camera.look.y,
            camera.look.z,
        );
        gl.uniform3fv_with_f32_array(at("u_ambient").as_ref(), &l.ambient);
        let n = l.lights.len().min(8);
        let mut pos = [0.0_f32; 32];
        let mut col = [0.0_f32; 24];
        for (i, (p, c)) in l.lights.iter().take(8).enumerate() {
            pos[i * 4..i * 4 + 4].copy_from_slice(p);
            col[i * 3..i * 3 + 3].copy_from_slice(c);
        }
        gl.uniform4fv_with_f32_array(at("u_lights").as_ref(), &pos);
        gl.uniform3fv_with_f32_array(at("u_light_col").as_ref(), &col);
        gl.uniform1i(at("u_light_count").as_ref(), n as i32);
        gl.uniform1f(at("u_lantern").as_ref(), l.lantern);
        gl.uniform1f(at("u_lantern_cos").as_ref(), l.lantern_cos);
        gl.uniform3fv_with_f32_array(at("u_moon_dir").as_ref(), &l.moon_dir);
        gl.uniform3fv_with_f32_array(at("u_moon_col").as_ref(), &l.moon);
        gl.uniform1f(at("u_flash").as_ref(), l.flash);
        gl.uniform3fv_with_f32_array(at("u_fog").as_ref(), &l.fog);
        gl.uniform2f(at("u_fog_range").as_ref(), l.fog_range[0], l.fog_range[1]);
        gl.uniform3fv_with_f32_array(at("u_shadow").as_ref(), &l.shadow);
        gl.uniform1f(at("u_time").as_ref(), camera.time);
    }

    /// The sky behind everything.
    pub fn sky(&self, camera: &Camera, top: [f32; 3], horizon: [f32; 3], ground: [f32; 3]) {
        let gl = &self.gl;
        gl.use_program(Some(&self.sky.program));
        gl.depth_mask(false);
        gl.disable(Gl::DEPTH_TEST);
        let inverse = camera.view_projection.inverse();
        gl.uniform_matrix4fv_with_f32_array(
            self.sky.at(gl, "u_inverse").as_ref(),
            false,
            &inverse.to_cols_array(),
        );
        gl.uniform3fv_with_f32_array(self.sky.at(gl, "u_top").as_ref(), &top);
        gl.uniform3fv_with_f32_array(self.sky.at(gl, "u_horizon").as_ref(), &horizon);
        gl.uniform3fv_with_f32_array(self.sky.at(gl, "u_ground").as_ref(), &ground);
        gl.bind_vertex_array(Some(&self.empty));
        gl.disable(Gl::CULL_FACE);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        gl.enable(Gl::CULL_FACE);
        gl.depth_mask(true);
        gl.enable(Gl::DEPTH_TEST);
    }

    /// Architecture under `lighting`.
    pub fn arch(&self, mesh: &GpuMesh, camera: &Camera, lighting: &Lighting) {
        if mesh.count == 0 {
            return;
        }
        let gl = &self.gl;
        gl.use_program(Some(&self.arch.program));
        self.set_lighting(&self.arch, camera, lighting);
        // Walls are seen from inside and out.
        gl.disable(Gl::CULL_FACE);
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
        gl.enable(Gl::CULL_FACE);
    }

    /// A toon mesh at `model`, with its licorice outline.
    pub fn mesh(
        &self,
        mesh: &GpuMesh,
        model: &Mat4,
        camera: &Camera,
        lighting: &Lighting,
        glow: bool,
    ) {
        if mesh.count == 0 {
            return;
        }
        let gl = &self.gl;
        let p = &self.mesh;
        gl.use_program(Some(&p.program));
        self.set_lighting(p, camera, lighting);
        gl.uniform_matrix4fv_with_f32_array(
            p.at(gl, "u_model").as_ref(),
            false,
            &model.to_cols_array(),
        );
        gl.uniform2f(p.at(gl, "u_viewport").as_ref(), camera.width, camera.height);
        gl.uniform1f(p.at(gl, "u_glow").as_ref(), if glow { 1.0 } else { 0.0 });
        gl.bind_vertex_array(Some(&mesh.vao));
        // The outline: back faces pushed out along the hull.
        gl.cull_face(Gl::FRONT);
        gl.uniform1f(p.at(gl, "u_outline").as_ref(), 2.6 * self.scale);
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
        gl.cull_face(Gl::BACK);
        gl.uniform1f(p.at(gl, "u_outline").as_ref(), 0.0);
        gl.draw_arrays(Gl::TRIANGLES, 0, mesh.count);
    }

    /// A die-cut card standing at `base`, turned to face the eye.
    #[allow(clippy::too_many_arguments)]
    pub fn card(
        &self,
        texture: &WebGlTexture,
        base: Vec3,
        size: [f32; 2],
        lift: f32,
        camera: &Camera,
        lighting: &Lighting,
    ) {
        let gl = &self.gl;
        let p = &self.card;
        gl.use_program(Some(&p.program));
        self.set_lighting(p, camera, lighting);
        let to_eye =
            Vec3::new(camera.eye.x - base.x, 0.0, camera.eye.z - base.z).normalize_or_zero();
        // Right of the card as the eye sees it: the eye's frame is mirrored
        // in z, so its right is to_eye x up, not up x to_eye.
        let right = to_eye.cross(Vec3::Y).normalize_or_zero();
        gl.uniform3f(p.at(gl, "u_base").as_ref(), base.x, base.y, base.z);
        gl.uniform3f(p.at(gl, "u_right").as_ref(), right.x, right.y, right.z);
        gl.uniform2f(p.at(gl, "u_size").as_ref(), size[0], size[1]);
        gl.uniform1f(p.at(gl, "u_lift").as_ref(), lift);
        gl.uniform3f(p.at(gl, "u_card_normal").as_ref(), to_eye.x, 0.3, to_eye.z);
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.uniform1i(p.at(gl, "u_image").as_ref(), 0);
        gl.disable(Gl::CULL_FACE);
        gl.bind_vertex_array(Some(&self.quad.vao));
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        gl.enable(Gl::CULL_FACE);
    }

    /// The vignette and grain over the view; `fade` darkens all of it.
    pub fn post(&self, camera: &Camera, vignette: f32, grain: f32, fade: f32, edge: [f32; 3]) {
        let gl = &self.gl;
        let p = &self.post;
        gl.use_program(Some(&p.program));
        gl.disable(Gl::DEPTH_TEST);
        gl.enable(Gl::BLEND);
        gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        gl.uniform2f(p.at(gl, "u_res").as_ref(), camera.width, camera.height);
        gl.uniform1f(p.at(gl, "u_vignette").as_ref(), vignette);
        gl.uniform1f(p.at(gl, "u_grain").as_ref(), grain);
        gl.uniform1f(p.at(gl, "u_time").as_ref(), camera.time);
        gl.uniform1f(p.at(gl, "u_fade").as_ref(), fade);
        gl.uniform3fv_with_f32_array(p.at(gl, "u_edge").as_ref(), &edge);
        gl.bind_vertex_array(Some(&self.empty));
        gl.disable(Gl::CULL_FACE);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        gl.enable(Gl::CULL_FACE);
        gl.disable(Gl::BLEND);
        gl.enable(Gl::DEPTH_TEST);
    }

    /// The UI layer over everything.
    pub fn ui(&self, texture: &WebGlTexture) {
        let gl = &self.gl;
        let p = &self.ui;
        gl.use_program(Some(&p.program));
        gl.disable(Gl::DEPTH_TEST);
        gl.enable(Gl::BLEND);
        gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.uniform1i(p.at(gl, "u_image").as_ref(), 0);
        gl.bind_vertex_array(Some(&self.empty));
        gl.disable(Gl::CULL_FACE);
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);
        gl.enable(Gl::CULL_FACE);
        gl.disable(Gl::BLEND);
        gl.enable(Gl::DEPTH_TEST);
    }
}

fn upload(gl: &Gl, data: &[f32], sizes: &[i32]) -> Result<GpuMesh, String> {
    let vao = gl.create_vertex_array().ok_or("no vertex array")?;
    let buffer = gl.create_buffer().ok_or("no buffer")?;
    gl.bind_vertex_array(Some(&vao));
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    let stride: i32 = sizes.iter().sum();
    let mut offset = 0;
    for (index, size) in sizes.iter().enumerate() {
        gl.enable_vertex_attrib_array(index as u32);
        gl.vertex_attrib_pointer_with_i32(
            index as u32,
            *size,
            Gl::FLOAT,
            false,
            stride * 4,
            offset * 4,
        );
        offset += size;
    }
    let array = js_sys::Float32Array::from(data);
    gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::STATIC_DRAW);
    gl.bind_vertex_array(None);
    Ok(GpuMesh {
        vao,
        buffer,
        count: data.len() as i32 / stride,
    })
}
