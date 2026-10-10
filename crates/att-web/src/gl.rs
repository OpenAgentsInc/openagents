//! Draws the scene with WebGL2 in the Grid's flat look: shaded faces and
//! edge lines drawn as they are, the box's dark glass, and additive light
//! (sparks and beams).

use glam::{Mat4, Vec3};
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram, WebGlUniformLocation,
    WebGlVertexArrayObject,
};

use crate::mesh::{LINE_STRIDE, Lines, Mesh, STRIDE};
use crate::scene::{self, Camera, Frame, Part};

const SOLID_VERTEX: &str = r"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_colour;
uniform mat4 u_view_proj;
uniform mat4 u_model;
out vec3 v_world;
out vec3 v_normal;
out vec3 v_colour;
void main() {
  vec4 world = u_model * vec4(a_pos, 1.0);
  v_world = world.xyz;
  v_normal = mat3(u_model) * a_normal;
  v_colour = a_colour;
  gl_Position = u_view_proj * world;
}
";

const SOLID_FRAGMENT: &str = r"#version 300 es
precision highp float;
in vec3 v_world;
in vec3 v_normal;
in vec3 v_colour;
uniform vec3 u_tint;
uniform vec3 u_glow;
uniform vec3 u_fog;
uniform float u_alpha;
out vec4 o_colour;
void main() {
  // The Grid's flat look: every colour is drawn as it is (faces carry
  // their shade), plus glow, fading to the field with distance.
  vec3 colour = v_colour * u_tint + u_glow;
  float fog = smoothstep(14.0, 34.0, length(v_world.xz));
  colour = mix(colour, u_fog, fog);
  o_colour = vec4(pow(clamp(colour, 0.0, 1.0), vec3(1.0 / 2.2)), u_alpha);
}
";

const LIGHT_VERTEX: &str = r"#version 300 es
layout(location = 0) in vec2 a_corner;
uniform mat4 u_view_proj;
uniform vec3 u_a;
uniform vec3 u_b;
uniform vec3 u_eye;
uniform vec3 u_right;
uniform vec3 u_up;
uniform float u_size;
uniform float u_ribbon;
out vec2 v_uv;
void main() {
  vec3 p;
  if (u_ribbon > 0.5) {
    float t = a_corner.x * 0.5 + 0.5;
    vec3 along = mix(u_a, u_b, t);
    vec3 dir = normalize(u_b - u_a);
    vec3 side = normalize(cross(dir, u_eye - along));
    p = along + side * a_corner.y * u_size;
    v_uv = vec2(0.0, a_corner.y);
  } else {
    p = u_a + (u_right * a_corner.x + u_up * a_corner.y) * u_size;
    v_uv = a_corner;
  }
  gl_Position = u_view_proj * vec4(p, 1.0);
}
";

const LIGHT_FRAGMENT: &str = r"#version 300 es
precision highp float;
in vec2 v_uv;
uniform vec3 u_colour;
out vec4 o_colour;
void main() {
  float r = length(v_uv);
  float core = exp(-r * r * 14.0);
  float halo = exp(-r * r * 3.5) * 0.45;
  float edge = 1.0 - smoothstep(0.75, 1.0, r);
  vec3 c = u_colour * (core * 1.6 + halo) * edge;
  o_colour = vec4(1.0 - exp(-c), 1.0);
}
";

struct GpuMesh {
    vao: WebGlVertexArrayObject,
    count: i32,
    mode: u32,
}

struct Uniforms {
    view_proj: Option<WebGlUniformLocation>,
    model: Option<WebGlUniformLocation>,
    tint: Option<WebGlUniformLocation>,
    glow: Option<WebGlUniformLocation>,
    fog: Option<WebGlUniformLocation>,
    alpha: Option<WebGlUniformLocation>,
}

struct LightUniforms {
    view_proj: Option<WebGlUniformLocation>,
    a: Option<WebGlUniformLocation>,
    b: Option<WebGlUniformLocation>,
    eye: Option<WebGlUniformLocation>,
    right: Option<WebGlUniformLocation>,
    up: Option<WebGlUniformLocation>,
    size: Option<WebGlUniformLocation>,
    ribbon: Option<WebGlUniformLocation>,
    colour: Option<WebGlUniformLocation>,
}

/// The renderer: the context, its programs and the uploaded meshes.
pub struct Renderer {
    gl: Gl,
    canvas: HtmlCanvasElement,
    solid: WebGlProgram,
    light: WebGlProgram,
    u: Uniforms,
    l: LightUniforms,
    statics: GpuMesh,
    lines: GpuMesh,
    parts: Vec<(Part, GpuMesh)>,
    quad: WebGlVertexArrayObject,
}

