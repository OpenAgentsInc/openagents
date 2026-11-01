// ---- generated: apps/openagents-web/gpu/watchtower.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 15>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t9) -> coax_t11 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t9 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t11;
  l_a = l_in_.x;
  l_o = coax_t11();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: vec2f;
  var l_q: vec2f;
  var l_r: f32;
  var l_c: vec3f;
  var l_mid: vec2f;
  var l_f: f32;
  var l_i: i32;
  var l_b: vec4f;
  var l_beat: f32;
  var l_vig: f32;
  var l_band: f32;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_q = coax_p_v2(((l_p.x / coax_at_u(i32(0i))) - 0x1.000000p-1f), ((l_p.y / coax_at_u(i32(1i))) - 0x1.000000p-1f));
  l_r = coax_p_wg_length2(l_q);
  l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-6f, 0x1.0624dep-5f, 0x1.47ae14p-5f), (0x1.000000p0f - coax_p_fmin(l_r, 0x1.000000p0f)));
  l_mid = coax_p_v2((l_p.x - coax_at_u(i32(4i))), (l_p.y - coax_at_u(i32(5i))));
  l_f = exp(((-(coax_p_wg_length2(l_mid) / coax_p_fmax(coax_p_fmax(coax_at_u(i32(0i)), coax_at_u(i32(1i))), 0x1.000000p0f))) * 0x1.99999ap0f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-3f, 0x1.eb851ep-3f), (coax_p_watchtower_grid(l_p) * (0x1.000000p-2f + (0x1.800000p-1f * l_f)))));
  {
    l_i = 0i;
    while ((l_i < 12i))
    {
      l_b = coax_p_wg_at4((12i + (i32(l_i) * 4i)));
      if ((l_b.z > l_b.x))
      {
        if (((((l_p.x >= l_b.x) && (l_p.x <= l_b.z)) && (l_p.y >= l_b.y)) && (l_p.y <= l_b.w)))
        {
          l_c = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_c, 0x1.47ae14p-3f), coax_p_v3(0x1.89374cp-7f, 0x1.cac084p-7f, 0x1.6872b0p-6f));
        }
      }
      l_i = (l_i + 1i);
    }
  }
  if ((coax_at_u(i32(7i)) > 0.0f))
  {
    l_beat = (0x1.333334p-1f + (0x1.99999ap-2f * sin((coax_at_u(i32(2i)) * 0x1.c00000p2f))));
    l_vig = coax_p_smoothstep(0x1.666666p-2f, 0x1.b33334p-1f, l_r);
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.000000p-1f, 0.0f, 0x1.47ae14p-5f), ((l_vig * coax_at_u(i32(7i))) * (0x1.19999ap-1f * l_beat)));
  }
  if ((coax_at_u(i32(8i)) > 0.0f))
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-2f, 0.0f, 0x1.47ae14p-5f), (coax_at_u(i32(8i)) * coax_p_smoothstep(0x1.99999ap-3f, 0x1.ccccccp-1f, l_r))));
  }
  if ((coax_at_u(i32(9i)) > 0.0f))
  {
    l_band = (coax_p_fract((coax_at_u(i32(2i)) * 0x1.666666p-2f)) * coax_at_u(i32(1i)));
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.666666p-2f, 0.0f, 0x1.99999ap-5f), (exp(((-abs((l_p.y - l_band))) / (coax_at_u(i32(1i)) * 0x1.47ae14p-6f))) * 0x1.333334p-1f)));
  }
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.dc28f6p-1f + (0x1.1eb852p-4f * sin((l_p.y * 0x1.b33334p0f)))));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
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
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
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
fn coax_p_watchtower_grid(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_s: f32;
  var l_gu: f32;
  var l_gv: f32;
  var l_x: f32;
  var l_z: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_d: f32;
  var l_line: f32;
  var l_mx: f32;
  var l_mz: f32;
  var l_dm: f32;
  var l_major: f32;
  l_s = coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f);
  l_gu = ((l_p.x - coax_at_u(i32(4i))) / (l_s * 0x1.bb67aep-1f));
  l_gv = ((l_p.y - coax_at_u(i32(5i))) / (l_s * 0x1.000000p-1f));
  l_x = ((l_gu + l_gv) * 0x1.000000p-1f);
  l_z = ((l_gv - l_gu) * 0x1.000000p-1f);
  l_dx = abs((coax_p_fract((l_x + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_dz = abs((coax_p_fract((l_z + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_d = ((coax_p_fmin(l_dx, l_dz) * l_s) * 0x1.3d70a4p-1f);
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.400000p0f, l_d));
  l_mx = abs((coax_p_fract(((l_x * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_mz = abs((coax_p_fract(((l_z * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_dm = ((coax_p_fmin(l_mx, l_mz) * l_s) * 0x1.3d70a4p1f);
  l_major = (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-2f, 0x1.666666p0f, l_dm));
  return ((l_line * 0x1.19999ap-1f) + (l_major * 0x1.ccccccp-2f));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}