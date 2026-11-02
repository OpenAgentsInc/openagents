@group(0) @binding(0) var<uniform> u: array<vec4f, 1>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var filter_image: texture_2d<f32>;
@group(0) @binding(2) var filter_sampler: sampler;
fn coax_p_render_filter_format_vs(p_vertex: u32, p_instance: u32) -> vec4f {
  var l_vertex: u32 = p_vertex;
  var l_instance: u32 = p_instance;
  var l_ff_points: array<vec2f, 3>;
  var l_ff_p: vec2f;
  l_ff_points = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)));
  l_ff_p = l_ff_points[i32(l_vertex)];
  return coax_p_v4(l_ff_p.x, l_ff_p.y, 0x1.000000p-1f, 0x1.000000p0f);
}
fn coax_p_render_filter_format_fs(p_position: vec4f) -> vec4f {
  var l_position: vec4f = p_position;
  return textureSampleLevel(filter_image, filter_sampler, coax_p_filter_format_uv(coax_at_u(i32(0i)), coax_at_u(i32(1i))), coax_at_u(i32(2i)));
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_filter_format_uv(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_v2(((0x1.000000p-1f + l_x) * 0x1.000000p-1f), ((0x1.000000p-1f + l_y) * 0x1.000000p-1f));
}
@vertex
fn render_filter_format_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_render_filter_format_vs(coax_vid, coax_iid);
  return coax_r;
}
@fragment
fn render_filter_format_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_render_filter_format_fs(coax_pos);
  return coax_r;
}
