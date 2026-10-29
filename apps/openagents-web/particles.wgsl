// A realm's particles: one instanced draw after the house, PER particles an
// emitter (verse/particles.coax), each a small quad facing the eye, added to
// the light behind it and hidden where the house is in front. Where a
// particle is and how bright comes from verse/particle-motion.coax,
// converted by coax/web/wgsl.py at build time into the generated block
// below. Group 0 is the realm's uniforms (realm.wgsl U, read up to its
// panels); group 1 the frame's emitters.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
  flash: vec4f,
  panel: array<vec4f, 56>,
}
@group(0) @binding(0) var<uniform> u: U;
struct Emitter { place: vec4f, color: vec4f, shape: vec4f, seed: vec4f }
@group(1) @binding(0) var<storage, read> emitters: array<Emitter>;

const PER: u32 = 64u;

struct P {
  @builtin(position) pos: vec4f,
  @location(0) corner: vec2f,
  @location(1) color: vec3f,
}
@vertex fn particle_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) n: u32) -> P {
  var o: P;
  let e = emitters[n / PER];
  let i = f32(n % PER);
  let quad = array(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0));
  o.corner = quad[v];
  if (i >= e.shape.y) { o.pos = vec4f(0.0, 0.0, 2.0, 1.0); o.color = vec3f(0.0); return o; }
  let kind = e.shape.x;
  let t = u.time;
  let speed = e.shape.w;
  let seed = e.seed.x;
  let off = vec3f(verse_particle_offset(kind, seed, i, t, speed, e.place.w, 0.0),
                  verse_particle_offset(kind, seed, i, t, speed, e.place.w, 1.0),
                  verse_particle_offset(kind, seed, i, t, speed, e.place.w, 2.0));
  let w = vec4f(e.place.xyz + off, 1.0);
  var c = vec4f(dot(u.r0, w), dot(u.r1, w), dot(u.r2, w), dot(u.r3, w));
  let half = verse_particle_size(kind, seed, i, t, speed, e.shape.z) * 0.5;
  let px = half * length(u.r1.xyz) / max(c.w, 0.01) * u.size.y * 0.5;
  let grow = max(1.0, 2.2 / max(px, 0.001));
  let s = half * grow;
  c.x += o.corner.x * s * length(u.r0.xyz);
  c.y += o.corner.y * s * length(u.r1.xyz);
  o.pos = c;
  let d = length(u.eye.xyz - w.xyz);
  let fog = exp(-d * d * 0.0006);
  o.color = e.color.rgb * verse_particle_glow(kind, seed, i, t, speed, e.color.w) * fog / sqrt(grow);
  return o;
}
fn under_panel(p: vec2f) -> bool {
  for (var k = 0; k < 28; k++) {
    if (f32(k) >= u.panels) { break; }
    let r = u.panel[k * 2];
    if (u.panel[k * 2 + 1].y > 0.0 && p.x >= r.x && p.x <= r.z && p.y >= r.y && p.y <= r.w) { return true; }
  }
  return false;
}
@fragment fn particle_fs(p: P) -> @location(0) vec4f {
  let r2 = dot(p.corner, p.corner);
  if (r2 >= 1.0 || under_panel(p.pos.xy)) { discard; }
  let a = exp(-r2 * 4.0) * (1.0 - r2);
  return vec4f(p.color * a, verse_haunt_bloom_share(max(p.color.r, max(p.color.g, p.color.b)), 1.0) * a);
}

