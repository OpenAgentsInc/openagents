// The water lab (waterlab.coax): one traced pass. A sea to the horizon on
// the spectral tiles waterlab.coax packs into the uniforms each frame
// (verse/water-spectrum.coax, phase W4 of docs/verse/water.md), breaking on
// a sloping beach, in the painted water look of docs/verse/art-styles.md 12
// (Everglade's river preset): depth bands, the painted sky in Schlick's
// Fresnel on a calmer normal, sparkles near the sun's reflection, white
// paint for whitecaps, surf and the waterline, and the hazy, cyan-green air
// of the owner's references.
struct U {
  size: vec4f,    // width, height, time, fade
  mode: vec4f,    // still, phone, scene, dpr
  eye: vec4f,     // xyz, tan of half the vertical field
  fwd: vec4f, right: vec4f, up: vec4f,
  sun: vec4f,     // toward the sun, strength
  sea: vec4f,     // level, cascade 0's patch, the fine patch, choppiness
  wind: vec4f,    // direction x, z, speed, significant height
  beach: vec4f,   // the still waterline's z, the slope, the peak's deep wavenumber, its omega
  pack: vec4f,    // the half floats' scales: height, slope, fine slope
  tiles: array<vec4u, 3072>,  // cascade 0: (height, foam), (sx, sz) a texel; then the fine slopes
}
@group(0) @binding(0) var<uniform> u: U;

const N = 64u;
const G = 9.81;

fn word(i: u32) -> u32 { return u.tiles[i >> 2u][i & 3u]; }
// Cascade 0 at a rest point: height (m), foam, slope x, slope z; bilinear, wrapped.
fn coarse(p: vec2f) -> vec4f {
  let q = p / u.sea.y * f32(N) - 0.5;
  let f = fract(q);
  let i = vec2i(floor(q));
  var acc = vec4f(0.0);
  for (var k = 0; k < 4; k++) {
    let o = vec2i(k & 1, k >> 1);
    let c = vec2u((i + o) & vec2i(i32(N) - 1));
    let t = c.y * N + c.x;
    let a = unpack2x16float(word(2u * t));
    let b = unpack2x16float(word(2u * t + 1u));
    let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
    acc += w * vec4f(a.x * u.pack.x, a.y, b.x * u.pack.y, b.y * u.pack.y);
  }
  return acc;
}
// The fine tile's slopes (the desktop's second cascade, the phone's baked tile).
fn fine(p: vec2f) -> vec2f {
  let q = p / u.sea.z * f32(N) - 0.5;
  let f = fract(q);
  let i = vec2i(floor(q));
  var acc = vec2f(0.0);
  for (var k = 0; k < 4; k++) {
    let o = vec2i(k & 1, k >> 1);
    let c = vec2u((i + o) & vec2i(i32(N) - 1));
    let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
    acc += w * unpack2x16float(word(2u * N * N + c.y * N + c.x)) * u.pack.z;
  }
  return acc;
}

fn hash2(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), s.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), s.x), s.y);
}

// The beach: the ground's height (m), rising toward +z through the still
// waterline at z = beach.x, gently uneven along x, levelling out at 40 m deep.
fn ground(p: vec2f) -> f32 {
  let wobble = 0.35 * sin(p.x * 0.045) + 0.18 * sin(p.x * 0.11 + 1.3);
  let z = p.y - u.beach.x - wobble / u.beach.y * 0.05;
  // out of the water the beach climbs to a berm, under it the shelf runs out gently
  let land = max(z, 0.0) * 0.09;
  return max(z * u.beach.y + land + 0.03 * noise(p * 0.3), -40.0);
}
fn depth_at(p: vec2f) -> f32 { return max(u.sea.x - ground(p), 0.0); }

