// ---- generated: apps/openagents-web/gpu/particles.coax (the module) by coax wgsl; do not edit
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
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec3f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 64>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> emitters: array<coax_t8>;
fn coax_p_particle_vs(p_v: u32, p_n: u32) -> coax_t12 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_o: coax_t12;
  var l_e: coax_t8;
  var l_i: f32;
  var l_quad: array<vec2f, 6>;
  var l_kind: f32;
  var l_t: f32;
  var l_speed: f32;
  var l_seed: f32;
  var l_off: vec3f;
  var l_w: vec4f;
  var l_c: vec4f;
  var l_half: f32;
  var l_px: f32;
  var l_grow: f32;
  var l_s: f32;
  var l_d: f32;
  var l_fog: f32;
  l_o = coax_t12();
  l_e = emitters[i32((l_n / 64u))];
  l_i = f32((l_n % 64u));
  l_quad = array<vec2f, 6>(coax_p_v2((-0x1.000000p0f), (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_o.y = l_quad[i32(l_v)];
  if ((l_i >= l_e.z.y))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    l_o.z = coax_p_v3(0.0f, 0.0f, 0.0f);
    return l_o;
  }
  l_kind = l_e.z.x;
  l_t = coax_at_u(i32(2i));
  l_speed = l_e.z.w;
  l_seed = l_e.w.x;
  l_off = coax_p_v3(coax_p_verse_particle_offset(l_kind, l_seed, l_i, l_t, l_speed, l_e.x.w, 0.0f), coax_p_verse_particle_offset(l_kind, l_seed, l_i, l_t, l_speed, l_e.x.w, 0x1.000000p0f), coax_p_verse_particle_offset(l_kind, l_seed, l_i, l_t, l_speed, l_e.x.w, 0x1.000000p1f));
  l_w = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_v3(l_e.x.x, l_e.x.y, l_e.x.z), l_off), 0x1.000000p0f);
  l_c = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_w), coax_p_wg_dot4(coax_p_wg_at4(12i), l_w), coax_p_wg_dot4(coax_p_wg_at4(16i), l_w), coax_p_wg_dot4(coax_p_wg_at4(20i), l_w));
  l_half = (coax_p_verse_particle_size(l_kind, l_seed, l_i, l_t, l_speed, l_e.z.z) * 0x1.000000p-1f);
  l_px = ((((l_half * coax_p_wg_length3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))))) / coax_p_fmax(l_c.w, 0x1.47ae14p-7f)) * coax_at_u(i32(1i))) * 0x1.000000p-1f);
  l_grow = coax_p_fmax(0x1.000000p0f, (0x1.19999ap1f / coax_p_fmax(l_px, 0x1.0624dep-10f)));
  l_s = (l_half * l_grow);
  l_c.x = (l_c.x + ((l_o.y.x * l_s) * coax_p_wg_length3(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))))));
  l_c.y = (l_c.y + ((l_o.y.y * l_s) * coax_p_wg_length3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))))));
  l_o.x = l_c;
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_v3(l_w.x, l_w.y, l_w.z)));
  l_fog = exp((((-l_d) * l_d) * 0x1.3a92a4p-11f));
  l_o.z = coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_e.y.x, l_e.y.y, l_e.y.z), coax_p_verse_particle_glow(l_kind, l_seed, l_i, l_t, l_speed, l_e.y.w)), l_fog), sqrt(l_grow));
  return l_o;
}
fn coax_p_particle_fs(p_p: coax_t12) -> vec4f {
  var l_p: coax_t12 = p_p;
  var l_r2: f32;
  var l_a: f32;
  l_r2 = coax_p_wg_dot2(l_p.y, l_p.y);
  if (((l_r2 >= 0x1.000000p0f) || coax_p_particles_under_panel(coax_p_v2(l_p.x.x, l_p.x.y))))
  {
    discard;
  }
  l_a = (exp(((-l_r2) * 0x1.000000p2f)) * (0x1.000000p0f - l_r2));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(l_p.z, l_a), (coax_p_verse_haunt_bloom_share(coax_p_fmax(l_p.z.x, coax_p_fmax(l_p.z.y, l_p.z.z)), 0x1.000000p0f) * l_a));
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
fn coax_p_verse_particle_offset(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_radius: f32, p_axis: f32) -> f32 {
  var l_kind: f32 = p_kind;
  var l_seed: f32 = p_seed;
  var l_i: f32 = p_i;
  var l_t: f32 = p_t;
  var l_speed: f32 = p_speed;
  var l_radius: f32 = p_radius;
  var l_axis: f32 = p_axis;
  var l_a: f32;
  var l_h: f32;
  var l_w: f32;
  var l_spread: f32;
  var l_drift: f32;
  var l_wander: f32;
  l_a = coax_p_verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  l_h = coax_p_verse_particle_hash(l_seed, l_i, (l_axis + 0x1.000000p0f));
  l_w = (l_t * l_speed);
  if ((l_kind < 0x1.000000p-1f))
  {
    if (((l_axis > 0x1.000000p-1f) && (l_axis < 0x1.800000p0f)))
    {
      return ((l_a * l_radius) * (0x1.19999ap-1f + (0x1.ccccccp-2f * l_h)));
    }
    l_spread = (((l_h - 0x1.000000p-1f) * 0x1.99999ap-5f) + ((sin((((l_w * (0x1.0cccccp1f + (l_h * 0x1.b33334p0f))) + (l_i * 0x1.266666p1f)) + (l_axis * 0x1.e66666p0f))) * 0x1.1eb852p-5f) * l_a));
    return (l_spread * (0x1.99999ap-2f + l_a));
  }
  if ((l_kind < 0x1.800000p0f))
  {
    l_drift = (sin((((l_w * (0x1.c28f5cp-4f + (l_h * 0x1.70a3d8p-4f))) + (l_i * 0x1.b33334p0f)) + (l_axis * 0x1.19999ap1f))) * 0x1.ccccccp-2f);
    if (((l_axis > 0x1.000000p-1f) && (l_axis < 0x1.800000p0f)))
    {
      return (((0x1.333334p-3f + (l_h * 0x1.e66666p0f)) + (l_drift * 0x1.000000p-1f)) - (l_a * 0x1.666666p-2f));
    }
    return ((((l_h * 0x1.000000p1f) - 0x1.000000p0f) * l_radius) + l_drift);
  }
  l_wander = ((sin(((l_w * (0x1.3d70a4p-2f + (l_h * 0x1.d70a3ep-3f))) + (l_i * 0x1.8cccccp1f))) * 0x1.ccccccp-1f) + (sin(((l_w * (0x1.75c290p-1f + (l_h * 0x1.a3d70ap-2f))) + (l_axis * 0x1.000000p2f))) * 0x1.666666p-2f));
  if (((l_axis > 0x1.000000p-1f) && (l_axis < 0x1.800000p0f)))
  {
    return ((0x1.666666p-2f + (l_h * 0x1.19999ap0f)) + (sin(((l_w * 0x1.0f5c28p-1f) + l_i)) * 0x1.000000p-2f));
  }
  return ((((l_h * 0x1.000000p1f) - 0x1.000000p0f) * l_radius) + l_wander);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_verse_particle_size(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_size: f32) -> f32 {
  var l_kind: f32 = p_kind;
  var l_seed: f32 = p_seed;
  var l_i: f32 = p_i;
  var l_t: f32 = p_t;
  var l_speed: f32 = p_speed;
  var l_size: f32 = p_size;
  var l_a: f32;
  l_a = coax_p_verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  if ((l_kind < 0x1.000000p-1f))
  {
    return (l_size * (0x1.000000p0f - (l_a * 0x1.333334p-1f)));
  }
  return (l_size * (0x1.666666p-1f + (0x1.333334p-1f * coax_p_verse_particle_hash(l_seed, l_i, 0x1.800000p2f))));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
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
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_verse_particle_glow(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_flicker: f32) -> f32 {
  var l_kind: f32 = p_kind;
  var l_seed: f32 = p_seed;
  var l_i: f32 = p_i;
  var l_t: f32 = p_t;
  var l_speed: f32 = p_speed;
  var l_flicker: f32 = p_flicker;
  var l_a: f32;
  var l_h: f32;
  var l_fade: f32;
  var l_edge: f32;
  var l_p: f32;
  var l_p2: f32;
  l_a = coax_p_verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  l_h = coax_p_verse_particle_hash(l_seed, l_i, 0x1.400000p2f);
  if ((l_kind < 0x1.000000p-1f))
  {
    l_fade = ((0x1.000000p0f - l_a) * coax_p_pm_lo(0x1.000000p0f, (l_a * 0x1.800000p3f)));
    return (l_fade * ((0x1.000000p0f - (l_flicker * 0x1.000000p-1f)) + ((l_flicker * 0x1.000000p-1f) * sin(((l_t * (0x1.100000p4f + (l_h * 0x1.200000p3f))) + l_i)))));
  }
  if ((l_kind < 0x1.800000p0f))
  {
    l_edge = (coax_p_pm_lo(l_a, (0x1.000000p0f - l_a)) * 0x1.400000p2f);
    return (coax_p_pm_lo(0x1.000000p0f, l_edge) * (0x1.666666p-2f + (0x1.4cccccp-1f * l_h)));
  }
  l_p = coax_p_pm_hi(0.0f, sin((((l_t * l_speed) * (0x1.ccccccp-1f + (l_h * 0x1.99999ap-1f))) + (l_i * 0x1.533334p2f))));
  l_p2 = (l_p * l_p);
  return (((l_p2 * l_p2) * l_p) * coax_p_pm_lo(0x1.000000p0f, (coax_p_pm_lo(l_a, (0x1.000000p0f - l_a)) * 0x1.000000p3f)));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_particles_under_panel(p_p: vec2f) -> bool {
  var l_p: vec2f = p_p;
  var l_k: i32;
  var l_r: vec4f;
  {
    l_k = 0i;
    while ((l_k < 28i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_r = coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 32i));
      if ((((((coax_at_u(i32(((i32(((l_k * 2i) + 1i)) * 4i) + 33i))) > 0.0f) && (l_p.x >= l_r.x)) && (l_p.x <= l_r.z)) && (l_p.y >= l_r.y)) && (l_p.y <= l_r.w)))
      {
        return true;
      }
      l_k = (l_k + 1i);
    }
  }
  return false;
}
fn coax_p_verse_haunt_bloom_share(p_peak: f32, p_glow: f32) -> f32 {
  var l_peak: f32 = p_peak;
  var l_glow: f32 = p_glow;
  var l_own: f32;
  var l_past: f32;
  l_own = coax_p_hl_min(0x1.ccccccp-1f, (coax_p_hl_max(0.0f, l_glow) * 0x1.99999ap-1f));
  l_past = coax_p_hl_min(0x1.000000p0f, (coax_p_hl_max(0.0f, (l_peak - 0x1.000000p0f)) / (l_peak + 0x1.000000p-1f)));
  return (l_own + (((0x1.000000p0f - l_own) * l_past) * 0x1.333334p-1f));
}
fn coax_p_verse_particle_age(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32) -> f32 {
  var l_kind: f32 = p_kind;
  var l_seed: f32 = p_seed;
  var l_i: f32 = p_i;
  var l_t: f32 = p_t;
  var l_speed: f32 = p_speed;
  return coax_p_pm_fract((((l_t * l_speed) / coax_p_verse_particle_life(l_kind)) + coax_p_verse_particle_hash(l_seed, l_i, 0.0f)));
}
fn coax_p_verse_particle_hash(p_seed: f32, p_i: f32, p_k: f32) -> f32 {
  var l_seed: f32 = p_seed;
  var l_i: f32 = p_i;
  var l_k: f32 = p_k;
  return coax_p_pm_fract((sin(((coax_p_pm_fract((((l_seed * 0x1.a64c30p-4f) + (l_i * 0x1.d1b718p-4f)) + (l_k * 0x1.2b6ae8p-4f))) * 0x1.37b334p8f) + (l_i * 0x1.5eb852p0f))) * 0x1.b595c2p8f));
}
fn coax_p_pm_lo(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_pm_hi(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hl_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hl_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_verse_particle_life(p_kind: f32) -> f32 {
  var l_kind: f32 = p_kind;
  if ((l_kind < 0x1.000000p-1f))
  {
    return 0x1.99999ap0f;
  }
  if ((l_kind < 0x1.800000p0f))
  {
    return 0x1.c00000p3f;
  }
  return 0x1.c00000p2f;
}
fn coax_p_pm_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
@vertex
fn particle_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_particle_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z);
}
@fragment
fn particle_fs(coax_in: coax_io_12) -> @location(0) vec4f {
  let coax_r = coax_p_particle_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z));
  return coax_r;
}