// ---- generated: verse/particle-motion.coax (verse_particle_offset, verse_particle_glow, verse_particle_size) by coax/web/wgsl.py; do not edit
fn coax_p_pm_fract(l_x: f32) -> f32 {
  return (l_x - floor(l_x));
}
fn verse_particle_life(l_kind: f32) -> f32 {
  if ((l_kind < 0.5))
  {
    return 1.60000002;
  }
  if ((l_kind < 1.5))
  {
    return 14.0;
  }
  return 7.0;
}
fn verse_particle_hash(l_seed: f32, l_i: f32, l_k: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  coax_t0 = (coax_p_pm_fract((((l_seed * 0.103100002) + (l_i * 0.113700002)) + (l_k * 0.0731000006))) * 311.700012);
  coax_t1 = (l_i * 1.37);
  return coax_p_pm_fract((sin((coax_t0 + coax_t1)) * 437.584991));
}
fn verse_particle_age(l_kind: f32, l_seed: f32, l_i: f32, l_t: f32, l_speed: f32) -> f32 {
  var coax_t2: f32;
  var coax_t3: f32;
  var coax_t4: f32;
  var coax_t5: f32;
  coax_t3 = (l_t * l_speed);
  coax_t4 = verse_particle_life(l_kind);
  coax_t2 = (coax_t3 / coax_t4);
  coax_t5 = verse_particle_hash(l_seed, l_i, 0.0);
  return coax_p_pm_fract((coax_t2 + coax_t5));
}
fn verse_particle_offset(l_kind: f32, l_seed: f32, l_i: f32, l_t: f32, l_speed: f32, l_radius: f32, l_axis: f32) -> f32 {
  var coax_t6: f32;
  var coax_t7: f32;
  var coax_t8: f32;
  var coax_t9: f32;
  var coax_t10: f32;
  var coax_t11: f32;
  var l_a: f32 = verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  var l_h: f32 = verse_particle_hash(l_seed, l_i, (l_axis + 1.0));
  var l_w: f32 = (l_t * l_speed);
  if ((l_kind < 0.5))
  {
    if (((l_axis > 0.5) && (l_axis < 1.5)))
    {
      return ((l_a * l_radius) * (0.550000012 + (0.449999988 * l_h)));
    }
    coax_t6 = ((l_h - 0.5) * 0.0500000007);
    coax_t7 = ((sin((((l_w * (2.0999999 + (l_h * 1.70000005))) + (l_i * 2.29999995)) + (l_axis * 1.89999998))) * 0.0350000001) * l_a);
    var l_spread: f32 = (coax_t6 + coax_t7);
    return (l_spread * (0.400000006 + l_a));
  }
  if ((l_kind < 1.5))
  {
    var l_drift: f32 = (sin((((l_w * (0.109999999 + (l_h * 0.0900000036))) + (l_i * 1.70000005)) + (l_axis * 2.20000005))) * 0.449999988);
    if (((l_axis > 0.5) && (l_axis < 1.5)))
    {
      return (((0.150000006 + (l_h * 1.89999998)) + (l_drift * 0.5)) - (l_a * 0.349999994));
    }
    return ((((l_h * 2.0) - 1.0) * l_radius) + l_drift);
  }
  coax_t8 = (sin(((l_w * (0.310000002 + (l_h * 0.230000004))) + (l_i * 3.0999999))) * 0.899999976);
  coax_t9 = (sin(((l_w * (0.730000019 + (l_h * 0.409999996))) + (l_axis * 4.0))) * 0.349999994);
  var l_wander: f32 = (coax_t8 + coax_t9);
  if (((l_axis > 0.5) && (l_axis < 1.5)))
  {
    coax_t10 = (0.349999994 + (l_h * 1.10000002));
    coax_t11 = (sin(((l_w * 0.529999971) + l_i)) * 0.25);
    return (coax_t10 + coax_t11);
  }
  return ((((l_h * 2.0) - 1.0) * l_radius) + l_wander);
}
fn coax_p_pm_lo(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_pm_hi(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn verse_particle_glow(l_kind: f32, l_seed: f32, l_i: f32, l_t: f32, l_speed: f32, l_flicker: f32) -> f32 {
  var coax_t12: f32;
  var coax_t13: f32;
  var coax_t14: f32;
  var coax_t15: f32;
  var coax_t16: f32;
  var coax_t17: f32;
  var coax_t18: f32;
  var coax_t19: f32;
  var coax_t20: f32;
  var coax_t21: f32;
  var l_a: f32 = verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  var l_h: f32 = verse_particle_hash(l_seed, l_i, 5.0);
  if ((l_kind < 0.5))
  {
    coax_t12 = (1.0 - l_a);
    coax_t13 = coax_p_pm_lo(1.0, (l_a * 12.0));
    var l_fade: f32 = (coax_t12 * coax_t13);
    coax_t14 = (1.0 - (l_flicker * 0.5));
    coax_t16 = (l_flicker * 0.5);
    coax_t17 = sin(((l_t * (17.0 + (l_h * 9.0))) + l_i));
    coax_t15 = (coax_t16 * coax_t17);
    return (l_fade * (coax_t14 + coax_t15));
  }
  if ((l_kind < 1.5))
  {
    var l_edge: f32 = (coax_p_pm_lo(l_a, (1.0 - l_a)) * 5.0);
    coax_t18 = coax_p_pm_lo(1.0, l_edge);
    coax_t19 = (0.349999994 + (0.649999976 * l_h));
    return (coax_t18 * coax_t19);
  }
  var l_p: f32 = coax_p_pm_hi(0.0, sin((((l_t * l_speed) * (0.899999976 + (l_h * 0.800000012))) + (l_i * 5.30000019))));
  var l_p2: f32 = (l_p * l_p);
  coax_t20 = ((l_p2 * l_p2) * l_p);
  coax_t21 = coax_p_pm_lo(1.0, (coax_p_pm_lo(l_a, (1.0 - l_a)) * 8.0));
  return (coax_t20 * coax_t21);
}
fn verse_particle_size(l_kind: f32, l_seed: f32, l_i: f32, l_t: f32, l_speed: f32, l_size: f32) -> f32 {
  var l_a: f32 = verse_particle_age(l_kind, l_seed, l_i, l_t, l_speed);
  if ((l_kind < 0.5))
  {
    return (l_size * (1.0 - (l_a * 0.600000024)));
  }
  return (l_size * (0.699999988 + (0.600000024 * verse_particle_hash(l_seed, l_i, 6.0))));
}
// ---- end generated
// ---- generated: verse/haunt-light.coax (verse_haunt_bloom_share) by coax/web/wgsl.py; do not edit
fn coax_p_hl_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hl_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn verse_haunt_bloom_share(l_peak: f32, l_glow: f32) -> f32 {
  var coax_t4: f32;
  var coax_t5: f32;
  var l_own: f32 = coax_p_hl_min(0.899999976, (coax_p_hl_max(0.0, l_glow) * 0.800000012));
  coax_t4 = coax_p_hl_max(0.0, (l_peak - 1.0));
  coax_t5 = (l_peak + 0.5);
  var l_past: f32 = coax_p_hl_min(1.0, (coax_t4 / coax_t5));
  return (l_own + (((1.0 - l_own) * l_past) * 0.600000024));
}
// ---- end generated
