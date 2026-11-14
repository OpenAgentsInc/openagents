// ---- generated: apps/openagents-web/gpu/cull-draw.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
}
struct coax_io_10 {
  @builtin(position) x: vec4f,
  @location(0) y: vec4f,
}
struct coax_t12 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
@group(0) @binding(0) var<uniform> du: array<vec4f, 5>;
fn coax_at_du(i: i32) -> f32 {
  return du[i / 4][i % 4];
}
@group(0) @binding(1) var<storage, read> lists: array<u32>;
@group(0) @binding(2) var<storage, read> info: array<u32>;
@group(0) @binding(3) var<storage, read> tris: array<u32>;
@group(0) @binding(4) var<storage, read> verts: array<vec4f>;
fn coax_p_cluster_vs(p_vid: u32, p_iid: u32) -> coax_t10 {
  let l_vid: u32 = p_vid;
  let l_iid: u32 = p_iid;
  var l_c: i32;
  var l_t: i32;
  var l_corner: i32;
  var l_at: i32;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_e: vec4f;
  var l_p: vec4f;
  var l_n: vec3f;
  var l_l: f32;
  var l_light: f32;
  var l_level: f32;
  var l_tint: vec4f;
  l_c = i32(lists[(i32(coax_at_du(i32(16i))) + i32(l_iid))]);
  l_t = i32((l_vid / 3u));
  l_corner = i32((l_vid % 3u));
  if ((l_t >= i32(info[((l_c * 8i) + 3i)])))
  {
    return coax_t10(coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f));
  }
  l_at = ((i32(info[((l_c * 8i) + 2i)]) + l_t) * 3i);
  l_a = verts[i32(tris[l_at])];
  l_b = verts[i32(tris[(l_at + 1i)])];
  l_e = verts[i32(tris[(l_at + 2i)])];
  l_p = verts[i32(tris[(l_at + l_corner)])];
  l_n = coax_p_cross3(coax_p_sub3(coax_p_v3_of(l_b), coax_p_v3_of(l_a)), coax_p_sub3(coax_p_v3_of(l_e), coax_p_v3_of(l_a)));
  l_l = coax_p_length3(l_n);
  l_light = 0x1.666666p-2f;
  if ((l_l > 0.0f))
  {
    l_light = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_fmin(0x1.000000p0f, (coax_p_fabs(dot(l_n, coax_p_v3(0x1.7ae148p-2f, 0x1.99999ap-1f, 0x1.e147aep-2f))) / l_l))));
  }
  l_level = f32(info[((l_c * 8i) + 6i)]);
  l_tint = coax_p_v4((0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_fract((l_level * 0x1.7ae148p-2f)))), (0x1.000000p-1f + (0x1.99999ap-2f * coax_p_fract(((l_level * 0x1.3851ecp-1f) + 0x1.333334p-2f)))), (0x1.ccccccp-2f + (0x1.000000p-1f * coax_p_fract(((l_level * 0x1.d70a3ep-3f) + 0x1.333334p-1f)))), 0x1.000000p0f);
  return coax_t10(coax_p_m4_apply(coax_p_m4_at(0i), l_p), coax_p_v4((l_tint.x * l_light), (l_tint.y * l_light), (l_tint.z * l_light), 0x1.000000p0f));
}
fn coax_p_cluster_fs(p_v: coax_t10) -> vec4f {
  let l_v: coax_t10 = p_v;
  return l_v.y;
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_v3_of(p_v: vec4f) -> vec3f {
  let l_v: vec4f = p_v;
  return vec3f(l_v.x, l_v.y, l_v.z);
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_fabs(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-l_x);
  }
  return l_x;
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_m4_at(p_at: i32) -> coax_t12 {
  let l_at: i32 = p_at;
  return coax_t12(coax_p_v4(coax_at_du(i32(l_at)), coax_at_du(i32((l_at + 1i))), coax_at_du(i32((l_at + 2i))), coax_at_du(i32((l_at + 3i)))), coax_p_v4(coax_at_du(i32((l_at + 4i))), coax_at_du(i32((l_at + 5i))), coax_at_du(i32((l_at + 6i))), coax_at_du(i32((l_at + 7i)))), coax_p_v4(coax_at_du(i32((l_at + 8i))), coax_at_du(i32((l_at + 9i))), coax_at_du(i32((l_at + 10i))), coax_at_du(i32((l_at + 11i)))), coax_p_v4(coax_at_du(i32((l_at + 12i))), coax_at_du(i32((l_at + 13i))), coax_at_du(i32((l_at + 14i))), coax_at_du(i32((l_at + 15i)))));
}
fn coax_p_m4_apply(p_m: coax_t12, p_v: vec4f) -> vec4f {
  let l_m: coax_t12 = p_m;
  let l_v: vec4f = p_v;
  return coax_p_add4(coax_p_add4(coax_p_scale4(l_m.x, l_v.x), coax_p_scale4(l_m.y, l_v.y)), coax_p_add4(coax_p_scale4(l_m.z, l_v.z), coax_p_scale4(l_m.w, l_v.w)));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  let l_a: vec4f = p_a;
  let l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
@vertex
fn cluster_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_10 {
  let coax_r = coax_p_cluster_vs(coax_vid, coax_iid);
  return coax_io_10(coax_r.x, coax_r.y);
}
@fragment
fn cluster_fs(coax_in: coax_io_10) -> @location(0) vec4f {
  let coax_r = coax_p_cluster_fs(coax_t10(coax_in.x, coax_in.y));
  return coax_r;
}