// Shallow water (verse/water-spectrum.coax wsp_surf): the peak wave's
// wavenumber over depth h (Eckart's form of w^2 = g k tanh(k h)), the
// shoaling gain sqrt(cg0 / cg), and how far it is past breaking.
fn shallow_k(k0: f32, h: f32) -> f32 { return k0 / sqrt(max(tanh(k0 * max(h, 0.01)), 1e-4)); }
fn shoal_gain(k0: f32, h: f32) -> f32 {
  let k = shallow_k(k0, h);
  let kh = k * max(h, 0.01);
  let n = 0.5 * (1.0 + 2.0 * kh / sinh(min(2.0 * kh, 20.0)));
  let c = sqrt(G / k * tanh(kh));
  let cg0 = 0.5 * sqrt(G / k0);
  return sqrt(cg0 / max(n * c, 1e-3));
}
// The surf's phase across the beach: the integral of k(h) along the way in.
fn surf_phase(p: vec2f) -> f32 {
  let k0 = u.beach.z;
  let h = depth_at(p);
  let reach = min(h, 3.0 / k0);
  var sum = 0.0;
  for (var i = 0; i < 8; i++) {
    let hh = (f32(i) + 0.5) / 8.0 * reach;
    sum += shallow_k(k0, hh);
  }
  return (sum / 8.0 * reach + (h - reach) * k0) / u.beach.y;
}
struct Surf { lift: f32, white: f32, gain: f32 }
fn surf(p: vec2f, t: f32) -> Surf {
  var o: Surf;
  let h = depth_at(p);
  let k0 = u.beach.z;
  o.gain = select(min(shoal_gain(k0, h), 2.2), 1.0, h > 3.0 / k0);
  let height = u.wind.w * 0.5 * o.gain;
  let breaking = clamp(height / (0.78 * max(h, 0.02)) - 0.6, 0.0, 1.0);
  // the waves come in shore-normal, their crests bending with the beach's wobble
  let phase = surf_phase(p) + u.beach.w * t + 0.6 * noise(vec2f(p.x * 0.03, 0.0)) * 6.28;
  let crest = pow(0.5 + 0.5 * cos(phase), 6.0);
  let line = pow(0.5 + 0.5 * cos(phase - 0.25), 40.0);
  o.lift = crest * height * 0.6 * smoothstep(0.0, 0.4, h) * (1.0 - 0.6 * breaking);
  let broken = noise(vec2f(p.x * 0.7, phase * 2.0)) * 0.6 + 0.4;
  // white paint on the breaking crest, and a thinner trail behind it
  let trail = pow(0.5 + 0.5 * cos(phase + 0.9), 10.0) * 0.6;
  o.white = breaking * smoothstep(0.45, 0.8, max(line, trail) * broken + 0.15 * breaking * breaking) * smoothstep(0.03, 0.2, h);
  return o;
}

// The sea's surface at rest point p: its height over the level.
fn sea_height(p: vec2f, t: f32) -> f32 {
  let h = depth_at(p);
  let taper = smoothstep(0.0, 0.6, h);
  let s = surf(p, t);
  // a trough never bares the bed: shallow water's waves ride over it
  let wave = max(coarse(p).x * min(s.gain, 1.6) * taper, -0.5 * h);
  return u.sea.x + wave + s.lift;
}

// The painted sky: zenith to a pale haze at the horizon, warm toward the sun.
fn sky(d: vec3f) -> vec3f {
  let up = clamp(d.y, 0.0, 1.0);
  let zenith = vec3f(0.36, 0.60, 0.80);
  let horizon = vec3f(0.80, 0.88, 0.86);
  var c = mix(horizon, zenith, pow(up, 0.55));
  let s = max(dot(d, u.sun.xyz), 0.0);
  c += vec3f(1.0, 0.86, 0.62) * (pow(s, 8.0) * 0.30 + pow(s, 900.0) * 1.2);
  // soft clouds, broad and pale
  if (d.y > 0.0) {
    let q = d.xz / (d.y + 0.15) * 1.6;
    let cl = smoothstep(0.55, 0.85, noise(q) * 0.65 + noise(q * 2.3 + 4.0) * 0.35);
    c = mix(c, vec3f(0.95, 0.96, 0.93), cl * 0.55 * smoothstep(0.0, 0.25, d.y));
  }
  return c;
}
// The air between the eye and what it sees: BotW's teal-green haze.
fn haze(c: vec3f, d: vec3f, dist: f32) -> vec3f {
  let a = 1.0 - exp(-dist / 420.0);
  let tint = mix(vec3f(0.74, 0.86, 0.84), vec3f(1.0, 0.90, 0.74), pow(max(dot(d, u.sun.xyz), 0.0), 6.0) * 0.5);
  return mix(c, tint, a * 0.85);
}

fn sand(p: vec2f, n: vec3f, wet: f32) -> vec3f {
  let dry = mix(vec3f(0.84, 0.77, 0.60), vec3f(0.78, 0.71, 0.55), noise(p * 1.7));
  let base = mix(dry, vec3f(0.64, 0.58, 0.45), wet);
  let light = 0.55 + 0.6 * max(dot(n, u.sun.xyz), 0.0);
  return base * light * vec3f(1.0, 0.97, 0.92);
}

