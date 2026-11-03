// ---- generated: apps/openagents-web/gpu/metahumanlab.coax (the module) by coax wgsl; do not edit
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 65>;
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
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_c = coax_p_mhlab_scene(l_p);
  l_c = coax_p_v3(coax_p_verse_style_tone(l_c.x, 0x1.333334p0f), coax_p_verse_style_tone(l_c.y, 0x1.333334p0f), coax_p_verse_style_tone(l_c.z, 0x1.333334p0f));
  if (((l_p.y < coax_at_u(i32(6i))) || (l_p.y > (coax_at_u(i32(1i)) - coax_at_u(i32(7i))))))
  {
    l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.c28f5cp-3f), coax_p_v3(0x1.47ae14p-7f, 0x1.89374cp-7f, 0x1.0624dep-6f));
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
  return coax_p_mhlab_make_f4_f3f(coax_p_scale3(coax_p_scale3(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_mhlab_scene(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_eye: vec3f;
  var l_f: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_rd: vec3f;
  var l_h: coax_t13;
  var l_c: vec3f;
  var l_back: coax_t13;
  var l_behind: vec3f;
  l_eye = coax_p_wg_at3(16i);
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_fwd = coax_p_wg_at3(20i);
  l_right = coax_p_wg_at3(24i);
  l_up = coax_p_wg_at3(28i);
  l_rd = coax_p_normalize3(coax_p_add3(l_fwd, coax_p_add3(coax_p_scale3(l_right, ((l_p.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(l_up, ((coax_at_u(i32(27i)) - l_p.y) / l_f)))));
  l_h = coax_p_mhlab_trace(l_eye, l_rd, false, true);
  l_c = coax_p_mhlab_shade(l_eye, l_rd, l_h, true);
  if (((coax_at_u(i32(4i)) > 0x1.000000p-1f) && (l_h.z > 0x1.000000p-1f)))
  {
    l_back = coax_p_mhlab_trace(l_eye, l_rd, false, false);
    l_behind = coax_p_mhlab_shade(l_eye, l_rd, l_back, false);
    l_c = coax_p_mix3(l_behind, coax_p_scale3(l_c, 0x1.99999ap-1f), 0x1.47ae14p-2f);
  }
  return l_c;
}
fn coax_p_verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
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
fn coax_p_mhlab_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
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
fn coax_p_mhlab_trace(p_ro: vec3f, p_rd: vec3f, p_shadow: bool, p_figure: bool) -> coax_t13 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_shadow: bool = p_shadow;
  var l_figure: bool = p_figure;
  var l_h: coax_t13;
  var l_bound: vec4f;
  var l_i: i32;
  var l_ca: vec4f;
  var l_cb: vec4f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_tc: f32;
  l_h = coax_t13();
  l_h.x = 0x1.dcd650p29f;
  l_h.y = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_h.z = (-0x1.000000p0f);
  l_h.w = (-0x1.000000p0f);
  if (((l_rd.y < (-0x1.a36e2ep-14f)) && (!l_shadow)))
  {
    l_h.x = ((-l_ro.y) / l_rd.y);
    l_h.z = 0.0f;
  }
  l_bound = coax_p_wg_at4(36i);
  if ((l_figure && coax_p_mhlab_near(l_ro, l_rd, coax_p_v3(l_bound.x, l_bound.y, l_bound.z), l_bound.w)))
  {
    l_i = 0i;
    while ((l_i < 26i))
    {
      l_ca = coax_p_wg_at4((52i + (l_i * 8i)));
      l_cb = coax_p_wg_at4((56i + (l_i * 8i)));
      if ((l_ca.w > 0.0f))
      {
        l_a = coax_p_v3(l_ca.x, l_ca.y, l_ca.z);
        l_b = coax_p_v3(l_cb.x, l_cb.y, l_cb.z);
        l_tc = coax_p_mhlab_capsule(l_ro, l_rd, l_a, l_b, l_ca.w);
        if (((l_tc > 0.0f) && (l_tc < l_h.x)))
        {
          l_h.x = l_tc;
          l_h.y = coax_p_mhlab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc)), l_a, l_b);
          l_h.z = l_cb.w;
          l_h.w = f32(l_i);
        }
      }
      l_i = (l_i + 1i);
    }
  }
  return l_h;
}
fn coax_p_mhlab_shade(p_eye: vec3f, p_rd: vec3f, p_h: coax_t13, p_figure: bool) -> vec3f {
  var l_eye: vec3f = p_eye;
  var l_rd: vec3f = p_rd;
  var l_h: coax_t13 = p_h;
  var l_figure: bool = p_figure;
  var l_sun: vec3f;
  var l_pos: vec3f;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_spec: f32;
  var l_bound: vec4f;
  var l_d: f32;
  var l_hc: vec3f;
  var l_hf: vec3f;
  var l_hu: vec3f;
  var l_hs: vec3f;
  var l_d_16: vec3f;
  var l_fw: f32;
  var l_upw: f32;
  var l_side: f32;
  var l_eye_d: f32;
  var l_ndl: f32;
  var l_lit: f32;
  var l_sh: coax_t13;
  var l_w: f32;
  var l_amb: f32;
  var l_sky: vec3f;
  var l_key: vec3f;
  var l_c: vec3f;
  var l_v: vec3f;
  var l_hv: vec3f;
  var l_rim: f32;
  var l_fog: f32;
  l_sun = coax_p_normalize3(coax_p_wg_at3(32i));
  if ((l_h.z < 0.0f))
  {
    return coax_p_mhlab_sky(l_rd, l_sun);
  }
  l_pos = coax_p_add3(l_eye, coax_p_scale3(l_rd, l_h.x));
  l_n = l_h.y;
  l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f);
  l_spec = 0.0f;
  if ((l_h.z < 0x1.000000p-1f))
  {
    l_albedo = coax_p_mhlab_floor(l_pos.x, l_pos.z);
    l_bound = coax_p_wg_at4(36i);
    l_d = coax_p_wg_length2(coax_p_v2((l_pos.x - l_bound.x), (l_pos.z - l_bound.z)));
    if (l_figure)
    {
      l_albedo = coax_p_scale3(l_albedo, (0x1.3d70a4p-1f + (0x1.851eb8p-2f * coax_p_smoothstep(0x1.99999ap-5f, 0x1.800000p-1f, l_d))));
    }
  }
  else
  {
    if ((l_h.z < 0x1.800000p0f))
    {
      l_albedo = coax_p_v3(0x1.0a3d70p-3f, 0x1.333334p-3f, 0x1.851eb8p-3f);
      l_spec = 0x1.eb851ep-4f;
    }
    else
    {
      if ((l_h.z < 0x1.400000p1f))
      {
        l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.000000p-1f, 0x1.99999ap-2f);
        l_spec = 0x1.99999ap-3f;
      }
      else
      {
        if ((l_h.z < 0x1.c00000p1f))
        {
          l_albedo = coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-5f, 0x1.1eb852p-4f);
          l_spec = 0x1.333334p-2f;
        }
        else
        {
          l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.000000p-1f, 0x1.99999ap-2f);
          l_spec = 0x1.99999ap-3f;
          l_hc = coax_p_wg_at3(40i);
          l_hf = coax_p_normalize3(coax_p_wg_at3(44i));
          l_hu = coax_p_normalize3(coax_p_wg_at3(48i));
          l_hs = coax_p_wg_cross3(l_hu, l_hf);
          l_d_16 = coax_p_normalize3(coax_p_sub3(l_pos, l_hc));
          l_fw = dot(l_d_16, l_hf);
          l_upw = dot(l_d_16, l_hu);
          l_side = dot(l_d_16, l_hs);
          if (((((l_upw > 0x1.ae147ap-2f) && (l_fw < 0x1.99999ap-1f)) || ((l_fw < (-0x1.333334p-3f)) && (l_upw > (-0x1.19999ap-1f)))) || ((l_upw > 0x1.333334p-3f) && (abs(l_side) > 0x1.a3d70ap-1f))))
          {
            l_albedo = coax_p_v3(0x1.99999ap-4f, 0x1.1eb852p-4f, 0x1.99999ap-5f);
            l_spec = 0x1.666666p-2f;
          }
          l_eye_d = coax_p_wg_length2(coax_p_v2((abs(l_side) - 0x1.5c28f6p-2f), (l_upw - 0x1.eb851ep-5f)));
          if (((l_fw > 0x1.666666p-1f) && (l_eye_d < 0x1.333334p-4f)))
          {
            l_albedo = coax_p_v3(0x1.ccccccp-1f, 0x1.c28f5cp-1f, 0x1.b33334p-1f);
            if ((l_eye_d < 0x1.70a3d8p-5f))
            {
              l_albedo = coax_p_v3(0x1.70a3d8p-3f, 0x1.0a3d70p-2f, 0x1.47ae14p-2f);
            }
            if ((l_eye_d < 0x1.6872b0p-6f))
            {
              l_albedo = coax_p_v3(0x1.47ae14p-6f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
            }
            l_spec = 0x1.99999ap-1f;
          }
          if ((((l_fw > 0x1.666666p-1f) && (abs((l_upw - 0x1.99999ap-3f)) < 0x1.99999ap-6f)) && (abs((abs(l_side) - 0x1.5c28f6p-2f)) < 0x1.c28f5cp-4f)))
          {
            l_albedo = coax_p_v3(0x1.eb851ep-4f, 0x1.47ae14p-4f, 0x1.eb851ep-5f);
          }
        }
      }
    }
  }
  l_ndl = dot(l_n, l_sun);
  l_lit = 0x1.000000p0f;
  if ((l_ndl > 0.0f))
  {
    l_sh = coax_p_mhlab_trace(coax_p_add3(l_pos, coax_p_scale3(l_n, 0x1.47ae14p-7f)), l_sun, true, l_figure);
    if ((l_sh.z > 0.0f))
    {
      l_lit = 0.0f;
    }
  }
  l_w = (coax_p_verse_style_wrap(l_ndl) * l_lit);
  l_amb = coax_p_verse_style_ambient(l_n.y, 0x1.000000p0f, 0x1.000000p-1f, 0x1.000000p0f);
  l_sky = coax_p_scale3(coax_p_v3(0x1.ae147ap-2f, 0x1.eb851ep-2f, 0x1.333334p-1f), (0x1.000000p-1f * l_amb));
  l_key = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.dc28f6p-1f, 0x1.a3d70ap-1f), (0x1.59999ap0f * l_w));
  l_c = coax_p_v3((l_albedo.x * (l_sky.x + l_key.x)), (l_albedo.y * (l_sky.y + l_key.y)), (l_albedo.z * (l_sky.z + l_key.z)));
  l_v = coax_p_scale3(l_rd, (-0x1.000000p0f));
  if ((l_h.z > 0x1.000000p-1f))
  {
    l_hv = coax_p_normalize3(coax_p_add3(l_sun, l_v));
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.c28f5cp-1f), ((l_spec * coax_pow_bg_vs(coax_p_saturate(dot(l_n, l_hv)), 0x1.800000p4f)) * l_lit)));
    l_rim = coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_n, l_v))), 0x1.800000p1f);
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.4cccccp-1f, 0x1.b33334p-1f), (l_rim * 0x1.000000p-2f)));
  }
  l_fog = (0x1.000000p0f - exp(((-l_h.x) * 0x1.1eb852p-5f)));
  return coax_p_mix3(l_c, coax_p_mhlab_sky(l_rd, l_sun), l_fog);
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_mhlab_near(p_ro: vec3f, p_rd: vec3f, p_c: vec3f, p_r: f32) -> bool {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_oc: vec3f;
  var l_b: f32;
  var l_k: f32;
  l_oc = coax_p_sub3(l_ro, l_c);
  l_b = dot(l_oc, l_rd);
  l_k = (dot(l_oc, l_oc) - (l_r * l_r));
  if ((l_k < 0.0f))
  {
    return true;
  }
  if ((l_b > 0.0f))
  {
    return false;
  }
  return (((l_b * l_b) - l_k) > 0.0f);
}
fn coax_p_mhlab_capsule(p_ro: vec3f, p_rd: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> f32 {
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
  l_baba = coax_p_fmax(dot(l_ba, l_ba), 0x1.ad7f2ap-24f);
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
fn coax_p_mhlab_capsule_n(p_p: vec3f, p_a: vec3f, p_b: vec3f) -> vec3f {
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
fn coax_p_mhlab_sky(p_rd: vec3f, p_sun: vec3f) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_sun: vec3f = p_sun;
  var l_up: f32;
  var l_c: vec3f;
  l_up = coax_p_saturate(l_rd.y);
  l_c = coax_p_mix3(coax_p_v3(0x1.000000p-1f, 0x1.0f5c28p-1f, 0x1.23d70ap-1f), coax_p_v3(0x1.99999ap-3f, 0x1.d70a3ep-3f, 0x1.28f5c2p-2f), coax_pow_bg_vs(l_up, 0x1.333334p-1f));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.800000p-1f), (coax_p_verse_style_halo(dot(l_rd, l_sun), 0x1.800000p2f) * 0x1.eb851ep-4f)));
  return l_c;
}
fn coax_p_mhlab_floor(p_x: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_c: vec3f;
  var l_lx: f32;
  var l_lz: f32;
  var l_line: f32;
  var l_sx: f32;
  var l_sz: f32;
  var l_fine: f32;
  l_c = coax_p_v3(0x1.d70a3ep-2f, 0x1.e147aep-2f, 0x1.f5c290p-2f);
  l_c = coax_p_scale3(l_c, (0x1.e147aep-1f + (0x1.eb851ep-5f * coax_p_verse_style_noise((l_x * 0x1.000000p1f), (l_z * 0x1.000000p1f)))));
  l_lx = abs((coax_p_fract((l_x + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_lz = abs((coax_p_fract((l_z + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.0624dep-8f, 0x1.cac084p-7f, coax_p_fmin(l_lx, l_lz)));
  l_sx = (abs((coax_p_fract(((l_x * 0x1.000000p2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p-2f);
  l_sz = (abs((coax_p_fract(((l_z * 0x1.000000p2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p-2f);
  l_fine = ((0x1.000000p0f - coax_p_smoothstep(0x1.0624dep-9f, 0x1.0624dep-7f, coax_p_fmin(l_sx, l_sz))) * (0x1.000000p0f - coax_p_smoothstep(0x1.000000p1f, 0x1.000000p2f, coax_p_wg_length2(coax_p_v2(l_x, l_z)))));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.333334p-2f, 0x1.3d70a4p-2f, 0x1.5c28f6p-2f), ((l_line * 0x1.19999ap-1f) + (l_fine * 0x1.c28f5cp-3f)));
  return l_c;
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
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
fn coax_p_verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
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
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  if (((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y)));
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