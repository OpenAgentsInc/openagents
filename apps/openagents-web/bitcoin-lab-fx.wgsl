// ---- generated: apps/openagents-web/gpu/bitcoin-lab-fx.coax (the module) by coax wgsl; do not edit
struct coax_t8 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t12 {
  x: vec4f,
  y: vec2f,
  z: vec3f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec3f,
  @location(2) @interpolate(flat) w: vec4f,
  @location(3) @interpolate(flat) f4: vec4f,
  @location(4) @interpolate(flat) f5: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 8>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> puffs: array<coax_t8>;
fn coax_p_particle_vs(p_v: u32, p_n: u32) -> coax_t12 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  return coax_p_qfx_gone();
}
fn coax_p_particle_over_vs(p_v: u32, p_n: u32) -> coax_t12 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  return coax_p_qfx_place(l_v, l_n);
}
fn coax_p_particle_fs(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
}
fn coax_p_particle_over_fs(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  var l_kind: f32;
  var l_f: f32;
  var l_rd: vec3f;
  var l_dist: f32;
  var l_fade: f32;
  var l_across: f32;
  var l_ends: f32;
  var l_r2: f32;
  var l_glow: f32;
  l_kind = l_p.f4.x;
  if ((l_kind == 0.0f))
  {
    l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
    l_rd = coax_p_normalize3(coax_p_add3(coax_p_wg_at3(20i), coax_p_add3(coax_p_scale3(coax_p_wg_at3(24i), ((l_p.x.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(coax_p_wg_at3(28i), ((coax_at_u(i32(27i)) - l_p.x.y) / l_f)))));
    return coax_p_qfx_rgba(coax_p_qfx_sky(l_rd, l_p.w.w), 0x1.000000p0f);
  }
  if ((l_kind == 0x1.000000p0f))
  {
    return coax_p_qfx_floor(l_p);
  }
  if ((l_kind == 0x1.000000p1f))
  {
    return coax_p_qfx_panel(l_p);
  }
  if ((l_kind == 0x1.800000p1f))
  {
    return coax_p_qfx_glyph(l_p);
  }
  l_dist = coax_p_wg_length3(coax_p_sub3(l_p.z, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))));
  l_fade = exp(((-l_dist) / 0x1.7c0000p7f));
  if ((l_kind == 0x1.400000p2f))
  {
    l_across = (exp(((-(l_p.y.y * l_p.y.y)) * 0x1.400000p2f)) * (0x1.000000p0f - (l_p.y.y * l_p.y.y)));
    l_ends = (0x1.000000p0f - coax_p_smoothstep(0x1.b851ecp-1f, 0x1.000000p0f, abs(l_p.y.x)));
    return coax_p_qfx_rgba(coax_p_scale3(coax_p_v3(l_p.w.x, l_p.w.y, l_p.w.z), (((l_across * l_ends) * l_p.w.w) * l_fade)), 0.0f);
  }
  l_r2 = dot(l_p.y, l_p.y);
  if ((l_r2 >= 0x1.000000p0f))
  {
    discard;
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_glow = (exp(((-l_r2) * 0x1.99999ap1f)) * (0x1.000000p0f - l_r2));
  return coax_p_qfx_rgba(coax_p_scale3(coax_p_v3(l_p.w.x, l_p.w.y, l_p.w.z), ((l_glow * l_p.w.w) * l_fade)), 0.0f);
}
fn coax_p_qfx_gone() -> coax_t12 {
  var l_o: coax_t12;
  l_o = coax_t12();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  return l_o;
}
fn coax_p_qfx_place(p_v: u32, p_n: u32) -> coax_t12 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_o: coax_t12;
  var l_e: coax_t8;
  var l_quad: array<vec2f, 6>;
  var l_c: vec2f;
  var l_kind: f32;
  var l_ax: vec3f;
  var l_ay: vec3f;
  var l_w: vec3f;
  var l_nrm: vec3f;
  var l_to_eye: vec3f;
  var l_lit: f32;
  l_o = coax_t12();
  l_e = puffs[i32(l_n)];
  l_quad = array<vec2f, 6>(coax_p_v2((-0x1.000000p0f), (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_c = l_quad[i32(l_v)];
  l_kind = floor((l_e.x.w + 0x1.000000p-1f));
  l_o.y = l_c;
  l_o.w = l_e.w;
  l_o.f4 = coax_p_v4(l_kind, l_e.y.w, l_e.z.w, 0.0f);
  if ((l_kind == 0.0f))
  {
    l_o.x = coax_p_v4(l_c.x, l_c.y, 0x1.fffeb0p-1f, 0x1.000000p0f);
    l_o.z = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_o.f5 = coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0.0f);
    return l_o;
  }
  l_ax = coax_p_v3(l_e.y.x, l_e.y.y, l_e.y.z);
  l_ay = coax_p_v3(l_e.z.x, l_e.z.y, l_e.z.z);
  l_w = coax_p_add3(coax_p_v3(l_e.x.x, l_e.x.y, l_e.x.z), coax_p_add3(coax_p_scale3(l_ax, l_c.x), coax_p_scale3(l_ay, l_c.y)));
  l_o.x = coax_p_qfx_clip(l_w);
  l_o.z = l_w;
  l_nrm = coax_p_normalize3(coax_p_add3(coax_p_cross3(l_ax, l_ay), coax_p_v3(0.0f, 0x1.0c6f7ap-20f, 0.0f)));
  l_to_eye = coax_p_sub3(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), coax_p_v3(l_e.x.x, l_e.x.y, l_e.x.z));
  if ((dot(l_nrm, l_to_eye) < 0.0f))
  {
    l_nrm = coax_p_scale3(l_nrm, (-0x1.000000p0f));
  }
  l_lit = ((0x1.5c28f6p-2f + (0x1.51eb86p-1f * coax_p_saturate(dot(l_nrm, coax_p_qfx_key())))) + (0x1.99999ap-4f * coax_p_saturate(l_nrm.y)));
  l_o.f5 = coax_p_v4(coax_p_wg_length3(l_ax), coax_p_wg_length3(l_ay), l_lit, 0.0f);
  return l_o;
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
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
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  var l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_qfx_sky(p_rd: vec3f, p_t: f32) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_t: f32 = p_t;
  var l_up: f32;
  var l_dome: vec3f;
  var l_band: f32;
  var l_c: vec3f;
  var l_g: vec2f;
  var l_cell: vec2f;
  var l_h: f32;
  var l_f: vec2f;
  var l_d: f32;
  var l_tw: f32;
  l_up = coax_p_saturate(l_rd.y);
  l_dome = coax_p_mix3(coax_p_v3(0x1.0624dep-8f, 0x1.6872b0p-7f, 0x1.89374cp-6f), coax_p_v3(0x1.a36e2ep-12f, 0x1.a36e2ep-11f, 0x1.47ae14p-9f), coax_pow_particle_vs(l_up, 0x1.19999ap-1f));
  l_band = exp(((-abs(l_rd.y)) * 0x1.200000p3f));
  l_c = coax_p_add3(l_dome, coax_p_scale3(coax_p_qfx_horizon(), (l_band * 0x1.19999ap0f)));
  if ((l_rd.y > 0x1.47ae14p-6f))
  {
    l_g = coax_p_v2((coax_p_wg_atan2(l_rd.x, l_rd.z) * 0x1.680000p6f), (l_rd.y * 0x1.2c0000p7f));
    l_cell = coax_p_v2(floor(l_g.x), floor(l_g.y));
    l_h = coax_p_render_noise2(l_cell.x, l_cell.y);
    if ((l_h > 0x1.ee147ap-1f))
    {
      l_f = coax_p_v2((coax_p_fract(l_g.x) - 0x1.000000p-1f), (coax_p_fract(l_g.y) - 0x1.000000p-1f));
      l_d = dot(l_f, l_f);
      l_tw = (0x1.800000p-1f + (0x1.000000p-2f * sin(((l_t * (0x1.333334p-1f + (l_h * 0x1.000000p1f))) + (l_h * 0x1.900000p8f)))));
      l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.800000p-1f, 0x1.000000p0f), (((exp(((-l_d) * 0x1.680000p6f)) * l_tw) * ((l_h - 0x1.ee147ap-1f) * 0x1.a00000p4f)) * coax_p_saturate((l_rd.y * 0x1.800000p2f)))));
    }
  }
  return l_c;
}
fn coax_p_qfx_rgba(p_c: vec3f, p_a: f32) -> vec4f {
  var l_c: vec3f = p_c;
  var l_a: f32 = p_a;
  return coax_p_v4(l_c.x, l_c.y, l_c.z, l_a);
}
fn coax_p_qfx_floor(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  var l_w: vec3f;
  var l_eye: vec3f;
  var l_dist: f32;
  var l_px: f32;
  var l_fine: f32;
  var l_bold: f32;
  var l_lane: f32;
  var l_rail: f32;
  var l_run: f32;
  var l_pulse: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_r2: f32;
  var l_pool: f32;
  var l_base: vec3f;
  var l_line: vec3f;
  var l_c: vec3f;
  l_w = l_p.z;
  l_eye = coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)));
  l_dist = coax_p_wg_length3(coax_p_sub3(l_w, l_eye));
  l_px = ((l_dist * l_dist) / (coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f) * coax_p_fmax(abs(l_eye.y), 0x1.333334p-2f)));
  l_fine = coax_p_fmax(coax_p_qfx_line(coax_p_qfx_cell(l_w.x, 0x1.000000p0f), (0x1.89374cp-8f + (l_px * 0x1.666666p-2f)), (l_px + 0x1.0624dep-8f)), coax_p_qfx_line(coax_p_qfx_cell(l_w.z, 0x1.000000p0f), (0x1.89374cp-8f + (l_px * 0x1.666666p-2f)), (l_px + 0x1.0624dep-8f)));
  l_bold = coax_p_fmax(coax_p_qfx_line(coax_p_qfx_cell(l_w.x, 0x1.000000p2f), (0x1.cac084p-7f + (l_px * 0x1.99999ap-2f)), (l_px + 0x1.89374cp-8f)), coax_p_qfx_line(coax_p_qfx_cell(l_w.z, 0x1.000000p2f), (0x1.cac084p-7f + (l_px * 0x1.99999ap-2f)), (l_px + 0x1.89374cp-8f)));
  l_fine = ((l_fine * exp(((-l_dist) / 0x1.100000p5f))) * coax_p_saturate((0x1.99999ap-5f / coax_p_fmax(l_px, 0x1.a36e2ep-14f))));
  l_bold = ((l_bold * exp(((-l_dist) / 0x1.e00000p6f))) * coax_p_saturate((0x1.c28f5cp-3f / coax_p_fmax(l_px, 0x1.a36e2ep-14f))));
  l_lane = (0x1.000000p0f - coax_p_smoothstep(0x1.2cccccp1f, 0x1.4cccccp1f, abs(l_w.x)));
  l_rail = (coax_p_qfx_line(abs((abs(l_w.x) - 0x1.400000p1f)), (0x1.1eb852p-5f + (l_px * 0x1.000000p-1f)), (l_px + 0x1.47ae14p-7f)) * exp(((-l_dist) / 0x1.540000p7f)));
  l_run = coax_p_fract(((l_w.z * 0x1.1eb852p-5f) + (l_p.w.w * 0x1.c28f5cp-4f)));
  l_pulse = exp(((-l_run) * 0x1.c00000p2f));
  l_dx = (l_w.x - l_p.f4.y);
  l_dz = (l_w.z - l_p.f4.z);
  l_r2 = ((l_dx * l_dx) + (l_dz * l_dz));
  l_pool = exp(((-l_r2) / 0x1.b80000p5f));
  l_base = coax_p_add3(coax_p_v3(0x1.a36e2ep-12f, 0x1.0624dep-10f, 0x1.0624dep-9f), coax_p_scale3(coax_p_v3(0x1.0624dep-10f, 0x1.89374cp-9f, 0x1.47ae14p-8f), l_lane));
  l_base = coax_p_add3(l_base, coax_p_scale3(coax_p_v3(0x1.0624dep-8f, 0x1.89374cp-7f, 0x1.47ae14p-6f), l_pool));
  l_line = coax_p_v3(0x1.47ae14p-6f, 0x1.0a3d70p-2f, 0x1.ae147ap-2f);
  l_c = coax_p_add3(l_base, coax_p_scale3(l_line, ((l_fine * 0x1.1eb852p-4f) + (l_bold * 0x1.333334p-2f))));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.eb851ep-4f, 0x1.3d70a4p-1f, 0x1.ccccccp-1f), (l_rail * (0x1.333334p-1f + (0x1.666666p0f * l_pulse)))));
  l_c = coax_p_scale3(l_c, (0x1.000000p0f - (0x1.70a3d8p-1f * exp(((-l_r2) * 0x1.99999ap1f)))));
  return coax_p_qfx_rgba(coax_p_qfx_fog(l_c, l_dist), 0x1.000000p0f);
}
fn coax_p_qfx_panel(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  var l_dist: f32;
  var l_ink: vec3f;
  var l_ex: f32;
  var l_ey: f32;
  var l_edge: f32;
  var l_wide: f32;
  var l_rim: f32;
  var l_inner: f32;
  var l_c: vec3f;
  var l_a: f32;
  l_dist = coax_p_wg_length3(coax_p_sub3(l_p.z, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))));
  l_ink = coax_p_v3(l_p.w.x, l_p.w.y, l_p.w.z);
  l_ex = ((0x1.000000p0f - abs(l_p.y.x)) * l_p.f5.x);
  l_ey = ((0x1.000000p0f - abs(l_p.y.y)) * l_p.f5.y);
  l_edge = coax_p_fmin(l_ex, l_ey);
  l_wide = (0x1.26e978p-6f + (l_dist * 0x1.205bc0p-10f));
  l_rim = ((0x1.000000p0f - coax_p_smoothstep(l_wide, (l_wide * 0x1.19999ap1f), l_edge)) * l_p.f4.z);
  l_inner = ((exp(((-l_edge) * 0x1.400000p2f)) * 0x1.c28f5cp-3f) * l_p.f4.z);
  l_c = coax_p_add3(coax_p_scale3(l_ink, (l_p.f5.z * 0x1.000000p-1f)), coax_p_scale3(l_ink, l_p.f4.y));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_add3(coax_p_scale3(l_ink, 0x1.000000p1f), coax_p_v3(0x1.89374cp-7f, 0x1.eb851ep-5f, 0x1.99999ap-4f)), ((l_rim * 0x1.333334p1f) + l_inner)));
  l_a = coax_p_saturate(l_p.w.w);
  return coax_p_qfx_rgba(coax_p_scale3(coax_p_qfx_fog(l_c, l_dist), l_a), l_a);
}
fn coax_p_qfx_glyph(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  var l_gx: f32;
  var l_gy: f32;
  var l_col: f32;
  var l_row: f32;
  var l_bits: u32;
  var l_at: u32;
  var l_fx: f32;
  var l_fy: f32;
  var l_m: f32;
  var l_dist: f32;
  var l_a: f32;
  l_gx = (((l_p.y.x * 0x1.000000p-1f) + 0x1.000000p-1f) * 0x1.400000p2f);
  l_gy = ((0x1.000000p-1f - (l_p.y.y * 0x1.000000p-1f)) * 0x1.c00000p2f);
  l_col = coax_p_clampf(floor(l_gx), 0.0f, 0x1.000000p2f);
  l_row = coax_p_clampf(floor(l_gy), 0.0f, 0x1.800000p2f);
  l_bits = coax_p_wg_u32_f(select(l_p.f4.z, l_p.f4.y, (l_row < 0x1.000000p2f)));
  l_at = coax_p_wg_u32_f(((select((l_row - 0x1.000000p2f), l_row, (l_row < 0x1.000000p2f)) * 0x1.400000p2f) + l_col));
  if ((((l_bits >> u32(l_at)) & 1u) == 0u))
  {
    discard;
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_fx = abs((coax_p_fract(l_gx) - 0x1.000000p-1f));
  l_fy = abs((coax_p_fract(l_gy) - 0x1.000000p-1f));
  l_m = (0x1.000000p0f - coax_p_smoothstep(0x1.70a3d8p-2f, 0x1.000000p-1f, coax_p_fmax(l_fx, l_fy)));
  l_dist = coax_p_wg_length3(coax_p_sub3(l_p.z, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))));
  l_a = (l_m * exp(((-l_dist) / 0x1.040000p8f)));
  return coax_p_qfx_rgba(coax_p_scale3(coax_p_v3(l_p.w.x, l_p.w.y, l_p.w.z), (l_p.w.w * l_a)), l_a);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
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
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_qfx_clip(p_w: vec3f) -> vec4f {
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
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_x = (((((0x1.000000p1f * coax_at_u(i32(23i))) / l_wd) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) / l_wd) * l_r));
  l_y = (((0x1.000000p0f - ((0x1.000000p1f * coax_at_u(i32(27i))) / l_ht)) * l_z) + (((0x1.000000p1f * l_f) / l_ht) * l_q));
  return coax_p_v4(l_x, l_y, ((0x1.c20000p9f * (l_z - 0x1.99999ap-4f)) / (0x1.c20000p9f - 0x1.99999ap-4f)), l_z);
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_qfx_key() -> vec3f {
  return coax_p_v3((-0x1.ae147ap-2f), 0x1.7ae148p-1f, 0x1.0a3d70p-1f);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_qfx_horizon() -> vec3f {
  return coax_p_v3(0x1.89374cp-8f, 0x1.89374cp-6f, 0x1.581062p-5f);
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
  var l_ax: f32;
  var l_ay: f32;
  var l_high: f32;
  var l_r: f32;
  l_ax = abs(l_x);
  l_ay = abs(l_y);
  l_high = coax_p_fmax(l_ax, l_ay);
  if ((l_high == 0.0f))
  {
    return 0.0f;
  }
  l_r = coax_p_wg_atan01((coax_p_fmin(l_ax, l_ay) / l_high));
  if ((l_ay > l_ax))
  {
    l_r = ((0x1.921fb6p1f * 0x1.000000p-1f) - l_r);
  }
  if ((l_x < 0.0f))
  {
    l_r = (0x1.921fb6p1f - l_r);
  }
  if ((l_y < 0.0f))
  {
    l_r = (-l_r);
  }
  return l_r;
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  if (((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_qfx_cell(p_x: f32, p_step: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_step: f32 = p_step;
  return abs(((coax_p_fract(((l_x / l_step) + 0x1.000000p-1f)) - 0x1.000000p-1f) * l_step));
}
fn coax_p_qfx_line(p_d: f32, p_wide: f32, p_soft: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_wide: f32 = p_wide;
  var l_soft: f32 = p_soft;
  return (0x1.000000p0f - coax_p_smoothstep(l_wide, (l_wide + l_soft), l_d));
}
fn coax_p_qfx_fog(p_c: vec3f, p_dist: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_dist: f32 = p_dist;
  return coax_p_mix3(l_c, coax_p_qfx_horizon(), (0x1.000000p0f - exp(((-l_dist) / 0x1.2c0000p7f))));
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
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  if ((!(l_x > 0.0f)))
  {
    return 0u;
  }
  if ((l_x >= 0x1.fffffep31f))
  {
    return 4294967040u;
  }
  return u32(l_x);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_atan01(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32;
  var l_p: f32;
  l_z = (l_x * l_x);
  l_p = (-0x1.09afcep-8f);
  l_p = ((l_p * l_z) + 0x1.662ca4p-6f);
  l_p = ((l_p * l_z) - 0x1.ca0388p-5f);
  l_p = ((l_p * l_z) + 0x1.8aefbep-4f);
  l_p = ((l_p * l_z) - 0x1.1cd8c6p-3f);
  l_p = ((l_p * l_z) + 0x1.98814cp-3f);
  l_p = ((l_p * l_z) - 0x1.554c38p-2f);
  l_p = ((l_p * l_z) + 0x1.ffffeap-1f);
  return (l_x * l_p);
}
fn coax_p_render_noise_coordinate_ok(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_noise_cell(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  var l_bounded: f32;
  l_bounded = coax_p_fmin(coax_p_fmax(l_x, (-0x1.000000p25f)), 0x1.fffffep24f);
  return u32(i32(floor((l_bounded * 0x1.000000p6f))));
}
fn coax_p_render_noise_mix2(p_x: u32, p_y: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_p_render_noise_permute(((l_x * 1597334677u) ^ (l_y * 3812015801u)));
}
fn coax_p_render_noise_unit(p_word: u32) -> f32 {
  var l_word: u32 = p_word;
  return (f32((l_word >> u32(8u))) * 0x1.000000p-24f);
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
@vertex
fn particle_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_particle_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn particle_over_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_particle_over_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@fragment
fn particle_fs(coax_in: coax_io_12) -> @location(0) vec4f {
  let coax_r = coax_p_particle_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}
@fragment
fn particle_over_fs(coax_in: coax_io_12) -> @location(0) vec4f {
  let coax_r = coax_p_particle_over_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}
fn coax_bits_particle_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_particle_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_particle_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_particle_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}