// The water in the painted look.
fn water(p: vec2f, world: vec3f, d: vec3f, dist: f32, t: f32) -> vec3f {
  let h = depth_at(p);
  let c = coarse(p);
  let s = surf(p, t);
  // slopes: cascade 0 shoaled, the fine cascade faded with the pixel's footprint
  let footprint = dist * 2.0 * u.eye.w / u.size.y / max(abs(d.y), 0.08);
  let keep = clamp(u.sea.z / 64.0 / max(footprint, 1e-4) - 0.5, 0.0, 1.0);
  let farfade = 1.0 - smoothstep(150.0, 900.0, dist);
  let slope = (c.zw * min(s.gain, 1.6) + fine(p * 1.0 + vec2f(u.wind.x, u.wind.y) * t * 0.3) * keep) * farfade;
  let n = normalize(vec3f(-slope.x, 1.0, -slope.y));
  let calm = normalize(vec3f(-slope.x / 3.0, 1.0, -slope.y / 3.0));
  // depth bands, each edge soft over 0.15 m, and a deeper band for the open sea
  let shallow = vec3f(0.30, 0.72, 0.74);
  let mid = vec3f(0.10, 0.45, 0.55);
  let deep = vec3f(0.05, 0.22, 0.36);
  let open = vec3f(0.035, 0.17, 0.31);
  var col = mix(shallow, mid, smoothstep(0.25, 0.55, h));
  col = mix(col, deep, smoothstep(1.35, 1.65, h));
  col = mix(col, open, smoothstep(6.0, 14.0, h));
  // the land's light on the bands, the waves' faces a little darker
  col *= 0.82 + 0.3 * max(dot(n, u.sun.xyz), 0.0);
  // the painted sky in Schlick's Fresnel, at most half of it
  let v = -d;
  let r = reflect(d, calm);
  let fres = 0.02 + 0.98 * pow(1.0 - max(dot(calm, v), 0.0), 5.0);
  col = mix(col, sky(vec3f(r.x, abs(r.y), r.z)), min(fres, 0.5));
  // sparkles on a grid within a few degrees of the sun's reflection, twinkling
  let rs = reflect(-u.sun.xyz, n);
  let cell = select(0.15, 0.25, u.mode.y > 0.5) * max(1.0, footprint / 0.05);
  let g = floor(world.xz / cell);
  let tw = hash2(g + floor(t * 3.0));
  let near = smoothstep(cos(0.07 + footprint * 0.02), 1.0, dot(rs, v));
  col += vec3f(1.0, 0.95, 0.85) * step(0.82, tw) * near * 1.4 * farfade;
  // whitecaps from the foam field, broken into paint by noise; the surf; the waterline
  let froth = noise(p * 1.4 + vec2f(t * 0.2, 0.0)) * 0.6 + noise(p * 4.1) * 0.4;
  let caps = smoothstep(0.45, 0.75, c.y * (0.6 + froth));
  // the waterline: where the water over the sand runs thin
  let thin = world.y - ground(p);
  let line = (1.0 - smoothstep(0.006, 0.02, thin + 0.004 * sin(p.x * 3.1 + t * 1.5))) * smoothstep(0.3, 0.6, froth + 0.25);
  let white = max(max(caps * 0.95, s.white * smoothstep(0.3, 0.6, froth + 0.3)), line);
  col = mix(col, vec3f(0.97, 0.98, 0.96) * (0.85 + 0.15 * max(dot(n, u.sun.xyz), 0.0)), clamp(white, 0.0, 1.0));
  return col;
}