/// The background, and the colour distance fades to.
fn fog() -> [f32; 3] {
    scene::field()
}

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

fn program(gl: &Gl, vertex: &str, fragment: &str) -> Result<WebGlProgram, String> {
    let vertex = compile(gl, Gl::VERTEX_SHADER, vertex)?;
    let fragment = compile(gl, Gl::FRAGMENT_SHADER, fragment)?;
    let program = gl.create_program().ok_or("no program")?;
    gl.attach_shader(&program, &vertex);
    gl.attach_shader(&program, &fragment);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default())
    }
}

fn upload(gl: &Gl, data: &[f32], layout: &[(u32, i32)], mode: u32) -> Result<GpuMesh, String> {
    let vao = gl.create_vertex_array().ok_or("no vertex array")?;
    gl.bind_vertex_array(Some(&vao));
    let buffer = gl.create_buffer().ok_or("no buffer")?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
    let array = js_sys::Float32Array::from(data);
    gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &array, Gl::STATIC_DRAW);
    let stride: i32 = layout.iter().map(|(_, n)| n).sum();
    let mut offset = 0;
    for &(location, size) in layout {
        gl.enable_vertex_attrib_array(location);
        gl.vertex_attrib_pointer_with_i32(location, size, Gl::FLOAT, false, stride * 4, offset * 4);
        offset += size;
    }
    gl.bind_vertex_array(None);
    Ok(GpuMesh {
        vao,
        count: (data.len() as i32) / stride,
        mode,
    })
}

fn upload_mesh(gl: &Gl, mesh: &Mesh) -> Result<GpuMesh, String> {
    debug_assert_eq!(STRIDE, 9);
    upload(gl, &mesh.data, &[(0, 3), (1, 3), (2, 3)], Gl::TRIANGLES)
}

fn upload_lines(gl: &Gl, lines: &Lines) -> Result<GpuMesh, String> {
    debug_assert_eq!(LINE_STRIDE, 6);
    // Lines have no normal: the colour goes to location 2, location 1 stays
    // at its default.
    upload(gl, &lines.data, &[(0, 3), (2, 3)], Gl::LINES)
}

