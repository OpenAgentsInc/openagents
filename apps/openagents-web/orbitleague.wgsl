// ---- generated: apps/openagents-web/gpu/orbitleague.coax (the module) by coax wgsl; do not edit
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
struct coax_t14 {
  x: f32,
  y: vec3f,
  z: vec3f,
  w: f32,
  f4: i32,
}
struct coax_t15 {
  x: f32,
  y: vec3f,
  z: f32,
  w: vec3f,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t17 {
  x: f32,
  y: vec3f,
  z: vec3f,
}
struct coax_t18 {
  x: f32,
  y: vec3f,
  z: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 108>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var<storage, read> mesh: array<f32>;
fn coax_p_bg_vs(p_i: u32, p__iid2: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid2: u32 = p__iid2;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p__vid3: u32, p__iid4: u32, p_in_: coax_t9) -> coax_t11 {
  var l__vid3: u32 = p__vid3;
  var l__iid4: u32 = p__iid4;
  var l_in_: coax_t9 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t11;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t11();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_HULL: array<vec4f, 45>;
  var l_px: vec2f;
  var l_ndc: vec2f;
  var l_ro: vec3f;
  var l_rd: vec3f;
  var l_hx: f32;
  var l_hz: f32;
  var l_gw: f32;
  var l_gd: f32;
  var l_h: coax_t14;
  var l_drum: bool;
  var l_t: f32;
  var l_p: vec3f;
  var l_sphere: bool;
  var l_t_17: f32;
  var l_p_18: vec3f;
  var l_t_19: f32;
  var l_p_20: vec3f;
  var l_on: bool;
  var l_c: vec3f;
  var l_p_23: vec3f;
  var l_flat: bool;
  var l_pq: vec3f;
  var l_w: vec2f;
  var l_ring: f32;
  var l_rr: vec3f;
  var l_q: vec3f;
  var l_rh: coax_t14;
  var l_k: f32;
  var l_fl: vec3f;
  var l_i: i32;
  var l_gl: coax_t15;
  var l_i_35: i32;
  var l_i_36: i32;
  l_HULL = array<vec4f, 45>(coax_p_v4(0.0f, (-0x1.000000p0f), 0.0f, 0x1.47ae14p-2f), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0x1.eb851ep-4f), coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.e66666p-1f), coax_p_v4((-0x1.000000p0f), 0.0f, 0.0f, 0x1.e66666p-1f), coax_p_v4(0x1.29930cp-1f, 0x1.a09d4ap-1f, 0.0f, 0x1.05e354p-1f), coax_p_v4((-0x1.29930cp-1f), 0x1.a09d4ap-1f, 0.0f, 0x1.05e354p-1f), coax_p_v4(0x1.47c84cp-1f, (-0x1.895182p-1f), 0.0f, 0x1.72617cp-1f), coax_p_v4((-0x1.47c84cp-1f), (-0x1.895182p-1f), 0.0f, 0x1.72617cp-1f), coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0x1.133334p1f), coax_p_v4(0.0f, 0x1.f6d5d0p-1f, 0x1.8240b8p-3f, 0x1.266666p-2f), coax_p_v4(0.0f, (-0x1.382a9ap-1f), 0x1.95cfaap-1f, 0x1.bd9e84p0f), coax_p_v4(0x1.ccccccp-2f, 0.0f, 0x1.c9374cp-1f, 0x1.0fe910p1f), coax_p_v4((-0x1.ccccccp-2f), 0.0f, 0x1.c9374cp-1f, 0x1.0fe910p1f), coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.0a3d70p1f), coax_p_v4(0.0f, 0x1.e346dcp-1f, (-0x1.525460p-2f), 0x1.72e48ep-1f), coax_p_v4(0x1.99652cp-2f, 0.0f, (-0x1.d54c98p-1f), 0x1.1566d0p1f), coax_p_v4((-0x1.99652cp-2f), 0.0f, (-0x1.d54c98p-1f), 0x1.1566d0p1f), coax_p_v4(0.0f, (-0x1.1c01a4p-1f), (-0x1.aa0902p-1f), 0x1.ccccccp0f), coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.851eb8p-3f), coax_p_v4((-0x1.000000p0f), 0.0f, 0.0f, 0x1.333334p-2f), coax_p_v4(0.0f, (-0x1.000000p0f), 0.0f, 0x1.333334p-2f), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0x1.99999ap-5f), coax_p_v4(0x1.586c22p-1f, 0x1.7ad42cp-1f, 0.0f, 0x1.8c7e28p-6f), coax_p_v4(0.0f, 0x1.586c22p-1f, 0x1.7ad42cp-1f, 0x1.ed288cp-3f), coax_p_v4(0.0f, 0x1.586c22p-1f, (-0x1.7ad42cp-1f), 0x1.ed288cp-3f), coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0x1.51eb86p-1f), coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.51eb86p-1f), coax_p_v4(0.0f, (-0x1.47c84cp-1f), 0x1.895182p-1f, 0x1.2703b0p-1f), coax_p_v4(0.0f, (-0x1.47c84cp-1f), (-0x1.895182p-1f), 0x1.2703b0p-1f), coax_p_v4(0.0f, 0x1.7487fcp-1f, 0x1.5f3b64p-1f, 0x1.f0d844p-2f), coax_p_v4(0.0f, (-0x1.7487fcp-1f), (-0x1.5f3b64p-1f), (-0x1.c7e282p-2f)), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0x1.d70a3ep-2f), coax_p_v4(0.0f, (-0x1.000000p0f), 0.0f, (-0x1.47ae14p-4f)), coax_p_v4(0x1.fc5d64p-1f, 0x1.e7d566p-4f, 0.0f, 0x1.559b3ep-1f), coax_p_v4((-0x1.fc5d64p-1f), 0x1.e7d566p-4f, 0.0f, 0x1.559b3ep-1f), coax_p_v4(0x1.6a0902p-1f, 0x1.6a0902p-1f, 0.0f, 0x1.5b8bacp-1f), coax_p_v4((-0x1.6a0902p-1f), 0x1.6a0902p-1f, 0.0f, 0x1.5b8bacp-1f), coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.000000p-2f), coax_p_v4((-0x1.000000p0f), 0.0f, 0.0f, 0x1.000000p-2f), coax_p_v4(0.0f, (-0x1.000000p0f), 0.0f, 0.0f), coax_p_v4(0.0f, 0x1.f53262p-1f, (-0x1.a2680ap-3f), 0x1.e57a78p-2f), coax_p_v4(0.0f, 0x1.525460p-2f, 0x1.e346dcp-1f, (-0x1.7dbf48p-2f)), coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.c00000p0f), coax_p_v4(0x1.586c22p-1f, 0x1.7ad42cp-1f, 0.0f, 0x1.621966p-2f), coax_p_v4((-0x1.586c22p-1f), 0x1.7ad42cp-1f, 0.0f, 0x1.621966p-2f));
  l_px = coax_p_v2(l_pos.x, l_pos.y);
  l_ndc = coax_p_v2((((l_px.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_px.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_ro = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_rd = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), coax_p_wg_mul_ff3(l_ndc.x, coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))))), coax_p_wg_mul_ff3(l_ndc.y, coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(37i)), coax_at_u(i32(38i))))));
  l_hx = coax_at_u(i32(80i));
  l_hz = coax_at_u(i32(81i));
  l_gw = coax_at_u(i32(83i));
  l_gd = coax_at_u(i32(85i));
  l_h = coax_p_orbitleague_miss();
  l_drum = coax_p_orbitleague_in_drum();
  if (l_drum)
  {
    l_t = coax_p_orbitleague_drum_t(l_ro, l_rd);
    l_p = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_t));
    if (((l_t < 0x1.7d7840p26f) && (abs(l_p.z) <= l_hz)))
    {
      l_h = coax_t14(l_t, coax_p_orbitleague_drum_n(l_p), coax_p_orbitleague_drum_col(l_p), 0.0f, 5i);
    }
  }
  l_sphere = coax_p_orbitleague_in_sphere();
  if (l_sphere)
  {
    l_t_17 = coax_p_orbitleague_sph_t(l_ro, l_rd);
    l_p_18 = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_t_17));
    if ((l_t_17 < 0x1.7d7840p26f))
    {
      l_h = coax_t14(l_t_17, coax_p_orbitleague_sph_n(l_p_18), coax_p_orbitleague_sph_col(l_p_18), 0.0f, 5i);
    }
  }
  if (((l_rd.y < 0.0f) && (!l_sphere)))
  {
    l_t_19 = ((-l_ro.y) / l_rd.y);
    l_p_20 = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_t_19));
    l_on = (((abs(l_p_20.x) <= l_hx) && (abs(l_p_20.z) <= l_hz)) || ((abs(l_p_20.x) <= l_gw) && (abs(l_p_20.z) <= (l_hz + l_gd))));
    if (l_drum)
    {
      l_on = (((abs(l_p_20.x) <= l_gw) && (abs(l_p_20.z) > l_hz)) && (abs(l_p_20.z) <= (l_hz + l_gd)));
    }
    if ((l_on && (l_t_19 < l_h.x)))
    {
      l_h = coax_t14(l_t_19, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_orbitleague_floor_col(l_p_20), 0.0f, 5i);
    }
  }
  l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_movers(l_ro, l_rd, l_HULL, true));
  l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_frames(l_ro, l_rd));
  if ((((l_h.x > 0x1.7d7840p26f) && (!l_drum)) && (!l_sphere)))
  {
    l_h = coax_p_orbitleague_ribs(l_ro, l_rd);
  }
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_h.x < 0x1.7d7840p26f))
  {
    l_p_23 = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_h.x));
    if ((l_h.f4 == 5i))
    {
      l_flat = ((!l_sphere) && ((!l_drum) || (abs(l_p_23.z) > l_hz)));
      l_pq = l_p_23;
      if ((!l_flat))
      {
        l_w = select(coax_p_orbitleague_drum_uv(l_p_23), coax_p_orbitleague_sph_uv(l_p_23), l_sphere);
        l_pq = coax_p_v3(l_w.x, 0.0f, l_w.y);
      }
      l_c = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_h.z, 0x1.b33334p-1f), select(coax_p_orbitleague_shadow(l_p_23), coax_p_orbitleague_drum_shadow(l_p_23), (!l_flat))), coax_p_orbitleague_underglow(l_p_23)), coax_p_orbitleague_pads(l_pq));
      if ((coax_at_u(i32(331i)) >= 0.0f))
      {
        l_ring = ((select(coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p_23.x, l_p_23.z), coax_p_v2(coax_at_u(i32(328i)), coax_at_u(i32(330i))))), coax_p_wg_length3(coax_p_wg_sub_f3f3(l_p_23, coax_p_v3(coax_at_u(i32(328i)), coax_at_u(i32(329i)), coax_at_u(i32(330i))))), (l_drum || l_sphere)) - (coax_p_orbitleague_blast_r(coax_at_u(i32(331i))) * 0x1.19999ap0f)) / 0x1.333334p0f);
        l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_orbitleague_team(coax_at_u(i32(332i))), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.333334p-2f), exp(((-l_ring) * l_ring))), coax_p_orbitleague_blast_fade(coax_at_u(i32(331i)))), 0x1.ccccccp-1f));
      }
      l_rr = coax_p_wg_reflect3(l_rd, l_h.y);
      l_q = coax_p_wg_add_f3f3(l_p_23, coax_p_wg_mul_f3f(l_h.y, 0x1.0624dep-9f));
      l_rh = coax_p_orbitleague_closer(coax_p_orbitleague_movers(l_q, l_rr, l_HULL, false), coax_p_orbitleague_frames(l_q, l_rr));
      l_k = (0x1.99999ap-3f + (0x1.333334p-2f * coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_rd, l_h.y))), 0x1.000000p2f)));
      if ((l_rh.x < 0x1.180000p6f))
      {
        l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_orbitleague_shade_hit(l_rh, l_rr), (l_k * exp(((-l_rh.x) * 0x1.ccccccp-2f))));
      }
      l_fl = coax_p_v3(0.0f, 0.0f, 0.0f);
      {
        l_i = 0i;
        while ((l_i < coax_p_orbitleague_ncars()))
        {
          l_fl = coax_p_wg_add_f3f3(l_fl, coax_p_orbitleague_flame(l_q, l_rr, coax_p_orbitleague_car(l_i), l_rh.x));
          l_i = (l_i + 1i);
        }
      }
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(l_fl, l_k));
    }
    else
    {
      l_c = coax_p_orbitleague_shade_hit(l_h, l_rd);
    }
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.99999ap-5f, 0x1.1eb852p-4f, 0x1.eb851ep-4f), coax_p_clampf((l_h.x / 0x1.b80000p7f), 0.0f, 0x1.000000p-1f));
  }
  else
  {
    l_c = coax_p_orbitleague_space(l_rd);
  }
  l_gl = coax_p_orbitleague_glass(l_ro, l_rd, l_h.x, l_HULL);
  if ((l_gl.z > 0.0f))
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, l_gl.y, l_gl.z);
  }
  l_c = coax_p_wg_add_f3f3(l_c, l_gl.w);
  {
    l_i_35 = 0i;
    while ((l_i_35 < coax_p_orbitleague_ncars()))
    {
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_flame(l_ro, l_rd, coax_p_orbitleague_car(l_i_35), l_h.x));
      l_i_35 = (l_i_35 + 1i);
    }
  }
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_trail(l_ro, l_rd, 0i, l_h.x, (coax_at_u(i32(43i)) * 0x1.800000p-1f), coax_p_v3(0x1.333334p-1f, 0x1.b33334p-1f, 0x1.000000p0f)));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_add_f3f3(coax_p_orbitleague_trail(l_ro, l_rd, 1i, l_h.x, 0x1.47ae14p-2f, coax_p_v3(0x1.ccccccp-2f, 0x1.666666p-1f, 0x1.000000p0f)), coax_p_orbitleague_trail(l_ro, l_rd, 2i, l_h.x, 0x1.47ae14p-2f, coax_p_v3(0x1.ccccccp-2f, 0x1.666666p-1f, 0x1.000000p0f))));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_blast(l_ro, l_rd, l_h.x));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_pad_orbs(l_ro, l_rd, l_h.x));
  {
    l_i_36 = 0i;
    while ((l_i_36 < coax_p_orbitleague_ncars()))
    {
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_lights(l_ro, l_rd, coax_p_orbitleague_car(l_i_36), l_h.x));
      l_i_36 = (l_i_36 + 1i);
    }
  }
  if ((((!l_drum) && (!l_sphere)) && ((l_rd.y > 0.0f) || (l_h.x > 0x1.7d7840p26f))))
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_orbitleague_rib_lights(l_ro, l_rd, l_h.x));
  }
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f3(l_c, coax_p_v3(0x1.b33334p-1f, 0x1.e66666p-1f, 0x1.266666p0f)), (coax_at_u(i32(7i)) * 0x1.000000p-1f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_orbitleague_team(coax_at_u(i32(31i))), coax_at_u(i32(27i))), 0x1.eb851ep-4f));
  return coax_p_wg_make_f4_f3f(coax_p_orbitleague_panels(l_px, l_c), 0x1.000000p0f);
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
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_orbitleague_miss() -> coax_t14 {
  return coax_t14(0x1.dcd650p29f, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f), 0.0f, 0i);
}
fn coax_p_orbitleague_in_drum() -> bool {
  return (coax_at_u(i32(86i)) > 0.0f);
}
fn coax_p_orbitleague_drum_t(p_ro: vec3f, p_rd: vec3f) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_r: f32;
  var l_o: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_disc: f32;
  var l_t: f32;
  l_r = coax_at_u(i32(86i));
  l_o = coax_p_v2(l_ro.x, (l_ro.y - l_r));
  l_a = coax_p_wg_dot2(coax_p_v2(l_rd.x, l_rd.y), coax_p_v2(l_rd.x, l_rd.y));
  if ((l_a < 0x1.5798eep-27f))
  {
    return 0x1.dcd650p29f;
  }
  l_b = coax_p_wg_dot2(l_o, coax_p_v2(l_rd.x, l_rd.y));
  l_c = (coax_p_wg_dot2(l_o, l_o) - (l_r * l_r));
  l_disc = ((l_b * l_b) - (l_a * l_c));
  if ((l_disc < 0.0f))
  {
    return 0x1.dcd650p29f;
  }
  l_t = (((-l_b) + sqrt(l_disc)) / l_a);
  return select(0x1.dcd650p29f, l_t, (l_t > 0.0f));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_orbitleague_drum_n(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  return coax_p_wg_normalize3(coax_p_v3((-l_p.x), (coax_at_u(i32(86i)) - l_p.y), 0.0f));
}
fn coax_p_orbitleague_drum_col(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_q: vec2f;
  var l_c: vec3f;
  var l_hz: f32;
  var l_hoop: f32;
  var l_k: i32;
  var l_side: vec3f;
  l_q = coax_p_orbitleague_drum_uv(l_p);
  l_c = coax_p_orbitleague_deck_col(coax_p_v3(l_q.x, 0.0f, l_q.y), 0x1.388000p13f);
  l_hz = coax_at_u(i32(81i));
  l_hoop = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_hoop = coax_p_fmax(l_hoop, (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, abs((l_q.y - ((-0x1.e00000p4f) + (f32(l_k) * 0x1.400000p4f)))))));
      l_k = (l_k + 1i);
    }
  }
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.333334p-1f, 0x1.000000p0f, 0x1.e66666p-1f), (l_hoop * 0x1.000000p-1f));
  l_side = select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_p.z > 0.0f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_side, (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-1f, (l_hz - abs(l_p.z))))), 0x1.333334p-1f));
  return l_c;
}
fn coax_p_orbitleague_in_sphere() -> bool {
  return (coax_at_u(i32(86i)) < 0.0f);
}
fn coax_p_orbitleague_sph_t(p_ro: vec3f, p_rd: vec3f) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_o: vec3f;
  var l_b: f32;
  var l_c: f32;
  var l_disc: f32;
  var l_t: f32;
  l_o = coax_p_wg_sub_f3f3(l_ro, coax_p_orbitleague_sph_c());
  l_b = coax_p_wg_dot3(l_o, l_rd);
  l_c = (coax_p_wg_dot3(l_o, l_o) - (coax_p_orbitleague_sr() * coax_p_orbitleague_sr()));
  l_disc = ((l_b * l_b) - l_c);
  if ((l_disc < 0.0f))
  {
    return 0x1.dcd650p29f;
  }
  l_t = ((-l_b) + sqrt(l_disc));
  return select(0x1.dcd650p29f, l_t, (l_t > 0.0f));
}
fn coax_p_orbitleague_sph_n(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  return coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_orbitleague_sph_c(), l_p));
}
fn coax_p_orbitleague_sph_col(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_q: vec2f;
  var l_c: vec3f;
  var l_hoop: f32;
  var l_k: i32;
  var l_side: vec3f;
  var l_rho: f32;
  var l_gr: f32;
  var l_g: vec2f;
  var l_mesh: f32;
  l_q = coax_p_orbitleague_sph_uv(l_p);
  l_c = coax_p_orbitleague_deck_col(coax_p_v3(l_q.x, 0.0f, l_q.y), 0x1.388000p13f);
  l_hoop = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_hoop = coax_p_fmax(l_hoop, (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, abs((abs(l_q.y) - (0x1.000000p3f + (f32(l_k) * 0x1.c00000p2f)))))));
      l_k = (l_k + 1i);
    }
  }
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.333334p-1f, 0x1.000000p0f, 0x1.e66666p-1f), (l_hoop * 0x1.99999ap-2f));
  l_side = select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_p.z > 0.0f));
  l_rho = coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.y), coax_p_wg_sw_xy_f3(coax_p_orbitleague_sph_c())));
  l_gr = coax_at_u(i32(87i));
  if ((l_rho < l_gr))
  {
    l_g = coax_p_wg_mul_f2f(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(coax_p_wg_div_f2f(coax_p_v2(l_p.x, l_p.y), 0x1.333334p-1f), 0x1.000000p-1f)), 0x1.000000p-1f)), 0x1.333334p-1f);
    l_mesh = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-7f, 0x1.47ae14p-5f, coax_p_fmin(l_g.x, l_g.y)));
    l_c = coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(l_side, 0x1.666666p-2f), coax_p_wg_mul_f3f(l_side, 0x1.4cccccp0f), (l_mesh * 0x1.666666p-1f));
  }
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_side, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.333334p-2f), exp((-(((l_rho - l_gr) / 0x1.000000p-2f) * ((l_rho - l_gr) / 0x1.000000p-2f))))), 0x1.333334p0f));
  return l_c;
}
fn coax_p_orbitleague_floor_col(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  return coax_p_orbitleague_deck_col(l_p, coax_at_u(i32(80i)));
}
fn coax_p_orbitleague_movers(p_ro: vec3f, p_rd: vec3f, p_HULL: array<vec4f, 45>, p_seen: bool) -> coax_t14 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_HULL: array<vec4f, 45> = p_HULL;
  var l_seen: bool = p_seen;
  var l_h: coax_t14;
  var l_i: i32;
  l_h = coax_p_orbitleague_ball_hit(l_ro, l_rd);
  {
    l_i = 0i;
    while ((l_i < coax_p_orbitleague_ncars()))
    {
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_car_hit(l_ro, l_rd, coax_p_orbitleague_car(l_i), l_HULL, l_seen));
      l_i = (l_i + 1i);
    }
  }
  return l_h;
}
fn coax_p_orbitleague_closer(p_a: coax_t14, p_b: coax_t14) -> coax_t14 {
  var l_a: coax_t14 = p_a;
  var l_b: coax_t14 = p_b;
  if ((l_b.x < l_a.x))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_orbitleague_frames(p_ro: vec3f, p_rd: vec3f) -> coax_t14 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_hz: f32;
  var l_gw: f32;
  var l_gh: f32;
  var l_h: coax_t14;
  var l_e: f32;
  var l_col: vec3f;
  var l_z: f32;
  var l_tz: f32;
  var l_q: vec3f;
  l_hz = coax_at_u(i32(81i));
  l_gw = coax_at_u(i32(83i));
  l_gh = coax_at_u(i32(84i));
  l_h = coax_p_orbitleague_miss();
  if (coax_p_orbitleague_in_sphere())
  {
    return l_h;
  }
  {
    l_e = (-0x1.000000p0f);
    while ((l_e <= 0x1.000000p0f))
    {
      l_col = coax_p_wg_mul_f3f(select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_e > 0.0f)), 0x1.4cccccp0f);
      l_z = (l_e * (l_hz + 0x1.333334p-3f));
      l_tz = ((l_z - l_ro.z) / l_rd.z);
      l_q = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_tz));
      if ((((((abs(l_rd.z) < 0x1.4f8b58p-17f) || (l_tz < 0.0f)) || (abs(l_q.x) > (l_gw + 0x1.800000p0f))) || (l_q.y > (l_gh + 0x1.800000p0f))) || (l_q.y < (-0x1.000000p-1f))))
      {
        {
          l_e = (l_e + 0x1.000000p1f);
          continue;
        }
      }
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3(((-l_gw) - 0x1.99999ap-3f), (l_gh * 0x1.000000p-1f), l_z), coax_p_v3(0x1.99999ap-3f, ((l_gh * 0x1.000000p-1f) + 0x1.99999ap-3f), 0x1.99999ap-3f), l_col, 3i));
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3((l_gw + 0x1.99999ap-3f), (l_gh * 0x1.000000p-1f), l_z), coax_p_v3(0x1.99999ap-3f, ((l_gh * 0x1.000000p-1f) + 0x1.99999ap-3f), 0x1.99999ap-3f), l_col, 3i));
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3(0.0f, (l_gh + 0x1.99999ap-3f), l_z), coax_p_v3((l_gw + 0x1.99999ap-2f), 0x1.99999ap-3f, 0x1.99999ap-3f), l_col, 3i));
      l_e = (l_e + 0x1.000000p1f);
    }
  }
  return l_h;
}
fn coax_p_orbitleague_ribs(p_ro: vec3f, p_rd: vec3f) -> coax_t14 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_hx: f32;
  var l_hh: f32;
  var l_h: coax_t14;
  var l_steel: vec3f;
  var l_k: i32;
  var l_z: f32;
  var l_tz: f32;
  var l_q: vec3f;
  l_hx = coax_at_u(i32(80i));
  l_hh = coax_at_u(i32(82i));
  l_h = coax_p_orbitleague_miss();
  l_steel = coax_p_v3(0x1.c28f5cp-4f, 0x1.eb851ep-4f, 0x1.333334p-3f);
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_z = ((-0x1.e00000p4f) + (f32(l_k) * 0x1.400000p4f));
      l_tz = ((l_z - l_ro.z) / l_rd.z);
      l_q = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_tz));
      if ((((((abs(l_rd.z) < 0x1.4f8b58p-17f) || (l_tz < 0.0f)) || (abs(l_q.x) > (l_hx + 0x1.800000p1f))) || (l_q.y > (l_hh + 0x1.800000p1f))) || ((abs(l_q.x) < (l_hx - 0x1.000000p0f)) && (l_q.y < (l_hh - 0x1.000000p0f)))))
      {
        {
          l_k = (l_k + 1i);
          continue;
        }
      }
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3(0.0f, (l_hh + 0x1.000000p0f), l_z), coax_p_v3((l_hx + 0x1.666666p0f), 0x1.ccccccp-2f, 0x1.333334p-1f), l_steel, 0i));
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3((l_hx + 0x1.e66666p-1f), (l_hh * 0x1.000000p-1f), l_z), coax_p_v3(0x1.ccccccp-2f, ((l_hh * 0x1.000000p-1f) + 0x1.666666p0f), 0x1.333334p-1f), l_steel, 0i));
      l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_ro, l_rd, coax_p_v3(((-l_hx) - 0x1.e66666p-1f), (l_hh * 0x1.000000p-1f), l_z), coax_p_v3(0x1.ccccccp-2f, ((l_hh * 0x1.000000p-1f) + 0x1.666666p0f), 0x1.333334p-1f), l_steel, 0i));
      l_k = (l_k + 1i);
    }
  }
  return l_h;
}
fn coax_p_orbitleague_sph_uv(p_p: vec3f) -> vec2f {
  var l_p: vec3f = p_p;
  var l_d: vec3f;
  l_d = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_p, coax_p_orbitleague_sph_c()));
  return coax_p_v2((coax_p_orbitleague_sr() * coax_p_wg_atan2(l_d.x, (-l_d.y))), (coax_p_orbitleague_sr() * coax_p_wg_asin(coax_p_clampf(l_d.z, (-0x1.000000p0f), 0x1.000000p0f))));
}
fn coax_p_orbitleague_drum_uv(p_p: vec3f) -> vec2f {
  var l_p: vec3f = p_p;
  return coax_p_v2((coax_at_u(i32(86i)) * coax_p_wg_atan2(l_p.x, (coax_at_u(i32(86i)) - l_p.y))), l_p.z);
}
fn coax_p_orbitleague_drum_shadow(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f;
  var l_k: f32;
  var l_bq: vec3f;
  var l_bh: f32;
  var l_bd: f32;
  var l_i: i32;
  var l_c: coax_t16;
  var l_q: vec3f;
  var l_h: f32;
  var l_f: vec3f;
  var l_lx: f32;
  var l_lz: f32;
  var l_s: f32;
  l_n = coax_p_orbitleague_curve_n(l_p);
  l_k = 0x1.000000p0f;
  l_bq = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i))), l_p);
  l_bh = coax_p_wg_dot3(l_bq, l_n);
  l_bd = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_bq, coax_p_wg_mul_f3f(l_n, l_bh)));
  l_k = (l_k * (0x1.000000p0f - ((0x1.19999ap-1f * (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(43i)) * 0x1.333334p-1f), (coax_at_u(i32(43i)) * (0x1.19999ap0f + (coax_p_fmax(l_bh, 0.0f) * 0x1.99999ap-5f))), l_bd))) * coax_p_stepf(0.0f, l_bh))));
  {
    l_i = 0i;
    while ((l_i < coax_p_orbitleague_ncars()))
    {
      l_c = coax_p_orbitleague_car(l_i);
      l_q = coax_p_wg_sub_f3f3(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z), l_p);
      l_h = coax_p_wg_dot3(l_q, l_n);
      l_f = coax_p_wg_sub_f3f3(l_q, coax_p_wg_mul_f3f(l_n, l_h));
      l_lx = abs(coax_p_wg_dot3(l_f, coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z)));
      l_lz = abs(coax_p_wg_dot3(l_f, coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z)));
      l_s = ((0x1.000000p0f - coax_p_smoothstep(0x1.b33334p-1f, (0x1.4cccccp0f + (coax_p_fmax(l_h, 0.0f) * 0x1.99999ap-5f)), l_lx)) * (0x1.000000p0f - coax_p_smoothstep(0x1.e66666p0f, (0x1.333334p1f + (coax_p_fmax(l_h, 0.0f) * 0x1.99999ap-5f)), l_lz)));
      l_k = (l_k * (0x1.000000p0f - ((0x1.000000p-1f * l_s) * coax_p_stepf(0.0f, l_h))));
      l_i = (l_i + 1i);
    }
  }
  return l_k;
}
fn coax_p_orbitleague_shadow(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_k: f32;
  var l_bd: f32;
  var l_bh: f32;
  var l_i: i32;
  var l_c: coax_t16;
  var l_q: vec3f;
  var l_lx: f32;
  var l_lz: f32;
  var l_h: f32;
  var l_s: f32;
  l_k = 0x1.000000p0f;
  l_bd = coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(coax_at_u(i32(40i)), coax_at_u(i32(42i)))));
  l_bh = coax_p_fmax((coax_at_u(i32(41i)) - l_p.y), 0.0f);
  l_k = (l_k * (0x1.000000p0f - ((0x1.19999ap-1f * (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(43i)) * 0x1.333334p-1f), (coax_at_u(i32(43i)) * (0x1.19999ap0f + (l_bh * 0x1.99999ap-5f))), l_bd))) * coax_p_stepf(l_p.y, coax_at_u(i32(41i))))));
  {
    l_i = 0i;
    while ((l_i < coax_p_orbitleague_ncars()))
    {
      l_c = coax_p_orbitleague_car(l_i);
      l_q = coax_p_wg_sub_f3f3(l_p, coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z));
      l_lx = abs(coax_p_wg_dot2(coax_p_v2(l_q.x, l_q.z), coax_p_v2(l_c.y.x, l_c.y.z)));
      l_lz = abs(coax_p_wg_dot2(coax_p_v2(l_q.x, l_q.z), coax_p_v2(l_c.w.x, l_c.w.z)));
      l_h = coax_p_fmax((l_c.x.y - l_p.y), 0.0f);
      l_s = ((0x1.000000p0f - coax_p_smoothstep(0x1.b33334p-1f, (0x1.4cccccp0f + (l_h * 0x1.99999ap-5f)), l_lx)) * (0x1.000000p0f - coax_p_smoothstep(0x1.e66666p0f, (0x1.333334p1f + (l_h * 0x1.99999ap-5f)), l_lz)));
      l_k = (l_k * (0x1.000000p0f - ((0x1.000000p-1f * l_s) * coax_p_stepf(l_p.y, l_c.x.y))));
      l_i = (l_i + 1i);
    }
  }
  return l_k;
}
fn coax_p_orbitleague_underglow(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_o: vec3f;
  var l_i: i32;
  var l_c: coax_t16;
  var l_q: vec3f;
  var l_lx: f32;
  var l_lz: f32;
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_i = 0i;
    while ((l_i < coax_p_orbitleague_ncars()))
    {
      l_c = coax_p_orbitleague_car(l_i);
      l_q = coax_p_wg_sub_f3f3(l_p, coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z));
      l_lx = coax_p_wg_dot3(l_q, coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z));
      l_lz = coax_p_wg_dot3(l_q, coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z));
      l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_orbitleague_team(coax_p_orbitleague_car_side(l_c)), exp((-(((l_lx * l_lx) / 0x1.19999ap0f) + ((l_lz * l_lz) / 0x1.400000p2f))))), coax_p_clampf((0x1.ccccccp0f - coax_p_orbitleague_over_floor(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z))), 0.0f, 0x1.000000p0f)), 0x1.99999ap-2f));
      l_i = (l_i + 1i);
    }
  }
  return l_o;
}
fn coax_p_orbitleague_pads(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_o: vec3f;
  var l_i: i32;
  var l_pd: vec4f;
  var l_d: f32;
  var l_r: f32;
  var l_ready: bool;
  var l_a: f32;
  var l_lit: f32;
  var l_pulse: f32;
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_pd = coax_p_wg_at4(((i32(l_i) * 4i) + 336i));
      l_d = coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_pd.x, l_pd.y)));
      l_r = select(0x1.ccccccp-1f, 0x1.800000p0f, (l_pd.z > 0x1.000000p-1f));
      if ((l_d > (l_r + 0x1.333334p-1f)))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_ready = (l_pd.w >= 0x1.ff7ceep-1f);
      l_a = ((coax_p_wg_atan2((l_p.x - l_pd.x), (l_p.z - l_pd.y)) / 0x1.921fb6p2f) + 0x1.000000p-1f);
      l_lit = select(0x1.70a3d8p-3f, 0x1.ccccccp-1f, (l_ready || (l_a < l_pd.w)));
      l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_ORBITLEAGUE_PAD_COL(), exp((-(((l_d - l_r) / 0x1.70a3d8p-4f) * ((l_d - l_r) / 0x1.70a3d8p-4f))))), l_lit));
      if (l_ready)
      {
        l_pulse = (0x1.99999ap-1f + ((0x1.99999ap-3f * coax_p_orbitleague_moving()) * sin(((coax_at_u(i32(2i)) * 0x1.000000p2f) + f32(l_i)))));
        l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_ORBITLEAGUE_PAD_COL(), (0x1.000000p0f - coax_p_smoothstep((l_r * 0x1.19999ap-1f), (l_r * 0x1.99999ap-1f), l_d))), 0x1.ccccccp-2f), l_pulse));
      }
      l_i = (l_i + 1i);
    }
  }
  return l_o;
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_orbitleague_blast_r(p_age: f32) -> f32 {
  var l_age: f32 = p_age;
  return (0x1.800000p0f + (0x1.c00000p4f * (0x1.000000p0f - exp(((-l_age) * 0x1.19999ap1f)))));
}
fn coax_p_orbitleague_team(p_t: f32) -> vec3f {
  var l_t: f32 = p_t;
  return select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_t > 0x1.000000p-1f));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_orbitleague_blast_fade(p_age: f32) -> f32 {
  var l_age: f32 = p_age;
  var l_k: f32;
  l_k = (0x1.000000p0f - coax_p_clampf((l_age / 0x1.99999ap0f), 0.0f, 0x1.000000p0f));
  return (l_k * l_k);
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_orbitleague_shade_hit(p_h: coax_t14, p_rd: vec3f) -> vec3f {
  var l_h: coax_t14 = p_h;
  var l_rd: vec3f = p_rd;
  var l_n: vec3f;
  var l_l: f32;
  var l_r: vec3f;
  var l_fres: f32;
  var l_c: vec3f;
  var l_glow: f32;
  l_n = l_h.y;
  if ((l_h.f4 == 3i))
  {
    return l_h.z;
  }
  l_l = coax_p_orbitleague_flat_light(l_n);
  l_r = coax_p_wg_reflect3(l_rd, l_n);
  l_fres = coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_n, coax_p_wg_neg_f3(l_rd)))), 0x1.000000p2f);
  l_c = coax_p_wg_mul_f3f(l_h.z, l_l);
  if ((l_h.f4 == 1i))
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_orbitleague_env(l_r), (0x1.eb851ep-5f + (0x1.000000p-1f * l_fres)));
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_smoothstep(0x1.dc28f6p-1f, 0x1.f0a3d8p-1f, coax_p_wg_dot3(l_r, coax_p_orbitleague_sun()))), 0x1.666666p-1f));
  }
  else
  {
    if ((l_h.f4 == 2i))
    {
      l_c = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_h.z, coax_p_wg_mul_f3f(coax_p_orbitleague_env(l_r), 0x1.ccccccp-1f)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_smoothstep(0x1.ccccccp-1f, 0x1.e66666p-1f, coax_p_wg_dot3(l_r, coax_p_orbitleague_sun()))), 0x1.333334p-1f));
    }
    else
    {
      if ((l_h.f4 == 4i))
      {
        l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_orbitleague_env(l_r), (0x1.99999ap-5f + (0x1.333334p-2f * l_fres)));
        l_glow = coax_p_clampf((coax_at_u(i32(88i)) / 0x1.900000p4f), 0.0f, 0x1.000000p0f);
        return coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.70a3d8p-4f), coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-1f, 0x1.ccccccp-1f, 0x1.000000p0f), 0x1.99999ap0f), (0x1.99999ap-3f + (0x1.99999ap-1f * l_glow))), l_h.w);
      }
    }
  }
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-3f, 0x1.333334p-2f, 0x1.333334p-1f), coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_n, coax_p_wg_neg_f3(l_rd)))), 0x1.800000p1f)), 0x1.000000p-1f));
  return coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.47ae14p-7f, 0x1.eb851ep-7f, 0x1.eb851ep-6f), (coax_p_clampf(l_h.w, 0.0f, 0x1.000000p0f) * 0x1.b33334p-1f));
}
fn coax_p_orbitleague_ncars() -> i32 {
  return select(2i, 4i, (coax_at_u(i32(89i)) > 0x1.800000p1f));
}
fn coax_p_orbitleague_car(p_i: i32) -> coax_t16 {
  var l_i: i32 = p_i;
  if ((l_i < 2i))
  {
    return coax_t16(coax_p_wg_at4(((i32(l_i) * 16i) + 48i)), coax_p_wg_at4(((i32(l_i) * 16i) + 52i)), coax_p_wg_at4(((i32(l_i) * 16i) + 56i)), coax_p_wg_at4(((i32(l_i) * 16i) + 60i)));
  }
  return coax_t16(coax_p_wg_at4(((i32((l_i - 2i)) * 16i) + 400i)), coax_p_wg_at4(((i32((l_i - 2i)) * 16i) + 404i)), coax_p_wg_at4(((i32((l_i - 2i)) * 16i) + 408i)), coax_p_wg_at4(((i32((l_i - 2i)) * 16i) + 412i)));
}
fn coax_p_orbitleague_flame(p_ro: vec3f, p_rd: vec3f, p_c: coax_t16, p_tmax: f32) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: coax_t16 = p_c;
  var l_tmax: f32 = p_tmax;
  var l_flick: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_ab: vec3f;
  var l_w0: vec3f;
  var l_aa: f32;
  var l_bb: f32;
  var l_cc: f32;
  var l_dd: f32;
  var l_ee: f32;
  var l_den: f32;
  var l_s: f32;
  var l_t: f32;
  var l_dist: f32;
  var l_r: f32;
  var l_core: f32;
  var l_halo: f32;
  var l_diamonds: f32;
  if ((l_c.x.w < 0x1.000000p-1f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_flick = (0x1.000000p0f + ((coax_p_orbitleague_moving() * 0x1.eb851ep-4f) * sin(((coax_at_u(i32(2i)) * 0x1.c80000p5f) + (coax_p_orbitleague_car_side(l_c) * 0x1.800000p1f)))));
  l_a = coax_p_wg_sub_f3f3(coax_p_wg_sub_f3f3(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z), coax_p_wg_mul_f3f(coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z), 0x1.0f5c28p1f)), coax_p_wg_mul_f3f(coax_p_v3(l_c.z.x, l_c.z.y, l_c.z.z), 0x1.99999ap-5f));
  l_b = coax_p_wg_sub_f3f3(l_a, coax_p_wg_mul_f3f(coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z), (0x1.19999ap1f * l_flick)));
  l_ab = coax_p_wg_sub_f3f3(l_b, l_a);
  l_w0 = coax_p_wg_sub_f3f3(l_ro, l_a);
  l_aa = coax_p_wg_dot3(l_rd, l_rd);
  l_bb = coax_p_wg_dot3(l_rd, l_ab);
  l_cc = coax_p_wg_dot3(l_ab, l_ab);
  l_dd = coax_p_wg_dot3(l_rd, l_w0);
  l_ee = coax_p_wg_dot3(l_ab, l_w0);
  l_den = ((l_aa * l_cc) - (l_bb * l_bb));
  l_s = 0.0f;
  l_t = 0.0f;
  if ((l_den > 0x1.0c6f7ap-20f))
  {
    l_s = coax_p_clampf((((l_bb * l_ee) - (l_cc * l_dd)) / l_den), 0.0f, l_tmax);
    l_t = coax_p_clampf((((l_aa * l_ee) - (l_bb * l_dd)) / l_den), 0.0f, 0x1.000000p0f);
  }
  l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_s)), coax_p_wg_add_f3f3(l_a, coax_p_wg_mul_f3f(l_ab, l_t))));
  l_r = (0x1.333334p-2f * (0x1.000000p0f - (l_t * 0x1.99999ap-1f)));
  l_core = exp((((-l_dist) * l_dist) / ((l_r * l_r) * 0x1.666666p-2f)));
  l_halo = (exp((((-l_dist) * l_dist) / ((l_r * l_r) * 0x1.000000p2f))) * 0x1.666666p-2f);
  l_diamonds = (0x1.000000p0f + ((0x1.000000p-1f * coax_pow_bg_vs((0x1.000000p-1f + (0x1.000000p-1f * cos(((l_t * 0x1.600000p4f) - ((coax_at_u(i32(2i)) * 0x1.e00000p4f) * coax_p_orbitleague_moving()))))), 0x1.800000p2f)) * (0x1.000000p0f - l_t)));
  return coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.ccccccp-1f, 0x1.f0a3d8p-1f, 0x1.000000p0f), l_core), l_diamonds), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p-2f, 0x1.000000p-1f, 0x1.000000p0f), l_halo)), (0x1.99999ap0f - l_t)), l_flick);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_orbitleague_space(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_c: vec3f;
  var l_band: f32;
  var l_wave: f32;
  var l_g: vec3f;
  var l_cell: vec3f;
  var l_h: f32;
  var l_at: vec3f;
  var l_r: f32;
  var l_tw: f32;
  var l_tint: vec3f;
  var l_sd: f32;
  var l_mc: vec3f;
  var l_mt: f32;
  var l_n: vec3f;
  var l_cr: f32;
  var l_mcol: vec3f;
  var l_pc: vec3f;
  var l_pt: f32;
  var l_n_20: vec3f;
  var l_l: f32;
  var l_lit: f32;
  var l_b: f32;
  var l_miss: f32;
  var l_rn: vec3f;
  var l_den: f32;
  var l_rt: f32;
  var l_rp: vec3f;
  var l_r_29: f32;
  var l_a: f32;
  var l_sb: f32;
  var l_shadowed: bool;
  var l_rl: f32;
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.89374cp-8f, 0x1.0624dep-7f, 0x1.47ae14p-6f), coax_p_v3(0x1.99999ap-6f, 0x1.47ae14p-6f, 0x1.c28f5cp-5f), coax_p_clampf((0x1.000000p-1f + (0x1.000000p-1f * l_d.y)), 0.0f, 0x1.000000p0f));
  l_band = coax_p_wg_dot3(l_d, coax_p_wg_normalize3(coax_p_v3(0x1.333334p-2f, 0x1.19999ap-1f, (-0x1.8f5c28p-1f))));
  l_wave = (0x1.19999ap-1f + ((0x1.ccccccp-2f * sin(((l_d.x * 0x1.c00000p2f) + (sin((l_d.z * 0x1.400000p2f)) * 0x1.000000p1f)))) * sin(((l_d.y * 0x1.200000p3f) + (l_d.z * 0x1.800000p1f)))));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-6f, 0x1.333334p-3f), coax_p_v3(0x1.47ae14p-6f, 0x1.70a3d8p-4f, 0x1.0a3d70p-3f), (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_d.x * 0x1.800000p1f) - (l_d.z * 0x1.000000p1f)))))), exp((((-l_band) * l_band) * 0x1.c00000p3f))), l_wave), 0x1.19999ap-1f));
  l_g = coax_p_wg_mul_f3f(l_d, 0x1.680000p7f);
  l_cell = coax_p_wg_floor_f3(l_g);
  l_h = coax_p_orbitleague_hash3(l_cell);
  if ((l_h > 0x1.eb851ep-1f))
  {
    l_at = coax_p_wg_add_f3f3(l_cell, coax_p_v3(coax_p_orbitleague_hash3(coax_p_wg_add_f3f(l_cell, 0x1.4cccccp0f)), coax_p_orbitleague_hash3(coax_p_wg_add_f3f(l_cell, 0x1.c66666p2f)), coax_p_orbitleague_hash3(coax_p_wg_add_f3f(l_cell, 0x1.d9999ap1f))));
    l_r = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_g, l_at));
    l_tw = (0x1.000000p0f - ((0x1.99999ap-3f * coax_p_orbitleague_moving()) * (0x1.000000p-1f + (0x1.000000p-1f * sin(((coax_at_u(i32(2i)) * (0x1.000000p-1f + (l_h * 0x1.800000p1f))) + (l_h * 0x1.900000p5f)))))));
    l_tint = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.800000p-1f, 0x1.b33334p-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.000000p0f), coax_p_fract((l_h * 0x1.280000p5f)));
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_tint, (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-5f, 0x1.1eb852p-2f, l_r))), select(0x1.000000p-1f, 0x1.000000p0f, (l_h > 0x1.fc6a7ep-1f))), l_tw));
  }
  l_sd = coax_p_wg_dot3(l_d, coax_p_orbitleague_sun());
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.dc28f6p-1f), (((coax_p_smoothstep(0x1.ffbe76p-1f, 0x1.ffe5cap-1f, l_sd) * 0x1.800000p1f) + (coax_pow_bg_vs(coax_p_fmax(l_sd, 0.0f), 0x1.900000p8f) * 0x1.666666p-1f)) + (coax_pow_bg_vs(coax_p_fmax(l_sd, 0.0f), 0x1.800000p4f) * 0x1.eb851ep-5f))));
  l_mc = coax_p_wg_mul_f3f(coax_p_wg_normalize3(coax_p_ORBITLEAGUE_MOON_D()), 0x1.f40000p10f);
  l_mt = coax_p_orbitleague_sphere_t(l_d, l_mc, 0x1.180000p6f);
  if ((l_mt > 0.0f))
  {
    l_n = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_mul_f3f(l_d, l_mt), l_mc));
    l_cr = coax_p_orbitleague_hash3(coax_p_wg_floor_f3(coax_p_wg_mul_f3f(l_n, 0x1.200000p3f)));
    l_mcol = coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.23d70ap-1f, 0x1.3d70a4p-1f), (0x1.b33334p-1f + (0x1.333334p-3f * l_cr)));
    l_mcol = coax_p_wg_mul_f3f(l_mcol, (0x1.000000p0f - ((0x1.000000p-2f * (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.333334p-2f, coax_p_wg_length3(coax_p_wg_sub_f3f(coax_p_wg_fract_f3(coax_p_wg_mul_f3f(l_n, 0x1.200000p3f)), 0x1.000000p-1f))))) * coax_p_stepf(0x1.333334p-1f, l_cr))));
    l_c = coax_p_wg_mul_f3f(l_mcol, (0x1.99999ap-5f + (0x1.e66666p-1f * coax_p_smoothstep((-0x1.99999ap-5f), 0x1.99999ap-3f, coax_p_wg_dot3(l_n, coax_p_orbitleague_sun())))));
  }
  l_pc = coax_p_orbitleague_planet_c();
  l_pt = coax_p_orbitleague_sphere_t(l_d, l_pc, 0x1.5e0000p9f);
  if ((l_pt > 0.0f))
  {
    l_n_20 = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_mul_f3f(l_d, l_pt), l_pc));
    l_l = coax_p_wg_dot3(l_n_20, coax_p_orbitleague_sun());
    l_lit = (0x1.99999ap-5f + (0x1.e66666p-1f * coax_p_smoothstep((-0x1.47ae14p-4f), 0x1.c28f5cp-3f, l_l)));
    l_c = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_orbitleague_planet_col(l_n_20), l_lit), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-2f, 0x1.333334p-1f, 0x1.000000p0f), coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(0.0f, coax_p_wg_dot3(l_n_20, coax_p_wg_neg_f3(l_d)))), 0x1.800000p1f)), 0x1.99999ap-1f), coax_p_smoothstep((-0x1.666666p-2f), 0x1.333334p-2f, l_l)));
  }
  else
  {
    l_b = coax_p_wg_dot3(l_pc, l_d);
    l_miss = (coax_p_wg_length3(coax_p_wg_sub_f3f3(l_pc, coax_p_wg_mul_f3f(l_d, l_b))) - 0x1.5e0000p9f);
    if (((l_b > 0.0f) && (l_miss > 0.0f)))
    {
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-2f, 0x1.333334p-1f, 0x1.000000p0f), exp(((-l_miss) / 0x1.900000p4f))), 0x1.ccccccp-2f));
    }
  }
  l_rn = coax_p_wg_normalize3(coax_p_ORBITLEAGUE_RING_N());
  l_den = coax_p_wg_dot3(l_d, l_rn);
  if ((abs(l_den) > 0x1.a36e2ep-14f))
  {
    l_rt = (coax_p_wg_dot3(l_pc, l_rn) / l_den);
    l_rp = coax_p_wg_sub_f3f3(coax_p_wg_mul_f3f(l_d, l_rt), l_pc);
    l_r_29 = (coax_p_wg_length3(l_rp) / 0x1.5e0000p9f);
    if (((((l_rt > 0.0f) && (l_r_29 > 0x1.59999ap0f)) && (l_r_29 < 0x1.200000p1f)) && ((l_pt < 0.0f) || (l_rt < l_pt))))
    {
      l_a = (0x1.666666p-2f + ((0x1.666666p-2f * sin((l_r_29 * 0x1.100000p5f))) * sin(((l_r_29 * 0x1.600000p3f) + 0x1.666666p-1f))));
      l_a = (l_a * ((coax_p_smoothstep(0x1.59999ap0f, 0x1.6b851ep0f, l_r_29) * (0x1.000000p0f - coax_p_smoothstep(0x1.133334p1f, 0x1.200000p1f, l_r_29))) * (0x1.000000p0f - (0x1.99999ap-1f * (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.eb851ep-6f, abs((l_r_29 - 0x1.c7ae14p0f))))))));
      l_sb = coax_p_wg_dot3(l_rp, coax_p_orbitleague_sun());
      l_shadowed = ((l_sb < 0.0f) && ((coax_p_wg_dot3(l_rp, l_rp) - (l_sb * l_sb)) < (0x1.5e0000p9f * 0x1.5e0000p9f)));
      l_rl = select((0x1.000000p-2f + (0x1.800000p-1f * abs(coax_p_wg_dot3(l_rn, coax_p_orbitleague_sun())))), 0x1.eb851ep-5f, l_shadowed);
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.7ae148p-1f, 0x1.8f5c28p-1f, 0x1.ccccccp-1f), l_rl), coax_p_clampf(l_a, 0.0f, 0x1.b33334p-1f));
    }
  }
  return l_c;
}
fn coax_p_orbitleague_glass(p_ro: vec3f, p_rd: vec3f, p_tmax: f32, p_HULL: array<vec4f, 45>) -> coax_t15 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_tmax: f32 = p_tmax;
  var l_HULL: array<vec4f, 45> = p_HULL;
  var l_hx: f32;
  var l_hz: f32;
  var l_hh: f32;
  var l_gw: f32;
  var l_gh: f32;
  var l_gd: f32;
  var l_out: coax_t15;
  var l_t: f32;
  var l_n: vec3f;
  var l_drum: bool;
  var l_s: f32;
  var l_s_17: f32;
  var l_s_18: f32;
  var l_s_19: f32;
  var l_s_20: f32;
  var l_p: vec3f;
  var l_mouth: bool;
  var l_uv: vec2f;
  var l_g: vec2f;
  var l_strut: f32;
  var l_e: f32;
  var l_side: vec3f;
  var l_fres: f32;
  var l_col: vec3f;
  var l_a: f32;
  var l_sh: f32;
  var l_add: vec3f;
  var l_rr: vec3f;
  var l_rh: coax_t14;
  var l_zs: f32;
  var l_tn: f32;
  var l_nn: vec3f;
  var l_back: f32;
  var l_s_39: f32;
  var l_s_40: f32;
  var l_s_41: f32;
  var l_q: vec3f;
  var l_uv_43: vec2f;
  var l_g_44: vec2f;
  var l_weave: f32;
  var l_col_46: vec3f;
  l_hx = coax_at_u(i32(80i));
  l_hz = coax_at_u(i32(81i));
  l_hh = coax_at_u(i32(82i));
  l_gw = coax_at_u(i32(83i));
  l_gh = coax_at_u(i32(84i));
  l_gd = coax_at_u(i32(85i));
  l_out = coax_t15(0x1.dcd650p29f, coax_p_v3(0.0f, 0.0f, 0.0f), 0.0f, coax_p_v3(0.0f, 0.0f, 0.0f));
  if (coax_p_orbitleague_in_sphere())
  {
    return l_out;
  }
  l_t = 0x1.dcd650p29f;
  l_n = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_drum = coax_p_orbitleague_in_drum();
  if (((!l_drum) && (l_rd.x > 0.0f)))
  {
    l_s = ((l_hx - l_ro.x) / l_rd.x);
    if ((l_s < l_t))
    {
      l_t = l_s;
      l_n = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
    }
  }
  if (((!l_drum) && (l_rd.x < 0.0f)))
  {
    l_s_17 = (((-l_hx) - l_ro.x) / l_rd.x);
    if ((l_s_17 < l_t))
    {
      l_t = l_s_17;
      l_n = coax_p_v3((-0x1.000000p0f), 0.0f, 0.0f);
    }
  }
  if (((!l_drum) && (l_rd.y > 0.0f)))
  {
    l_s_18 = ((l_hh - l_ro.y) / l_rd.y);
    if ((l_s_18 < l_t))
    {
      l_t = l_s_18;
      l_n = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
    }
  }
  if ((l_rd.z > 0.0f))
  {
    l_s_19 = ((l_hz - l_ro.z) / l_rd.z);
    if ((l_s_19 < l_t))
    {
      l_t = l_s_19;
      l_n = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
    }
  }
  if ((l_rd.z < 0.0f))
  {
    l_s_20 = (((-l_hz) - l_ro.z) / l_rd.z);
    if ((l_s_20 < l_t))
    {
      l_t = l_s_20;
      l_n = coax_p_v3(0.0f, 0.0f, (-0x1.000000p0f));
    }
  }
  if (((l_t > l_tmax) || (l_t > 0x1.7d7840p26f)))
  {
    return l_out;
  }
  l_p = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_t));
  l_mouth = (((l_n.z != 0.0f) && (abs(l_p.x) < l_gw)) && (l_p.y < l_gh));
  if ((!l_mouth))
  {
    l_uv = coax_p_v2(l_p.x, l_p.z);
    if ((l_n.x != 0.0f))
    {
      l_uv = coax_p_v2(l_p.z, l_p.y);
    }
    else
    {
      if ((l_n.z != 0.0f))
      {
        l_uv = coax_p_v2(l_p.x, l_p.y);
      }
    }
    l_g = coax_p_wg_mul_f2f(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(coax_p_wg_div_f2f(l_uv, 0x1.400000p2f), 0x1.000000p-1f)), 0x1.000000p-1f)), 0x1.400000p2f);
    l_strut = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.70a3d8p-4f, coax_p_fmin(l_g.x, l_g.y)));
    l_e = coax_p_fmin((l_hx - abs(l_p.x)), (l_hz - abs(l_p.z)));
    if ((l_n.x != 0.0f))
    {
      l_e = coax_p_fmin((l_hz - abs(l_p.z)), coax_p_fmin(l_p.y, (l_hh - l_p.y)));
    }
    if ((l_n.z != 0.0f))
    {
      l_e = coax_p_fmin((l_hx - abs(l_p.x)), coax_p_fmin(l_p.y, (l_hh - l_p.y)));
    }
    if (l_drum)
    {
      l_e = (coax_at_u(i32(86i)) - coax_p_wg_length2(coax_p_v2(l_p.x, (l_p.y - coax_at_u(i32(86i))))));
    }
    l_side = select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_p.z > 0.0f));
    l_fres = coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_rd, l_n))), 0x1.000000p2f);
    l_col = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.333334p-2f, 0x1.ccccccp-2f, 0x1.4cccccp-1f), l_side, 0x1.666666p-2f);
    l_a = ((0x1.1eb852p-5f + (0x1.333334p-2f * l_fres)) + (l_strut * 0x1.99999ap-3f));
    l_sh = coax_p_fract((((((l_uv.x * 0x1.99999ap-1f) + l_uv.y) * 0x1.70a3d8p-5f) + (l_n.x * 0x1.333334p-2f)) + (l_n.z * 0x1.333334p-1f)));
    l_a = (l_a + (0x1.1eb852p-4f * ((coax_p_smoothstep(0.0f, 0x1.eb851ep-6f, l_sh) * (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.eb851ep-4f, l_sh))) + ((0x1.000000p-1f * coax_p_smoothstep(0x1.99999ap-3f, 0x1.c28f5cp-3f, l_sh)) * (0x1.000000p0f - coax_p_smoothstep(0x1.c28f5cp-3f, 0x1.0a3d70p-2f, l_sh))))));
    if ((l_n.z != 0.0f))
    {
      l_col = coax_p_wg_mix_f3f3f(l_col, l_side, 0x1.99999ap-2f);
      l_a = (l_a + 0x1.eb851ep-6f);
    }
    l_add = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_side, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.000000p-2f), exp((((-l_e) * l_e) / 0x1.eb851ep-6f))), 0x1.ccccccp-1f);
    l_rr = coax_p_wg_reflect3(l_rd, l_n);
    l_rh = coax_p_orbitleague_movers(coax_p_wg_sub_f3f3(l_p, coax_p_wg_mul_f3f(l_n, 0x1.47ae14p-6f)), l_rr, l_HULL, false);
    if ((l_rh.x < 0x1.400000p6f))
    {
      l_add = coax_p_wg_add_f3f3(l_add, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_orbitleague_shade_hit(l_rh, l_rr), (0x1.47ae14p-3f + (0x1.99999ap-2f * l_fres))), exp(((-l_rh.x) * 0x1.47ae14p-6f))));
    }
    return coax_t15(l_t, l_col, coax_p_clampf(l_a, 0.0f, 0x1.000000p0f), l_add);
  }
  l_zs = coax_p_wg_sign(l_n.z);
  l_tn = 0x1.dcd650p29f;
  l_nn = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_back = (((l_zs * (l_hz + l_gd)) - l_ro.z) / l_rd.z);
  if ((l_back > l_t))
  {
    l_tn = l_back;
    l_nn = coax_p_v3(0.0f, 0.0f, l_zs);
  }
  if ((l_rd.x > 0.0f))
  {
    l_s_39 = ((l_gw - l_ro.x) / l_rd.x);
    if (((l_s_39 > l_t) && (l_s_39 < l_tn)))
    {
      l_tn = l_s_39;
      l_nn = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
    }
  }
  if ((l_rd.x < 0.0f))
  {
    l_s_40 = (((-l_gw) - l_ro.x) / l_rd.x);
    if (((l_s_40 > l_t) && (l_s_40 < l_tn)))
    {
      l_tn = l_s_40;
      l_nn = coax_p_v3((-0x1.000000p0f), 0.0f, 0.0f);
    }
  }
  if ((l_rd.y > 0.0f))
  {
    l_s_41 = ((l_gh - l_ro.y) / l_rd.y);
    if (((l_s_41 > l_t) && (l_s_41 < l_tn)))
    {
      l_tn = l_s_41;
      l_nn = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
    }
  }
  if ((l_tn > l_tmax))
  {
    return l_out;
  }
  l_q = coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_tn));
  l_uv_43 = coax_p_v2(l_q.x, l_q.y);
  if ((l_nn.x != 0.0f))
  {
    l_uv_43 = coax_p_v2(l_q.z, l_q.y);
  }
  else
  {
    if ((l_nn.y != 0.0f))
    {
      l_uv_43 = coax_p_v2(l_q.x, l_q.z);
    }
  }
  l_g_44 = coax_p_wg_mul_f2f(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(coax_p_wg_div_f2f(l_uv_43, 0x1.333334p-1f), 0x1.000000p-1f)), 0x1.000000p-1f)), 0x1.333334p-1f);
  l_weave = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-7f, 0x1.47ae14p-5f, coax_p_fmin(l_g_44.x, l_g_44.y)));
  l_col_46 = select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_zs > 0.0f));
  return coax_t15(l_tn, coax_p_wg_mul_f3f(l_col_46, 0x1.333334p0f), coax_p_clampf((0x1.eb851ep-4f + (l_weave * 0x1.4cccccp-1f)), 0.0f, 0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f));
}
fn coax_p_orbitleague_trail(p_ro: vec3f, p_rd: vec3f, p_k: i32, p_tmax: f32, p_width: f32, p_col: vec3f) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_k: i32 = p_k;
  var l_tmax: f32 = p_tmax;
  var l_width: f32 = p_width;
  var l_col: vec3f = p_col;
  var l_b: vec4f;
  var l_m: vec3f;
  var l_bq: f32;
  var l_o: vec3f;
  var l_i: i32;
  var l_p0: vec4f;
  var l_p1: vec4f;
  var l_ab: vec3f;
  var l_w0: vec3f;
  var l_bb: f32;
  var l_cc: f32;
  var l_dd: f32;
  var l_ee: f32;
  var l_den: f32;
  var l_t: f32;
  var l_q: vec3f;
  var l_s: f32;
  var l_dist: f32;
  var l_age: f32;
  var l_r: f32;
  l_b = coax_p_wg_at4(((i32((l_k * 9i)) * 4i) + 220i));
  if ((l_b.w < 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_m = coax_p_wg_sub_f3f3(l_ro, coax_p_v3(l_b.x, l_b.y, l_b.z));
  l_bq = coax_p_wg_dot3(l_m, l_rd);
  if (((coax_p_wg_dot3(l_m, l_m) - (l_bq * l_bq)) > (l_b.w * l_b.w)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_i = 0i;
    while ((l_i < 7i))
    {
      l_p0 = coax_p_wg_at4(((i32((((l_k * 9i) + 1i) + l_i)) * 4i) + 220i));
      l_p1 = coax_p_wg_at4(((i32((((l_k * 9i) + 2i) + l_i)) * 4i) + 220i));
      if (((l_p0.w <= 0.0f) && (l_p1.w <= 0.0f)))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_ab = coax_p_wg_sub_f3f3(coax_p_v3(l_p1.x, l_p1.y, l_p1.z), coax_p_v3(l_p0.x, l_p0.y, l_p0.z));
      l_w0 = coax_p_wg_sub_f3f3(l_ro, coax_p_v3(l_p0.x, l_p0.y, l_p0.z));
      l_bb = coax_p_wg_dot3(l_rd, l_ab);
      l_cc = coax_p_wg_dot3(l_ab, l_ab);
      l_dd = coax_p_wg_dot3(l_rd, l_w0);
      l_ee = coax_p_wg_dot3(l_ab, l_w0);
      l_den = (l_cc - (l_bb * l_bb));
      l_t = 0.0f;
      if ((l_den > 0x1.0c6f7ap-20f))
      {
        l_t = coax_p_clampf(((l_ee - (l_bb * l_dd)) / l_den), 0.0f, 0x1.000000p0f);
      }
      l_q = coax_p_wg_add_f3f3(coax_p_v3(l_p0.x, l_p0.y, l_p0.z), coax_p_wg_mul_f3f(l_ab, l_t));
      l_s = coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_q, l_ro), l_rd);
      if (((l_s < 0.0f) || (l_s > l_tmax)))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_s)), l_q));
      l_age = ((f32(l_i) + l_t) / 0x1.c00000p2f);
      l_r = (l_width * (0x1.000000p0f - (0x1.666666p-1f * l_age)));
      l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_col, ((exp((((-l_dist) * l_dist) / ((l_r * l_r) * 0x1.99999ap-3f))) * 0x1.ccccccp-1f) + (exp((((-l_dist) * l_dist) / (l_r * l_r))) * 0x1.333334p-2f))), coax_p_mix(l_p0.w, l_p1.w, l_t)), (0x1.000000p0f - l_age)));
      l_i = (l_i + 1i);
    }
  }
  return l_o;
}
fn coax_p_orbitleague_blast(p_ro: vec3f, p_rd: vec3f, p_tmax: f32) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_tmax: f32 = p_tmax;
  var l_age: f32;
  var l_c: vec3f;
  var l_r: f32;
  var l_s: f32;
  var l_dd: f32;
  var l_rim: f32;
  var l_col: vec3f;
  var l_heart: f32;
  l_age = coax_at_u(i32(331i));
  if ((l_age < 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_c = coax_p_v3(coax_at_u(i32(328i)), coax_at_u(i32(329i)), coax_at_u(i32(330i)));
  l_r = coax_p_orbitleague_blast_r(l_age);
  l_s = coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_c, l_ro), l_rd);
  l_dd = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_s)), l_c));
  if ((((l_s < (-l_r)) || (l_dd > (l_r + 0x1.800000p2f))) || ((l_s - l_r) > l_tmax)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_rim = exp((-(((l_r - l_dd) / (0x1.000000p0f + (l_r * 0x1.333334p-3f))) * ((l_r - l_dd) / (0x1.000000p0f + (l_r * 0x1.333334p-3f))))));
  l_col = coax_p_wg_mix_f3f3f(coax_p_orbitleague_team(coax_at_u(i32(332i))), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.333334p-2f);
  l_heart = ((exp((((-l_dd) * l_dd) / (0x1.000000p3f + (l_age * 0x1.e00000p5f)))) * coax_p_fmax(0.0f, (0x1.000000p0f - (l_age * 0x1.99999ap0f)))) * 0x1.400000p1f);
  return coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_col, ((l_rim * 0x1.19999ap1f) + (0x1.99999ap-3f * coax_p_stepf(l_dd, l_r)))), coax_p_orbitleague_blast_fade(l_age)), coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_col, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.333334p-1f), l_heart));
}
fn coax_p_orbitleague_pad_orbs(p_ro: vec3f, p_rd: vec3f, p_tmax: f32) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_tmax: f32 = p_tmax;
  var l_o: vec3f;
  var l_i: i32;
  var l_pd: vec4f;
  var l_bob: f32;
  var l_at: vec3f;
  var l_f: vec3f;
  var l_f_10: vec3f;
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_i = 0i;
    while ((l_i < 6i))
    {
      l_pd = coax_p_wg_at4(((i32(l_i) * 4i) + 336i));
      if ((l_pd.w < 0x1.ff7ceep-1f))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_bob = (0x1.4cccccp0f + ((0x1.333334p-3f * coax_p_orbitleague_moving()) * sin(((coax_at_u(i32(2i)) * 0x1.000000p1f) + f32(l_i)))));
      l_at = coax_p_v3(l_pd.x, l_bob, l_pd.y);
      if (coax_p_orbitleague_in_drum())
      {
        l_f = coax_p_orbitleague_drum_at(l_pd.x, l_pd.y);
        l_at = coax_p_wg_add_f3f3(l_f, coax_p_wg_mul_f3f(coax_p_orbitleague_drum_n(l_f), l_bob));
      }
      if (coax_p_orbitleague_in_sphere())
      {
        l_f_10 = coax_p_orbitleague_sph_at(l_pd.x, l_pd.y);
        l_at = coax_p_wg_add_f3f3(l_f_10, coax_p_wg_mul_f3f(coax_p_orbitleague_sph_n(l_f_10), l_bob));
      }
      l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_ORBITLEAGUE_PAD_COL(), ((coax_p_orbitleague_halo(l_ro, l_rd, l_at, l_tmax, 0x1.666666p-2f) * 0x1.666666p0f) + (coax_p_orbitleague_halo(l_ro, l_rd, l_at, l_tmax, 0x1.000000p0f) * 0x1.000000p-2f))));
      l_i = (l_i + 1i);
    }
  }
  return l_o;
}
fn coax_p_orbitleague_lights(p_ro: vec3f, p_rd: vec3f, p_c: coax_t16, p_tmax: f32) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: coax_t16 = p_c;
  var l_tmax: f32 = p_tmax;
  var l_f: vec3f;
  var l_m: vec3f;
  var l_along: f32;
  var l_front: f32;
  var l_back: f32;
  var l_o: vec3f;
  var l_s: f32;
  var l_hp: vec3f;
  var l_tp: vec3f;
  l_f = coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z);
  l_m = coax_p_wg_sub_f3f3(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z), l_ro);
  l_along = coax_p_wg_dot3(l_m, l_rd);
  if ((((l_along < 0.0f) || ((coax_p_wg_dot3(l_m, l_m) - (l_along * l_along)) > 0x1.200000p3f)) || (l_along > 0x1.680000p6f)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_front = coax_p_fmax(0.0f, (-coax_p_wg_dot3(l_rd, l_f)));
  l_back = coax_p_fmax(0.0f, coax_p_wg_dot3(l_rd, l_f));
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_s = (-0x1.000000p0f);
    while ((l_s <= 0x1.000000p0f))
    {
      l_hp = coax_p_wg_add_f3f3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z), coax_p_wg_mul_f3f(coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z), (l_s * 0x1.000000p-1f))), coax_p_wg_mul_f3f(coax_p_v3(l_c.z.x, l_c.z.y, l_c.z.z), 0x1.c28f5cp-4f)), coax_p_wg_mul_f3f(l_f, 0x1.147ae2p1f));
      l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.800000p-1f, 0x1.c28f5cp-1f, 0x1.000000p0f), coax_p_orbitleague_halo(l_ro, l_rd, l_hp, l_tmax, 0x1.000000p-2f)), l_front), l_front), 0x1.333334p0f));
      l_s = (l_s + 0x1.000000p1f);
    }
  }
  l_tp = coax_p_wg_sub_f3f3(coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z), coax_p_wg_mul_f3f(l_f, 0x1.0cccccp1f));
  l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_orbitleague_team(coax_p_orbitleague_car_side(l_c)), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.333334p-2f), coax_p_orbitleague_halo(l_ro, l_rd, l_tp, l_tmax, 0x1.666666p-2f)), l_back), 0x1.000000p-1f));
  return l_o;
}
fn coax_p_orbitleague_rib_lights(p_ro: vec3f, p_rd: vec3f, p_tmax: f32) -> vec3f {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_tmax: f32 = p_tmax;
  var l_hx: f32;
  var l_hh: f32;
  var l_o: vec3f;
  var l_k: i32;
  var l_z: f32;
  var l_blink: f32;
  var l_s: f32;
  l_hx = coax_at_u(i32(80i));
  l_hh = coax_at_u(i32(82i));
  l_o = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_z = ((-0x1.e00000p4f) + (f32(l_k) * 0x1.400000p4f));
      l_blink = select(0x1.000000p0f, (0x1.000000p-2f + (0x1.800000p-1f * coax_p_stepf(0x1.000000p-1f, coax_p_fract(((coax_at_u(i32(2i)) * 0x1.000000p-1f) + (f32(l_k) * 0x1.000000p-2f)))))), (coax_at_u(i32(4i)) < 0x1.000000p-1f));
      {
        l_s = (-0x1.000000p0f);
        while ((l_s <= 0x1.000000p0f))
        {
          l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-1f, 0x1.000000p0f, 0x1.e66666p-1f), coax_p_orbitleague_halo(l_ro, l_rd, coax_p_v3((l_s * (l_hx + 0x1.e66666p-1f)), (l_hh + 0x1.800000p0f), l_z), l_tmax, 0x1.666666p-2f)), l_blink));
          l_s = (l_s + 0x1.000000p1f);
        }
      }
      l_k = (l_k + 1i);
    }
  }
  return l_o;
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_orbitleague_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 16i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_orbitleague_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 92i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 92i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
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
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_orbitleague_deck_col(p_p: vec3f, p_hx: f32) -> vec3f {
  var l_p: vec3f = p_p;
  var l_hx: f32 = p_hx;
  var l_hz: f32;
  var l_gw: f32;
  var l_c: vec3f;
  var l_g: vec2f;
  var l_line: f32;
  var l_box: vec2f;
  var l_side: vec3f;
  var l_edge: f32;
  l_hz = coax_at_u(i32(81i));
  l_gw = coax_at_u(i32(83i));
  l_c = select(coax_p_v3(0x1.333334p-4f, 0x1.70a3d8p-4f, 0x1.147ae2p-3f), coax_p_v3(0x1.c28f5cp-4f, 0x1.333334p-4f, 0x1.000000p-3f), (l_p.z > 0.0f));
  l_g = coax_p_wg_mul_f2f(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(coax_p_wg_div_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.400000p2f), 0x1.000000p-1f)), 0x1.000000p-1f)), 0x1.400000p2f);
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.000000p0f - (0x1.000000p-2f * (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.1eb852p-4f, coax_p_fmin(l_g.x, l_g.y))))));
  l_line = 0.0f;
  l_line = coax_p_fmax(l_line, (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.47ae14p-3f, abs(l_p.z))));
  l_line = coax_p_fmax(l_line, (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.47ae14p-3f, abs((coax_p_wg_length2(coax_p_v2(l_p.x, l_p.z)) - 0x1.200000p3f)))));
  l_box = coax_p_v2((abs(l_p.x) - (l_gw + 0x1.400000p2f)), (abs(l_p.z) - (l_hz - 0x1.800000p3f)));
  l_line = coax_p_fmax(l_line, (((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.47ae14p-3f, abs(l_box.y))) * coax_p_stepf(abs(l_p.x), (l_gw + 0x1.400000p2f))) * coax_p_stepf(abs(l_p.z), l_hz)));
  l_line = coax_p_fmax(l_line, (((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.47ae14p-3f, abs(l_box.x))) * coax_p_stepf((l_hz - 0x1.800000p3f), abs(l_p.z))) * coax_p_stepf(abs(l_p.z), l_hz)));
  l_line = coax_p_fmax(l_line, ((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.47ae14p-3f, abs((abs(l_p.z) - l_hz)))) * coax_p_stepf(abs(l_p.x), l_gw)));
  l_side = select(coax_p_ORBITLEAGUE_BLUE(), coax_p_ORBITLEAGUE_MAGENTA(), (l_p.z > 0.0f));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mix_f3f3f(coax_p_v3(0x1.b33334p-1f, 0x1.ccccccp-1f, 0x1.000000p0f), l_side, 0x1.666666p-2f), (l_line * 0x1.b33334p-1f));
  l_edge = coax_p_fmin((l_hx - abs(l_p.x)), (l_hz - abs(l_p.z)));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_side, (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-1f, l_edge))), 0x1.333334p-1f), coax_p_stepf(l_gw, (abs(l_p.x) + (coax_p_stepf(abs(l_p.z), l_hz) * 0x1.8c0000p6f)))));
  return l_c;
}
fn coax_p_ORBITLEAGUE_MAGENTA() -> vec3f {
  return coax_p_v3(0x1.000000p0f, 0x1.c28f5cp-3f, 0x1.70a3d8p-1f);
}
fn coax_p_ORBITLEAGUE_BLUE() -> vec3f {
  return coax_p_v3(0x1.70a3d8p-3f, 0x1.eb851ep-2f, 0x1.000000p0f);
}
fn coax_p_orbitleague_sph_c() -> vec3f {
  return coax_p_v3(0.0f, coax_p_orbitleague_sr(), 0.0f);
}
fn coax_p_orbitleague_sr() -> f32 {
  return (-coax_at_u(i32(86i)));
}
fn coax_p_wg_sw_xy_f3(p_p: vec3f) -> vec2f {
  var l_p: vec3f = p_p;
  return coax_p_v2(l_p.x, l_p.y);
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
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
fn coax_p_orbitleague_ball_hit(p_ro: vec3f, p_rd: vec3f) -> coax_t14 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_m: vec3f;
  var l_b: f32;
  var l_c: f32;
  var l_disc: f32;
  var l_t: f32;
  var l_n: vec3f;
  var l_l: vec3f;
  var l_a: vec3f;
  var l_seam: f32;
  var l_pole: f32;
  var l_col: vec3f;
  l_m = coax_p_wg_sub_f3f3(l_ro, coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i))));
  l_b = coax_p_wg_dot3(l_m, l_rd);
  l_c = (coax_p_wg_dot3(l_m, l_m) - (coax_at_u(i32(43i)) * coax_at_u(i32(43i))));
  l_disc = ((l_b * l_b) - l_c);
  if ((l_disc < 0.0f))
  {
    return coax_p_orbitleague_miss();
  }
  l_t = ((-l_b) - sqrt(l_disc));
  if ((l_t < 0.0f))
  {
    return coax_p_orbitleague_miss();
  }
  l_n = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_m, coax_p_wg_mul_f3f(l_rd, l_t)));
  l_l = coax_p_orbitleague_qrot(coax_p_wg_make_f4_f3f(coax_p_wg_neg_f3(coax_p_v3(coax_at_u(i32(44i)), coax_at_u(i32(45i)), coax_at_u(i32(46i)))), coax_at_u(i32(47i))), l_n);
  l_a = coax_p_wg_abs_f3(l_l);
  l_seam = coax_p_fmin(coax_p_fmin(l_a.x, l_a.y), l_a.z);
  l_pole = coax_p_fmax(coax_p_fmax(l_a.x, l_a.y), l_a.z);
  l_col = coax_p_v3(0x1.ccccccp-1f, 0x1.d70a3ep-1f, 0x1.eb851ep-1f);
  if ((l_pole > 0x1.ee147ap-1f))
  {
    l_col = coax_p_v3(0x1.333334p-2f, 0x1.70a3d8p-2f, 0x1.eb851ep-2f);
  }
  return coax_t14(l_t, l_n, l_col, (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.c28f5cp-5f, l_seam)), 4i);
}
fn coax_p_orbitleague_car_hit(p_ro: vec3f, p_rd: vec3f, p_c: coax_t16, p_HULL: array<vec4f, 45>, p_seen: bool) -> coax_t14 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: coax_t16 = p_c;
  var l_HULL: array<vec4f, 45> = p_HULL;
  var l_seen: bool = p_seen;
  var l_q: vec3f;
  var l_o: vec3f;
  var l_d: vec3f;
  var l_bq: f32;
  var l_oo: f32;
  var l_side: f32;
  var l_accent: vec3f;
  var l_pk: i32;
  var l_paint: vec3f;
  var l_decal: i32;
  var l_h: coax_t14;
  var l_meshed: bool;
  var l_m: coax_t17;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_col: vec3f;
  var l_mat: i32;
  var l_b: coax_t18;
  var l_p_24: vec3f;
  var l_n_25: vec3f;
  var l_col_26: vec3f;
  var l_mat_27: i32;
  var l_i: i32;
  var l_sx: f32;
  var l_wz: f32;
  var l_f: coax_t18;
  var l_w: coax_t18;
  var l_s: f32;
  var l_k: coax_t18;
  var l_hc: vec3f;
  var l_ht: f32;
  var l_n_37: vec3f;
  var l_col_38: vec3f;
  var l_mat_39: i32;
  var l_steer: f32;
  var l_i_41: i32;
  var l_wx: f32;
  var l_x: f32;
  var l_wz_44: f32;
  var l_z: f32;
  var l_wo: vec3f;
  var l_wd: vec3f;
  var l_cs: f32;
  var l_sn: f32;
  var l_wh: coax_t14;
  var l_cs_51: f32;
  var l_sn_52: f32;
  l_q = coax_p_wg_sub_f3f3(l_ro, coax_p_v3(l_c.x.x, l_c.x.y, l_c.x.z));
  l_o = coax_p_v3(coax_p_wg_dot3(l_q, coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z)), coax_p_wg_dot3(l_q, coax_p_v3(l_c.z.x, l_c.z.y, l_c.z.z)), coax_p_wg_dot3(l_q, coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z)));
  l_d = coax_p_v3(coax_p_wg_dot3(l_rd, coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z)), coax_p_wg_dot3(l_rd, coax_p_v3(l_c.z.x, l_c.z.y, l_c.z.z)), coax_p_wg_dot3(l_rd, coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z)));
  l_bq = coax_p_wg_dot3(l_o, l_d);
  l_oo = coax_p_wg_dot3(l_o, l_o);
  if ((((l_oo - (l_bq * l_bq)) > 0x1.b33334p2f) || ((l_bq > 0.0f) && (l_oo > 0x1.b33334p2f))))
  {
    return coax_p_orbitleague_miss();
  }
  l_side = coax_p_orbitleague_car_side(l_c);
  l_accent = coax_p_orbitleague_team(l_side);
  l_pk = coax_p_orbitleague_car_paint(l_c);
  l_paint = coax_p_orbitleague_paint_colour((l_pk % 8i), l_side);
  l_decal = (l_pk / 8i);
  l_h = coax_p_orbitleague_miss();
  l_meshed = (l_seen && (mesh[0i] > 0x1.000000p-1f));
  if (l_meshed)
  {
    l_m = coax_p_orbitleague_mesh_cast(l_o, l_d);
    if ((l_m.x < 0x1.7d7840p26f))
    {
      l_p = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_m.x));
      l_n = l_m.y;
      l_col = l_m.z;
      l_mat = 0i;
      if ((l_m.z.x < 0.0f))
      {
        l_col = coax_p_wg_mul_f3f(l_paint, (0x1.800000p-1f + (0x1.000000p-2f * coax_p_clampf(l_m.z.y, 0.0f, 0x1.000000p0f))));
        l_mat = 1i;
        if ((((l_decal == 1i) && (l_n.y > 0x1.000000p-1f)) && (abs((abs(l_p.x) - 0x1.0a3d70p-2f)) < 0x1.1eb852p-4f)))
        {
          l_col = l_accent;
        }
        if ((((l_decal == 2i) && (abs(l_n.x) > 0x1.000000p-1f)) && ((coax_p_wg_i32_f((floor((l_p.z * 0x1.800000p1f)) + floor(((l_p.y * 0x1.800000p1f) + 0x1.400000p3f)))) & 1i) == 0i)))
        {
          l_col = l_accent;
        }
        if (((l_decal == 3i) && (l_p.z > 0x1.666666p-2f)))
        {
          l_col = l_accent;
        }
      }
      else
      {
        if (((l_m.z.x > 0x1.99999ap-3f) && (l_m.z.x > (0x1.800000p1f * l_m.z.y))))
        {
          l_col = coax_p_wg_mul_f3f(l_m.z, 0x1.400000p1f);
          l_mat = 3i;
        }
        else
        {
          if (((coax_p_fmax(coax_p_fmax(l_m.z.x, l_m.z.y), l_m.z.z) < 0x1.99999ap-5f) && (l_n.y > 0x1.000000p-2f)))
          {
            l_mat = 2i;
          }
        }
      }
      l_h = coax_t14(l_m.x, l_n, l_col, 0.0f, l_mat);
    }
  }
  else
  {
    l_b = coax_p_orbitleague_hull_cast(l_o, l_d, 0i, 18i, l_HULL);
    if ((l_b.x < l_h.x))
    {
      l_p_24 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_b.x));
      l_n_25 = l_b.y;
      l_col_26 = l_paint;
      l_mat_27 = 1i;
      if ((((l_decal == 1i) && (l_n_25.y > 0x1.000000p-1f)) && (abs((abs(l_p_24.x) - 0x1.0a3d70p-2f)) < 0x1.1eb852p-4f)))
      {
        l_col_26 = l_accent;
      }
      if ((((l_decal == 2i) && (abs(l_n_25.x) > 0x1.000000p-1f)) && ((coax_p_wg_i32_f((floor((l_p_24.z * 0x1.800000p1f)) + floor(((l_p_24.y * 0x1.800000p1f) + 0x1.400000p3f)))) & 1i) == 0i)))
      {
        l_col_26 = l_accent;
      }
      if (((l_decal == 3i) && (l_p_24.z > 0x1.666666p-2f)))
      {
        l_col_26 = l_accent;
      }
      if (((l_p_24.y < (-0x1.99999ap-3f)) && (l_n_25.y < 0x1.000000p-1f)))
      {
        l_col_26 = coax_p_v3(0x1.99999ap-5f, 0x1.c28f5cp-5f, 0x1.1eb852p-4f);
        l_mat_27 = 0i;
      }
      if (((((l_n_25.y > 0x1.ccccccp-1f) && (abs(l_p_24.x) < 0x1.333334p-1f)) && (l_p_24.z > (-0x1.000000p0f))) && (l_p_24.z < 0x1.19999ap-1f)))
      {
        l_col_26 = coax_p_v3(0x1.1eb852p-5f, 0x1.47ae14p-5f, 0x1.c28f5cp-5f);
        l_mat_27 = 0i;
      }
      if ((((l_n_25.y > 0x1.000000p-1f) && (abs(l_p_24.x) < 0x1.0a3d70p-3f)) && (l_p_24.z > 0x1.333334p-1f)))
      {
        l_col_26 = coax_p_v3(0x1.d70a3ep-1f, 0x1.e147aep-1f, 0x1.000000p0f);
      }
      if ((((((l_n_25.z > 0x1.333334p-2f) && (l_p_24.z > 0x1.b33334p0f)) && (l_p_24.y > (-0x1.5c28f6p-3f))) && (l_p_24.y < (-0x1.99999ap-5f))) && (abs(l_p_24.x) > 0x1.99999ap-3f)))
      {
        l_col_26 = coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.dc28f6p-1f, 0x1.000000p0f), 0x1.ccccccp0f);
        l_mat_27 = 3i;
      }
      if ((((l_n_25.z < (-0x1.99999ap-1f)) && (abs(l_p_24.y) < 0x1.1eb852p-5f)) && (abs(l_p_24.x) < 0x1.99999ap-1f)))
      {
        l_col_26 = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_accent, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), 0x1.ccccccp-2f), 0x1.ccccccp0f);
        l_mat_27 = 3i;
      }
      l_h = coax_t14(l_b.x, l_n_25, l_col_26, l_b.z, l_mat_27);
    }
    {
      l_i = 0i;
      while ((l_i < 4i))
      {
        l_sx = select((-0x1.000000p0f), 0x1.000000p0f, ((l_i & 1i) == 1i));
        l_wz = select(0x1.666666p0f, (-0x1.666666p0f), (l_i >= 2i));
        l_f = coax_p_orbitleague_hull_cast(coax_p_v3(((l_o.x * l_sx) - 0x1.b851ecp-1f), l_o.y, (l_o.z - l_wz)), coax_p_v3((l_d.x * l_sx), l_d.y, l_d.z), 18i, 11i, l_HULL);
        if ((l_f.x < l_h.x))
        {
          l_h = coax_t14(l_f.x, coax_p_v3((l_f.y.x * l_sx), l_f.y.y, l_f.y.z), l_paint, l_f.z, 1i);
        }
        l_i = (l_i + 1i);
      }
    }
    l_w = coax_p_orbitleague_hull_cast(l_o, l_d, 29i, 8i, l_HULL);
    if ((l_w.x < l_h.x))
    {
      l_h = coax_t14(l_w.x, l_w.y, coax_p_v3(0x1.47ae14p-6f, 0x1.1eb852p-5f, 0x1.1eb852p-4f), l_w.z, 2i);
    }
    {
      l_s = (-0x1.000000p0f);
      while ((l_s <= 0x1.000000p0f))
      {
        l_k = coax_p_orbitleague_hull_cast(coax_p_v3(((l_o.x * l_s) - 0x1.47ae14p-2f), l_o.y, l_o.z), coax_p_v3((l_d.x * l_s), l_d.y, l_d.z), 37i, 8i, l_HULL);
        if ((l_k.x < l_h.x))
        {
          l_h = coax_t14(l_k.x, coax_p_v3((l_k.y.x * l_s), l_k.y.y, l_k.y.z), l_paint, l_k.z, 1i);
        }
        l_s = (l_s + 0x1.000000p1f);
      }
    }
    l_hc = coax_p_v3(0x1.47ae14p-2f, 0x1.99999ap-2f, (-0x1.c28f5cp-3f));
    l_ht = coax_p_orbitleague_sphere_t(l_d, coax_p_wg_sub_f3f3(l_hc, l_o), 0x1.99999ap-3f);
    if (((l_ht > 0.0f) && (l_ht < l_h.x)))
    {
      l_n_37 = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_ht)), l_hc));
      l_col_38 = coax_p_v3(0x1.d70a3ep-1f, 0x1.dc28f6p-1f, 0x1.e66666p-1f);
      l_mat_39 = 0i;
      if ((((l_n_37.z > 0x1.333334p-2f) && (l_n_37.y < 0x1.000000p-1f)) && (l_n_37.y > (-0x1.666666p-2f))))
      {
        l_col_38 = coax_p_v3(0x1.eb851ep-6f, 0x1.eb851ep-5f, 0x1.eb851ep-4f);
        l_mat_39 = 2i;
      }
      else
      {
        if (((abs(l_n_37.x) < 0x1.eb851ep-4f) && (l_n_37.y > 0.0f)))
        {
          l_col_38 = l_accent;
          l_mat_39 = 1i;
        }
      }
      l_h = coax_t14(l_ht, l_n_37, l_col_38, 0.0f, l_mat_39);
    }
    l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_o, l_d, coax_p_v3(0.0f, 0x1.b851ecp-2f, (-0x1.dc28f6p0f)), coax_p_v3(0x1.ccccccp-1f, 0x1.eb851ep-6f, 0x1.5c28f6p-3f), coax_p_v3(0x1.1eb852p-4f, 0x1.333334p-4f, 0x1.70a3d8p-4f), 1i));
    l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_o, l_d, coax_p_v3(0.0f, 0x1.9eb852p-2f, (-0x1.dc28f6p0f)), coax_p_v3(0x1.d70a3ep-1f, 0x1.47ae14p-7f, 0x1.666666p-3f), l_accent, 1i));
    l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_o, l_d, coax_p_v3((-0x1.000000p-1f), 0x1.0a3d70p-2f, (-0x1.dc28f6p0f)), coax_p_v3(0x1.eb851ep-6f, 0x1.333334p-3f, 0x1.1eb852p-4f), coax_p_v3(0x1.1eb852p-4f, 0x1.333334p-4f, 0x1.70a3d8p-4f), 0i));
    l_h = coax_p_orbitleague_closer(l_h, coax_p_orbitleague_box_hit(l_o, l_d, coax_p_v3(0x1.000000p-1f, 0x1.0a3d70p-2f, (-0x1.dc28f6p0f)), coax_p_v3(0x1.eb851ep-6f, 0x1.333334p-3f, 0x1.1eb852p-4f), coax_p_v3(0x1.1eb852p-4f, 0x1.333334p-4f, 0x1.70a3d8p-4f), 0i));
  }
  l_steer = l_c.w.w;
  {
    l_i_41 = 0i;
    while ((l_i_41 < 4i))
    {
      l_wx = select(0x1.b851ecp-1f, 0x1.8dd2f2p-1f, l_meshed);
      l_x = select((-l_wx), l_wx, ((l_i_41 & 1i) == 1i));
      l_wz_44 = select(0x1.666666p0f, 0x1.5ef9dcp0f, l_meshed);
      l_z = select(l_wz_44, (-l_wz_44), (l_i_41 >= 2i));
      l_wo = coax_p_wg_sub_f3f3(l_o, coax_p_v3(l_x, select((-0x1.28f5c2p-1f), (-0x1.5db22ep-1f), l_meshed), l_z));
      l_wd = l_d;
      if ((l_i_41 < 2i))
      {
        l_cs = cos((-l_steer));
        l_sn = sin((-l_steer));
        l_wo = coax_p_v3(((l_cs * l_wo.x) - (l_sn * l_wo.z)), l_wo.y, ((l_sn * l_wo.x) + (l_cs * l_wo.z)));
        l_wd = coax_p_v3(((l_cs * l_wd.x) - (l_sn * l_wd.z)), l_wd.y, ((l_sn * l_wd.x) + (l_cs * l_wd.z)));
      }
      l_wh = coax_p_orbitleague_wheel_hit(l_wo, l_wd, coax_p_v3(0.0f, 0.0f, 0.0f), select(0x1.99999ap-2f, 0x1.3020c4p-2f, l_meshed), select(0x1.333334p-3f, 0x1.041894p-3f, l_meshed), l_c.z.w);
      if (((l_wh.x < l_h.x) && (l_i_41 < 2i)))
      {
        l_cs_51 = cos(l_steer);
        l_sn_52 = sin(l_steer);
        l_wh.y = coax_p_v3(((l_cs_51 * l_wh.y.x) - (l_sn_52 * l_wh.y.z)), l_wh.y.y, ((l_sn_52 * l_wh.y.x) + (l_cs_51 * l_wh.y.z)));
      }
      l_h = coax_p_orbitleague_closer(l_h, l_wh);
      l_i_41 = (l_i_41 + 1i);
    }
  }
  if ((l_h.x < 0x1.7d7840p26f))
  {
    l_h.y = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_ff3(l_h.y.x, coax_p_v3(l_c.y.x, l_c.y.y, l_c.y.z)), coax_p_wg_mul_ff3(l_h.y.y, coax_p_v3(l_c.z.x, l_c.z.y, l_c.z.z))), coax_p_wg_mul_ff3(l_h.y.z, coax_p_v3(l_c.w.x, l_c.w.y, l_c.w.z)));
  }
  return l_h;
}
fn coax_p_orbitleague_box_hit(p_o: vec3f, p_d: vec3f, p_c: vec3f, p_h: vec3f, p_col: vec3f, p_mat: i32) -> coax_t14 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_c: vec3f = p_c;
  var l_h: vec3f = p_h;
  var l_col: vec3f = p_col;
  var l_mat: i32 = p_mat;
  var l_inv: vec3f;
  var l_t0: vec3f;
  var l_t1: vec3f;
  var l_tn: vec3f;
  var l_tf: vec3f;
  var l_tnear: f32;
  var l_tfar: f32;
  var l_p: vec3f;
  var l_e: vec3f;
  var l_n: vec3f;
  var l_mid: f32;
  l_inv = coax_p_wg_div_ff3(0x1.000000p0f, l_d);
  l_t0 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_wg_sub_f3f3(l_c, l_h), l_o), l_inv);
  l_t1 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_c, l_h), l_o), l_inv);
  l_tn = coax_p_wg_min_f3f3(l_t0, l_t1);
  l_tf = coax_p_wg_max_f3f3(l_t0, l_t1);
  l_tnear = coax_p_fmax(coax_p_fmax(l_tn.x, l_tn.y), l_tn.z);
  l_tfar = coax_p_fmin(coax_p_fmin(l_tf.x, l_tf.y), l_tf.z);
  if ((((l_tnear > l_tfar) || (l_tfar < 0.0f)) || (l_tnear < 0.0f)))
  {
    return coax_p_orbitleague_miss();
  }
  l_p = coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tnear)), l_c);
  l_e = coax_p_wg_sub_f3f3(l_h, coax_p_wg_abs_f3(l_p));
  l_n = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_tnear == l_tn.x))
  {
    l_n = coax_p_v3((-coax_p_wg_sign(l_d.x)), 0.0f, 0.0f);
  }
  else
  {
    if ((l_tnear == l_tn.y))
    {
      l_n = coax_p_v3(0.0f, (-coax_p_wg_sign(l_d.y)), 0.0f);
    }
    else
    {
      l_n = coax_p_v3(0.0f, 0.0f, (-coax_p_wg_sign(l_d.z)));
    }
  }
  l_mid = ((((l_e.x + l_e.y) + l_e.z) - coax_p_fmin(coax_p_fmin(l_e.x, l_e.y), l_e.z)) - coax_p_fmax(coax_p_fmax(l_e.x, l_e.y), l_e.z));
  return coax_t14(l_tnear, l_n, l_col, (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-6f, 0x1.99999ap-5f, l_mid)), l_mat);
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
fn coax_p_wg_asin(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_wg_atan2(l_x, sqrt(coax_p_fmax((0x1.000000p0f - (l_x * l_x)), 0.0f)));
}
fn coax_p_orbitleague_curve_n(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  if (coax_p_orbitleague_in_sphere())
  {
    return coax_p_orbitleague_sph_n(l_p);
  }
  return coax_p_orbitleague_drum_n(l_p);
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
fn coax_p_orbitleague_car_side(p_c: coax_t16) -> f32 {
  var l_c: coax_t16 = p_c;
  return (l_c.y.w - (0x1.000000p1f * floor((l_c.y.w * 0x1.000000p-1f))));
}
fn coax_p_orbitleague_over_floor(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  if (coax_p_orbitleague_in_sphere())
  {
    return (coax_p_orbitleague_sr() - coax_p_wg_length3(coax_p_wg_sub_f3f3(l_p, coax_p_orbitleague_sph_c())));
  }
  if (coax_p_orbitleague_in_drum())
  {
    return (coax_at_u(i32(86i)) - coax_p_wg_length2(coax_p_v2(l_p.x, (l_p.y - coax_at_u(i32(86i))))));
  }
  return l_p.y;
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_ORBITLEAGUE_PAD_COL() -> vec3f {
  return coax_p_v3(0x1.800000p-1f, 0x1.ccccccp-1f, 0x1.000000p0f);
}
fn coax_p_orbitleague_moving() -> f32 {
  return select(0x1.000000p0f, 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_orbitleague_flat_light(p_n: vec3f) -> f32 {
  var l_n: vec3f = p_n;
  var l_l: f32;
  var l_band: f32;
  l_l = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_orbitleague_sun()));
  l_band = select(select(0x1.19999ap-1f, 0x1.99999ap-1f, (l_l > 0x1.000000p-2f)), 0x1.000000p0f, (l_l > 0x1.333334p-1f));
  return (0x1.666666p-2f + ((0x1.4cccccp-1f * l_band) * (0x1.666666p-1f + (0x1.333334p-2f * coax_p_fmax(0.0f, l_n.y)))));
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_orbitleague_env(p_r: vec3f) -> vec3f {
  var l_r: vec3f = p_r;
  var l_c: vec3f;
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.eb851ep-6f, 0x1.1eb852p-5f, 0x1.99999ap-5f), coax_p_v3(0x1.99999ap-5f, 0x1.1eb852p-4f, 0x1.0a3d70p-3f), coax_p_smoothstep((-0x1.333334p-3f), 0x1.000000p-2f, l_r.y));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-2f, 0x1.000000p-1f, 0x1.4cccccp-1f), coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_wg_normalize3(coax_p_ORBITLEAGUE_PLANET_D())), 0.0f), 0x1.400000p3f)), 0x1.666666p-1f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.dc28f6p-1f), coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_orbitleague_sun()), 0.0f), 0x1.e00000p5f)), 0x1.333334p0f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.666666p-2f, 0x1.19999ap-1f, 0x1.000000p0f), exp((((-l_r.y) * l_r.y) * 0x1.c20000p9f))), 0x1.000000p-1f));
  return l_c;
}
fn coax_p_orbitleague_sun() -> vec3f {
  return coax_p_wg_normalize3(coax_p_ORBITLEAGUE_SUN());
}
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_orbitleague_hash3(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot3(l_p, coax_p_v3(0x1.fc6666p6f, 0x1.37b334p8f, 0x1.2accccp6f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_ORBITLEAGUE_MOON_D() -> vec3f {
  return coax_p_v3(0x1.000000p-1f, 0x1.47ae14p-2f, (-0x1.000000p0f));
}
fn coax_p_orbitleague_sphere_t(p_d: vec3f, p_c: vec3f, p_r: f32) -> f32 {
  var l_d: vec3f = p_d;
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_b: f32;
  var l_disc: f32;
  l_b = coax_p_wg_dot3(l_c, l_d);
  l_disc = (((l_b * l_b) - coax_p_wg_dot3(l_c, l_c)) + (l_r * l_r));
  if ((l_disc < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return (l_b - sqrt(l_disc));
}
fn coax_p_wg_fract_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y), coax_p_fract(l_p0.z));
}
fn coax_p_wg_sub_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x - l_b), (l_a.y - l_b), (l_a.z - l_b));
}
fn coax_p_orbitleague_planet_c() -> vec3f {
  return coax_p_wg_mul_f3f(coax_p_wg_normalize3(coax_p_ORBITLEAGUE_PLANET_D()), 0x1.770000p11f);
}
fn coax_p_orbitleague_planet_col(p_n: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_pole: vec3f;
  var l_la: f32;
  var l_b: f32;
  var l_col: vec3f;
  var l_storm: vec3f;
  l_pole = coax_p_wg_normalize3(coax_p_ORBITLEAGUE_RING_N());
  l_la = coax_p_wg_dot3(l_n, l_pole);
  l_b = ((l_la + (sin(coax_p_wg_dot3(l_n, coax_p_v3(0x1.c00000p2f, 0x1.000000p0f, 0x1.400000p2f))) * 0x1.1eb852p-5f)) + (sin(((l_la * 0x1.f00000p4f) + (l_n.x * 0x1.400000p2f))) * 0x1.89374cp-7f));
  l_col = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.47ae14p-3f, 0x1.70a3d8p-2f, 0x1.000000p-1f), coax_p_v3(0x1.28f5c2p-1f, 0x1.7ae148p-1f, 0x1.b851ecp-1f), (0x1.000000p-1f + (0x1.000000p-1f * sin((l_b * 0x1.500000p4f)))));
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_v3(0x1.99999ap-2f, 0x1.5c28f6p-2f, 0x1.3d70a4p-1f), ((0x1.000000p-1f + (0x1.000000p-1f * sin(((l_b * 0x1.a80000p5f) + 0x1.4cccccp0f)))) * 0x1.666666p-2f));
  l_storm = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(l_n, coax_p_wg_normalize3(coax_p_v3(0x1.333334p-2f, (-0x1.99999ap-5f), (-0x1.e66666p-1f)))), coax_p_v3(0x1.000000p0f, 0x1.19999ap1f, 0x1.000000p0f));
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_v3(0x1.b851ecp-1f, 0x1.dc28f6p-1f, 0x1.f5c290p-1f), ((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-5f, 0x1.5c28f6p-4f, coax_p_wg_length3(l_storm))) * 0x1.b33334p-1f));
  return l_col;
}
fn coax_p_ORBITLEAGUE_RING_N() -> vec3f {
  return coax_p_v3(0x1.c28f5cp-3f, 0x1.000000p0f, (-0x1.333334p-2f));
}
fn coax_p_wg_sign(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x > 0.0f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return 0.0f;
}
fn coax_p_orbitleague_drum_at(p_x: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_a: f32;
  l_a = (l_x / coax_at_u(i32(86i)));
  return coax_p_v3((coax_at_u(i32(86i)) * sin(l_a)), (coax_at_u(i32(86i)) * (0x1.000000p0f - cos(l_a))), l_z);
}
fn coax_p_orbitleague_sph_at(p_x: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_a: f32;
  var l_f: f32;
  l_a = (l_x / coax_p_orbitleague_sr());
  l_f = (l_z / coax_p_orbitleague_sr());
  return coax_p_v3(((coax_p_orbitleague_sr() * cos(l_f)) * sin(l_a)), (coax_p_orbitleague_sr() - ((coax_p_orbitleague_sr() * cos(l_f)) * cos(l_a))), (coax_p_orbitleague_sr() * sin(l_f)));
}
fn coax_p_orbitleague_halo(p_ro: vec3f, p_rd: vec3f, p_p: vec3f, p_tmax: f32, p_r: f32) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_p: vec3f = p_p;
  var l_tmax: f32 = p_tmax;
  var l_r: f32 = p_r;
  var l_s: f32;
  var l_dd: f32;
  l_s = coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_p, l_ro), l_rd);
  if (((l_s < 0.0f) || (l_s > (l_tmax + 0x1.333334p-2f))))
  {
    return 0.0f;
  }
  l_dd = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_ro, coax_p_wg_mul_f3f(l_rd, l_s)), l_p));
  return exp((((-l_dd) * l_dd) / (l_r * l_r)));
}
fn coax_p_orbitleague_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_over: bool;
  var l_lit: bool;
  var l_fill: vec3f;
  var l_line: vec3f;
  var l_w: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_a: f32;
  if ((l_s.y <= 0.0f))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = (0x1.800000p2f * l_s.x);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.z >= 0x1.000000p1f);
  l_lit = (l_over || ((l_s.z >= 0x1.000000p0f) && (l_s.z < 0x1.000000p1f)));
  l_fill = select(coax_p_v3(0x1.99999ap-5f, 0x1.1eb852p-4f, 0x1.eb851ep-4f), coax_p_v3(0x1.99999ap-4f, 0x1.333334p-3f, 0x1.000000p-2f), l_over);
  l_line = select(coax_p_v3(0x1.666666p-2f, 0x1.19999ap-1f, 0x1.b33334p-1f), coax_p_v3(0x1.333334p-1f, 0x1.b33334p-1f, 0x1.000000p0f), l_lit);
  l_w = (0x1.400000p0f * l_s.x);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.y, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, (l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_orbitleague_qrot(p_q: vec4f, p_v: vec3f) -> vec3f {
  var l_q: vec4f = p_q;
  var l_v: vec3f = p_v;
  var l_t: vec3f;
  l_t = coax_p_wg_mul_ff3(0x1.000000p1f, coax_p_wg_cross3(coax_p_v3(l_q.x, l_q.y, l_q.z), l_v));
  return coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_v, coax_p_wg_mul_ff3(l_q.w, l_t)), coax_p_wg_cross3(coax_p_v3(l_q.x, l_q.y, l_q.z), l_t));
}
fn coax_p_wg_abs_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(abs(l_p0.x), abs(l_p0.y), abs(l_p0.z));
}
fn coax_p_orbitleague_car_paint(p_c: coax_t16) -> i32 {
  var l_c: coax_t16 = p_c;
  return coax_p_wg_i32_f(floor(((l_c.y.w * 0x1.000000p-1f) + 0x1.000000p-2f)));
}
fn coax_p_orbitleague_paint_colour(p_k: i32, p_side: f32) -> vec3f {
  var l_k: i32 = p_k;
  var l_side: f32 = p_side;
  var l__sw1: i32;
  {
    l__sw1 = l_k;
    if ((l__sw1 == 1i))
    {
      return coax_p_v3(0x1.ccccccp-1f, 0x1.d70a3ep-1f, 0x1.e66666p-1f);
    }
    else
    {
      if ((l__sw1 == 2i))
      {
        return coax_p_v3(0x1.47ae14p-3f, 0x1.5c28f6p-3f, 0x1.99999ap-3f);
      }
      else
      {
        if ((l__sw1 == 3i))
        {
          return coax_p_v3(0x1.19999ap-1f, 0x1.28f5c2p-1f, 0x1.3d70a4p-1f);
        }
        else
        {
          if ((l__sw1 == 4i))
          {
            return coax_p_v3(0x1.47ae14p-4f, 0x1.851eb8p-2f, 0x1.70a3d8p-3f);
          }
          else
          {
            if ((l__sw1 == 5i))
            {
              return coax_p_v3(0x1.19999ap-1f, 0x1.ccccccp-1f, 0x1.eb851ep-4f);
            }
            else
            {
              if ((l__sw1 == 6i))
              {
                return coax_p_v3(0x1.99999ap-5f, 0x1.800000p-1f, 0x1.666666p-1f);
              }
              else
              {
                if ((l__sw1 == 7i))
                {
                  return coax_p_v3(0x1.e66666p-1f, 0x1.c28f5cp-1f, 0x1.333334p-3f);
                }
                else
                {
                  return coax_p_orbitleague_team(l_side);
                }
              }
            }
          }
        }
      }
    }
  }
  return vec3f();
}
fn coax_p_orbitleague_mesh_cast(p_o: vec3f, p_d: vec3f) -> coax_t17 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_r: coax_t17;
  var l_dd: vec3f;
  var l_inv: vec3f;
  var l_tris: i32;
  var l_best: i32;
  var l_stack: array<i32, 32>;
  var l_sp: i32;
  var l_b: i32;
  var l_t0: vec3f;
  var l_t1: vec3f;
  var l_tn: vec3f;
  var l_tf: vec3f;
  var l_near: f32;
  var l_far: f32;
  var l_a: i32;
  var l_c: i32;
  var l_back: bool;
  var l_k: i32;
  var l_w: i32;
  var l_v0: vec3f;
  var l_e1: vec3f;
  var l_e2: vec3f;
  var l_pv: vec3f;
  var l_det: f32;
  var l_id: f32;
  var l_sv: vec3f;
  var l_uu: f32;
  var l_qv: vec3f;
  var l_vv: f32;
  var l_t: f32;
  var l_e1_33: vec3f;
  var l_e2_34: vec3f;
  var l_n: vec3f;
  l_r = coax_t17(0x1.dcd650p29f, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_dd = coax_p_v3(select(l_d.x, 0x1.5798eep-27f, (abs(l_d.x) < 0x1.5798eep-27f)), select(l_d.y, 0x1.5798eep-27f, (abs(l_d.y) < 0x1.5798eep-27f)), select(l_d.z, 0x1.5798eep-27f, (abs(l_d.z) < 0x1.5798eep-27f)));
  l_inv = coax_p_wg_div_ff3(0x1.000000p0f, l_dd);
  l_tris = coax_p_wg_i32_f(mesh[3i]);
  l_best = (-1i);
  l_stack = array<i32, 32>();
  l_stack[0i] = 0i;
  l_sp = 1i;
  while ((l_sp > 0i))
  {
    l_sp = (l_sp - 1i);
    l_b = (16i + (l_stack[i32(l_sp)] * 8i));
    l_t0 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_v3(mesh[i32(l_b)], mesh[i32((l_b + 1i))], mesh[i32((l_b + 2i))]), l_o), l_inv);
    l_t1 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_v3(mesh[i32((l_b + 4i))], mesh[i32((l_b + 5i))], mesh[i32((l_b + 6i))]), l_o), l_inv);
    l_tn = coax_p_wg_min_f3f3(l_t0, l_t1);
    l_tf = coax_p_wg_max_f3f3(l_t0, l_t1);
    l_near = coax_p_fmax(coax_p_fmax(l_tn.x, l_tn.y), l_tn.z);
    l_far = coax_p_fmin(coax_p_fmin(l_tf.x, l_tf.y), l_tf.z);
    if (((l_far < coax_p_fmax(l_near, 0.0f)) || (l_near > l_r.x)))
    {
      continue;
    }
    l_a = coax_p_wg_i32_f(mesh[i32((l_b + 3i))]);
    l_c = coax_p_wg_i32_f(mesh[i32((l_b + 7i))]);
    if ((l_a >= 0i))
    {
      l_back = select(select((l_d.z < 0.0f), (l_d.y < 0.0f), (l_c == 1i)), (l_d.x < 0.0f), (l_c == 0i));
      if ((l_sp < 31i))
      {
        l_stack[i32(l_sp)] = select((l_a + 1i), l_a, l_back);
        l_stack[i32((l_sp + 1i))] = select(l_a, (l_a + 1i), l_back);
        l_sp = (l_sp + 2i);
      }
      continue;
    }
    l_k = ((-l_a) - 1i);
    while ((l_k < (((-l_a) - 1i) + l_c)))
    {
      l_w = (l_tris + (l_k * 12i));
      l_v0 = coax_p_v3(mesh[i32(l_w)], mesh[i32((l_w + 1i))], mesh[i32((l_w + 2i))]);
      l_e1 = coax_p_v3(mesh[i32((l_w + 3i))], mesh[i32((l_w + 4i))], mesh[i32((l_w + 5i))]);
      l_e2 = coax_p_v3(mesh[i32((l_w + 6i))], mesh[i32((l_w + 7i))], mesh[i32((l_w + 8i))]);
      l_pv = coax_p_wg_cross3(l_d, l_e2);
      l_det = coax_p_wg_dot3(l_e1, l_pv);
      if ((abs(l_det) > 0x1.197998p-40f))
      {
        l_id = (0x1.000000p0f / l_det);
        l_sv = coax_p_wg_sub_f3f3(l_o, l_v0);
        l_uu = (coax_p_wg_dot3(l_sv, l_pv) * l_id);
        l_qv = coax_p_wg_cross3(l_sv, l_e1);
        l_vv = (coax_p_wg_dot3(l_d, l_qv) * l_id);
        l_t = (coax_p_wg_dot3(l_e2, l_qv) * l_id);
        if (((((l_uu >= 0.0f) && (l_vv >= 0.0f)) && ((l_uu + l_vv) <= 0x1.000000p0f)) && ((l_t > 0x1.a36e2ep-14f) && (l_t < l_r.x))))
        {
          l_r.x = l_t;
          l_best = l_w;
        }
      }
      l_k = (l_k + 1i);
    }
  }
  if ((l_best >= 0i))
  {
    l_e1_33 = coax_p_v3(mesh[i32((l_best + 3i))], mesh[i32((l_best + 4i))], mesh[i32((l_best + 5i))]);
    l_e2_34 = coax_p_v3(mesh[i32((l_best + 6i))], mesh[i32((l_best + 7i))], mesh[i32((l_best + 8i))]);
    l_n = coax_p_wg_normalize3(coax_p_wg_cross3(l_e1_33, l_e2_34));
    l_r.y = select(l_n, coax_p_wg_neg_f3(l_n), (coax_p_wg_dot3(l_n, l_d) > 0.0f));
    l_r.z = coax_p_v3(mesh[i32((l_best + 9i))], mesh[i32((l_best + 10i))], mesh[i32((l_best + 11i))]);
  }
  return l_r;
}
fn coax_p_wg_i32_f(p_x: f32) -> i32 {
  var l_x: f32 = p_x;
  if ((l_x != l_x))
  {
    return 0i;
  }
  if ((l_x <= (-0x1.000000p31f)))
  {
    return (-2147483647i - 1i);
  }
  if ((l_x >= 0x1.fffffep30f))
  {
    return 2147483520i;
  }
  return i32(l_x);
}
fn coax_p_orbitleague_hull_cast(p_o: vec3f, p_d: vec3f, p_first: i32, p_count: i32, p_HULL: array<vec4f, 45>) -> coax_t18 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_first: i32 = p_first;
  var l_count: i32 = p_count;
  var l_HULL: array<vec4f, 45> = p_HULL;
  var l_tn: f32;
  var l_tf: f32;
  var l_nn: vec3f;
  var l_k: i32;
  var l_i: i32;
  var l_pl: vec4f;
  var l_den: f32;
  var l_num: f32;
  var l_t: f32;
  var l_p: vec3f;
  var l_e: f32;
  var l_i_16: i32;
  var l_pl_17: vec4f;
  l_tn = (-0x1.dcd650p29f);
  l_tf = 0x1.dcd650p29f;
  l_nn = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_k = (-1i);
  {
    l_i = l_first;
    while ((l_i < (l_first + l_count)))
    {
      l_pl = l_HULL[i32(l_i)];
      l_den = coax_p_wg_dot3(coax_p_v3(l_pl.x, l_pl.y, l_pl.z), l_d);
      l_num = (l_pl.w - coax_p_wg_dot3(coax_p_v3(l_pl.x, l_pl.y, l_pl.z), l_o));
      if ((abs(l_den) < 0x1.ad7f2ap-24f))
      {
        if ((l_num < 0.0f))
        {
          return coax_t18(0x1.dcd650p29f, l_nn, 0.0f);
        }
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_t = (l_num / l_den);
      if ((l_den < 0.0f))
      {
        if ((l_t > l_tn))
        {
          l_tn = l_t;
          l_nn = coax_p_v3(l_pl.x, l_pl.y, l_pl.z);
          l_k = l_i;
        }
      }
      else
      {
        l_tf = coax_p_fmin(l_tf, l_t);
      }
      l_i = (l_i + 1i);
    }
  }
  if ((((l_k < 0i) || (l_tn > l_tf)) || (l_tn < 0.0f)))
  {
    return coax_t18(0x1.dcd650p29f, l_nn, 0.0f);
  }
  l_p = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tn));
  l_e = 0x1.dcd650p29f;
  {
    l_i_16 = l_first;
    while ((l_i_16 < (l_first + l_count)))
    {
      if ((l_i_16 != l_k))
      {
        l_pl_17 = l_HULL[i32(l_i_16)];
        l_e = coax_p_fmin(l_e, (l_pl_17.w - coax_p_wg_dot3(coax_p_v3(l_pl_17.x, l_pl_17.y, l_pl_17.z), l_p)));
      }
      l_i_16 = (l_i_16 + 1i);
    }
  }
  return coax_t18(l_tn, l_nn, (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.cac084p-6f, l_e)));
}
fn coax_p_orbitleague_wheel_hit(p_o: vec3f, p_d: vec3f, p_c: vec3f, p_r: f32, p_w: f32, p_spin: f32) -> coax_t14 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_w: f32 = p_w;
  var l_spin: f32 = p_spin;
  var l_q: vec3f;
  var l_a: f32;
  var l_b: f32;
  var l_cc: f32;
  var l_best: coax_t14;
  var l_disc: f32;
  var l_t: f32;
  var l_p: vec3f;
  var l_s: f32;
  var l_t_15: f32;
  var l_p_16: vec3f;
  var l_rr: f32;
  var l_ang: f32;
  var l_spoke: f32;
  var l_rim: bool;
  var l_col: vec3f;
  l_q = coax_p_wg_sub_f3f3(l_o, l_c);
  l_a = ((l_d.y * l_d.y) + (l_d.z * l_d.z));
  l_b = ((l_q.y * l_d.y) + (l_q.z * l_d.z));
  l_cc = (((l_q.y * l_q.y) + (l_q.z * l_q.z)) - (l_r * l_r));
  l_best = coax_p_orbitleague_miss();
  l_disc = ((l_b * l_b) - (l_a * l_cc));
  if (((l_disc >= 0.0f) && (l_a > 0x1.0c6f7ap-20f)))
  {
    l_t = (((-l_b) - sqrt(l_disc)) / l_a);
    l_p = coax_p_wg_add_f3f3(l_q, coax_p_wg_mul_f3f(l_d, l_t));
    if (((l_t > 0.0f) && (abs(l_p.x) <= l_w)))
    {
      l_best = coax_t14(l_t, coax_p_wg_normalize3(coax_p_v3(0.0f, l_p.y, l_p.z)), coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.eb851ep-5f), (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.eb851ep-5f, (l_w - abs(l_p.x)))), 0i);
    }
  }
  {
    l_s = (-0x1.000000p0f);
    while ((l_s <= 0x1.000000p0f))
    {
      if ((abs(l_d.x) > 0x1.0c6f7ap-20f))
      {
        l_t_15 = (((l_s * l_w) - l_q.x) / l_d.x);
        l_p_16 = coax_p_wg_add_f3f3(l_q, coax_p_wg_mul_f3f(l_d, l_t_15));
        l_rr = coax_p_wg_length2(coax_p_v2(l_p_16.y, l_p_16.z));
        if ((((l_t_15 > 0.0f) && (l_t_15 < l_best.x)) && (l_rr <= l_r)))
        {
          l_ang = (((coax_p_wg_atan2(l_p_16.y, l_p_16.z) + l_spin) * 0x1.400000p2f) / 0x1.921fb6p2f);
          l_spoke = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.99999ap-3f, abs((coax_p_fract(l_ang) - 0x1.000000p-1f))));
          l_rim = ((l_rr < (l_r * 0x1.51eb86p-1f)) && ((l_spoke > 0x1.000000p-1f) || (l_rr < (l_r * 0x1.47ae14p-3f))));
          l_col = select(coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.eb851ep-5f), coax_p_v3(0x1.8f5c28p-1f, 0x1.a3d70ap-1f, 0x1.c28f5cp-1f), l_rim);
          l_best = coax_t14(l_t_15, coax_p_v3(l_s, 0.0f, 0.0f), l_col, (((0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-6f, 0x1.47ae14p-5f, abs((l_rr - (l_r * 0x1.51eb86p-1f))))) + 0x1.000000p0f) - coax_p_smoothstep(0x1.47ae14p-6f, 0x1.47ae14p-5f, (l_r - l_rr))), select(0i, 1i, l_rim));
        }
      }
      l_s = (l_s + 0x1.000000p1f);
    }
  }
  return l_best;
}
fn coax_p_wg_div_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a / l_b.x), (l_a / l_b.y), (l_a / l_b.z));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
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
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_ORBITLEAGUE_PLANET_D() -> vec3f {
  return coax_p_v3((-0x1.000000p-1f), 0x1.333334p-2f, 0x1.000000p0f);
}
fn coax_p_ORBITLEAGUE_SUN() -> vec3f {
  return coax_p_v3((-0x1.000000p-1f), 0x1.3d70a4p-1f, (-0x1.ccccccp-2f));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
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