// The traced pass.
struct Hit { t: f32, water: bool }
fn trace(o: vec3f, d: vec3f, t0: f32) -> Hit {
  var hit = Hit(1e9, false);
  let amp = u.wind.w * 1.3 + 0.4;
  // the beach: march the ground until it rises over the ray
  var t = 0.0;
  var prev = o.y - ground(o.xz);
  for (var i = 0; i < 64; i++) {
    let step = 0.4 + t * 0.06;
    let nt = t + step;
    let q = o + d * nt;
    let gap = q.y - ground(q.xz);
    if (gap < 0.0) { hit.t = t + step * prev / max(prev - gap, 1e-4); break; }
    prev = gap; t = nt;
    if (t > 900.0) { break; }
  }
  // the water: between the planes the waves can reach, then secant refinement
  if (d.y < 0.0 || o.y < u.sea.x + amp) {
    let top = (o.y - (u.sea.x + amp)) / max(-d.y, 1e-4);
    let bottom = (o.y - (u.sea.x - amp)) / max(-d.y, 1e-4);
    var a = max(top, 0.0);
    var b = select(min(bottom, 2000.0), 2000.0, d.y >= 0.0);
    let steps = 14;
    var fa = (o + d * a).y - sea_height((o + d * a).xz, t0);
    var found = false;
    for (var i = 1; i <= steps; i++) {
      let tb = a + (b - a) * f32(i) / f32(steps);
      let q = o + d * tb;
      let fb = q.y - sea_height(q.xz, t0);
      if (fb < 0.0) { b = tb; found = true; break; }
      a = tb; fa = fb;
    }
    if (found) {
      var fb = (o + d * b).y - sea_height((o + d * b).xz, t0);
      for (var i = 0; i < 4; i++) {
        let tm = a + (b - a) * fa / max(fa - fb, 1e-5);
        let fm = (o + d * tm).y - sea_height((o + d * tm).xz, t0);
        if (fm > 0.0) { a = tm; fa = fm; } else { b = tm; fb = fm; }
      }
      let tw = a + (b - a) * fa / max(fa - fb, 1e-5);
      if (tw < hit.t) { hit = Hit(tw, true); }
    } else if (d.y < 0.0 && bottom > 2000.0 && 2000.0 < hit.t) {
      hit = Hit((o.y - u.sea.x) / -d.y, true);
    }
  }
  return hit;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) frag: vec4f) -> @location(0) vec4f {
  let ndc = vec2f(frag.x / u.size.x * 2.0 - 1.0, 1.0 - frag.y / u.size.y * 2.0);
  let aspect = u.size.x / u.size.y;
  let d = normalize(u.fwd.xyz + u.right.xyz * ndc.x * u.eye.w * aspect + u.up.xyz * ndc.y * u.eye.w);
  let o = u.eye.xyz;
  let t = u.size.z;
  let hit = trace(o, d, t);
  var c: vec3f;
  if (hit.t > 1e8) {
    c = haze(sky(d), d, 3000.0 * (1.0 - smoothstep(0.0, 0.2, d.y)));
  } else {
    let w = o + d * hit.t;
    if (hit.water) {
      c = water(w.xz, w, d, hit.t, t);
    } else {
      let e = 0.05;
      let n = normalize(vec3f(ground(w.xz - vec2f(e, 0.0)) - ground(w.xz + vec2f(e, 0.0)), 2.0 * e,
                              ground(w.xz - vec2f(0.0, e)) - ground(w.xz + vec2f(0.0, e))));
      // the swash leaves the sand wet a little above the still waterline
      let wet = 1.0 - smoothstep(0.08, 0.3, w.y - u.sea.x);
      c = sand(w.xz, n, wet);
    }
    c = haze(c, d, hit.t);
  }
  return vec4f(c, 1.0);
}

// The strokes (none yet): host.js draws them over the pass.
struct V { @builtin(position) pos: vec4f, @location(0) tint: vec4f }
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) e: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  o.tint = c;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f { return vec4f(v.tint.rgb, 1.0); }