impl Renderer {
    /// Opens WebGL2 on `canvas`; `Err` when the browser has none.
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let options = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options, &"antialias".into(), &true.into());
        let _ = js_sys::Reflect::set(&options, &"alpha".into(), &false.into());
        let gl = canvas
            .get_context_with_context_options("webgl2", &options)
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<Gl>().ok())
            .ok_or("no WebGL2")?;
        let solid = program(&gl, SOLID_VERTEX, SOLID_FRAGMENT)?;
        let light = program(&gl, LIGHT_VERTEX, LIGHT_FRAGMENT)?;
        let at = |p: &WebGlProgram, name: &str| gl.get_uniform_location(p, name);
        let u = Uniforms {
            view_proj: at(&solid, "u_view_proj"),
            model: at(&solid, "u_model"),
            tint: at(&solid, "u_tint"),
            glow: at(&solid, "u_glow"),
            fog: at(&solid, "u_fog"),
            alpha: at(&solid, "u_alpha"),
        };
        let l = LightUniforms {
            view_proj: at(&light, "u_view_proj"),
            a: at(&light, "u_a"),
            b: at(&light, "u_b"),
            eye: at(&light, "u_eye"),
            right: at(&light, "u_right"),
            up: at(&light, "u_up"),
            size: at(&light, "u_size"),
            ribbon: at(&light, "u_ribbon"),
            colour: at(&light, "u_colour"),
        };
        let statics = scene::statics();
        let mut parts = Vec::new();
        for part in Part::ALL {
            let mesh = if part.is_lines() {
                upload_lines(&gl, &scene::part_lines(part))?
            } else {
                upload_mesh(&gl, &scene::part_mesh(part))?
            };
            parts.push((part, mesh));
        }
        let corners: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
        ];
        let quad = upload(&gl, &corners, &[(0, 2)], Gl::TRIANGLES)?.vao;
        Ok(Self {
            statics: upload_mesh(&gl, &statics.solid)?,
            lines: upload_lines(&gl, &statics.lines)?,
            parts,
            quad,
            u,
            l,
            solid,
            light,
            gl,
            canvas,
        })
    }

    /// Matches the drawing buffer to the canvas's size on screen, and
    /// returns that size in CSS pixels.
    pub fn fit(&self, pixel_ratio: f64) -> (f64, f64) {
        let (w, h) = (
            f64::from(self.canvas.client_width()),
            f64::from(self.canvas.client_height()),
        );
        let ratio = pixel_ratio.clamp(1.0, 2.0);
        let (pw, ph) = ((w * ratio).round() as u32, (h * ratio).round() as u32);
        if pw > 0 && ph > 0 && (self.canvas.width() != pw || self.canvas.height() != ph) {
            self.canvas.set_width(pw);
            self.canvas.set_height(ph);
        }
        (w, h)
    }

    fn mesh(&self, part: Part) -> &GpuMesh {
        &self
            .parts
            .iter()
            .find(|(p, _)| *p == part)
            .expect("every part")
            .1
    }

    fn draw_mesh(&self, mesh: &GpuMesh) {
        self.gl.bind_vertex_array(Some(&mesh.vao));
        self.gl.draw_arrays(mesh.mode, 0, mesh.count);
    }

    /// Draws one frame.
    pub fn draw(&self, camera: &Camera, frame: &Frame) {
        let gl = &self.gl;
        gl.viewport(
            0,
            0,
            self.canvas.width() as i32,
            self.canvas.height() as i32,
        );
        let bg = fog().map(|c| c.clamp(0.0, 1.0).powf(1.0 / 2.2));
        gl.clear_color(bg[0], bg[1], bg[2], 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        gl.enable(Gl::DEPTH_TEST);
        gl.depth_mask(true);
        gl.disable(Gl::BLEND);
        gl.disable(Gl::CULL_FACE);

        gl.use_program(Some(&self.solid));
        let u = &self.u;
        gl.uniform_matrix4fv_with_f32_array(
            u.view_proj.as_ref(),
            false,
            &camera.view_proj.to_cols_array(),
        );
        gl.uniform3fv_with_f32_array(u.fog.as_ref(), &fog());
        let set = |model: &Mat4, tint: [f32; 3], glow: [f32; 3], alpha: f32| {
            gl.uniform_matrix4fv_with_f32_array(u.model.as_ref(), false, &model.to_cols_array());
            gl.uniform3fv_with_f32_array(u.tint.as_ref(), &tint);
            gl.uniform3fv_with_f32_array(u.glow.as_ref(), &glow);
            gl.uniform1f(u.alpha.as_ref(), alpha);
        };
        // Faces.
        set(&Mat4::IDENTITY, [1.0; 3], [0.0; 3], 1.0);
        self.draw_mesh(&self.statics);
        for d in frame
            .draws
            .iter()
            .filter(|d| d.alpha >= 1.0 && !d.part.is_lines())
        {
            set(&d.model, d.tint, d.glow, 1.0);
            self.draw_mesh(self.mesh(d.part));
        }
        // Lines.
        set(&Mat4::IDENTITY, [1.0; 3], [0.0; 3], 1.0);
        self.draw_mesh(&self.lines);
        for d in frame.draws.iter().filter(|d| d.part.is_lines()) {
            set(&d.model, d.tint, d.glow, 1.0);
            self.draw_mesh(self.mesh(d.part));
        }
        // Glass.
        gl.enable(Gl::BLEND);
        gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        for d in frame
            .draws
            .iter()
            .filter(|d| d.alpha < 1.0 && !d.part.is_lines())
        {
            set(&d.model, d.tint, d.glow, d.alpha);
            self.draw_mesh(self.mesh(d.part));
        }
        // Light: beams then sparks, added.
        gl.blend_func(Gl::ONE, Gl::ONE);
        gl.use_program(Some(&self.light));
        let l = &self.l;
        gl.uniform_matrix4fv_with_f32_array(
            l.view_proj.as_ref(),
            false,
            &camera.view_proj.to_cols_array(),
        );
        gl.uniform3fv_with_f32_array(l.eye.as_ref(), &camera.eye.to_array());
        let forward = (camera.target - camera.eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        gl.uniform3fv_with_f32_array(l.right.as_ref(), &right.to_array());
        gl.uniform3fv_with_f32_array(l.up.as_ref(), &up.to_array());
        gl.bind_vertex_array(Some(&self.quad));
        gl.uniform1f(l.ribbon.as_ref(), 1.0);
        for beam in &frame.beams {
            gl.uniform3fv_with_f32_array(l.a.as_ref(), &beam.from.to_array());
            gl.uniform3fv_with_f32_array(l.b.as_ref(), &beam.to.to_array());
            gl.uniform1f(l.size.as_ref(), beam.width);
            gl.uniform3fv_with_f32_array(l.colour.as_ref(), &beam.colour);
            gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        }
        gl.uniform1f(l.ribbon.as_ref(), 0.0);
        for spark in &frame.sparks {
            gl.uniform3fv_with_f32_array(l.a.as_ref(), &spark.at.to_array());
            gl.uniform1f(l.size.as_ref(), spark.size);
            gl.uniform3fv_with_f32_array(l.colour.as_ref(), &spark.colour);
            gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        }
        gl.depth_mask(true);
        gl.disable(Gl::BLEND);
        gl.bind_vertex_array(None);
    }
}
