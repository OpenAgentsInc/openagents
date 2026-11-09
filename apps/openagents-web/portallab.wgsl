// ---- generated: apps/openagents-web/gpu/portallab.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: f32,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: f32,
}
struct coax_t12 {
  x: vec4f,
}
struct coax_in_12 {
  @location(0) x: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 16>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> mesh: array<vec4f>;
@group(1) @binding(1) var baked: texture_2d_array<f32>;
@group(1) @binding(2) var bake: sampler;
fn coax_p_bg_vs(p_i: u32, p_j: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l_j: u32 = p_j;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_i: u32, p_j: u32) -> coax_t11 {
  var l_i: u32 = p_i;
  var l_j: u32 = p_j;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_y: f32;
  var l_fly: bool;
  var l_view: vec4f;
  var l_clip: vec4f;
  var l_uv: vec2f;
  var l_px: f32;
  l_a = mesh[(i32(l_i) * 2i)];
  l_b = mesh[((i32(l_i) * 2i) + 1i)];
  l_y = l_a.y;
  l_fly = (l_a.w > 0x1.400000p1f);
  if (l_fly)
  {
    l_y = (l_y + ((sin((coax_at_u(i32(2i)) + (l_a.x * 0x1.900000p6f))) * l_b.z) * 0x1.99999ap-3f));
  }
  l_view = coax_p_m4_apply(coax_p_m4_at(16i), coax_p_v4(l_a.x, l_y, l_a.z, 0x1.000000p0f));
  l_clip = coax_p_m4_apply(coax_p_m4_at(32i), l_view);
  l_uv = coax_p_v2(l_b.x, l_b.y);
  if (l_fly)
  {
    l_px = ((coax_at_u(i32(7i)) * l_b.z) * coax_at_u(i32(3i)));
    l_clip = coax_p_v4((l_clip.x + ((l_b.x * l_px) / coax_at_u(i32(0i)))), (l_clip.y + ((l_b.y * l_px) / coax_at_u(i32(1i)))), l_clip.z, l_clip.w);
    l_uv = coax_p_v2(((l_b.x * 0x1.000000p-1f) + 0x1.000000p-1f), ((l_b.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  }
  return coax_t11(l_clip, l_uv, l_a.w);
}
fn coax_p_fx_vs(p_i: u32, p_j: u32, p_a: coax_t12) -> vec4f {
  var l_i: u32 = p_i;
  var l_j: u32 = p_j;
  var l_a: coax_t12 = p_a;
  return coax_p_v4(0x1.000000p1f, 0x1.000000p1f, 0.0f, 0x1.000000p0f);
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_v4(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i)), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t11) -> vec4f {
  var l_s: coax_t11 = p_s;
  var l_texel: vec4f;
  var l_k: f32;
  l_texel = textureSample(baked, bake, l_s.y, 0i);
  if ((l_s.z < 0x1.000000p-1f))
  {
    return coax_p_v4(coax_p_portal_oetf(l_texel.x), coax_p_portal_oetf(l_texel.y), coax_p_portal_oetf(l_texel.z), 0x1.000000p0f);
  }
  if ((l_s.z < 0x1.800000p0f))
  {
    return coax_p_v4(0x1.000000p0f, 0x1.000000p0f, (0x1.ca0000p7f / 0x1.fe0000p7f), 0x1.000000p0f);
  }
  if ((l_s.z < 0x1.400000p1f))
  {
    l_k = coax_p_portal_strength(l_s.y, coax_at_u(i32(2i)));
    return coax_p_v4(coax_p_portal_oetf(coax_p_mix(coax_at_u(i32(4i)), coax_at_u(i32(8i)), l_k)), coax_p_portal_oetf(coax_p_mix(coax_at_u(i32(5i)), coax_at_u(i32(9i)), l_k)), coax_p_portal_oetf(coax_p_mix(coax_at_u(i32(6i)), coax_at_u(i32(10i)), l_k)), 0x1.000000p0f);
  }
  return coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, coax_p_portal_glow(l_s.y));
}
fn coax_p_fx_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
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
fn coax_p_m4_at(p_at: i32) -> coax_t15 {
  var l_at: i32 = p_at;
  return coax_t15(coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i)))), coax_p_v4(coax_at_u(i32((l_at + 4i))), coax_at_u(i32((l_at + 5i))), coax_at_u(i32((l_at + 6i))), coax_at_u(i32((l_at + 7i)))), coax_p_v4(coax_at_u(i32((l_at + 8i))), coax_at_u(i32((l_at + 9i))), coax_at_u(i32((l_at + 10i))), coax_at_u(i32((l_at + 11i)))), coax_p_v4(coax_at_u(i32((l_at + 12i))), coax_at_u(i32((l_at + 13i))), coax_at_u(i32((l_at + 14i))), coax_at_u(i32((l_at + 15i)))));
}
fn coax_p_m4_apply(p_m: coax_t15, p_v: vec4f) -> vec4f {
  var l_m: coax_t15 = p_m;
  var l_v: vec4f = p_v;
  return coax_p_add4(coax_p_add4(coax_p_scale4(l_m.x, l_v.x), coax_p_scale4(l_m.y, l_v.y)), coax_p_add4(coax_p_scale4(l_m.z, l_v.z), coax_p_scale4(l_m.w, l_v.w)));
}
fn coax_p_portal_oetf(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x <= 0x1.9a5c38p-9f))
  {
    return coax_p_saturate((l_x * 0x1.9d70a4p3f));
  }
  return coax_p_saturate(((0x1.0e147ap0f * coax_pow_bg_vs(l_x, 0x1.aaa8ecp-2f)) - 0x1.c28f5cp-5f));
}
fn coax_p_portal_strength(p_uv: vec2f, p_t: f32) -> f32 {
  var l_uv: vec2f = p_uv;
  var l_t: f32 = p_t;
  var l_shift: f32;
  var l_strength: f32;
  var l_dx: f32;
  var l_dy: f32;
  l_shift = coax_p_portal_cnoise(coax_p_v3((l_uv.x * 0x1.400000p2f), (l_uv.y * 0x1.400000p2f), (l_t * 0x1.99999ap-4f)));
  l_strength = coax_p_portal_cnoise(coax_p_v3(((l_uv.x + l_shift) * 0x1.400000p2f), ((l_uv.y + l_shift) * 0x1.400000p2f), (l_t * 0x1.99999ap-3f)));
  l_dx = (l_uv.x - 0x1.000000p-1f);
  l_dy = (l_uv.y - 0x1.000000p-1f);
  l_strength = ((l_strength + (sqrt(((l_dx * l_dx) + (l_dy * l_dy))) * 0x1.400000p2f)) - 0x1.666666p0f);
  return (l_strength + (coax_p_stepf((-0x1.99999ap-3f), l_strength) * 0x1.99999ap-1f));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_portal_glow(p_uv: vec2f) -> f32 {
  var l_uv: vec2f = p_uv;
  var l_dx: f32;
  var l_dy: f32;
  l_dx = (l_uv.x - 0x1.000000p-1f);
  l_dy = (l_uv.y - 0x1.000000p-1f);
  return coax_p_saturate(((0x1.99999ap-5f / coax_p_fmax(sqrt(((l_dx * l_dx) + (l_dy * l_dy))), 0x1.a36e2ep-14f)) - 0x1.99999ap-4f));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_portal_cnoise(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_x0: f32;
  var l_y0: f32;
  var l_z0: f32;
  var l_x1: f32;
  var l_y1: f32;
  var l_z1: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_n000: f32;
  var l_n100: f32;
  var l_n010: f32;
  var l_n110: f32;
  var l_n001: f32;
  var l_n101: f32;
  var l_n011: f32;
  var l_n111: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_fz: f32;
  var l_z00: f32;
  var l_z10: f32;
  var l_z01: f32;
  var l_z11: f32;
  l_x0 = coax_p_portal_mod(floor(l_p.x), 0x1.210000p8f);
  l_y0 = coax_p_portal_mod(floor(l_p.y), 0x1.210000p8f);
  l_z0 = coax_p_portal_mod(floor(l_p.z), 0x1.210000p8f);
  l_x1 = coax_p_portal_mod((floor(l_p.x) + 0x1.000000p0f), 0x1.210000p8f);
  l_y1 = coax_p_portal_mod((floor(l_p.y) + 0x1.000000p0f), 0x1.210000p8f);
  l_z1 = coax_p_portal_mod((floor(l_p.z) + 0x1.000000p0f), 0x1.210000p8f);
  l_a = coax_p_v3(coax_p_fract(l_p.x), coax_p_fract(l_p.y), coax_p_fract(l_p.z));
  l_b = coax_p_v3((l_a.x - 0x1.000000p0f), (l_a.y - 0x1.000000p0f), (l_a.z - 0x1.000000p0f));
  l_n000 = coax_p_portal_corner(l_x0, l_y0, l_z0, l_a);
  l_n100 = coax_p_portal_corner(l_x1, l_y0, l_z0, coax_p_v3(l_b.x, l_a.y, l_a.z));
  l_n010 = coax_p_portal_corner(l_x0, l_y1, l_z0, coax_p_v3(l_a.x, l_b.y, l_a.z));
  l_n110 = coax_p_portal_corner(l_x1, l_y1, l_z0, coax_p_v3(l_b.x, l_b.y, l_a.z));
  l_n001 = coax_p_portal_corner(l_x0, l_y0, l_z1, coax_p_v3(l_a.x, l_a.y, l_b.z));
  l_n101 = coax_p_portal_corner(l_x1, l_y0, l_z1, coax_p_v3(l_b.x, l_a.y, l_b.z));
  l_n011 = coax_p_portal_corner(l_x0, l_y1, l_z1, coax_p_v3(l_a.x, l_b.y, l_b.z));
  l_n111 = coax_p_portal_corner(l_x1, l_y1, l_z1, l_b);
  l_fx = coax_p_portal_fade(l_a.x);
  l_fy = coax_p_portal_fade(l_a.y);
  l_fz = coax_p_portal_fade(l_a.z);
  l_z00 = coax_p_mix(l_n000, l_n001, l_fz);
  l_z10 = coax_p_mix(l_n100, l_n101, l_fz);
  l_z01 = coax_p_mix(l_n010, l_n011, l_fz);
  l_z11 = coax_p_mix(l_n110, l_n111, l_fz);
  return (0x1.19999ap1f * coax_p_mix(coax_p_mix(l_z00, l_z01, l_fy), coax_p_mix(l_z10, l_z11, l_fy), l_fx));
}
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  if ((l_x < l_edge))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_portal_mod(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return (l_x - (l_y * floor((l_x / l_y))));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_portal_corner(p_x: f32, p_y: f32, p_z: f32, p_f: vec3f) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_f: vec3f = p_f;
  return dot(coax_p_portal_grad(coax_p_portal_hash(l_x, l_y, l_z)), l_f);
}
fn coax_p_portal_fade(p_t: f32) -> f32 {
  var l_t: f32 = p_t;
  return (((l_t * l_t) * l_t) * ((l_t * ((l_t * 0x1.800000p2f) - 0x1.e00000p3f)) + 0x1.400000p3f));
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_portal_hash(p_x: f32, p_y: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return coax_p_portal_permute((coax_p_portal_permute((coax_p_portal_permute(l_x) + l_y)) + l_z));
}
fn coax_p_portal_grad(p_h: f32) -> vec3f {
  var l_h: f32 = p_h;
  var l_gx: f32;
  var l_gy: f32;
  var l_gz: f32;
  var l_sz: f32;
  var l_g: vec3f;
  l_gx = (l_h / 0x1.c00000p2f);
  l_gy = (coax_p_fract((floor(l_gx) / 0x1.c00000p2f)) - 0x1.000000p-1f);
  l_gx = coax_p_fract(l_gx);
  l_gz = ((0x1.000000p-1f - abs(l_gx)) - abs(l_gy));
  l_sz = coax_p_stepf(l_gz, 0.0f);
  l_gx = (l_gx - (l_sz * (coax_p_stepf(0.0f, l_gx) - 0x1.000000p-1f)));
  l_gy = (l_gy - (l_sz * (coax_p_stepf(0.0f, l_gy) - 0x1.000000p-1f)));
  l_g = coax_p_v3(l_gx, l_gy, l_gz);
  return coax_p_scale3(l_g, (0x1.caf7c0p0f - (0x1.b51cb8p-1f * dot(l_g, l_g))));
}
fn coax_p_portal_permute(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_portal_mod((((l_x * 0x1.100000p5f) + 0x1.000000p0f) * l_x), 0x1.210000p8f);
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_12) -> @builtin(position) vec4f {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t12(coax_in.x));
  return coax_r;
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z));
  return coax_r;
}
@fragment
fn fx_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_pos);
  return coax_r;
}
fn coax_bits_bg_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_bg_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_bg_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_bg_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}