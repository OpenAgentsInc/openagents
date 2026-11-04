// ---- generated: apps/openagents-web/gpu/splatlab-fx.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_t14 {
  x: vec3f,
  y: f32,
  z: vec3f,
  w: vec3f,
  f4: vec3f,
  f5: f32,
}
struct coax_t15 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: f32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) @interpolate(flat) w: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 64>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> splats: array<coax_t10>;
@group(1) @binding(1) var order: texture_2d<f32>;
@group(2) @binding(0) var<uniform> sp: array<vec4f, 1>;
fn coax_at_sp(i: i32) -> f32 {
  return sp[i / 4][i % 4];
}
@group(2) @binding(1) var prev: texture_2d<f32>;
fn coax_p_sort_vs(p_i: u32, p__iid: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid: u32 = p__iid;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_particle_over_vs(p_v: u32, p_n: u32) -> coax_t15 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_s: coax_t14;
  if ((f32(l_n) >= coax_at_u(i32(6i))))
  {
    return coax_p_sl_gone();
  }
  l_s = coax_p_sl_get(coax_p_sl_nth(l_n));
  if (((l_s.y <= 0.0f) && (coax_p_length3(l_s.f4) <= 0.0f)))
  {
    return coax_p_sl_gone();
  }
  return coax_p_sl_place(l_v, l_s);
}
fn coax_p_particle_vs(p_v: u32, p_n: u32) -> coax_t15 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_i: u32;
  var l_s: coax_t14;
  if ((f32(l_n) >= coax_at_u(i32(6i))))
  {
    return coax_p_sl_gone();
  }
  l_i = coax_p_sl_nth(l_n);
  if ((!coax_p_sl_glows(l_i)))
  {
    return coax_p_sl_gone();
  }
  l_s = coax_p_sl_get(l_i);
  if ((l_s.f5 <= 0.0f))
  {
    return coax_p_sl_gone();
  }
  return coax_p_sl_place(l_v, l_s);
}
fn coax_p_key_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_w: u32;
  var l_i: u32;
  var l_s: coax_t14;
  var l_rel: vec3f;
  var l_z: f32;
  l_w = u32(coax_at_sp(i32(2i)));
  l_i = ((u32(l_pos.y) * l_w) + u32(l_pos.x));
  if ((f32(l_i) >= coax_at_sp(i32(3i))))
  {
    return coax_p_v4(0x1.c363ccp127f, f32(l_i), 0.0f, 0.0f);
  }
  l_s = coax_p_sl_get(l_i);
  l_rel = coax_p_sub3(l_s.x, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))));
  l_z = dot(l_rel, coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))));
  if (((l_z <= coax_at_u(i32(9i))) || (((l_s.y <= 0.0f) && (l_s.f5 <= 0.0f)) && (coax_p_length3(l_s.f4) <= 0.0f))))
  {
    return coax_p_v4(0x1.93e594p99f, f32(l_i), 0.0f, 0.0f);
  }
  return coax_p_v4((-l_z), f32(l_i), 0.0f, 0.0f);
}
fn coax_p_sort_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_w: u32;
  var l_x: u32;
  var l_y: u32;
  var l_i: u32;
  var l_k: u32;
  var l_j: u32;
  var l_l: u32;
  var l_me: vec4f;
  var l_them: vec4f;
  var l_rising: bool;
  var l_first: bool;
  var l_smaller: bool;
  l_w = u32(coax_at_sp(i32(2i)));
  l_x = u32(l_pos.x);
  l_y = u32(l_pos.y);
  l_i = ((l_y * l_w) + l_x);
  l_k = u32(coax_at_sp(i32(0i)));
  l_j = u32(coax_at_sp(i32(1i)));
  l_l = (l_i ^ l_j);
  l_me = textureLoad(prev, vec2i(i32(l_x), i32(l_y)), 0i);
  l_them = textureLoad(prev, vec2i(i32((l_l % l_w)), i32((l_l / l_w))), 0i);
  l_rising = ((l_i & l_k) == 0u);
  l_first = (l_l > l_i);
  l_smaller = coax_p_sl_before(l_them, l_me);
  if ((l_first == l_rising))
  {
    if (l_smaller)
    {
      return l_them;
    }
    return l_me;
  }
  if (l_smaller)
  {
    return l_me;
  }
  return l_them;
}
fn coax_p_particle_over_fs(p_p: coax_t15) -> vec4f {
  var l_p: coax_t15 = p_p;
  var l_g: f32;
  var l_a: f32;
  l_g = coax_p_sl_weight(l_p);
  l_a = coax_p_fmin((l_p.z.w * l_g), 0x1.fae148p-1f);
  if (((l_a < 0x1.0624dep-9f) && ((l_g * ((l_p.z.x + l_p.z.y) + l_p.z.z)) < 0x1.0624dep-9f)))
  {
    discard;
  }
  return coax_p_v4((l_p.z.x * l_g), (l_p.z.y * l_g), (l_p.z.z * l_g), l_a);
}
fn coax_p_particle_fs(p_p: coax_t15) -> vec4f {
  var l_p: coax_t15 = p_p;
  var l_g: f32;
  l_g = coax_p_sl_weight(l_p);
  if (((l_g * l_p.w) < 0x1.89374cp-9f))
  {
    discard;
  }
  return coax_p_v4(0.0f, 0.0f, 0.0f, coax_p_fmin(((l_g * l_p.w) * 0x1.000000p-1f), 0x1.000000p0f));
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
fn coax_p_sl_gone() -> coax_t15 {
  var l_o: coax_t15;
  l_o = coax_t15();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  return l_o;
}
fn coax_p_sl_nth(p_n: u32) -> u32 {
  var l_n: u32 = p_n;
  var l_w: u32;
  var l_t: vec4f;
  l_w = u32(coax_at_u(i32(7i)));
  l_t = textureLoad(order, vec2i(i32((l_n % l_w)), i32((l_n / l_w))), 0i);
  return u32(l_t.y);
}
fn coax_p_sl_get(p_i: u32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_fi: f32;
  var l_t: f32;
  var l_e: i32;
  var l_o: i32;
  var l_kind: f32;
  var l_first: f32;
  var l_at: vec3f;
  var l_size: f32;
  var l_j: u32;
  var l_to: vec3f;
  var l_p: f32;
  l_fi = f32(l_i);
  if ((l_fi < coax_at_u(i32(5i))))
  {
    return coax_p_sl_static(splats[i32(l_i)]);
  }
  l_t = coax_at_u(i32(2i));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 16i;
    for (l_e = coax_from; l_e < coax_to; l_e = l_e + 1i)
    {
      l_o = (64i + (l_e * 12i));
      l_kind = coax_at_u(i32((l_o + 3i)));
      l_first = coax_at_u(i32((l_o + 4i)));
      if ((((l_kind > 0.0f) && (l_fi >= l_first)) && (l_fi < (l_first + coax_at_u(i32((l_o + 5i)))))))
      {
        l_at = coax_p_v3(coax_at_u(i32(l_o)), coax_at_u(i32((l_o + 1i))), coax_at_u(i32((l_o + 2i))));
        l_size = coax_at_u(i32((l_o + 6i)));
        l_j = (l_i + u32(coax_at_u(i32((l_o + 7i)))));
        l_to = coax_p_v3(coax_at_u(i32((l_o + 8i))), coax_at_u(i32((l_o + 9i))), coax_at_u(i32((l_o + 10i))));
        l_p = coax_at_u(i32((l_o + 11i)));
        if ((l_kind == 0x1.000000p0f))
        {
          return coax_p_sl_flame(l_j, l_at, l_size, l_p, l_t, coax_p_v3(0x1.19999ap-1f, 0x1.eb851ep-5f, 0x1.47ae14p-7f), coax_p_v3(0x1.000000p0f, 0x1.000000p-1f, 0x1.47ae14p-4f), 0x1.47ae14p-2f);
        }
        if ((l_kind == 0x1.800000p2f))
        {
          return coax_p_sl_flame(l_j, l_at, l_size, l_p, (l_t * 0x1.99999ap-1f), coax_p_v3(0x1.c28f5cp-3f, 0x1.47ae14p-5f, 0x1.19999ap-1f), coax_p_v3(0x1.ccccccp-2f, 0x1.000000p0f, 0x1.19999ap-1f), 0x1.99999ap-2f);
        }
        if ((l_kind == 0x1.000000p1f))
        {
          return coax_p_sl_smoke(l_j, l_at, l_size, l_p, l_to, l_t, 0x1.000000p0f);
        }
        if ((l_kind == 0x1.800000p1f))
        {
          return coax_p_sl_ember(l_j, l_at, l_size, l_p, l_t, coax_p_v3(0x1.000000p0f, 0x1.19999ap-1f, 0x1.333334p-3f));
        }
        if ((l_kind == 0x1.000000p2f))
        {
          return coax_p_sl_fireball(l_j, l_at, l_to, l_size, l_p, l_t);
        }
        if ((l_kind == 0x1.400000p2f))
        {
          return coax_p_sl_blast(l_j, l_at, l_to, l_size, l_p, l_t);
        }
        if ((l_kind == 0x1.c00000p2f))
        {
          return coax_p_sl_motes(l_j, l_at, l_size, l_p, l_t);
        }
        if ((l_kind == 0x1.000000p3f))
        {
          return coax_p_sl_field(l_j, l_at, l_size, coax_at_u(i32((l_o + 5i))));
        }
        return coax_p_sl_none();
      }
    }
  }
  return coax_p_sl_none();
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_sl_place(p_v: u32, p_s: coax_t14) -> coax_t15 {
  var l_v: u32 = p_v;
  var l_s: coax_t14 = p_s;
  var l_o: coax_t15;
  var l_eye: vec3f;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_rel: vec3f;
  var l_z: f32;
  var l_r: f32;
  var l_q: f32;
  var l_f: f32;
  var l_t0: vec3f;
  var l_t1: vec3f;
  var l_st0: vec3f;
  var l_st1: vec3f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_mid: f32;
  var l_rad: f32;
  var l_l1: f32;
  var l_l2: f32;
  var l_e1: vec2f;
  var l_e2: vec2f;
  var l_r1: f32;
  var l_r2: f32;
  var l_quad: array<vec2f, 6>;
  var l_k: vec2f;
  var l_off: vec2f;
  var l_centre: vec4f;
  l_o = coax_t15();
  l_eye = coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)));
  l_fwd = coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)));
  l_right = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_up = coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i)));
  l_rel = coax_p_sub3(l_s.x, l_eye);
  l_z = dot(l_rel, l_fwd);
  if ((l_z <= coax_at_u(i32(9i))))
  {
    return coax_p_sl_gone();
  }
  l_r = dot(l_rel, l_right);
  l_q = dot(l_rel, l_up);
  l_f = coax_at_u(i32(19i));
  l_t0 = coax_p_sub3(coax_p_scale3(l_right, (l_f / l_z)), coax_p_scale3(l_fwd, ((l_f * l_r) / (l_z * l_z))));
  l_t1 = coax_p_sub3(coax_p_scale3(l_up, (l_f / l_z)), coax_p_scale3(l_fwd, ((l_f * l_q) / (l_z * l_z))));
  l_st0 = coax_p_v3((((l_s.z.x * l_t0.x) + (l_s.z.y * l_t0.y)) + (l_s.z.z * l_t0.z)), (((l_s.z.y * l_t0.x) + (l_s.w.x * l_t0.y)) + (l_s.w.y * l_t0.z)), (((l_s.z.z * l_t0.x) + (l_s.w.y * l_t0.y)) + (l_s.w.z * l_t0.z)));
  l_st1 = coax_p_v3((((l_s.z.x * l_t1.x) + (l_s.z.y * l_t1.y)) + (l_s.z.z * l_t1.z)), (((l_s.z.y * l_t1.x) + (l_s.w.x * l_t1.y)) + (l_s.w.y * l_t1.z)), (((l_s.z.z * l_t1.x) + (l_s.w.y * l_t1.y)) + (l_s.w.z * l_t1.z)));
  l_a = (dot(l_t0, l_st0) + 0x1.333334p-2f);
  l_b = dot(l_t0, l_st1);
  l_c = (dot(l_t1, l_st1) + 0x1.333334p-2f);
  l_mid = (0x1.000000p-1f * (l_a + l_c));
  l_rad = sqrt(coax_p_fmax((((0x1.000000p-2f * (l_a - l_c)) * (l_a - l_c)) + (l_b * l_b)), 0.0f));
  l_l1 = (l_mid + l_rad);
  l_l2 = coax_p_fmax((l_mid - l_rad), 0x1.99999ap-4f);
  l_e1 = coax_p_v2(0x1.000000p0f, 0.0f);
  if ((abs(l_b) > 0x1.0c6f7ap-20f))
  {
    l_e1 = coax_p_sl_norm2(coax_p_v2(l_b, (l_l1 - l_a)));
  }
  else
  {
    if ((l_c > l_a))
    {
      l_e1 = coax_p_v2(0.0f, 0x1.000000p0f);
    }
  }
  l_e2 = coax_p_v2((-l_e1.y), l_e1.x);
  l_r1 = coax_p_fmin((0x1.800000p1f * sqrt(l_l1)), 0x1.000000p11f);
  l_r2 = coax_p_fmin((0x1.800000p1f * sqrt(l_l2)), 0x1.000000p11f);
  l_quad = array<vec2f, 6>(coax_p_v2((-0x1.000000p0f), (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_k = l_quad[i32(l_v)];
  l_off = coax_p_add2(coax_p_scale2(l_e1, (l_k.x * l_r1)), coax_p_scale2(l_e2, (l_k.y * l_r2)));
  l_centre = coax_p_sl_clip(l_s.x);
  l_o.x = coax_p_v4((l_centre.x + (((0x1.000000p1f * l_off.x) / coax_p_fmax(coax_at_u(i32(0i)), 0x1.000000p0f)) * l_centre.w)), (l_centre.y + (((0x1.000000p1f * l_off.y) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)) * l_centre.w)), l_centre.z, l_centre.w);
  l_o.y = coax_p_scale2(l_k, 0x1.800000p1f);
  l_o.z = coax_p_v4(l_s.f4.x, l_s.f4.y, l_s.f4.z, l_s.y);
  l_o.w = l_s.f5;
  return l_o;
}
fn coax_p_sl_glows(p_i: u32) -> bool {
  var l_i: u32 = p_i;
  var l_fi: f32;
  var l_e: i32;
  var l_o: i32;
  var l_kind: f32;
  l_fi = f32(l_i);
  if ((l_fi < coax_at_u(i32(5i))))
  {
    return (splats[i32(l_i)].z.w > 0.0f);
  }
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 16i;
    for (l_e = coax_from; l_e < coax_to; l_e = l_e + 1i)
    {
      l_o = (64i + (l_e * 12i));
      l_kind = coax_at_u(i32((l_o + 3i)));
      if ((((l_kind > 0.0f) && (l_fi >= coax_at_u(i32((l_o + 4i))))) && (l_fi < (coax_at_u(i32((l_o + 4i))) + coax_at_u(i32((l_o + 5i)))))))
      {
        return ((l_kind != 0x1.000000p1f) && (l_kind != 0x1.000000p3f));
      }
    }
  }
  return false;
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
fn coax_p_sl_before(p_a: vec4f, p_b: vec4f) -> bool {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return ((l_a.x < l_b.x) || ((l_a.x == l_b.x) && (l_a.y < l_b.y)));
}
fn coax_p_sl_weight(p_p: coax_t15) -> f32 {
  var l_p: coax_t15 = p_p;
  var l_d2: f32;
  l_d2 = dot(l_p.y, l_p.y);
  if ((l_d2 > (0x1.800000p1f * 0x1.800000p1f)))
  {
    return 0.0f;
  }
  return exp(((-0x1.000000p-1f) * l_d2));
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
fn coax_p_sl_static(p_st: coax_t10) -> coax_t14 {
  var l_st: coax_t10 = p_st;
  var l_s: coax_t14;
  var l_p: f32;
  var l_bb: f32;
  var l_gg: f32;
  var l_rr: f32;
  var l_lin: vec3f;
  l_s = coax_t14();
  l_s.x = coax_p_v3(l_st.x.x, l_st.x.y, l_st.x.z);
  l_s.y = l_st.x.w;
  l_s.z = coax_p_v3(l_st.y.x, l_st.y.y, l_st.y.z);
  l_s.w = coax_p_v3(l_st.y.w, l_st.z.x, l_st.z.y);
  l_p = l_st.z.z;
  l_bb = floor((l_p / 0x1.000000p16f));
  l_gg = floor(((l_p - (l_bb * 0x1.000000p16f)) / 0x1.000000p8f));
  l_rr = ((l_p - (l_bb * 0x1.000000p16f)) - (l_gg * 0x1.000000p8f));
  l_lin = coax_p_v3(coax_pow_sort_vs((l_rr / 0x1.fe0000p7f), 0x1.19999ap1f), coax_pow_sort_vs((l_gg / 0x1.fe0000p7f), 0x1.19999ap1f), coax_pow_sort_vs((l_bb / 0x1.fe0000p7f), 0x1.19999ap1f));
  l_s.f4 = coax_p_scale3(l_lin, l_st.x.w);
  l_s.f5 = l_st.z.w;
  return l_s;
}
fn coax_p_sl_flame(p_i: u32, p_base: vec3f, p_size: f32, p_height: f32, p_t: f32, p_cool: vec3f, p_hot: vec3f, p_bright: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_base: vec3f = p_base;
  var l_size: f32 = p_size;
  var l_height: f32 = p_height;
  var l_t: f32 = p_t;
  var l_cool: vec3f = p_cool;
  var l_hot: vec3f = p_hot;
  var l_bright: f32 = p_bright;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_h5: f32;
  var l_tongue: u32;
  var l_g0: f32;
  var l_g1: f32;
  var l_g2: f32;
  var l_g3: f32;
  var l_tr: f32;
  var l_ta: f32;
  var l_tall: f32;
  var l_beat: f32;
  var l_period: f32;
  var l_ph: f32;
  var l_y: f32;
  var l_narrow: f32;
  var l_jit: f32;
  var l_sway: f32;
  var l_lick: f32;
  var l_ja: f32;
  var l_jr: f32;
  var l_cx: f32;
  var l_cz: f32;
  var l_across: f32;
  var l_along: f32;
  var l_d: vec3f;
  var l_heat: f32;
  var l_c: vec3f;
  var l_fade: f32;
  var l_k: f32;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_h5 = coax_p_sl_hash(l_i, 6u);
  l_tongue = u32(floor((l_h0 * 0x1.600000p4f)));
  l_g0 = coax_p_sl_hash(l_tongue, 11u);
  l_g1 = coax_p_sl_hash(l_tongue, 12u);
  l_g2 = coax_p_sl_hash(l_tongue, 13u);
  l_g3 = coax_p_sl_hash(l_tongue, 14u);
  l_tr = ((l_size * 0x1.99999ap-1f) * sqrt(l_g0));
  l_ta = (0x1.921fb6p2f * l_g1);
  l_tall = ((l_height * (0x1.666666p-2f + (0x1.4cccccp-1f * l_g2))) * (0x1.000000p0f - (0x1.ccccccp-2f * (l_tr / coax_p_fmax(l_size, 0x1.0624dep-10f)))));
  l_beat = (0x1.800000p-1f + (0x1.000000p-2f * sin(((l_t * (0x1.800000p1f + (0x1.000000p1f * l_g3))) + (l_g1 * 0x1.921fb6p2f)))));
  l_period = (0x1.ccccccp-2f + (0x1.000000p-1f * l_h1));
  l_ph = coax_p_fract(((l_t / l_period) + l_h2));
  l_y = ((l_tall * l_beat) * coax_pow_sort_vs(l_ph, 0x1.ccccccp-1f));
  l_narrow = (0x1.000000p0f - (0x1.b33334p-1f * l_ph));
  l_jit = ((l_size * 0x1.eb851ep-4f) * l_narrow);
  l_sway = (((sin((((l_t * 0x1.266666p1f) + (l_g3 * 0x1.921fb6p2f)) + (l_y * 0x1.19999ap1f))) * 0x1.333334p-2f) * l_size) * l_ph);
  l_lick = (((cos((((l_t * 0x1.d9999ap1f) + (l_g0 * 0x1.921fb6p2f)) + (l_y * 0x1.733334p1f))) * 0x1.c28f5cp-3f) * l_size) * l_ph);
  l_ja = (0x1.921fb6p2f * l_h3);
  l_jr = (l_jit * sqrt(l_h4));
  l_cx = (((l_base.x + ((cos(l_ta) * l_tr) * l_narrow)) + (cos(l_ja) * l_jr)) + l_sway);
  l_cz = (((l_base.z + ((sin(l_ta) * l_tr) * l_narrow)) + (sin(l_ja) * l_jr)) + l_lick);
  l_s.x = coax_p_v3(l_cx, (l_base.y + l_y), l_cz);
  l_across = ((l_size * (0x1.333334p-4f - (0x1.47ae14p-5f * l_ph))) + 0x1.0624dep-7f);
  l_along = (l_across * (0x1.19999ap1f + (0x1.000000p1f * l_h5)));
  l_d = coax_p_sl_unit(coax_p_v3((l_sway * 0x1.666666p0f), 0x1.000000p0f, (l_lick * 0x1.666666p0f)));
  l_s = coax_p_sl_stretch(l_s, l_d, l_along, l_across);
  l_heat = ((0x1.000000p0f - l_ph) * (0x1.000000p0f - (0x1.99999ap-2f * (l_tr / coax_p_fmax(l_size, 0x1.0624dep-10f)))));
  l_c = coax_p_mix3(l_cool, l_hot, coax_p_smoothstep(0x1.333334p-2f, 0x1.e66666p-1f, l_heat));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.000000p0f, 0x1.dc28f6p-1f, 0x1.8f5c28p-1f), (coax_p_smoothstep(0x1.b33334p-1f, 0x1.000000p0f, l_heat) * 0x1.333334p-1f));
  l_fade = (coax_p_smoothstep(0.0f, 0x1.47ae14p-4f, l_ph) * (0x1.000000p0f - coax_p_smoothstep(0x1.ccccccp-2f, 0x1.000000p0f, l_ph)));
  l_k = ((l_bright * l_fade) * (0x1.333334p-2f + (0x1.666666p-1f * l_heat)));
  l_s.f4 = coax_p_scale3(l_c, l_k);
  l_s.y = (0x1.47ae14p-4f * l_fade);
  l_s.f5 = ((0x1.eb851ep-4f * l_fade) * l_heat);
  return l_s;
}
fn coax_p_sl_smoke(p_i: u32, p_base: vec3f, p_size: f32, p_height: f32, p_wind: vec3f, p_t: f32, p_dark: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_base: vec3f = p_base;
  var l_size: f32 = p_size;
  var l_height: f32 = p_height;
  var l_wind: vec3f = p_wind;
  var l_t: f32 = p_t;
  var l_dark: f32 = p_dark;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_h5: f32;
  var l_period: f32;
  var l_ph: f32;
  var l_y: f32;
  var l_spread: f32;
  var l_r: f32;
  var l_curl: f32;
  var l_drift: f32;
  var l_sig: f32;
  var l_dens: f32;
  var l_grey: f32;
  var l_warm: f32;
  var l_c: vec3f;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_h5 = coax_p_sl_hash(l_i, 6u);
  l_period = (0x1.400000p2f + (0x1.c00000p1f * l_h0));
  l_ph = coax_p_fract(((l_t / l_period) + l_h1));
  l_y = ((l_height * l_ph) * (0x1.99999ap-1f + (0x1.99999ap-2f * l_h5)));
  l_spread = (l_size * (0x1.99999ap-2f + (0x1.19999ap1f * l_ph)));
  l_r = (l_spread * sqrt(l_h3));
  l_curl = (((sin((((l_t * 0x1.333334p-1f) + (l_h4 * 0x1.921fb6p2f)) + (l_y * 0x1.ccccccp-2f))) * 0x1.000000p-1f) * l_ph) * l_size);
  l_drift = (((l_ph * l_ph) * l_height) * 0x1.ccccccp-2f);
  l_s.x = coax_p_v3((((l_base.x + (cos((0x1.921fb6p2f * l_h2)) * l_r)) + (l_wind.x * l_drift)) + l_curl), (l_base.y + l_y), (((l_base.z + (sin((0x1.921fb6p2f * l_h2)) * l_r)) + (l_wind.z * l_drift)) - (l_curl * 0x1.333334p-1f)));
  l_sig = ((l_size * (0x1.ccccccp-2f + (0x1.e66666p0f * l_ph))) * (0x1.666666p-1f + (0x1.333334p-1f * l_h4)));
  l_s = coax_p_sl_stretch(l_s, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (l_sig * 0x1.99999ap-1f), l_sig);
  l_dens = (((coax_p_smoothstep(0.0f, 0x1.eb851ep-4f, l_ph) * (0x1.000000p0f - coax_p_smoothstep(0x1.ccccccp-2f, 0x1.000000p0f, l_ph))) * 0x1.99999ap-3f) * l_dark);
  l_grey = (0x1.99999ap-6f + (0x1.70a3d8p-5f * l_h4));
  l_warm = ((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-2f, l_ph)) * 0x1.000000p-1f);
  l_c = coax_p_add3(coax_p_v3(l_grey, (l_grey * 0x1.051eb8p0f), (l_grey * 0x1.147ae2p0f)), coax_p_scale3(coax_p_v3(0x1.ccccccp-1f, 0x1.666666p-2f, 0x1.99999ap-4f), l_warm));
  l_s.f4 = coax_p_scale3(l_c, l_dens);
  l_s.y = l_dens;
  return l_s;
}
fn coax_p_sl_ember(p_i: u32, p_base: vec3f, p_size: f32, p_height: f32, p_t: f32, p_hot: vec3f) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_base: vec3f = p_base;
  var l_size: f32 = p_size;
  var l_height: f32 = p_height;
  var l_t: f32 = p_t;
  var l_hot: vec3f = p_hot;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_period: f32;
  var l_ph: f32;
  var l_a: f32;
  var l_r: f32;
  var l_d: vec3f;
  var l_fade: f32;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_period = (0x1.666666p0f + (0x1.ccccccp0f * l_h0));
  l_ph = coax_p_fract(((l_t / l_period) + l_h1));
  l_a = ((0x1.921fb6p2f * l_h2) + (l_t * (0x1.333334p-1f + l_h4)));
  l_r = ((l_size * (0x1.333334p-2f + (0x1.333334p0f * l_ph))) * sqrt(l_h3));
  l_s.x = coax_p_v3((l_base.x + (cos(l_a) * l_r)), ((l_base.y + ((l_height * 0x1.800000p0f) * l_ph)) + (sin(((l_t * 0x1.c00000p2f) + (l_h4 * 0x1.400000p4f))) * 0x1.99999ap-5f)), (l_base.z + (sin(l_a) * l_r)));
  l_d = coax_p_sl_unit(coax_p_v3(((-sin(l_a)) * 0x1.333334p-1f), 0x1.000000p0f, (cos(l_a) * 0x1.333334p-1f)));
  l_s = coax_p_sl_stretch(l_s, l_d, 0x1.70a3d8p-5f, 0x1.26e978p-7f);
  l_fade = (((0x1.000000p0f - l_ph) * coax_p_smoothstep(0.0f, 0x1.99999ap-5f, l_ph)) * coax_p_step_on(coax_p_fract(((l_t * 0x1.800000p1f) + (l_h4 * 0x1.c00000p2f))), 0x1.333334p-3f));
  l_s.f4 = coax_p_scale3(l_hot, (0x1.000000p2f * l_fade));
  l_s.y = 0.0f;
  l_s.f5 = l_fade;
  return l_s;
}
fn coax_p_sl_fireball(p_i: u32, p_from: vec3f, p_to: vec3f, p_size: f32, p_period: f32, p_t: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_from: vec3f = p_from;
  var l_to: vec3f = p_to;
  var l_size: f32 = p_size;
  var l_period: f32 = p_period;
  var l_t: f32 = p_t;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_cyc: f32;
  var l_fly: f32;
  var l_k: f32;
  var l_back: f32;
  var l_at: f32;
  var l_p: vec3f;
  var l_jit: vec3f;
  var l_w: f32;
  var l_e: f32;
  var l_dir: vec3f;
  var l_r: f32;
  var l_lift: f32;
  var l_turb: f32;
  var l_sig: f32;
  var l_heat: f32;
  var l_c: vec3f;
  var l_fade: f32;
  var l_show: f32;
  var l_r_30: f32;
  var l_up: f32;
  var l_sig_32: f32;
  var l_g: f32;
  var l_warm: f32;
  var l_dens: f32;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_cyc = coax_p_fract((l_t / l_period));
  l_fly = 0x1.47ae14p-3f;
  if ((l_cyc < l_fly))
  {
    if ((l_h0 > 0x1.99999ap-3f))
    {
      return l_s;
    }
    l_k = (l_cyc / l_fly);
    l_back = (l_h3 * 0x1.666666p-2f);
    l_at = coax_p_fmax((l_k - l_back), 0.0f);
    l_p = coax_p_mix3(l_from, l_to, l_at);
    l_p.y = (l_p.y + (sin((l_at * 0x1.921fb6p1f)) * 0x1.333334p0f));
    l_jit = coax_p_scale3(coax_p_sl_sphere(l_h1, l_h2), (0x1.eb851ep-4f * (0x1.000000p0f + (l_back * 0x1.800000p1f))));
    l_s.x = coax_p_add3(l_p, l_jit);
    l_s = coax_p_sl_stretch(l_s, coax_p_sl_unit(coax_p_sub3(l_to, l_from)), 0x1.eb851ep-4f, 0x1.eb851ep-5f);
    l_w = (0x1.000000p0f - (l_back * 0x1.400000p1f));
    l_s.f4 = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.666666p-1f, 0x1.333334p-2f), (0x1.99999ap-1f * coax_p_fmax(l_w, 0.0f)));
    l_s.y = 0x1.99999ap-5f;
    l_s.f5 = (0x1.333334p-2f * coax_p_fmax(l_w, 0.0f));
    return l_s;
  }
  l_e = ((l_cyc - l_fly) / (0x1.000000p0f - l_fly));
  l_dir = coax_p_sl_sphere(l_h1, l_h2);
  if ((l_h0 < 0x1.3d70a4p-1f))
  {
    l_r = ((l_size * (0x1.000000p0f - exp(((-0x1.c00000p2f) * l_e)))) * (0x1.ccccccp-2f + (0x1.19999ap-1f * l_h3)));
    l_lift = (((l_e * l_e) * l_size) * 0x1.333334p-1f);
    l_s.x = coax_p_add3(coax_p_add3(l_to, coax_p_scale3(l_dir, l_r)), coax_p_v3(0.0f, l_lift, 0.0f));
    l_turb = (((sin(((l_h4 * 0x1.e00000p4f) + (l_e * 0x1.200000p3f))) * 0x1.333334p-3f) * l_size) * l_e);
    l_s.x = coax_p_add3(l_s.x, coax_p_scale3(coax_p_sl_sphere(l_h4, l_h3), l_turb));
    l_sig = ((l_size * (0x1.70a3d8p-4f + (0x1.eb851ep-4f * l_e))) * (0x1.333334p-1f + (0x1.99999ap-1f * l_h4)));
    l_s = coax_p_sl_stretch(l_s, l_dir, (l_sig * (0x1.99999ap0f - l_e)), l_sig);
    l_heat = exp(((-0x1.99999ap1f) * l_e));
    l_c = coax_p_mix3(coax_p_v3(0x1.666666p-1f, 0x1.eb851ep-4f, 0x1.eb851ep-6f), coax_p_v3(0x1.000000p0f, 0x1.19999ap-1f, 0x1.eb851ep-4f), coax_p_smoothstep(0x1.333334p-3f, 0x1.333334p-1f, l_heat));
    l_c = coax_p_mix3(l_c, coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.99999ap-1f), coax_p_smoothstep(0x1.800000p-1f, 0x1.000000p0f, l_heat));
    l_fade = (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.800000p-1f, l_e));
    l_s.f4 = coax_p_scale3(l_c, ((0x1.ae147ap-2f * l_heat) * l_fade));
    l_s.y = (0x1.eb851ep-4f * l_fade);
    l_s.f5 = ((0x1.99999ap-3f * l_heat) * l_fade);
    return l_s;
  }
  l_show = (coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, l_e) * (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-1f, 0x1.000000p0f, l_e)));
  l_r_30 = ((l_size * (0x1.99999ap-2f + (0x1.666666p-1f * l_e))) * (0x1.99999ap-2f + (0x1.333334p-1f * l_h3)));
  l_up = ((l_e * l_size) * 0x1.99999ap0f);
  l_s.x = coax_p_add3(coax_p_add3(l_to, coax_p_scale3(coax_p_v3(l_dir.x, (l_dir.y * 0x1.000000p-1f), l_dir.z), l_r_30)), coax_p_v3(0.0f, l_up, 0.0f));
  l_sig_32 = ((l_size * (0x1.47ae14p-3f + (0x1.333334p-2f * l_e))) * (0x1.666666p-1f + (0x1.333334p-1f * l_h4)));
  l_s = coax_p_sl_stretch(l_s, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (l_sig_32 * 0x1.b33334p-1f), l_sig_32);
  l_g = (0x1.eb851ep-6f + (0x1.47ae14p-5f * l_h4));
  l_warm = ((0x1.000000p0f - coax_p_smoothstep(0x1.333334p-3f, 0x1.ccccccp-2f, l_e)) * 0x1.333334p-1f);
  l_dens = (0x1.000000p-1f * l_show);
  l_s.f4 = coax_p_scale3(coax_p_add3(coax_p_v3(l_g, l_g, (l_g * 0x1.0cccccp0f)), coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.99999ap-2f, 0x1.99999ap-4f), l_warm)), l_dens);
  l_s.y = l_dens;
  return l_s;
}
fn coax_p_sl_blast(p_i: u32, p_from: vec3f, p_to: vec3f, p_size: f32, p_period: f32, p_t: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_from: vec3f = p_from;
  var l_to: vec3f = p_to;
  var l_size: f32 = p_size;
  var l_period: f32 = p_period;
  var l_t: f32 = p_t;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_h5: f32;
  var l_bolt: f32;
  var l_head: f32;
  var l_fade: f32;
  var l_d: vec3f;
  var l_span: f32;
  var l_ax: vec3f;
  var l_e1: vec3f;
  var l_e2: vec3f;
  var l_flick: f32;
  var l_r: f32;
  var l_k: f32;
  var l_along: f32;
  var l_ang: f32;
  var l_wob: f32;
  var l_crack: f32;
  var l_side: vec3f;
  var l_core: f32;
  var l_c: vec3f;
  var l_k_31: f32;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_h5 = coax_p_sl_hash(l_i, 6u);
  l_bolt = coax_p_fract(((l_t / l_period) * 0x1.800000p1f));
  l_head = coax_p_smoothstep(0.0f, 0x1.70a3d8p-3f, l_bolt);
  l_fade = (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.333334p-1f, l_bolt));
  if ((l_fade <= 0.0f))
  {
    return l_s;
  }
  l_d = coax_p_sub3(l_to, l_from);
  l_span = coax_p_length3(l_d);
  l_ax = coax_p_sl_unit(l_d);
  l_e1 = coax_p_sl_unit(coax_p_cross3(l_ax, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  l_e2 = coax_p_cross3(l_e1, l_ax);
  l_flick = coax_p_sl_hash(l_i, (u32(floor((l_t * 0x1.800000p4f))) + 7u));
  if ((l_h0 > 0x1.ccccccp-1f))
  {
    l_r = ((l_size * (0x1.333334p-1f + (0x1.19999ap1f * l_h3))) * coax_p_smoothstep(0x1.99999ap-4f, 0x1.333334p-2f, l_bolt));
    l_s.x = coax_p_add3(l_to, coax_p_scale3(coax_p_sl_sphere(l_h1, l_h2), l_r));
    l_s = coax_p_sl_stretch(l_s, coax_p_sl_sphere(l_h1, l_h2), (l_size * 0x1.000000p-1f), (l_size * 0x1.70a3d8p-3f));
    l_k = ((l_fade * coax_p_smoothstep(0x1.eb851ep-4f, 0x1.99999ap-3f, l_bolt)) * (0x1.000000p-1f + l_flick));
    l_s.f4 = coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.000000p-2f, 0x1.000000p0f), (0x1.666666p-1f * l_k));
    l_s.y = (0x1.99999ap-5f * l_k);
    l_s.f5 = l_k;
    return l_s;
  }
  l_along = l_h1;
  if ((l_along > l_head))
  {
    return l_s;
  }
  l_ang = ((((l_along * l_span) * 0x1.200000p3f) + (l_t * 0x1.600000p4f)) + select(0x1.921fb6p1f, 0.0f, (l_h2 < 0x1.000000p-1f)));
  l_wob = ((l_size * (0x1.666666p-2f + (0x1.4cccccp-1f * sin((l_along * 0x1.921fb6p1f))))) * sqrt(l_h3));
  l_crack = (((l_flick - 0x1.000000p-1f) * l_size) * 0x1.99999ap-1f);
  l_side = coax_p_add3(coax_p_scale3(l_e1, ((cos(l_ang) * l_wob) + l_crack)), coax_p_scale3(l_e2, (sin(l_ang) * l_wob)));
  l_s.x = coax_p_add3(coax_p_add3(l_from, coax_p_scale3(l_d, l_along)), l_side);
  l_s = coax_p_sl_stretch(l_s, l_ax, (l_size * (0x1.333334p-1f + l_h4)), (l_size * 0x1.eb851ep-4f));
  if ((l_h5 < 0x1.1eb852p-2f))
  {
    l_s.f4 = coax_p_scale3(coax_p_v3(0x1.eb851ep-6f, 0.0f, 0x1.99999ap-5f), (0x1.333334p-1f * l_fade));
    l_s.y = (0x1.333334p-1f * l_fade);
    return l_s;
  }
  l_core = (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-1f, (l_wob / coax_p_fmax(l_size, 0x1.0624dep-10f))));
  l_c = coax_p_mix3(coax_p_v3(0x1.ae147ap-2f, 0x1.eb851ep-4f, 0x1.e66666p-1f), coax_p_v3(0x1.d70a3ep-1f, 0x1.99999ap-1f, 0x1.000000p0f), l_core);
  l_k_31 = (l_fade * (0x1.333334p-1f + (0x1.99999ap-1f * l_flick)));
  l_s.f4 = coax_p_scale3(l_c, (0x1.19999ap0f * l_k_31));
  l_s.y = (0x1.eb851ep-5f * l_k_31);
  l_s.f5 = ((0x1.99999ap-2f * l_k_31) * (0x1.000000p-1f + l_core));
  return l_s;
}
fn coax_p_sl_motes(p_i: u32, p_base: vec3f, p_size: f32, p_height: f32, p_t: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_base: vec3f = p_base;
  var l_size: f32 = p_size;
  var l_height: f32 = p_height;
  var l_t: f32 = p_t;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_a: f32;
  var l_r: f32;
  var l_y: f32;
  var l_tw: f32;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_a = ((0x1.921fb6p2f * l_h0) + (l_t * (0x1.333334p-3f + (0x1.000000p-2f * l_h1))));
  l_r = (l_size * (0x1.800000p-1f + (0x1.000000p-1f * l_h2)));
  l_y = ((l_height * l_h3) + (sin(((l_t * 0x1.ccccccp-1f) + (l_h4 * 0x1.921fb6p2f))) * 0x1.99999ap-2f));
  l_s.x = coax_p_v3((l_base.x + (cos(l_a) * l_r)), (l_base.y + l_y), (l_base.z + (sin(l_a) * l_r)));
  l_s = coax_p_sl_stretch(l_s, coax_p_sl_unit(coax_p_v3((-sin(l_a)), 0x1.99999ap-3f, cos(l_a))), 0x1.eb851ep-5f, 0x1.26e978p-6f);
  l_tw = (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_t * 0x1.800000p1f) + (l_h4 * 0x1.400000p5f)))));
  l_s.f4 = coax_p_scale3(coax_p_mix3(coax_p_v3(0x1.666666p-2f, 0x1.333334p-3f, 0x1.000000p0f), coax_p_v3(0x1.000000p-1f, 0x1.000000p0f, 0x1.333334p-1f), l_h1), (0x1.ccccccp-1f * l_tw));
  l_s.y = 0.0f;
  l_s.f5 = (0x1.000000p-1f * l_tw);
  return l_s;
}
fn coax_p_sl_field(p_i: u32, p_base: vec3f, p_size: f32, p_count: f32) -> coax_t14 {
  var l_i: u32 = p_i;
  var l_base: vec3f = p_base;
  var l_size: f32 = p_size;
  var l_count: f32 = p_count;
  var l_s: coax_t14;
  var l_h0: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_x: f32;
  var l_z: f32;
  var l_y: f32;
  var l_gap: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_n: vec3f;
  var l_m: f32;
  var l_c: vec3f;
  var l_tall: f32;
  var l_c_20: vec3f;
  l_s = coax_p_sl_none();
  l_h0 = coax_p_sl_hash(l_i, 1u);
  l_h1 = coax_p_sl_hash(l_i, 2u);
  l_h2 = coax_p_sl_hash(l_i, 3u);
  l_h3 = coax_p_sl_hash(l_i, 4u);
  l_h4 = coax_p_sl_hash(l_i, 5u);
  l_x = (((0x1.000000p1f * l_h0) - 0x1.000000p0f) * l_size);
  l_z = (((0x1.000000p1f * l_h1) - 0x1.000000p0f) * l_size);
  l_y = coax_p_sl_field_y(l_x, l_z);
  l_gap = ((0x1.000000p1f * l_size) / sqrt(coax_p_fmax(l_count, 0x1.000000p0f)));
  if ((l_h2 < 0x1.70a3d8p-1f))
  {
    l_dx = (coax_p_sl_field_y((l_x + 0x1.99999ap-4f), l_z) - l_y);
    l_dz = (coax_p_sl_field_y(l_x, (l_z + 0x1.99999ap-4f)) - l_y);
    l_n = coax_p_sl_unit(coax_p_v3(((-l_dx) * 0x1.400000p3f), 0x1.000000p0f, ((-l_dz) * 0x1.400000p3f)));
    l_s.x = coax_p_v3((l_base.x + l_x), (l_base.y + l_y), (l_base.z + l_z));
    l_s = coax_p_sl_stretch(l_s, l_n, 0x1.47ae14p-6f, (l_gap * 0x1.19999ap0f));
    l_m = (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_x * 0x1.7ae148p-2f) + (l_z * 0x1.d70a3ep-3f)))));
    l_c = coax_p_mix3(coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-4f, 0x1.47ae14p-6f), coax_p_v3(0x1.47ae14p-3f, 0x1.c28f5cp-3f, 0x1.99999ap-5f), (l_m * l_h3));
    l_c = coax_p_mix3(l_c, coax_p_v3(0x1.70a3d8p-3f, 0x1.0a3d70p-3f, 0x1.47ae14p-4f), coax_p_smoothstep(0x1.b33334p-1f, 0x1.000000p0f, l_h4));
    l_s.f4 = coax_p_scale3(l_c, 0x1.e66666p-1f);
    l_s.y = 0x1.e66666p-1f;
    return l_s;
  }
  l_tall = (0x1.333334p-3f + (0x1.000000p-1f * l_h3));
  l_s.x = coax_p_v3((l_base.x + l_x), ((l_base.y + l_y) + (l_tall * 0x1.000000p-1f)), (l_base.z + l_z));
  l_s = coax_p_sl_stretch(l_s, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (l_tall * 0x1.ccccccp-2f), (l_gap * 0x1.333334p-1f));
  l_c_20 = coax_p_mix3(coax_p_v3(0x1.47ae14p-5f, 0x1.99999ap-4f, 0x1.47ae14p-6f), coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-3f, 0x1.47ae14p-5f), l_h4);
  if ((l_h3 > 0x1.dc28f6p-1f))
  {
    l_c_20 = coax_p_mix3(coax_p_v3(0x1.333334p-1f, 0x1.000000p-1f, 0x1.47ae14p-4f), coax_p_v3(0x1.000000p-1f, 0x1.eb851ep-4f, 0x1.333334p-2f), l_h4);
  }
  l_s.f4 = coax_p_scale3(l_c_20, 0x1.ccccccp-1f);
  l_s.y = 0x1.ccccccp-1f;
  return l_s;
}
fn coax_p_sl_none() -> coax_t14 {
  var l_s: coax_t14;
  l_s = coax_t14();
  l_s.x = coax_p_v3(0.0f, (-0x1.f40000p9f), 0.0f);
  l_s.y = 0.0f;
  l_s.z = coax_p_v3(0x1.a36e2ep-14f, 0.0f, 0.0f);
  l_s.w = coax_p_v3(0x1.a36e2ep-14f, 0.0f, 0x1.a36e2ep-14f);
  l_s.f4 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_s.f5 = 0.0f;
  return l_s;
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
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
fn coax_p_sl_norm2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_l: f32;
  l_l = coax_p_length2(l_a);
  if ((l_l < 0x1.ad7f2ap-24f))
  {
    return coax_p_v2(0x1.000000p0f, 0.0f);
  }
  return coax_p_scale2(l_a, (0x1.000000p0f / l_l));
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_add2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return vec2f((l_a.x + l_b.x), (l_a.y + l_b.y));
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
fn coax_p_sl_hash(p_i: u32, p_k: u32) -> f32 {
  var l_i: u32 = p_i;
  var l_k: u32 = p_k;
  var l_s: u32;
  var l_w: u32;
  var l_r: u32;
  l_s = (((l_i + (l_k * 2654435769u)) * 747796405u) + 2891336453u);
  l_w = (((l_s >> u32(((l_s >> u32(28u)) + 4u))) ^ l_s) * 277803737u);
  l_r = ((l_w >> u32(22u)) ^ l_w);
  return (f32((l_r >> u32(8u))) / 0x1.000000p24f);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_sl_unit(p_v: vec3f) -> vec3f {
  var l_v: vec3f = p_v;
  var l_l: f32;
  l_l = coax_p_length3(l_v);
  if ((l_l < 0x1.0c6f7ap-20f))
  {
    return coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  }
  return coax_p_scale3(l_v, (0x1.000000p0f / l_l));
}
fn coax_p_sl_stretch(p_s: coax_t14, p_d: vec3f, p_along: f32, p_across: f32) -> coax_t14 {
  var l_s: coax_t14 = p_s;
  var l_d: vec3f = p_d;
  var l_along: f32 = p_along;
  var l_across: f32 = p_across;
  var l_o: coax_t14;
  var l_a2: f32;
  var l_e: f32;
  l_o = l_s;
  l_a2 = (l_across * l_across);
  l_e = ((l_along * l_along) - l_a2);
  l_o.z = coax_p_v3((l_a2 + ((l_e * l_d.x) * l_d.x)), ((l_e * l_d.x) * l_d.y), ((l_e * l_d.x) * l_d.z));
  l_o.w = coax_p_v3((l_a2 + ((l_e * l_d.y) * l_d.y)), ((l_e * l_d.y) * l_d.z), (l_a2 + ((l_e * l_d.z) * l_d.z)));
  return l_o;
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_step_on(p_x: f32, p_low: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  if ((l_x < 0x1.47ae14p-4f))
  {
    return l_low;
  }
  return 0x1.000000p0f;
}
fn coax_p_sl_sphere(p_a: f32, p_b: f32) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_z: f32;
  var l_r: f32;
  var l_t: f32;
  l_z = ((0x1.000000p1f * l_a) - 0x1.000000p0f);
  l_r = sqrt(coax_p_fmax((0x1.000000p0f - (l_z * l_z)), 0.0f));
  l_t = (0x1.921fb6p2f * l_b);
  return coax_p_v3((l_r * cos(l_t)), l_z, (l_r * sin(l_t)));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_sl_field_y(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return ((((0x1.666666p0f * sin((l_x * 0x1.0a3d70p-3f))) * cos((l_z * 0x1.c28f5cp-4f))) + (0x1.000000p-1f * sin(((l_x * 0x1.a3d70ap-2f) + (l_z * 0x1.28f5c2p-2f))))) + (0x1.99999ap-3f * sin(((l_x * 0x1.4cccccp0f) - (l_z * 0x1.ccccccp-1f)))));
}
fn coax_p_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
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
fn sort_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_sort_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn particle_over_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_particle_over_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@vertex
fn particle_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_particle_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@fragment
fn key_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_key_fs(coax_pos);
  return coax_r;
}
@fragment
fn sort_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_sort_fs(coax_pos);
  return coax_r;
}
@fragment
fn particle_over_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_particle_over_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_r;
}
@fragment
fn particle_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_particle_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_r;
}
fn coax_bits_sort_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_sort_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_sort_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_sort_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}