// The painterly style's shading (verse/style.coax), written by build.sh.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter) by coax/web/wgsl.py; do not edit
fn coax_p_st_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_sat(l_x: f32) -> f32 {
  return coax_p_st_min(1.0, coax_p_st_max(0.0, l_x));
}
fn verse_style_wrap(l_ndl: f32) -> f32 {
  return coax_p_st_sat(((l_ndl + 0.25) / 1.25));
}
fn verse_style_soft_step(l_e: f32, l_s: f32, l_x: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  coax_t0 = (l_x - (l_e - l_s));
  coax_t1 = coax_p_st_max((2.0 * l_s), 9.99999997e-07);
  var l_t: f32 = coax_p_st_sat((coax_t0 / coax_t1));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_style_bands(l_w: f32, l_e0: f32, l_e1: f32, l_s: f32, l_mid: f32) -> f32 {
  var coax_t2: f32;
  var coax_t3: f32;
  var coax_t4: f32;
  var coax_t5: f32;
  coax_t2 = (l_mid * verse_style_soft_step(l_e0, l_s, l_w));
  coax_t4 = (1.0 - l_mid);
  coax_t5 = verse_style_soft_step(l_e1, l_s, l_w);
  coax_t3 = (coax_t4 * coax_t5);
  return (coax_t2 + coax_t3);
}
fn verse_style_bands2(l_w: f32, l_e: f32, l_s: f32) -> f32 {
  return verse_style_soft_step(l_e, l_s, l_w);
}
fn verse_style_shade(l_albedo: f32, l_shadow_light: f32, l_lit_light: f32, l_band: f32) -> f32 {
  var coax_t6: f32;
  var coax_t7: f32;
  coax_t6 = (l_lit_light - l_shadow_light);
  coax_t7 = coax_p_st_sat(l_band);
  return (l_albedo * (l_shadow_light + (coax_t6 * coax_t7)));
}
fn verse_style_cool(l_c: f32, l_tint: f32, l_tint_lum: f32, l_amount: f32) -> f32 {
  var coax_t8: f32;
  var coax_t9: f32;
  var coax_t10: f32;
  var coax_t11: f32;
  coax_t9 = coax_p_st_max(l_tint, 9.99999997e-07);
  coax_t10 = coax_p_st_max(l_tint_lum, 9.99999997e-07);
  coax_t8 = (coax_t9 / coax_t10);
  coax_t11 = coax_p_st_max(l_amount, 0.0);
  return (l_c * pow(coax_t8, coax_t11));
}
fn verse_style_keep_value(l_c: f32, l_before: f32, l_after: f32) -> f32 {
  var coax_t12: f32;
  var coax_t13: f32;
  coax_t12 = (l_c * l_before);
  coax_t13 = coax_p_st_max(l_after, 9.99999997e-07);
  return (coax_t12 / coax_t13);
}
fn verse_style_banded(l_shadow_c: f32, l_lit_c: f32, l_band: f32) -> f32 {
  var coax_t14: f32;
  var coax_t15: f32;
  coax_t14 = (l_lit_c - l_shadow_c);
  coax_t15 = coax_p_st_sat(l_band);
  return (l_shadow_c + (coax_t14 * coax_t15));
}
fn verse_style_ambient(l_ny: f32, l_sky: f32, l_ground: f32, l_scale: f32) -> f32 {
  var coax_t16: f32;
  var coax_t17: f32;
  coax_t16 = (l_sky - l_ground);
  coax_t17 = coax_p_st_sat(((l_ny * 0.5) + 0.5));
  return ((l_ground + (coax_t16 * coax_t17)) * l_scale);
}
fn verse_style_dab(l_rdl: f32, l_cut: f32, l_strength: f32) -> f32 {
  return select(0.0, l_strength, (l_rdl >= l_cut));
}
fn verse_style_rim(l_ndv: f32, l_ndl: f32, l_start: f32, l_full: f32, l_strength: f32) -> f32 {
  var l_t: f32 = coax_p_st_sat(((l_ndv - l_start) / (l_full - l_start)));
  var l_edge: f32 = ((l_t * l_t) * (3.0 - (2.0 * l_t)));
  return ((l_edge * (0.349999994 + (0.649999976 * coax_p_st_sat((l_ndl + 0.300000012))))) * l_strength);
}
fn verse_style_haze(l_d: f32, l_y: f32, l_base_y: f32, l_start: f32, l_density: f32, l_cap: f32, l_falloff: f32) -> f32 {
  var coax_t20: f32;
  var coax_t21: f32;
  var coax_t22: f32;
  var coax_t23: f32;
  coax_t20 = (-l_density);
  coax_t21 = coax_p_st_max((l_d - l_start), 0.0);
  var l_dist: f32 = (1.0 - exp((coax_t20 * coax_t21)));
  coax_t22 = (-coax_p_st_max((l_y - l_base_y), 0.0));
  coax_t23 = coax_p_st_max(l_falloff, 0.00100000005);
  var l_height: f32 = exp((coax_t22 / coax_t23));
  return coax_p_st_min((l_dist * (1.0 + ((l_height - 1.0) * 0.600000024))), l_cap);
}
fn verse_style_haze_glow(l_vdl: f32) -> f32 {
  var l_a: f32 = coax_p_st_sat(l_vdl);
  var l_a2: f32 = (l_a * l_a);
  var l_a4: f32 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0.449999988);
}
fn verse_style_haze_teal(l_ch: f32) -> f32 {
  if ((l_ch < 0.5))
  {
    return 0.699999988;
  }
  if ((l_ch < 1.5))
  {
    return 1.0;
  }
  return 0.839999974;
}
fn verse_style_haze_hue(l_h: f32, l_ch: f32, l_f: f32, l_cap: f32) -> f32 {
  var l_t: f32 = verse_style_haze_teal(l_ch);
  var l_far: f32 = verse_style_soft_step(((0.150000006 + l_cap) * 0.5), ((l_cap - 0.150000006) * 0.5), l_f);
  return (l_h * (l_t + ((1.0 - l_t) * l_far)));
}
fn verse_style_fog(l_c: f32, l_haze_c: f32, l_f: f32) -> f32 {
  var coax_t24: f32;
  var coax_t25: f32;
  coax_t24 = (l_haze_c - l_c);
  coax_t25 = coax_p_st_sat(l_f);
  return (l_c + (coax_t24 * coax_t25));
}
fn verse_style_sky_t(l_up: f32) -> f32 {
  return pow(coax_p_st_max(l_up, 9.99999997e-07), 0.550000012);
}
fn verse_style_sky_band(l_up: f32, l_lift: f32) -> f32 {
  return (l_lift * (1.0 - coax_p_st_sat((abs(l_up) / 0.104500003))));
}
fn verse_style_halo(l_cos_t: f32, l_k: f32) -> f32 {
  var coax_t26: f32;
  var coax_t27: f32;
  coax_t26 = (-l_k);
  coax_t27 = coax_p_st_max((1.0 - l_cos_t), 0.0);
  return exp((coax_t26 * coax_t27));
}
fn verse_style_hash(l_x: f32, l_y: f32) -> f32 {
  var l_v: f32 = (sin(((l_x * 127.099998) + (l_y * 311.700012))) * 43758.5469);
  return (l_v - floor(l_v));
}
fn verse_style_noise(l_x: f32, l_y: f32) -> f32 {
  var l_ix: f32 = floor(l_x);
  var l_iy: f32 = floor(l_y);
  var l_fx: f32 = (l_x - l_ix);
  var l_fy: f32 = (l_y - l_iy);
  var l_sx: f32 = ((l_fx * l_fx) * (3.0 - (2.0 * l_fx)));
  var l_sy: f32 = ((l_fy * l_fy) * (3.0 - (2.0 * l_fy)));
  var l_a: f32 = verse_style_hash(l_ix, l_iy);
  var l_b: f32 = verse_style_hash((l_ix + 1.0), l_iy);
  var l_c: f32 = verse_style_hash(l_ix, (l_iy + 1.0));
  var l_d: f32 = verse_style_hash((l_ix + 1.0), (l_iy + 1.0));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn verse_style_cloud_field(l_x: f32, l_z: f32, l_s1: f32, l_s2: f32, l_octaves: f32) -> f32 {
  var coax_t28: f32;
  var coax_t29: f32;
  var l_n: f32 = verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 1.5))
  {
    return l_n;
  }
  coax_t28 = (l_n * 0.649999976);
  coax_t29 = (verse_style_noise(((l_x * l_s2) + 3.0999999), ((l_z * l_s2) + 7.69999981)) * 0.349999994);
  return (coax_t28 + coax_t29);
}
fn verse_style_cloud_cover(l_n: f32, l_cover: f32, l_cut_lo: f32, l_cut_hi: f32) -> f32 {
  var coax_t30: f32;
  var coax_t31: f32;
  coax_t30 = verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0.5), ((l_cut_hi - l_cut_lo) * 0.5), (l_n + ((l_cover - 0.349999994) * 0.400000006)));
  coax_t31 = verse_style_soft_step(0.0250000004, 0.0250000004, l_cover);
  return (coax_t30 * coax_t31);
}
fn verse_style_cloud_lit(l_n: f32, l_toward: f32) -> f32 {
  return coax_p_st_sat((((l_n - l_toward) * 5.0) + 0.550000012));
}
fn verse_style_cloud_fade(l_up: f32) -> f32 {
  return (verse_style_soft_step(0.0850000009, 0.075000003, l_up) * 0.920000017);
}
fn verse_style_tone(l_c: f32, l_exposure: f32) -> f32 {
  return (1.0 - exp(((-coax_p_st_max(l_c, 0.0)) * l_exposure)));
}
fn verse_style_saturation(l_c: f32, l_lum: f32, l_amount: f32) -> f32 {
  return coax_p_st_max(0.0, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn verse_style_lift(l_c: f32, l_lift: f32) -> f32 {
  return (l_lift + ((1.0 - l_lift) * l_c));
}
fn verse_style_luma(l_r: f32, l_g: f32, l_b: f32) -> f32 {
  return (((0.212599993 * l_r) + (0.715200007 * l_g)) + (0.0722000003 * l_b));
}
fn verse_style_bloom_share(l_lum: f32, l_threshold: f32, l_knee: f32) -> f32 {
  var coax_t40: f32;
  var coax_t41: f32;
  if ((l_lum <= 0.0))
  {
    return 0.0;
  }
  var l_k: f32 = coax_p_st_max(l_knee, 9.99999975e-06);
  coax_t40 = coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0);
  coax_t41 = (2.0 * l_k);
  var l_q: f32 = coax_p_st_min(coax_t40, coax_t41);
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (4.0 * l_k)), (l_lum - l_threshold)) / l_lum), 1.0);
}
fn verse_style_bloom_mix(l_c: f32, l_bloom: f32, l_amount: f32) -> f32 {
  return (l_c + (l_bloom * l_amount));
}
fn verse_style_shadow_reach(l_on_phone: f32) -> f32 {
  if ((l_on_phone > 0.5))
  {
    return 40.0;
  }
  return 60.0;
}
fn verse_style_shadow_snap(l_x: f32, l_reach: f32, l_size: f32) -> f32 {
  var coax_t18: f32;
  var coax_t19: f32;
  coax_t18 = (2.0 * l_reach);
  coax_t19 = coax_p_st_max(l_size, 1.0);
  var l_texel: f32 = (coax_t18 / coax_t19);
  return (floor(((l_x / l_texel) + 0.5)) * l_texel);
}
fn verse_style_shadow_across(l_x: f32, l_reach: f32) -> f32 {
  return (l_x / coax_p_st_max(l_reach, 0.00100000005));
}
fn verse_style_shadow_depth(l_along: f32, l_reach: f32) -> f32 {
  return (0.5 - (l_along / (4.0 * coax_p_st_max(l_reach, 0.00100000005))));
}
fn verse_style_shadow_taps(l_on_phone: f32) -> f32 {
  if ((l_on_phone > 0.5))
  {
    return 4.0;
  }
  return 9.0;
}
fn verse_style_shadow_radius(l_on_phone: f32) -> f32 {
  return (2.5 + (0.0 * l_on_phone));
}
fn verse_style_shadow_tap(l_k: f32, l_on_phone: f32, l_which: f32) -> f32 {
  var l_x: f32 = 0.0;
  var l_y: f32 = 0.0;
  if ((l_on_phone > 0.5))
  {
    if ((l_k < 0.5))
    {
      l_x = (-0.600000024);
      l_y = (-0.25);
    }
    else
    if ((l_k < 1.5))
    {
      l_x = 0.25;
      l_y = (-0.600000024);
    }
    else
    if ((l_k < 2.5))
    {
      l_x = 0.600000024;
      l_y = 0.25;
    }
    else
    {
      l_x = (-0.25);
      l_y = 0.600000024;
    }
  }
  else
  {
    if ((l_k < 0.5))
    {
      l_x = 0.0;
      l_y = 0.0;
    }
    else
    if ((l_k < 1.5))
    {
      l_x = (-0.326000005);
      l_y = (-0.405999988);
    }
    else
    if ((l_k < 2.5))
    {
      l_x = (-0.839999974);
      l_y = (-0.074000001);
    }
    else
    if ((l_k < 3.5))
    {
      l_x = (-0.69599998);
      l_y = 0.456999987;
    }
    else
    if ((l_k < 4.5))
    {
      l_x = (-0.202999994);
      l_y = 0.620999992;
    }
    else
    if ((l_k < 5.5))
    {
      l_x = 0.962000012;
      l_y = (-0.194999993);
    }
    else
    if ((l_k < 6.5))
    {
      l_x = 0.47299999;
      l_y = (-0.479999989);
    }
    else
    if ((l_k < 7.5))
    {
      l_x = 0.518999994;
      l_y = 0.76700002;
    }
    else
    {
      l_x = 0.185000002;
      l_y = (-0.893000007);
    }
  }
  if ((l_which > 0.5))
  {
    return l_y;
  }
  return l_x;
}
fn verse_style_contact(l_cast_lit: f32, l_blob: f32, l_strength: f32) -> f32 {
  return coax_p_st_min(l_cast_lit, (1.0 - (l_strength * coax_p_st_sat(l_blob))));
}
fn verse_style_shaft_source(l_cos_t: f32) -> f32 {
  var coax_t42: f32;
  var coax_t43: f32;
  coax_t42 = (-24.0);
  coax_t43 = coax_p_st_max((1.0 - l_cos_t), 0.0);
  return exp((coax_t42 * coax_t43));
}
fn verse_style_shaft_fade(l_x: f32, l_y: f32, l_margin: f32) -> f32 {
  var coax_t44: f32;
  var coax_t45: f32;
  coax_t44 = coax_p_st_max((-l_x), (l_x - 1.0));
  coax_t45 = coax_p_st_max((-l_y), (l_y - 1.0));
  var l_out: f32 = coax_p_st_max(coax_p_st_max(coax_t44, coax_t45), 0.0);
  return (1.0 - verse_style_soft_step((l_margin * 0.5), (l_margin * 0.5), l_out));
}
fn verse_style_shaft_tap(l_i: f32, l_decay: f32) -> f32 {
  return pow(coax_p_st_max(l_decay, 9.99999997e-07), l_i);
}
fn verse_style_shaft_total(l_n: f32, l_decay: f32) -> f32 {
  var coax_t46: f32;
  var coax_t47: f32;
  if ((abs((1.0 - l_decay)) < 9.99999997e-07))
  {
    return l_n;
  }
  coax_t46 = (1.0 - pow(coax_p_st_max(l_decay, 9.99999997e-07), l_n));
  coax_t47 = (1.0 - l_decay);
  return (coax_t46 / coax_t47);
}
fn verse_style_shaft(l_sum: f32, l_total: f32, l_weight: f32, l_fade: f32) -> f32 {
  var coax_t48: f32;
  var coax_t49: f32;
  coax_t48 = ((l_sum / coax_p_st_max(l_total, 9.99999997e-07)) * l_weight);
  coax_t49 = coax_p_st_sat(l_fade);
  return (coax_t48 * coax_t49);
}
fn verse_style_gust(l_x: f32, l_z: f32, l_t: f32, l_wx: f32, l_wz: f32, l_speed: f32, l_scale: f32) -> f32 {
  return ((2.0 * verse_style_noise(((l_x - ((l_wx * l_speed) * l_t)) * l_scale), ((l_z - ((l_wz * l_speed) * l_t)) * l_scale))) - 1.0);
}
fn verse_style_wind(l_along: f32, l_gust: f32, l_t: f32, l_amp: f32, l_wavelength: f32, l_speed: f32) -> f32 {
  var coax_t32: f32;
  var coax_t33: f32;
  var l_k: f32 = (6.28318548 / coax_p_st_max(l_wavelength, 0.00100000005));
  coax_t32 = (0.600000024 * sin(((l_k * l_along) - ((l_k * l_speed) * l_t))));
  coax_t33 = (0.400000006 * l_gust);
  return (l_amp * (coax_t32 + coax_t33));
}
fn verse_style_bend(l_theta: f32, l_height: f32, l_share: f32) -> f32 {
  return (((sin(l_theta) * l_height) * l_share) * l_share);
}
fn verse_style_grass_tilt(l_theta: f32, l_amp: f32, l_tilt_most: f32) -> f32 {
  var coax_t34: f32;
  var coax_t35: f32;
  coax_t34 = (-1.0);
  coax_t35 = coax_p_st_min(1.0, (l_theta / coax_p_st_max(l_amp, 9.99999975e-05)));
  return (l_tilt_most * coax_p_st_max(coax_t34, coax_t35));
}
fn verse_style_blade_height(l_h: f32, l_low: f32, l_high: f32) -> f32 {
  var coax_t36: f32;
  var coax_t37: f32;
  coax_t36 = (l_high - l_low);
  coax_t37 = coax_p_st_sat(l_h);
  return (l_low + (coax_t36 * coax_t37));
}
fn verse_style_grass_fade(l_d: f32, l_radius: f32, l_fade: f32) -> f32 {
  return (1.0 - verse_style_soft_step((l_radius - (l_fade * 0.5)), (l_fade * 0.5), l_d));
}
fn verse_style_grass_blades(l_radius: f32, l_density: f32) -> f32 {
  return (((2.0 * l_radius) * l_radius) * l_density);
}
fn verse_style_grass_clear(l_past: f32) -> f32 {
  return verse_style_soft_step(0.125, 0.125, l_past);
}
fn verse_style_segment(l_x: f32, l_z: f32, l_ax: f32, l_az: f32, l_bx: f32, l_bz: f32) -> f32 {
  var coax_t38: f32;
  var coax_t39: f32;
  var l_ux: f32 = (l_bx - l_ax);
  var l_uz: f32 = (l_bz - l_az);
  var l_px: f32 = (l_x - l_ax);
  var l_pz: f32 = (l_z - l_az);
  coax_t38 = ((l_px * l_ux) + (l_pz * l_uz));
  coax_t39 = coax_p_st_max(((l_ux * l_ux) + (l_uz * l_uz)), 9.99999997e-07);
  var l_h: f32 = coax_p_st_sat((coax_t38 / coax_t39));
  var l_dx: f32 = (l_px - (l_ux * l_h));
  var l_dz: f32 = (l_pz - (l_uz * l_h));
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn verse_style_part(l_d: f32, l_radius: f32) -> f32 {
  return coax_p_st_sat((1.0 - (l_d / coax_p_st_max(l_radius, 0.00100000005))));
}
fn verse_style_tip(l_c: f32, l_ch: f32, l_lift: f32) -> f32 {
  if ((l_ch < 0.5))
  {
    return (l_c * (1.0 + (l_lift * 1.29999995)));
  }
  if ((l_ch < 1.5))
  {
    return (l_c * (1.0 + l_lift));
  }
  return l_c;
}
fn verse_style_jitter(l_c: f32, l_ch: f32, l_h: f32, l_jitter: f32) -> f32 {
  var l_j: f32 = (((l_h * 2.0) - 1.0) * l_jitter);
  if ((l_ch < 0.5))
  {
    return (l_c * (1.0 + l_j));
  }
  if ((l_ch < 1.5))
  {
    return l_c;
  }
  return (l_c * (1.0 - l_j));
}
// ---- end generated
