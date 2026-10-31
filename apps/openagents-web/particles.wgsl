// A realm's particles: one instanced draw after the house, PER particles an
// emitter (verse/particles.coax), each a small quad facing the eye, added to
// the light behind it and hidden where the house is in front. Where a
// particle is and how bright comes from verse/particle-motion.coax,
// written as WGSL by coax wgsl at build time into the generated block
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

// ---- generated: verse/particle-motion.coax (verse_particle_offset, verse_particle_glow, verse_particle_size) by coax wgsl; do not edit
fn verse_particle_offset(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_radius: f32, p_axis: f32) -> f32 {
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
fn verse_particle_glow(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_flicker: f32) -> f32 {
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
fn verse_particle_size(p_kind: f32, p_seed: f32, p_i: f32, p_t: f32, p_speed: f32, p_size: f32) -> f32 {
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
fn coax_p_pm_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
// ---- end generated
// ---- generated: verse/haunt-light.coax (verse_haunt_bloom_share) by coax wgsl; do not edit
fn verse_haunt_bloom_share(p_peak: f32, p_glow: f32) -> f32 {
  var l_peak: f32 = p_peak;
  var l_glow: f32 = p_glow;
  var l_own: f32;
  var l_past: f32;
  l_own = coax_p_hl_min(0x1.ccccccp-1f, (coax_p_hl_max(0.0f, l_glow) * 0x1.99999ap-1f));
  l_past = coax_p_hl_min(0x1.000000p0f, (coax_p_hl_max(0.0f, (l_peak - 0x1.000000p0f)) / (l_peak + 0x1.000000p-1f)));
  return (l_own + (((0x1.000000p0f - l_own) * l_past) * 0x1.333334p-1f));
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
fn coax_p_hl_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
// ---- end generated
