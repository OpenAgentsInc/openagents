// ---- generated: apps/openagents-web/gpu/splatlab.coax (the module) by coax wgsl; do not edit
struct coax_t12 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: f32,
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: f32,
}
struct coax_t13 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_13 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_15 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_16 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 80>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> mesh: array<vec4f>;
@group(1) @binding(0) var bloom_src: texture_2d<f32>;
@group(1) @binding(1) var bloom_lin: sampler;
@group(1) @binding(2) var<uniform> bloom_step: array<vec4f, 1>;
fn coax_at_bloom_step(i: i32) -> f32 {
  return bloom_step[i / 4][i % 4];
}
@group(1) @binding(0) var post_light: texture_2d<f32>;
@group(1) @binding(1) var post_quarter: texture_2d<f32>;
@group(1) @binding(2) var post_eighth: texture_2d<f32>;
@group(1) @binding(5) var post_lin: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t12 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t12;
  var l_a: vec4f;
  var l_b: vec4f;
  l_o = coax_t12();
  l_a = mesh[i32((l_id * 2u))];
  l_b = mesh[i32(((l_id * 2u) + 1u))];
  l_o.y = coax_p_v3(l_a.x, l_a.y, l_a.z);
  l_o.x = coax_p_sl_clip(l_o.y);
  l_o.z = coax_p_v3(l_b.x, l_b.y, l_b.z);
  l_o.w = coax_p_sl_unpack(l_a.w);
  l_o.f4 = l_b.w;
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t13) -> coax_t14 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t13 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t14;
  l_a = l_in_.x;
  l_o = coax_t14();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t15 {
  var l_pos: vec4f = p_pos;
  var l_rd: vec3f;
  var l_c: vec3f;
  l_rd = coax_p_sl_ray(coax_p_v2(l_pos.x, l_pos.y));
  l_c = coax_p_sl_sky(l_rd);
  return coax_t15(coax_p_v4(l_c.x, l_c.y, l_c.z, 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_scene_fs(p_s: coax_t12, p_front: bool) -> coax_t16 {
  var l_s: coax_t12 = p_s;
  var l_front: bool = p_front;
  var l_n: vec3f;
  var l_moon: vec3f;
  var l_sky: f32;
  var l_amb: vec3f;
  var l_dif: vec3f;
  var l_fire: vec3f;
  var l_c: vec3f;
  var l_share: f32;
  var l_fl: f32;
  var l_d: f32;
  l_n = coax_p_normalize3(l_s.z);
  if ((!l_front))
  {
    l_n = coax_p_scale3(l_n, (-0x1.000000p0f));
  }
  l_moon = coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i)));
  l_sky = (0x1.000000p-1f + (0x1.000000p-1f * l_n.y));
  l_amb = coax_p_add3(coax_p_scale3(coax_p_v3(0x1.47ae14p-6f, 0x1.99999ap-6f, 0x1.70a3d8p-5f), l_sky), coax_p_scale3(coax_p_v3(0x1.eb851ep-6f, 0x1.89374cp-7f, 0x1.89374cp-8f), ((0x1.000000p0f - l_sky) * coax_at_u(i32(11i)))));
  l_dif = coax_p_scale3(coax_p_v3(0x1.47ae14p-3f, 0x1.70a3d8p-3f, 0x1.eb851ep-3f), (coax_p_fmax(dot(l_n, l_moon), 0.0f) * coax_at_u(i32(35i))));
  l_fire = coax_p_sl_firelight(l_s.y, l_n);
  l_c = coax_p_mul3(l_s.w, coax_p_add3(coax_p_add3(l_amb, l_dif), l_fire));
  l_share = 0.0f;
  if (((l_s.f4 > 0x1.000000p-1f) && (l_s.f4 < 0x1.800000p0f)))
  {
    l_fl = (0x1.b33334p-1f + (0x1.333334p-3f * sin(((coax_at_u(i32(2i)) * 0x1.200000p3f) + (l_s.y.x * 0x1.800000p1f)))));
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.19999ap-1f, 0x1.99999ap-3f), (0x1.666666p0f * l_fl)));
    l_share = 0x1.333334p-1f;
  }
  l_d = coax_p_length3(coax_p_sub3(l_s.y, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.47ae14p-6f, 0x1.eb851ep-7f, 0x1.47ae14p-6f), (0x1.000000p0f - exp(((-l_d) * 0x1.89374cp-7f))));
  return coax_t16(coax_p_v4(l_c.x, l_c.y, l_c.z, l_share), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_dims: vec2f;
  var l_texel: vec2f;
  var l_uv: vec2f;
  var l_c: vec3f;
  var l_k: i32;
  var l_ox: f32;
  var l_oy: f32;
  var l_t: vec4f;
  var l_d: vec2f;
  var l_m: vec4f;
  var l_c_14: vec3f;
  var l_a1: vec4f;
  var l_b1: vec4f;
  var l_a2: vec4f;
  var l_b2: vec4f;
  l_dims = coax_p_v2(f32(textureDimensions(bloom_src).x), f32(textureDimensions(bloom_src).y));
  l_texel = coax_p_v2((0x1.000000p0f / l_dims.x), (0x1.000000p0f / l_dims.y));
  l_uv = coax_p_v2(((l_pos.x * coax_at_bloom_step(i32(3i))) * l_texel.x), ((l_pos.y * coax_at_bloom_step(i32(3i))) * l_texel.y));
  if ((coax_at_bloom_step(i32(2i)) < 0x1.000000p-1f))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 4i;
      for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
      {
        l_ox = ((f32((l_k % 2i)) * 0x1.000000p1f) - 0x1.000000p0f);
        l_oy = ((f32((l_k / 2i)) * 0x1.000000p1f) - 0x1.000000p0f);
        l_t = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_ox * l_texel.x)), (l_uv.y + (l_oy * l_texel.y))), 0.0f);
        l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(l_t.x, l_t.y, l_t.z), coax_p_clampf(l_t.w, 0.0f, 0x1.000000p0f)));
      }
    }
    return coax_p_v4((l_c.x * 0x1.000000p-2f), (l_c.y * 0x1.000000p-2f), (l_c.z * 0x1.000000p-2f), 0x1.000000p0f);
  }
  l_d = coax_p_v2((coax_at_bloom_step(i32(0i)) * l_texel.x), (coax_at_bloom_step(i32(1i)) * l_texel.y));
  l_m = textureSampleLevel(bloom_src, bloom_lin, l_uv, 0.0f);
  l_c_14 = coax_p_scale3(coax_p_v3(l_m.x, l_m.y, l_m.z), 0x1.d0e560p-3f);
  l_a1 = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.628f5cp0f)), (l_uv.y + (l_d.y * 0x1.628f5cp0f))), 0.0f);
  l_b1 = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.628f5cp0f)), (l_uv.y - (l_d.y * 0x1.628f5cp0f))), 0.0f);
  l_a2 = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.9d9168p1f)), (l_uv.y + (l_d.y * 0x1.9d9168p1f))), 0.0f);
  l_b2 = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.9d9168p1f)), (l_uv.y - (l_d.y * 0x1.9d9168p1f))), 0.0f);
  l_c_14 = coax_p_add3(l_c_14, coax_p_scale3(coax_p_add3(coax_p_v3(l_a1.x, l_a1.y, l_a1.z), coax_p_v3(l_b1.x, l_b1.y, l_b1.z)), 0x1.439582p-2f));
  l_c_14 = coax_p_add3(l_c_14, coax_p_scale3(coax_p_add3(coax_p_v3(l_a2.x, l_a2.y, l_a2.z), coax_p_v3(l_b2.x, l_b2.y, l_b2.z)), 0x1.1eb852p-4f));
  return coax_p_v4(l_c_14.x, l_c_14.y, l_c_14.z, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_l: vec4f;
  var l_uv: vec2f;
  var l_q: vec4f;
  var l_e: vec4f;
  var l_b: vec3f;
  var l_k: f32;
  var l_c: vec3f;
  l_l = textureLoad(post_light, vec2i(i32(l_pos.x), i32(l_pos.y)), 0i);
  l_uv = coax_p_v2((l_pos.x / coax_p_fmax(coax_at_u(i32(0i)), 0x1.000000p0f)), (l_pos.y / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)));
  l_q = textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f);
  l_e = textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f);
  l_b = coax_p_add3(coax_p_scale3(coax_p_v3(l_q.x, l_q.y, l_q.z), 0x1.666666p-1f), coax_p_scale3(coax_p_v3(l_e.x, l_e.y, l_e.z), 0x1.ccccccp-1f));
  l_k = coax_at_u(i32(8i));
  l_c = coax_p_v3(coax_p_sl_srgb(coax_p_sl_aces(((l_l.x + l_b.x) * l_k))), coax_p_sl_srgb(coax_p_sl_aces(((l_l.y + l_b.y) * l_k))), coax_p_sl_srgb(coax_p_sl_aces(((l_l.z + l_b.z) * l_k))));
  if (((l_pos.y < coax_at_u(i32(36i))) || (l_pos.y > (coax_at_u(i32(1i)) - coax_at_u(i32(37i))))))
  {
    l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.000000p-2f), coax_p_v3(0x1.0624dep-7f, 0x1.47ae14p-7f, 0x1.cac084p-7f));
  }
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t14) -> vec4f {
  var l_v: coax_t14 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  var l_k: f32;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  l_k = (((l_core + l_halo) * l_v.z.z) * coax_at_u(i32(3i)));
  return coax_p_v4((l_v.w.x * l_k), (l_v.w.y * l_k), (l_v.w.z * l_k), 0x1.000000p0f);
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
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_sl_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_rel: vec3f;
  var l_z: f32;
  var l_r: f32;
  var l_q: f32;
  var l_wd: f32;
  var l_ht: f32;
  var l_f: f32;
  var l_x: f32;
  var l_y: f32;
  l_rel = coax_p_v3((l_w.x - coax_at_u(i32(16i))), (l_w.y - coax_at_u(i32(17i))), (l_w.z - coax_at_u(i32(18i))));
  l_z = (((l_rel.x * coax_at_u(i32(20i))) + (l_rel.y * coax_at_u(i32(21i)))) + (l_rel.z * coax_at_u(i32(22i))));
  l_r = (((l_rel.x * coax_at_u(i32(24i))) + (l_rel.y * coax_at_u(i32(25i)))) + (l_rel.z * coax_at_u(i32(26i))));
  l_q = (((l_rel.x * coax_at_u(i32(28i))) + (l_rel.y * coax_at_u(i32(29i)))) + (l_rel.z * coax_at_u(i32(30i))));
  l_wd = coax_p_fmax(coax_at_u(i32(0i)), 0x1.000000p0f);
  l_ht = coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f);
  l_f = coax_at_u(i32(19i));
  l_x = (((((0x1.000000p1f * coax_at_u(i32(23i))) / l_wd) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) / l_wd) * l_r));
  l_y = (((0x1.000000p0f - ((0x1.000000p1f * coax_at_u(i32(27i))) / l_ht)) * l_z) + (((0x1.000000p1f * l_f) / l_ht) * l_q));
  return coax_p_v4(l_x, l_y, (l_z - coax_at_u(i32(9i))), l_z);
}
fn coax_p_sl_unpack(p_p: f32) -> vec3f {
  var l_p: f32 = p_p;
  var l_bb: f32;
  var l_gg: f32;
  var l_rr: f32;
  l_bb = floor((l_p / 0x1.000000p16f));
  l_gg = floor(((l_p - (l_bb * 0x1.000000p16f)) / 0x1.000000p8f));
  l_rr = ((l_p - (l_bb * 0x1.000000p16f)) - (l_gg * 0x1.000000p8f));
  return coax_p_v3(coax_pow_bg_vs((l_rr / 0x1.fe0000p7f), 0x1.19999ap1f), coax_pow_bg_vs((l_gg / 0x1.fe0000p7f), 0x1.19999ap1f), coax_pow_bg_vs((l_bb / 0x1.fe0000p7f), 0x1.19999ap1f));
}
fn coax_p_sl_ray(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_f: f32;
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  return coax_p_normalize3(coax_p_add3(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), coax_p_add3(coax_p_scale3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), ((l_p.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), ((coax_at_u(i32(27i)) - l_p.y) / l_f)))));
}
fn coax_p_sl_sky(p_rd: vec3f) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_y: f32;
  var l_top: vec3f;
  var l_low: vec3f;
  var l_c: vec3f;
  var l_glow: f32;
  var l_moon: vec3f;
  var l_m: f32;
  l_y = l_rd.y;
  l_top = coax_p_v3(0x1.89374cp-8f, 0x1.26e978p-7f, 0x1.99999ap-6f);
  l_low = coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-6f, 0x1.47ae14p-5f);
  l_c = coax_p_mix3(l_low, l_top, coax_p_smoothstep((-0x1.99999ap-5f), 0x1.19999ap-1f, l_y));
  l_glow = (exp(((-coax_p_fmax(l_y, 0.0f)) * 0x1.c00000p2f)) * coax_at_u(i32(11i)));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.70a3d8p-4f, 0x1.1eb852p-5f, 0x1.89374cp-7f), l_glow));
  l_moon = coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i)));
  l_m = dot(l_rd, l_moon);
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-1f, 0x1.666666p-1f), ((coax_p_smoothstep(0x1.ffa440p-1f, 0x1.ffcb92p-1f, l_m) * coax_at_u(i32(35i))) * 0x1.800000p1f)));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.70a3d8p-4f), (coax_pow_bg_vs(coax_p_fmax(l_m, 0.0f), 0x1.e00000p5f) * coax_at_u(i32(35i)))));
  if ((l_y < 0.0f))
  {
    l_c = coax_p_mix3(l_c, coax_p_v3(0x1.47ae14p-7f, 0x1.0624dep-7f, 0x1.0624dep-7f), coax_p_smoothstep(0.0f, (-0x1.eb851ep-4f), l_y));
  }
  return l_c;
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
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
fn coax_p_sl_firelight(p_w: vec3f, p_n: vec3f) -> vec3f {
  var l_w: vec3f = p_w;
  var l_n: vec3f = p_n;
  var l_c: vec3f;
  var l_t: f32;
  var l_e: i32;
  var l_o: i32;
  var l_kind: f32;
  var l_size: f32;
  var l_at: vec3f;
  var l_d: vec3f;
  var l_r2: f32;
  var l_l: vec3f;
  var l_ph: f32;
  var l_flick: f32;
  var l_k: f32;
  var l_hue: vec3f;
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_t = coax_at_u(i32(2i));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 16i;
    for (l_e = coax_from; l_e < coax_to; l_e = l_e + 1i)
    {
      l_o = (64i + (l_e * 12i));
      l_kind = coax_at_u(i32((l_o + 3i)));
      if (((l_kind == 0x1.000000p0f) || (l_kind == 0x1.800000p2f)))
      {
        l_size = coax_at_u(i32((l_o + 6i)));
        l_at = coax_p_v3(coax_at_u(i32(l_o)), (coax_at_u(i32((l_o + 1i))) + (coax_at_u(i32((l_o + 11i))) * 0x1.333334p-2f)), coax_at_u(i32((l_o + 2i))));
        l_d = coax_p_sub3(l_at, l_w);
        l_r2 = dot(l_d, l_d);
        l_l = coax_p_scale3(l_d, (0x1.000000p0f / sqrt(coax_p_fmax(l_r2, 0x1.a36e2ep-14f))));
        l_ph = (coax_at_u(i32((l_o + 7i))) * 0x1.7ae148p-2f);
        l_flick = ((0x1.a3d70ap-1f + (0x1.99999ap-4f * sin(((l_t * 0x1.600000p3f) + l_ph)))) + (0x1.47ae14p-4f * sin(((l_t * 0x1.14ccccp4f) + (l_ph * 0x1.000000p1f)))));
        l_k = ((coax_p_fmax(dot(l_n, l_l), 0.0f) * 0x1.b33334p-1f) + 0x1.333334p-3f);
        l_hue = select(coax_p_v3(0x1.ccccccp-2f, 0x1.333334p-2f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.ae147ap-2f, 0x1.eb851ep-4f), (l_kind == 0x1.000000p0f));
        l_c = coax_p_add3(l_c, coax_p_scale3(l_hue, ((((((l_k * l_flick) * l_size) * l_size) * 0x1.800000p2f) * coax_at_u(i32(11i))) / (0x1.000000p0f + l_r2))));
      }
    }
  }
  return l_c;
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_sl_aces(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(((l_x * ((0x1.4147aep1f * l_x) + 0x1.eb851ep-6f)) / ((l_x * ((0x1.370a3ep1f * l_x) + 0x1.2e147ap-1f)) + 0x1.1eb852p-3f)), 0.0f, 0x1.000000p0f);
}
fn coax_p_sl_srgb(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x <= 0x1.9a5c38p-9f))
  {
    return (l_x * 0x1.9d70a4p3f);
  }
  return ((0x1.0e147ap0f * coax_pow_bg_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f);
}
fn coax_p_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_13) -> coax_io_14 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_15 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_15(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_12, @builtin(front_facing) coax_front: bool) -> coax_out_16 {
  let coax_r = coax_p_scene_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4), coax_front);
  return coax_out_16(coax_r.x, coax_r.y);
}
@fragment
fn bloom_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bloom_fs(coax_pos);
  return coax_r;
}
@fragment
fn post_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_post_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_14) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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