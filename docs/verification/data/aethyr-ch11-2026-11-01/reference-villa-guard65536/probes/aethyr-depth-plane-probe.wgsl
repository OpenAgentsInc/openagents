struct coax_t8 {
  x: vec4f,
  y: f32,
}
struct coax_io_8 {
  @builtin(position) x: vec4f,
  @location(0) y: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 1>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
fn coax_p_render_depth_plane_probe_vs(p_vertex: u32, p_instance: u32) -> coax_t8 {
  var l_vertex: u32 = p_vertex;
  var l_instance: u32 = p_instance;
  var l_dpp_x: f32;
  var l_dpp_y: f32;
  var l_dpp_z: f32;
  var l_dpp_w: f32;
  var l_dpp_value: f32;
  l_dpp_x = select(select(0x1.6c0000p3f, 0x1.ba0000p5f, (l_vertex == 1u)), (0x1.000000p2f + coax_at_u(i32(0i))), (l_vertex == 0u));
  l_dpp_y = select(select(0x1.ce0000p5f, 0x1.040000p3f, (l_vertex == 1u)), 0x1.000000p2f, (l_vertex == 0u));
  l_dpp_z = select(select(0x1.800000p-2f, 0x1.c00000p-1f, (l_vertex == 1u)), 0x1.000000p-3f, (l_vertex == 0u));
  l_dpp_w = select(select(select(0x1.000000p-1f, 0x1.000000p1f, (l_vertex == 1u)), 0x1.000000p0f, (l_vertex == 0u)), 0x1.000000p0f, (coax_at_u(i32(1i)) == 0.0f));
  l_dpp_value = select(select(0x1.000000p-1f, 0x1.800000p-1f, (l_vertex == 1u)), 0x1.000000p-2f, (l_vertex == 0u));
  return coax_t8(coax_p_v4((((l_dpp_x / 0x1.000000p5f) - 0x1.000000p0f) * l_dpp_w), ((0x1.000000p0f - (l_dpp_y / 0x1.000000p5f)) * l_dpp_w), (l_dpp_z * l_dpp_w), l_dpp_w), l_dpp_value);
}
fn coax_p_render_depth_plane_probe_fs(p_v: coax_t8) -> vec4f {
  var l_v: coax_t8 = p_v;
  return coax_p_v4(l_v.x.z, l_v.y, l_v.x.w, 0x1.000000p0f);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
@vertex
fn render_depth_plane_probe_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_8 {
  let coax_r = coax_p_render_depth_plane_probe_vs(coax_vid, coax_iid);
  return coax_io_8(coax_r.x, coax_r.y);
}
@fragment
fn render_depth_plane_probe_fs(coax_in: coax_io_8) -> @location(0) vec4f {
  let coax_r = coax_p_render_depth_plane_probe_fs(coax_t8(coax_in.x, coax_in.y));
  return coax_r;
}
