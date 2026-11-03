// ---- generated: apps/openagents-web/gpu/fivelab.coax (the module) by coax wgsl; do not edit
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
struct coax_t13 {
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
  z: f32,
  w: vec3f,
  f4: f32,
  f5: vec3f,
  f6: vec3f,
  f7: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 442>;
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
  var l_c: vec3f;
  var l_l: f32;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((coax_at_u(i32(10i)) > 0x1.000000p-1f))
  {
    l_c = coax_p_fivelab_glade(l_p);
    l_c = coax_p_v3(coax_p_verse_style_tone(l_c.x, 0x1.266666p0f), coax_p_verse_style_tone(l_c.y, 0x1.266666p0f), coax_p_verse_style_tone(l_c.z, 0x1.266666p0f));
    l_l = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
    l_c = coax_p_v3(coax_p_verse_style_saturation(l_c.x, l_l, 0x1.1eb852p0f), coax_p_verse_style_saturation(l_c.y, l_l, 0x1.1eb852p0f), coax_p_verse_style_saturation(l_c.z, l_l, 0x1.1eb852p0f));
    if ((l_p.y < coax_at_u(i32(11i))))
    {
      l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.c28f5cp-3f), coax_p_v3(0x1.47ae14p-7f, 0x1.89374cp-7f, 0x1.0624dep-6f));
    }
  }
  else
  {
    l_c = coax_p_fivelab_flat(l_p);
  }
  if ((coax_at_u(i32(14i)) > coax_at_u(i32(12i))))
  {
    if (((((l_p.x >= coax_at_u(i32(12i))) && (l_p.x <= coax_at_u(i32(14i)))) && (l_p.y >= coax_at_u(i32(13i)))) && (l_p.y <= coax_at_u(i32(15i)))))
    {
      l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.1eb852p-3f), coax_p_v3(0x1.89374cp-7f, 0x1.cac084p-7f, 0x1.47ae14p-6f));
    }
  }
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
  return coax_p_wg_make_f4_f3f(coax_p_scale3(coax_p_scale3(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_fivelab_glade(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_t: f32;
  var l_eye: vec3f;
  var l_f: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_sun: vec3f;
  var l_rd: vec3f;
  var l_h: coax_t13;
  var l_pos: vec3f;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_glow: vec3f;
  var l_i: i32;
  var l_za: vec4f;
  var l_zb: vec4f;
  var l_vine: f32;
  var l_mote: f32;
  var l_i_20: i32;
  var l_tk: vec4f;
  var l_ts: vec4f;
  var l_d: f32;
  var l_ring: f32;
  var l_tm: f32;
  var l_tc: vec3f;
  var l_leaf: f32;
  var l_who: i32;
  var l_tk_29: vec4f;
  var l_ts_30: vec4f;
  var l_kd: f32;
  var l_tm_32: f32;
  var l_druid: bool;
  var l_pact: bool;
  var l_barb: bool;
  var l_monk: bool;
  var l_bones: bool;
  var l_wiz: bool;
  var l_bard: bool;
  var l_cler: bool;
  var l_figh: bool;
  var l_pal: bool;
  var l_rang: bool;
  var l_rog: bool;
  var l_sorc: bool;
  var l_vine_46: f32;
  var l_wr: f32;
  var l_hx: f32;
  var l_rim: f32;
  var l_ndl: f32;
  var l_lit: f32;
  var l_sh: coax_t13;
  var l_w: f32;
  var l_band: f32;
  var l_amb: f32;
  var l_shadow_light: vec3f;
  var l_lit_light: vec3f;
  var l_c: vec3f;
  var l_hz: f32;
  var l_hc: vec3f;
  l_t = coax_at_u(i32(2i));
  l_eye = coax_p_wg_at3(16i);
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_fwd = coax_p_wg_at3(20i);
  l_right = coax_p_wg_at3(24i);
  l_up = coax_p_wg_at3(28i);
  l_sun = coax_p_normalize3(coax_p_wg_at3(32i));
  l_rd = coax_p_normalize3(coax_p_add3(l_fwd, coax_p_add3(coax_p_scale3(l_right, ((l_p.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(l_up, ((coax_at_u(i32(27i)) - l_p.y) / l_f)))));
  l_h = coax_p_fivelab_trace(l_eye, l_rd, false);
  if ((l_h.z < 0.0f))
  {
    return coax_p_fivelab_sky(l_rd, l_sun, l_t);
  }
  l_pos = coax_p_add3(l_eye, coax_p_scale3(l_rd, l_h.x));
  l_n = l_h.y;
  l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f);
  l_glow = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_h.z < 0x1.000000p-1f))
  {
    l_albedo = coax_p_fivelab_grass(l_pos.x, l_pos.z, l_t);
    {
      l_i = 0i;
      while ((l_i < 8i))
      {
        l_za = coax_p_wg_at4((1700i + (l_i * 8i)));
        l_zb = coax_p_wg_at4((1704i + (l_i * 8i)));
        if ((l_zb.x > 0x1.000000p-1f))
        {
          if (((((l_pos.x >= l_za.x) && (l_pos.x <= l_za.z)) && (l_pos.z >= l_za.y)) && (l_pos.z <= l_za.w)))
          {
            if ((l_zb.x < 0x1.800000p0f))
            {
              l_vine = coax_p_verse_style_noise((l_pos.x * 0x1.800000p1f), (l_pos.z * 0x1.800000p1f));
              l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-3f, 0x1.47ae14p-5f), (0x1.19999ap-1f + (0x1.666666p-2f * l_vine)));
              l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.99999ap-4f, 0x1.333334p-2f, 0x1.99999ap-5f), (0x1.333334p-3f * coax_p_smoothstep(0x1.19999ap-1f, 0x1.99999ap-1f, l_vine))));
            }
            else
            {
              l_mote = coax_p_verse_style_noise(((l_pos.x * 0x1.000000p1f) + (l_t * 0x1.666666p-1f)), ((l_pos.z * 0x1.000000p1f) - (l_t * 0x1.000000p-1f)));
              l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-2f, 0x1.000000p0f), (0x1.70a3d8p-3f + (0x1.666666p-2f * coax_p_smoothstep(0x1.333334p-1f, 0x1.ccccccp-1f, l_mote)))));
            }
          }
        }
        l_i = (l_i + 1i);
      }
    }
    {
      l_i_20 = 0i;
      while ((l_i_20 < 208i))
      {
        l_tk = coax_p_wg_at4((36i + (l_i_20 * 8i)));
        l_ts = coax_p_wg_at4((40i + (l_i_20 * 8i)));
        if ((l_tk.z > (-0x1.000000p-1f)))
        {
          l_d = coax_p_wg_length2(coax_p_v2((l_pos.x - l_tk.x), (l_pos.z - l_tk.y)));
          l_albedo = coax_p_scale3(l_albedo, (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_smoothstep(0x1.333334p-3f, 0x1.666666p-1f, l_d))));
          l_ring = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.47ae14p-4f, abs((l_d - 0x1.5c28f6p-1f))));
          l_tm = coax_p_fivelab_team(l_tk.z);
          l_tc = coax_p_v3(0x1.666666p-2f, 0x1.e66666p-1f, 0x1.ccccccp-2f);
          if ((l_tm > 0x1.000000p-1f))
          {
            l_tc = coax_p_v3(0x1.19999ap-1f, 0x1.b33334p-1f, 0x1.000000p0f);
          }
          if ((l_tm > 0x1.800000p0f))
          {
            l_tc = coax_p_v3(0x1.ccccccp-1f, 0x1.b851ecp-1f, 0x1.7ae148p-1f);
          }
          if ((l_ts.y < 0x1.800000p0f))
          {
            l_glow = coax_p_add3(l_glow, coax_p_scale3(l_tc, (l_ring * (0x1.000000p-2f + (l_ts.w * 0x1.99999ap0f)))));
          }
        }
        l_i_20 = (l_i_20 + 1i);
      }
    }
  }
  else
  {
    if ((l_h.z < 0x1.800000p0f))
    {
      l_albedo = coax_p_v3(0x1.eb851ep-3f, 0x1.333334p-3f, 0x1.47ae14p-4f);
      l_albedo = coax_p_scale3(l_albedo, (0x1.99999ap-1f + (0x1.99999ap-2f * coax_p_verse_style_noise((l_pos.y * 0x1.000000p2f), ((l_pos.x + l_pos.z) * 0x1.800000p2f)))));
    }
    else
    {
      if ((l_h.z < 0x1.400000p1f))
      {
        l_leaf = coax_p_verse_style_noise(((l_pos.x * 0x1.4cccccp0f) + l_pos.y), ((l_pos.z * 0x1.4cccccp0f) - l_pos.y));
        l_albedo = coax_p_mix3(coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-3f, 0x1.eb851ep-5f), coax_p_v3(0x1.333334p-2f, 0x1.ccccccp-2f, 0x1.99999ap-4f), l_leaf);
        l_n = coax_p_normalize3(coax_p_add3(l_n, coax_p_scale3(coax_p_v3((l_leaf - 0x1.000000p-1f), (coax_p_verse_style_noise((l_pos.x * 0x1.0cccccp1f), (l_pos.z * 0x1.0cccccp1f)) - 0x1.000000p-1f), (0x1.000000p-1f - l_leaf)), 0x1.ccccccp-1f)));
      }
      else
      {
        l_who = i32(l_h.w);
        l_tk_29 = coax_p_wg_at4((36i + (l_who * 8i)));
        l_ts_30 = coax_p_wg_at4((40i + (l_who * 8i)));
        l_kd = coax_p_fivelab_kind(l_tk_29.z);
        l_tm_32 = coax_p_fivelab_team(l_tk_29.z);
        l_druid = coax_p_fivelab_is(l_kd, 0.0f);
        l_pact = coax_p_fivelab_is(l_kd, 0x1.000000p0f);
        l_barb = coax_p_fivelab_is(l_kd, 0x1.000000p1f);
        l_monk = coax_p_fivelab_is(l_kd, 0x1.800000p1f);
        l_bones = coax_p_fivelab_is(l_kd, 0x1.000000p2f);
        l_wiz = coax_p_fivelab_is(l_kd, 0x1.400000p2f);
        l_bard = coax_p_fivelab_is(l_kd, 0x1.800000p2f);
        l_cler = coax_p_fivelab_is(l_kd, 0x1.c00000p2f);
        l_figh = coax_p_fivelab_is(l_kd, 0x1.000000p3f);
        l_pal = coax_p_fivelab_is(l_kd, 0x1.200000p3f);
        l_rang = coax_p_fivelab_is(l_kd, 0x1.400000p3f);
        l_rog = coax_p_fivelab_is(l_kd, 0x1.600000p3f);
        l_sorc = coax_p_fivelab_is(l_kd, 0x1.800000p3f);
        if ((l_h.z < 0x1.c00000p1f))
        {
          l_albedo = coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-2f, 0x1.47ae14p-3f);
          if (l_pact)
          {
            l_albedo = coax_p_v3(0x1.47ae14p-2f, 0x1.47ae14p-5f, 0x1.99999ap-5f);
            if (((l_pos.y < 0x1.ae147ap-2f) || (l_pos.y > 0x1.4cccccp0f)))
            {
              l_albedo = coax_p_v3(0x1.800000p-1f, 0x1.19999ap-1f, 0x1.47ae14p-3f);
            }
          }
          if (l_barb)
          {
            l_albedo = coax_p_v3(0x1.7ae148p-1f, 0x1.0a3d70p-1f, 0x1.851eb8p-2f);
            if ((l_pos.y < 0x1.99999ap-1f))
            {
              l_albedo = coax_p_scale3(coax_p_v3(0x1.70a3d8p-2f, 0x1.000000p-2f, 0x1.333334p-3f), (0x1.99999ap-1f + (0x1.99999ap-2f * coax_p_verse_style_noise((l_pos.x * 0x1.c00000p3f), (l_pos.y * 0x1.c00000p3f)))));
            }
          }
          if (l_monk)
          {
            l_albedo = coax_p_v3(0x1.d70a3ep-1f, 0x1.19999ap-1f, 0x1.eb851ep-4f);
            if ((l_pos.y > 0x1.266666p0f))
            {
              l_albedo = coax_p_v3(0x1.8f5c28p-1f, 0x1.28f5c2p-1f, 0x1.c28f5cp-2f);
            }
          }
          if (l_bones)
          {
            l_albedo = coax_p_v3(0x1.b851ecp-1f, 0x1.a3d70ap-1f, 0x1.666666p-1f);
            if ((((l_pos.y > 0x1.70a3d8p-1f) && (l_pos.y < 0x1.147ae2p0f)) && (coax_p_fract((l_pos.y * 0x1.c00000p3f)) < 0x1.ccccccp-2f)))
            {
              l_albedo = coax_p_v3(0x1.eb851ep-4f, 0x1.99999ap-4f, 0x1.70a3d8p-4f);
            }
            if (((l_pos.y > 0x1.ccccccp-2f) && (l_pos.y < 0x1.3d70a4p-1f)))
            {
              l_albedo = coax_p_v3(0x1.000000p-2f, 0x1.c28f5cp-3f, 0x1.70a3d8p-3f);
            }
          }
          if (l_wiz)
          {
            l_albedo = coax_p_v3(0x1.eb851ep-4f, 0x1.5c28f6p-3f, 0x1.ccccccp-2f);
            if ((l_pos.y < 0x1.ae147ap-2f))
            {
              l_albedo = coax_p_v3(0x1.666666p-1f, 0x1.70a3d8p-1f, 0x1.99999ap-1f);
            }
          }
          if (l_bard)
          {
            l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.0a3d70p-3f, 0x1.851eb8p-2f);
            if ((abs((coax_p_fract(((l_pos.y + (l_pos.x - l_tk_29.x)) * 0x1.19999ap1f)) - 0x1.000000p-1f)) < 0x1.1eb852p-4f))
            {
              l_albedo = coax_p_v3(0x1.ccccccp-1f, 0x1.70a3d8p-1f, 0x1.000000p-2f);
            }
          }
          if (l_cler)
          {
            l_albedo = coax_p_v3(0x1.c28f5cp-1f, 0x1.b851ecp-1f, 0x1.99999ap-1f);
            if ((abs((l_pos.y - 0x1.051eb8p0f)) < 0x1.99999ap-5f))
            {
              l_albedo = coax_p_v3(0x1.ccccccp-1f, 0x1.70a3d8p-1f, 0x1.000000p-2f);
            }
          }
          if (l_figh)
          {
            l_albedo = coax_p_scale3(coax_p_v3(0x1.000000p-1f, 0x1.0a3d70p-1f, 0x1.1eb852p-1f), (0x1.800000p-1f + (0x1.000000p-1f * coax_p_verse_style_noise((l_pos.y * 0x1.400000p5f), (coax_p_wg_atan2((l_pos.z - l_tk_29.y), (l_pos.x - l_tk_29.x)) * 0x1.200000p3f)))));
            if ((l_pos.y < 0x1.3d70a4p-1f))
            {
              l_albedo = coax_p_v3(0x1.333334p-2f, 0x1.c28f5cp-3f, 0x1.1eb852p-3f);
            }
          }
          if (l_pal)
          {
            l_albedo = coax_p_v3(0x1.99999ap-1f, 0x1.a3d70ap-1f, 0x1.c28f5cp-1f);
            if ((l_pos.y > 0x1.3851ecp0f))
            {
              l_albedo = coax_p_v3(0x1.c28f5cp-1f, 0x1.666666p-1f, 0x1.1eb852p-2f);
            }
          }
          if (l_rang)
          {
            l_albedo = coax_p_v3(0x1.51eb86p-2f, 0x1.0a3d70p-2f, 0x1.333334p-3f);
            if ((l_pos.y > 0x1.000000p0f))
            {
              l_albedo = coax_p_v3(0x1.5c28f6p-3f, 0x1.147ae2p-2f, 0x1.eb851ep-4f);
            }
          }
          if (l_rog)
          {
            l_albedo = coax_p_v3(0x1.70a3d8p-4f, 0x1.70a3d8p-4f, 0x1.c28f5cp-4f);
          }
          if (l_sorc)
          {
            l_albedo = coax_p_v3(0x1.51eb86p-1f, 0x1.333334p-2f, 0x1.99999ap-4f);
            if ((coax_p_fract(((l_pos.y * 0x1.800000p3f) + (0x1.000000p-1f * coax_p_fract((coax_p_wg_atan2((l_pos.z - l_tk_29.y), (l_pos.x - l_tk_29.x)) * 0x1.99999ap0f))))) < 0x1.000000p-2f))
            {
              l_albedo = coax_p_v3(0x1.666666p-2f, 0x1.0a3d70p-3f, 0x1.99999ap-5f);
            }
          }
          if (((abs((l_pos.y - 0x1.a3d70ap-1f)) < 0x1.99999ap-5f) && (!l_bones)))
          {
            l_albedo = coax_p_v3(0x1.99999ap-3f, 0x1.3d70a4p-1f, 0x1.0a3d70p-2f);
            if ((l_tm_32 > 0x1.000000p-1f))
            {
              l_albedo = coax_p_v3(0x1.1eb852p-2f, 0x1.000000p-1f, 0x1.b851ecp-1f);
            }
          }
          if (coax_p_fivelab_mark(l_ts_30.z, 0.0f))
          {
            l_vine_46 = coax_p_fract(((l_pos.y * 0x1.c00000p2f) + (coax_p_wg_atan2((l_pos.z - l_tk_29.y), (l_pos.x - l_tk_29.x)) * 0x1.333334p0f)));
            if (((l_pos.y < 0x1.ccccccp-1f) && (l_vine_46 < 0x1.333334p-2f)))
            {
              l_albedo = coax_p_v3(0x1.47ae14p-4f, 0x1.47ae14p-2f, 0x1.99999ap-5f);
            }
          }
        }
        else
        {
          if ((l_h.z < 0x1.200000p2f))
          {
            l_albedo = coax_p_v3(0x1.8f5c28p-1f, 0x1.28f5c2p-1f, 0x1.c28f5cp-2f);
            if ((l_pos.y > 0x1.9eb852p0f))
            {
              l_albedo = coax_p_v3(0x1.eb851ep-3f, 0x1.47ae14p-3f, 0x1.70a3d8p-4f);
            }
            if (l_monk)
            {
              l_albedo = coax_p_v3(0x1.8f5c28p-1f, 0x1.28f5c2p-1f, 0x1.c28f5cp-2f);
            }
            if (l_bones)
            {
              l_albedo = coax_p_v3(0x1.ccccccp-1f, 0x1.bd70a4p-1f, 0x1.851eb8p-1f);
              if ((((l_n.y < 0x1.666666p-2f) && (l_n.y > (-0x1.99999ap-4f))) && (abs((coax_p_fract(((coax_p_wg_atan2(l_n.z, l_n.x) * 0x1.800000p1f) / 0x1.921fb6p2f)) - 0x1.000000p-1f)) < 0x1.eb851ep-4f)))
              {
                l_albedo = coax_p_v3(0x1.eb851ep-6f, 0x1.47ae14p-6f, 0x1.47ae14p-6f);
                l_glow = coax_p_add3(l_glow, coax_p_v3(0x1.000000p-2f, 0x1.47ae14p-4f, 0x1.99999ap-5f));
              }
            }
            if ((l_barb && (l_pos.y > 0x1.a8f5c2p0f)))
            {
              l_albedo = coax_p_v3(0x1.19999ap-1f, 0x1.000000p-2f, 0x1.47ae14p-4f);
            }
            if ((l_n.y > 0x1.333334p-2f))
            {
              if (l_wiz)
              {
                l_albedo = coax_p_v3(0x1.eb851ep-4f, 0x1.5c28f6p-3f, 0x1.ccccccp-2f);
              }
              if (l_bard)
              {
                l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.0a3d70p-3f, 0x1.851eb8p-2f);
              }
              if (l_figh)
              {
                l_albedo = coax_p_v3(0x1.19999ap-1f, 0x1.23d70ap-1f, 0x1.3d70a4p-1f);
              }
              if (l_pal)
              {
                l_albedo = coax_p_v3(0x1.a3d70ap-1f, 0x1.ae147ap-1f, 0x1.ccccccp-1f);
              }
              if (l_rang)
              {
                l_albedo = coax_p_v3(0x1.5c28f6p-3f, 0x1.147ae2p-2f, 0x1.eb851ep-4f);
              }
              if (l_rog)
              {
                l_albedo = coax_p_v3(0x1.1eb852p-4f, 0x1.1eb852p-4f, 0x1.70a3d8p-4f);
              }
              if (l_sorc)
              {
                l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.333334p-2f, 0x1.47ae14p-4f);
              }
            }
            if ((((l_rog && (l_n.y > (-0x1.99999ap-3f))) && (l_n.y < 0x1.333334p-2f)) && (abs((coax_p_fract(((coax_p_wg_atan2(l_n.z, l_n.x) * 0x1.800000p1f) / 0x1.921fb6p2f)) - 0x1.000000p-1f)) > 0x1.99999ap-3f)))
            {
              l_albedo = coax_p_v3(0x1.1eb852p-4f, 0x1.1eb852p-4f, 0x1.70a3d8p-4f);
            }
            if (l_pact)
            {
              if ((l_pos.y > 0x1.99999ap0f))
              {
                l_albedo = coax_p_v3(0x1.0a3d70p-2f, 0x1.eb851ep-6f, 0x1.47ae14p-5f);
              }
            }
            if ((coax_p_fivelab_mark(l_ts_30.z, 0x1.000000p1f) && (abs((l_pos.y - 0x1.947ae2p0f)) < 0x1.70a3d8p-5f)))
            {
              l_albedo = coax_p_v3(0x1.47ae14p-6f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
            }
          }
          else
          {
            l_albedo = coax_p_v3(0x1.99999ap-2f, 0x1.1eb852p-2f, 0x1.1eb852p-3f);
            if ((l_pos.y > 0x1.c00000p0f))
            {
              if (l_pact)
              {
                l_glow = coax_p_add3(l_glow, coax_p_v3(0x1.b33334p-1f, 0x1.666666p-2f, 0x1.000000p0f));
              }
              if (l_druid)
              {
                l_glow = coax_p_add3(l_glow, coax_p_v3(0x1.333334p-1f, 0x1.ccccccp-1f, 0x1.000000p-1f));
              }
              if (l_wiz)
              {
                l_glow = coax_p_add3(l_glow, coax_p_v3(0x1.333334p-1f, 0x1.99999ap-1f, 0x1.000000p0f));
              }
            }
            if (l_bard)
            {
              l_albedo = coax_p_v3(0x1.3d70a4p-1f, 0x1.851eb8p-2f, 0x1.47ae14p-3f);
            }
            if (l_cler)
            {
              l_albedo = coax_p_v3(0x1.666666p-2f, 0x1.eb851ep-3f, 0x1.eb851ep-4f);
              if ((l_pos.y > 0x1.f5c290p-1f))
              {
                l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.7ae148p-1f, 0x1.8f5c28p-1f);
              }
            }
            if (l_figh)
            {
              l_albedo = coax_p_v3(0x1.851eb8p-1f, 0x1.8f5c28p-1f, 0x1.ae147ap-1f);
              if ((l_pos.y < 0x1.70a3d8p-1f))
              {
                l_albedo = coax_p_v3(0x1.333334p-3f, 0x1.99999ap-4f, 0x1.eb851ep-5f);
              }
              if ((abs((l_pos.y - 0x1.7ae148p-1f)) < 0x1.eb851ep-6f))
              {
                l_albedo = coax_p_v3(0x1.99999ap-1f, 0x1.3d70a4p-1f, 0x1.c28f5cp-3f);
              }
            }
            if (l_pal)
            {
              l_albedo = coax_p_v3(0x1.1eb852p-3f, 0x1.eb851ep-3f, 0x1.3d70a4p-1f);
              if (((abs((l_pos.y - 0x1.f5c290p-1f)) < 0x1.70a3d8p-5f) || (abs((l_pos.z - (l_tk_29.y + 0x1.eb851ep-4f))) < 0x1.70a3d8p-5f)))
              {
                l_albedo = coax_p_v3(0x1.d70a3ep-1f, 0x1.ccccccp-1f, 0x1.ae147ap-1f);
              }
              if (((l_pos.y > 0x1.4cccccp0f) || (l_pos.y < 0x1.3d70a4p-1f)))
              {
                l_albedo = coax_p_v3(0x1.c28f5cp-1f, 0x1.666666p-1f, 0x1.1eb852p-2f);
              }
            }
            if (l_rang)
            {
              l_albedo = coax_p_v3(0x1.000000p-2f, 0x1.47ae14p-3f, 0x1.47ae14p-4f);
            }
            if (l_rog)
            {
              l_albedo = coax_p_v3(0x1.666666p-1f, 0x1.70a3d8p-1f, 0x1.851eb8p-1f);
            }
            if (l_sorc)
            {
              l_albedo = coax_p_v3(0x1.000000p0f, 0x1.333334p-1f, 0x1.000000p-2f);
              l_glow = coax_p_add3(l_glow, coax_p_v3(0x1.000000p0f, 0x1.ae147ap-2f, 0x1.eb851ep-4f));
            }
            if (l_pact)
            {
              l_albedo = coax_p_v3(0x1.eb851ep-5f, 0x1.99999ap-5f, 0x1.eb851ep-5f);
            }
            if (l_bones)
            {
              l_albedo = coax_p_v3(0x1.3d70a4p-1f, 0x1.47ae14p-1f, 0x1.5c28f6p-1f);
            }
            if (l_barb)
            {
              l_albedo = coax_p_v3(0x1.ae147ap-2f, 0x1.333334p-2f, 0x1.47ae14p-3f);
              if ((l_pos.y > 0x1.733334p0f))
              {
                l_albedo = coax_p_v3(0x1.666666p-1f, 0x1.70a3d8p-1f, 0x1.851eb8p-1f);
              }
            }
          }
        }
        if (coax_p_fivelab_mark(l_ts_30.z, 0x1.000000p0f))
        {
          l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.99999ap-2f, 0x1.19999ap-1f, 0x1.47ae14p-4f), 0x1.ccccccp-2f);
        }
        if (coax_p_fivelab_mark(l_ts_30.z, 0x1.800000p1f))
        {
          l_albedo = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), (coax_p_verse_style_luma(l_albedo.x, l_albedo.y, l_albedo.z) * 0x1.99999ap-1f));
        }
        if (coax_p_fivelab_mark(l_ts_30.z, 0x1.000000p2f))
        {
          l_wr = coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f))))), 0x1.800000p1f);
          l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.ccccccp-1f, 0x1.333334p-3f, 0x1.000000p-2f), (l_wr * 0x1.ccccccp-1f)));
        }
        if (coax_p_fivelab_mark(l_ts_30.z, 0x1.400000p2f))
        {
          l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.b33334p-1f, 0x1.666666p-1f, 0x1.333334p-2f), 0x1.666666p-2f);
          l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.666666p-2f), (((0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f))))) * (0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f)))))) * 0x1.99999ap-1f)));
        }
        if (coax_p_fivelab_mark(l_ts_30.z, 0x1.800000p2f))
        {
          l_hx = coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f))))), 0x1.000000p2f);
          l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-3f, 0x1.99999ap-1f), (l_hx * (0x1.666666p-1f + (0x1.333334p-2f * sin(((l_t * 0x1.200000p3f) + (l_pos.y * 0x1.800000p2f))))))));
        }
        if ((l_ts_30.x > 0x1.800000p0f))
        {
          l_albedo = coax_p_scale3(coax_p_v3(0x1.99999ap-2f, 0x1.99999ap-2f, 0x1.ae147ap-2f), (coax_p_verse_style_luma(l_albedo.x, l_albedo.y, l_albedo.z) * 0x1.19999ap1f));
        }
        if ((l_ts_30.y > 0x1.000000p-1f))
        {
          l_rim = ((0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f))))) * (0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f))))));
          l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.666666p-1f, 0x1.99999ap-2f, 0x1.000000p0f), ((0x1.666666p-2f + (0x1.000000p-2f * sin((l_t * 0x1.400000p2f)))) + (l_rim * 0x1.ccccccp-1f))));
        }
      }
    }
  }
  l_ndl = dot(l_n, l_sun);
  l_lit = 0x1.000000p0f;
  if ((l_ndl > 0.0f))
  {
    l_sh = coax_p_fivelab_trace(coax_p_add3(l_pos, coax_p_scale3(l_n, 0x1.47ae14p-6f)), l_sun, true);
    if ((l_sh.z > 0.0f))
    {
      l_lit = 0.0f;
    }
  }
  l_w = (coax_p_verse_style_wrap(l_ndl) * l_lit);
  l_band = coax_p_verse_style_bands(l_w, 0x1.99999ap-3f, 0x1.28f5c2p-1f, 0x1.eb851ep-5f, 0x1.19999ap-1f);
  if ((l_h.z > 0x1.400000p1f))
  {
    l_band = coax_p_verse_style_bands2(l_w, 0x1.99999ap-2f, 0x1.47ae14p-4f);
  }
  l_amb = coax_p_verse_style_ambient(l_n.y, 0x1.000000p0f, 0x1.19999ap-1f, 0x1.000000p0f);
  l_shadow_light = coax_p_scale3(coax_p_v3(0x1.70a3d8p-2f, 0x1.c28f5cp-2f, 0x1.3d70a4p-1f), (0x1.3d70a4p-1f * l_amb));
  l_lit_light = coax_p_add3(coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.7ae148p-1f), 0x1.400000p0f), coax_p_scale3(l_shadow_light, 0x1.333334p-1f));
  l_c = coax_p_v3(coax_p_verse_style_shade(l_albedo.x, l_shadow_light.x, l_lit_light.x, l_band), coax_p_verse_style_shade(l_albedo.y, l_shadow_light.y, l_lit_light.y, l_band), coax_p_verse_style_shade(l_albedo.z, l_shadow_light.z, l_lit_light.z, l_band));
  l_c = coax_p_add3(l_c, l_glow);
  l_hz = (0x1.000000p0f - exp(((-l_h.x) * 0x1.6872b0p-7f)));
  l_hc = coax_p_mix3(coax_p_v3(0x1.70a3d8p-1f, 0x1.99999ap-1f, 0x1.ae147ap-1f), coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.800000p-1f), (coax_p_verse_style_halo(dot(l_rd, l_sun), 0x1.400000p2f) * 0x1.000000p-1f));
  return coax_p_mix3(l_c, l_hc, (l_hz * 0x1.b33334p-1f));
}
fn coax_p_verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn coax_p_verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
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
fn coax_p_fivelab_flat(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_r: f32;
  var l_c: vec3f;
  var l_gx1: f32;
  var l_gy1: f32;
  var l_inside: f32;
  l_q = coax_p_v2(((l_p.x / coax_at_u(i32(0i))) - 0x1.000000p-1f), ((l_p.y / coax_at_u(i32(1i))) - 0x1.000000p-1f));
  l_r = coax_p_wg_length2(l_q);
  l_c = coax_p_scale3(coax_p_v3(0x1.eb851ep-6f, 0x1.16872cp-5f, 0x1.47ae14p-5f), (0x1.000000p0f - coax_p_fmin(l_r, 0x1.000000p0f)));
  l_gx1 = (coax_at_u(i32(4i)) + (coax_at_u(i32(8i)) * coax_at_u(i32(6i))));
  l_gy1 = (coax_at_u(i32(5i)) + (coax_at_u(i32(9i)) * coax_at_u(i32(6i))));
  l_inside = 0x1.70a3d8p-3f;
  if (((((l_p.x >= coax_at_u(i32(4i))) && (l_p.x <= l_gx1)) && (l_p.y >= coax_at_u(i32(5i)))) && (l_p.y <= l_gy1)))
  {
    l_inside = 0x1.000000p0f;
    l_c = coax_p_add3(l_c, coax_p_v3(0x1.89374cp-7f, 0x1.0624dep-6f, 0x1.cac084p-7f));
  }
  return coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.99999ap-3f, 0x1.d70a3ep-3f, 0x1.c28f5cp-3f), (coax_p_fivelab_lines(l_p) * l_inside)));
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
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
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  var l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_fivelab_trace(p_ro: vec3f, p_rd: vec3f, p_shadow: bool) -> coax_t13 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_shadow: bool = p_shadow;
  var l_h: coax_t13;
  var l_tg: f32;
  var l_i: i32;
  var l_tr: vec4f;
  var l_ex: vec2f;
  var l_front: f32;
  var l_k: f32;
  var l_c1: vec3f;
  var l_tc: f32;
  var l_c2: vec3f;
  var l_tc2: f32;
  var l_ta: vec3f;
  var l_tb: vec3f;
  var l_tt: f32;
  var l_i_18: i32;
  var l_tk: vec4f;
  var l_bo: vec3f;
  var l_bb: f32;
  var l_bh: f32;
  var l_near: bool;
  var l_bs: f32;
  var l_bd: coax_t14;
  var l_t1: f32;
  var l_t2: f32;
  var l_t3: f32;
  l_h = coax_t13();
  l_h.x = 0x1.dcd650p29f;
  l_h.y = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_h.z = (-0x1.000000p0f);
  l_h.w = (-0x1.000000p0f);
  if ((l_rd.y < (-0x1.a36e2ep-14f)))
  {
    l_tg = ((-l_ro.y) / l_rd.y);
    if ((!l_shadow))
    {
      l_h.x = l_tg;
      l_h.z = 0.0f;
    }
  }
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_tr = coax_p_fivelab_tree(l_i);
      l_ex = coax_p_v2(coax_at_u(i32(16i)), coax_at_u(i32(18i)));
      l_front = (((l_tr.x * l_ex.x) + (l_tr.z * l_ex.y)) / coax_p_fmax((coax_p_wg_length2(coax_p_v2(l_tr.x, l_tr.z)) * coax_p_wg_length2(l_ex)), 0x1.0624dep-10f));
      if ((l_front > 0x1.ccccccp-2f))
      {
        l_tr.y = (-0x1.000000p0f);
      }
      l_k = l_tr.w;
      if ((l_tr.y < 0.0f))
      {
        l_k = 0.0f;
      }
      l_c1 = coax_p_v3(l_tr.x, (0x1.266666p2f * l_k), l_tr.z);
      l_tc = coax_p_fivelab_sphere(l_ro, l_rd, l_c1, (0x1.59999ap1f * l_k));
      if (((l_tc > 0.0f) && (l_tc < l_h.x)))
      {
        l_h.x = l_tc;
        l_h.y = coax_p_normalize3(coax_p_sub3(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc)), l_c1));
        l_h.z = 0x1.000000p1f;
        l_h.w = f32(l_i);
      }
      l_c2 = coax_p_v3((l_tr.x + (0x1.ccccccp-1f * l_k)), (0x1.933334p2f * l_k), (l_tr.z - (0x1.99999ap-2f * l_k)));
      l_tc2 = coax_p_fivelab_sphere(l_ro, l_rd, l_c2, (0x1.e66666p0f * l_k));
      if (((l_tc2 > 0.0f) && (l_tc2 < l_h.x)))
      {
        l_h.x = l_tc2;
        l_h.y = coax_p_normalize3(coax_p_sub3(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc2)), l_c2));
        l_h.z = 0x1.000000p1f;
        l_h.w = f32(l_i);
      }
      l_ta = coax_p_v3(l_tr.x, 0.0f, l_tr.z);
      l_tb = coax_p_v3(l_tr.x, (0x1.b33334p1f * l_k), l_tr.z);
      l_tt = coax_p_fivelab_capsule(l_ro, l_rd, l_ta, l_tb, (0x1.47ae14p-2f * l_k));
      if (((l_tt > 0.0f) && (l_tt < l_h.x)))
      {
        l_h.x = l_tt;
        l_h.y = coax_p_fivelab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tt)), l_ta, l_tb);
        l_h.z = 0x1.000000p0f;
        l_h.w = f32(l_i);
      }
      l_i = (l_i + 1i);
    }
  }
  {
    l_i_18 = 0i;
    while ((l_i_18 < 208i))
    {
      l_tk = coax_p_wg_at4((36i + (l_i_18 * 8i)));
      l_bo = coax_p_sub3(l_ro, coax_p_v3(l_tk.x, 0x1.e66666p-1f, l_tk.y));
      l_bb = dot(l_bo, l_rd);
      l_bh = ((l_bb * l_bb) - (dot(l_bo, l_bo) - 0x1.200000p1f));
      l_near = false;
      if ((l_bh > 0.0f))
      {
        l_bs = sqrt(l_bh);
        l_near = ((((-l_bb) + l_bs) > 0.0f) && (((-l_bb) - l_bs) < l_h.x));
      }
      if (((l_tk.z > (-0x1.000000p-1f)) && l_near))
      {
        l_bd = coax_p_fivelab_body(l_i_18);
        l_t1 = coax_p_fivelab_capsule(l_ro, l_rd, l_bd.x, l_bd.y, l_bd.z);
        if (((l_t1 > 0.0f) && (l_t1 < l_h.x)))
        {
          l_h.x = l_t1;
          l_h.y = coax_p_fivelab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_t1)), l_bd.x, l_bd.y);
          l_h.z = 0x1.800000p1f;
          l_h.w = f32(l_i_18);
        }
        l_t2 = coax_p_fivelab_sphere(l_ro, l_rd, l_bd.w, l_bd.f4);
        if (((l_t2 > 0.0f) && (l_t2 < l_h.x)))
        {
          l_h.x = l_t2;
          l_h.y = coax_p_normalize3(coax_p_sub3(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_t2)), l_bd.w));
          l_h.z = 0x1.000000p2f;
          l_h.w = f32(l_i_18);
        }
        l_t3 = coax_p_fivelab_capsule(l_ro, l_rd, l_bd.f5, l_bd.f6, l_bd.f7);
        if (((l_t3 > 0.0f) && (l_t3 < l_h.x)))
        {
          l_h.x = l_t3;
          l_h.y = coax_p_fivelab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_t3)), l_bd.f5, l_bd.f6);
          l_h.z = 0x1.400000p2f;
          l_h.w = f32(l_i_18);
        }
      }
      l_i_18 = (l_i_18 + 1i);
    }
  }
  return l_h;
}
fn coax_p_fivelab_sky(p_rd: vec3f, p_sun: vec3f, p_t: f32) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_sun: vec3f = p_sun;
  var l_t: f32 = p_t;
  var l_up: f32;
  var l_st: f32;
  var l_c: vec3f;
  var l_halo: f32;
  var l_disc: f32;
  var l_k: f32;
  var l_x: f32;
  var l_z: f32;
  var l_n: f32;
  var l_cover: f32;
  var l_lit: f32;
  var l_cloud: vec3f;
  l_up = coax_p_fmax(l_rd.y, 0.0f);
  l_st = coax_p_verse_style_sky_t(l_up);
  l_c = coax_p_mix3(coax_p_v3(0x1.8f5c28p-1f, 0x1.ae147ap-1f, 0x1.b851ecp-1f), coax_p_v3(0x1.47ae14p-2f, 0x1.0a3d70p-1f, 0x1.ae147ap-1f), l_st);
  l_halo = coax_p_verse_style_halo(dot(l_rd, l_sun), 0x1.c00000p2f);
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), (l_halo * 0x1.ccccccp-2f)));
  l_disc = coax_p_verse_style_halo(dot(l_rd, l_sun), 0x1.c20000p9f);
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b33334p-1f), (l_disc * 0x1.800000p1f)));
  if ((l_rd.y > 0x1.47ae14p-6f))
  {
    l_k = (0x1.2c0000p8f / l_rd.y);
    l_x = ((l_rd.x * l_k) + (l_t * 0x1.000000p1f));
    l_z = (l_rd.z * l_k);
    l_n = coax_p_verse_style_cloud_field(l_x, l_z, 0x1.a36e2ep-9f, 0x1.2a3056p-7f, 0x1.000000p1f);
    l_cover = (coax_p_verse_style_cloud_cover(l_n, 0x1.99999ap-2f, 0x1.eb851ep-2f, 0x1.3d70a4p-1f) * coax_p_verse_style_cloud_fade(l_rd.y));
    l_lit = coax_p_verse_style_cloud_lit(l_n, coax_p_verse_style_cloud_field((l_x + (l_sun.x * 0x1.400000p5f)), (l_z + (l_sun.z * 0x1.400000p5f)), 0x1.a36e2ep-9f, 0x1.2a3056p-7f, 0x1.000000p1f));
    l_cloud = coax_p_mix3(coax_p_v3(0x1.3d70a4p-1f, 0x1.51eb86p-1f, 0x1.851eb8p-1f), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), l_lit);
    l_c = coax_p_mix3(l_c, l_cloud, l_cover);
  }
  return l_c;
}
fn coax_p_fivelab_grass(p_x: f32, p_z: f32, p_t: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: f32 = p_t;
  var l_n1: f32;
  var l_n2: f32;
  var l_dab: f32;
  var l_c: vec3f;
  var l_hw: f32;
  var l_hd: f32;
  var l_sq: f32;
  var l_inside: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_lx: f32;
  var l_lz: f32;
  var l_line: f32;
  l_n1 = coax_p_verse_style_noise((l_x * 0x1.666666p-2f), (l_z * 0x1.666666p-2f));
  l_n2 = coax_p_verse_style_noise(((l_x * 0x1.b33334p0f) + 0x1.400000p2f), ((l_z * 0x1.b33334p0f) + 0x1.000000p1f));
  l_dab = coax_p_verse_style_noise((l_x * 0x1.800000p2f), (l_z * 0x1.800000p2f));
  l_c = coax_p_mix3(coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-2f, 0x1.1eb852p-4f), coax_p_v3(0x1.51eb86p-2f, 0x1.d70a3ep-2f, 0x1.0a3d70p-3f), ((l_n1 * 0x1.4cccccp-1f) + (l_n2 * 0x1.666666p-2f)));
  l_c = coax_p_scale3(l_c, (0x1.c28f5cp-1f + (0x1.eb851ep-3f * l_dab)));
  l_hw = coax_at_u(i32(1764i));
  l_hd = coax_at_u(i32(1765i));
  l_sq = coax_p_fmax(coax_at_u(i32(1766i)), 0x1.99999ap-4f);
  l_inside = ((0x1.000000p0f - coax_p_smoothstep(l_hw, (l_hw + 0x1.333334p-2f), abs(l_x))) * (0x1.000000p0f - coax_p_smoothstep(l_hd, (l_hd + 0x1.333334p-2f), abs(l_z))));
  l_c = coax_p_mix3(l_c, coax_p_add3(coax_p_scale3(l_c, 0x1.a3d70ap-1f), coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.47ae14p-6f)), (l_inside * 0x1.333334p-1f));
  l_gx = ((l_x + l_hw) / l_sq);
  l_gz = ((l_z + l_hd) / l_sq);
  l_lx = (abs((coax_p_fract((l_gx + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_sq);
  l_lz = (abs((coax_p_fract((l_gz + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_sq);
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.70a3d8p-5f, coax_p_fmin(l_lx, l_lz)));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.19999ap-1f, 0x1.333334p-1f, 0x1.70a3d8p-2f), ((l_line * l_inside) * 0x1.000000p-1f));
  return l_c;
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_verse_style_noise(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = coax_p_verse_style_hash(l_ix, l_iy);
  l_b = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = coax_p_verse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_fivelab_team(p_look: f32) -> f32 {
  var l_look: f32 = p_look;
  return floor(((l_look + 0x1.000000p-1f) / 0x1.000000p4f));
}
fn coax_p_fivelab_kind(p_look: f32) -> f32 {
  var l_look: f32 = p_look;
  return (l_look - (0x1.000000p4f * floor(((l_look + 0x1.000000p-1f) / 0x1.000000p4f))));
}
fn coax_p_fivelab_is(p_kind: f32, p_k: f32) -> bool {
  var l_kind: f32 = p_kind;
  var l_k: f32 = p_k;
  return (abs((l_kind - l_k)) < 0x1.000000p-1f);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
fn coax_p_fivelab_mark(p_marks: f32, p_k: f32) -> bool {
  var l_marks: f32 = p_marks;
  var l_k: f32 = p_k;
  return (coax_p_wg_fmod(floor((l_marks / coax_p_wg_exp2(l_k))), 0x1.000000p1f) > 0x1.000000p-1f);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_verse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e0: f32 = p_e0;
  var l_e1: f32 = p_e1;
  var l_s: f32 = p_s;
  var l_mid: f32 = p_mid;
  return ((l_mid * coax_p_verse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_verse_style_soft_step(l_e1, l_s, l_w)));
}
fn coax_p_verse_style_bands2(p_w: f32, p_e: f32, p_s: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  return coax_p_verse_style_soft_step(l_e, l_s, l_w);
}
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_verse_style_shade(p_albedo: f32, p_shadow_light: f32, p_lit_light: f32, p_band: f32) -> f32 {
  var l_albedo: f32 = p_albedo;
  var l_shadow_light: f32 = p_shadow_light;
  var l_lit_light: f32 = p_lit_light;
  var l_band: f32 = p_band;
  return (l_albedo * (l_shadow_light + ((l_lit_light - l_shadow_light) * coax_p_st_sat(l_band))));
}
fn coax_p_verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
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
fn coax_p_fivelab_lines(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_s: f32;
  var l_x: f32;
  var l_y: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_line: f32;
  var l_mx: f32;
  var l_my: f32;
  var l_major: f32;
  l_s = coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f);
  l_x = ((l_p.x - coax_at_u(i32(4i))) / l_s);
  l_y = ((l_p.y - coax_at_u(i32(5i))) / l_s);
  l_dx = (abs((coax_p_fract((l_x + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s);
  l_dy = (abs((coax_p_fract((l_y + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s);
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-2f, 0x1.19999ap0f, coax_p_fmin(l_dx, l_dy)));
  l_mx = ((abs((coax_p_fract(((l_x * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s) * 0x1.000000p2f);
  l_my = ((abs((coax_p_fract(((l_y * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s) * 0x1.000000p2f);
  l_major = (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-2f, 0x1.666666p0f, coax_p_fmin(l_mx, l_my)));
  return ((l_line * 0x1.333334p-1f) + (l_major * 0x1.99999ap-2f));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_fivelab_tree(p_i: i32) -> vec4f {
  var l_i: i32 = p_i;
  var l_fi: f32;
  var l_a: f32;
  var l_rx: f32;
  var l_rz: f32;
  var l_size: f32;
  l_fi = f32(l_i);
  l_a = ((l_fi * 0x1.921fb6p-2f) + (0x1.666666p-2f * sin((l_fi * 0x1.266666p1f))));
  l_rx = ((coax_at_u(i32(1764i)) + 0x1.600000p2f) + (0x1.400000p1f * coax_p_verse_style_hash(l_fi, 0x1.800000p1f)));
  l_rz = ((coax_at_u(i32(1765i)) + 0x1.400000p2f) + (0x1.400000p1f * coax_p_verse_style_hash(l_fi, 0x1.c00000p2f)));
  l_size = (0x1.99999ap-1f + (0x1.ccccccp-2f * coax_p_verse_style_hash(l_fi, 0x1.600000p3f)));
  return coax_p_v4((cos(l_a) * l_rx), 0.0f, (sin(l_a) * l_rz), l_size);
}
fn coax_p_fivelab_sphere(p_ro: vec3f, p_rd: vec3f, p_c: vec3f, p_r: f32) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_oc: vec3f;
  var l_b: f32;
  var l_k: f32;
  var l_h: f32;
  var l_t: f32;
  l_oc = coax_p_sub3(l_ro, l_c);
  l_b = dot(l_oc, l_rd);
  l_k = (dot(l_oc, l_oc) - (l_r * l_r));
  l_h = ((l_b * l_b) - l_k);
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = ((-l_b) - sqrt(l_h));
  if ((l_t < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return l_t;
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_fivelab_capsule(p_ro: vec3f, p_rd: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_r: f32 = p_r;
  var l_ba: vec3f;
  var l_oa: vec3f;
  var l_baba: f32;
  var l_bard: f32;
  var l_baoa: f32;
  var l_rdoa: f32;
  var l_oaoa: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_h: f32;
  var l_t: f32;
  var l_y: f32;
  var l_oc: vec3f;
  var l_b2: f32;
  var l_c2: f32;
  var l_h2: f32;
  var l_t2: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_oa = coax_p_sub3(l_ro, l_a);
  l_baba = dot(l_ba, l_ba);
  l_bard = dot(l_ba, l_rd);
  l_baoa = dot(l_ba, l_oa);
  l_rdoa = dot(l_rd, l_oa);
  l_oaoa = dot(l_oa, l_oa);
  l_qa = (l_baba - (l_bard * l_bard));
  l_qb = ((l_baba * l_rdoa) - (l_baoa * l_bard));
  l_qc = (((l_baba * l_oaoa) - (l_baoa * l_baoa)) - ((l_r * l_r) * l_baba));
  l_h = ((l_qb * l_qb) - (l_qa * l_qc));
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = (((-l_qb) - sqrt(l_h)) / coax_p_fmax(l_qa, 0x1.0c6f7ap-20f));
  l_y = (l_baoa + (l_t * l_bard));
  if (((l_y > 0.0f) && (l_y < l_baba)))
  {
    if ((l_t < 0.0f))
    {
      return (-0x1.000000p0f);
    }
    return l_t;
  }
  l_oc = l_oa;
  if ((l_y >= l_baba))
  {
    l_oc = coax_p_sub3(l_ro, l_b);
  }
  l_b2 = dot(l_rd, l_oc);
  l_c2 = (dot(l_oc, l_oc) - (l_r * l_r));
  l_h2 = ((l_b2 * l_b2) - l_c2);
  if ((l_h2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t2 = ((-l_b2) - sqrt(l_h2));
  if ((l_t2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return l_t2;
}
fn coax_p_fivelab_capsule_n(p_p: vec3f, p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_ba: vec3f;
  var l_pa: vec3f;
  var l_h: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_pa = coax_p_sub3(l_p, l_a);
  l_h = coax_p_saturate((dot(l_pa, l_ba) / coax_p_fmax(dot(l_ba, l_ba), 0x1.0c6f7ap-20f)));
  return coax_p_normalize3(coax_p_sub3(l_pa, coax_p_scale3(l_ba, l_h)));
}
fn coax_p_fivelab_body(p_i: i32) -> coax_t14 {
  var l_i: i32 = p_i;
  var l_t: vec4f;
  var l_s: vec4f;
  var l_o: coax_t14;
  var l_kd: f32;
  var l_big: f32;
  var l_monk: bool;
  var l_bones: bool;
  l_t = coax_p_wg_at4((36i + (l_i * 8i)));
  l_s = coax_p_wg_at4((40i + (l_i * 8i)));
  l_o = coax_t14();
  l_kd = coax_p_fivelab_kind(l_t.z);
  l_big = 0x1.000000p0f;
  if (coax_p_fivelab_is(l_kd, 0x1.000000p1f))
  {
    l_big = 0x1.3851ecp0f;
  }
  if ((coax_p_fivelab_is(l_kd, 0x1.000000p3f) || coax_p_fivelab_is(l_kd, 0x1.200000p3f)))
  {
    l_big = 0x1.19999ap0f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.600000p3f))
  {
    l_big = 0x1.e147aep-1f;
  }
  l_monk = coax_p_fivelab_is(l_kd, 0x1.800000p1f);
  l_bones = coax_p_fivelab_is(l_kd, 0x1.000000p2f);
  if (l_bones)
  {
    l_big = 0x1.3d70a4p-1f;
  }
  l_o.f7 = 0x1.47ae14p-5f;
  if ((l_s.x > 0x1.000000p-1f))
  {
    l_o.x = coax_p_v3((l_t.x - 0x1.19999ap-1f), 0x1.eb851ep-3f, l_t.y);
    l_o.y = coax_p_v3((l_t.x + 0x1.ccccccp-2f), 0x1.eb851ep-3f, l_t.y);
    l_o.z = 0x1.eb851ep-3f;
    l_o.w = coax_p_v3((l_t.x + 0x1.a3d70ap-1f), 0x1.99999ap-3f, l_t.y);
    l_o.f4 = 0x1.851eb8p-3f;
    l_o.f5 = coax_p_v3((l_t.x - 0x1.333334p-2f), 0x1.47ae14p-5f, (l_t.y + 0x1.ae147ap-2f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.000000p0f), 0x1.47ae14p-5f, (l_t.y + 0x1.19999ap-1f));
    if ((l_monk || coax_p_fivelab_is(l_kd, 0x1.800000p3f)))
    {
      l_o.f5 = coax_p_v3(l_t.x, (-0x1.200000p3f), l_t.y);
      l_o.f6 = coax_p_v3(l_t.x, (-0x1.000000p3f), l_t.y);
    }
    return l_o;
  }
  l_o.x = coax_p_v3(l_t.x, 0x1.47ae14p-2f, l_t.y);
  l_o.y = coax_p_v3(l_t.x, (0x1.1eb852p0f * coax_p_fmin(l_big, 0x1.147ae2p0f)), l_t.y);
  l_o.z = (0x1.333334p-2f * l_big);
  l_o.w = coax_p_v3(l_t.x, (0x1.8f5c28p0f * coax_p_fmin(l_big, 0x1.147ae2p0f)), l_t.y);
  l_o.f4 = 0x1.99999ap-3f;
  l_o.f5 = coax_p_v3((l_t.x + (0x1.ae147ap-2f * l_big)), 0.0f, (l_t.y + 0x1.47ae14p-4f));
  l_o.f6 = coax_p_v3((l_t.x + (0x1.ae147ap-2f * l_big)), 0x1.d9999ap0f, (l_t.y + 0x1.47ae14p-4f));
  if (l_monk)
  {
    l_o.f5 = coax_p_v3(l_t.x, (-0x1.200000p3f), l_t.y);
    l_o.f6 = coax_p_v3(l_t.x, (-0x1.000000p3f), l_t.y);
  }
  if (l_bones)
  {
    l_o.y = coax_p_v3(l_t.x, 0x1.1eb852p0f, l_t.y);
    l_o.w = coax_p_v3(l_t.x, 0x1.800000p0f, l_t.y);
    l_o.f4 = 0x1.5c28f6p-3f;
    l_o.f5 = coax_p_v3((l_t.x + 0x1.333334p-2f), 0x1.800000p-1f, (l_t.y + 0x1.99999ap-4f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.ccccccp-2f), 0x1.666666p0f, (l_t.y + 0x1.c28f5cp-3f));
  }
  if (coax_p_fivelab_is(l_kd, 0x1.800000p2f))
  {
    l_o.f5 = coax_p_v3((l_t.x - 0x1.99999ap-4f), 0x1.ae147ap-1f, (l_t.y + 0x1.333334p-2f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.333334p-2f), 0x1.2e147ap0f, (l_t.y + 0x1.333334p-2f));
    l_o.f7 = 0x1.99999ap-4f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.c00000p2f))
  {
    l_o.f5 = coax_p_v3((l_t.x + 0x1.99999ap-2f), 0x1.333334p-1f, (l_t.y + 0x1.99999ap-4f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.99999ap-2f), 0x1.1eb852p0f, (l_t.y + 0x1.99999ap-4f));
    l_o.f7 = 0x1.0a3d70p-4f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.000000p3f))
  {
    l_o.f5 = coax_p_v3((l_t.x + 0x1.ae147ap-2f), 0x1.666666p-2f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.ae147ap-2f), 0x1.000000p1f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f7 = 0x1.99999ap-5f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.200000p3f))
  {
    l_o.f5 = coax_p_v3((l_t.x - 0x1.c28f5cp-2f), 0x1.8f5c28p-1f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f6 = coax_p_v3((l_t.x - 0x1.c28f5cp-2f), 0x1.2e147ap0f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f7 = 0x1.000000p-2f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.400000p3f))
  {
    l_o.f5 = coax_p_v3((l_t.x + 0x1.ae147ap-2f), 0x1.851eb8p-2f, l_t.y);
    l_o.f6 = coax_p_v3((l_t.x + 0x1.000000p-1f), 0x1.d1eb86p0f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f7 = 0x1.1eb852p-5f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.600000p3f))
  {
    l_o.f5 = coax_p_v3((l_t.x + 0x1.5c28f6p-2f), 0x1.3d70a4p-1f, (l_t.y + 0x1.70a3d8p-3f));
    l_o.f6 = coax_p_v3((l_t.x + 0x1.000000p-1f), 0x1.0a3d70p0f, (l_t.y + 0x1.47ae14p-2f));
    l_o.f7 = 0x1.1eb852p-5f;
  }
  if (coax_p_fivelab_is(l_kd, 0x1.800000p3f))
  {
    l_o.f5 = coax_p_v3((l_t.x + 0x1.d70a3ep-2f), 0x1.4cccccp0f, (l_t.y + 0x1.eb851ep-4f));
    l_o.f6 = l_o.f5;
    l_o.f7 = 0x1.eb851ep-4f;
  }
  return l_o;
}
fn coax_p_verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_bg_vs(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn coax_p_verse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_s1: f32 = p_s1;
  var l_s2: f32 = p_s2;
  var l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = coax_p_verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (coax_p_verse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn coax_p_verse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_cover: f32 = p_cover;
  var l_cut_lo: f32 = p_cut_lo;
  var l_cut_hi: f32 = p_cut_hi;
  return (coax_p_verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_verse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn coax_p_verse_style_cloud_fade(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return (coax_p_verse_style_soft_step(0x1.5c28f6p-4f, 0x1.333334p-4f, l_up) * 0x1.d70a3ep-1f);
}
fn coax_p_verse_style_cloud_lit(p_n: f32, p_toward: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_toward: f32 = p_toward;
  return coax_p_st_sat((((l_n - l_toward) * 0x1.400000p2f) + 0x1.19999ap-1f));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
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
fn coax_p_wg_exp2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return exp((l_x * 0x1.62e430p-1f));
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
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
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
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