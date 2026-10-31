// Belt 2200's two passes (belt.coax decides every number): the sky behind
// space (verse/stars.coax: the starfield, the Sun's disc and corona; the
// block below is generated from it), the Belt's dust, the worlds as shaded
// discs (bands, clouds, caps, Saturn's rings, air at the limb) and the
// HUD's glass; then the strokes: glowing line art as the homepage draws it
// (newland.wgsl).

struct U {
  size: vec2f, time: f32, fade: f32,
  sun: vec4f,                 // x, y, radius (device pixels), dpr
  misc: vec4f,                // the stars' drift x, y, the panel count, the disc count
  panels: array<vec4f, 16>,   // per panel: x0, y0, x1, y1 (device pixels), then cut, alpha, warm, 0
  belt: vec4f,                // the Sun x, y (device pixels), an AU in pixels (0: no dust), the dust's turn
  discs: array<vec4f, 24>,    // per world: x, y, radius (device pixels), kind; then the sunward x, y, seed, spin
}
@group(0) @binding(0) var<uniform> u: U;

// ---- generated: verse/stars.coax (verse_starfield, verse_sun_disc, verse_sun_corona) by coax/web/wgsl.py; do not edit
fn coax_p_stars_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_stars_fract(l_x: f32) -> f32 {
  return (l_x - floor(l_x));
}
fn coax_p_stars_hash(l_x: f32, l_y: f32) -> f32 {
  return coax_p_stars_fract((sin(((l_x * 127.099998) + (l_y * 311.700012))) * 43758.5469));
}
fn verse_star_layer(l_x: f32, l_y: f32, l_cell: f32, l_density: f32, l_size: f32, l_t: f32, l_seed: f32) -> f32 {
  var l_cx: f32 = floor((l_x / l_cell));
  var l_cy: f32 = floor((l_y / l_cell));
  var l_pick: f32 = coax_p_stars_hash((l_cx + l_seed), (l_cy - (l_seed * 0.370000005)));
  if ((l_pick < (1.0 - l_density)))
  {
    return 0.0;
  }
  var l_ox: f32 = ((0.200000003 + (0.600000024 * coax_p_stars_hash(((l_cx * 1.70000005) + l_seed), (l_cy + 3.0999999)))) * l_cell);
  var l_oy: f32 = ((0.200000003 + (0.600000024 * coax_p_stars_hash((l_cx - 2.29999995), ((l_cy * 1.29999995) + l_seed)))) * l_cell);
  var l_dx: f32 = (l_x - ((l_cx * l_cell) + l_ox));
  var l_dy: f32 = (l_y - ((l_cy * l_cell) + l_oy));
  var l_d2: f32 = (((l_dx * l_dx) + (l_dy * l_dy)) / (l_size * l_size));
  var l_bright: f32 = (0.349999994 + (0.649999976 * coax_p_stars_hash((l_cx + 11.0), (l_cy + (l_seed * 2.0)))));
  var l_twinkle: f32 = (0.800000012 + (0.200000003 * sin(((l_t * (0.699999988 + (2.0 * l_pick))) + (l_pick * 40.0)))));
  return ((exp((-l_d2)) * l_bright) * l_twinkle);
}
fn verse_starfield(l_x: f32, l_y: f32, l_drift_x: f32, l_drift_y: f32, l_t: f32, l_dpr: f32) -> f32 {
  var l_k: f32 = coax_p_stars_max(l_dpr, 0.5);
  var l_far: f32 = (verse_star_layer((l_x + (l_drift_x * 0.0199999996)), (l_y + (l_drift_y * 0.0199999996)), (9.0 * l_k), 0.119999997, (0.550000012 * l_k), l_t, 1.0) * 0.550000012);
  var l_mid: f32 = (verse_star_layer((l_x + (l_drift_x * 0.0500000007)), (l_y + (l_drift_y * 0.0500000007)), (23.0 * l_k), 0.100000001, (0.800000012 * l_k), l_t, 7.0) * 0.850000024);
  var l_near: f32 = (verse_star_layer((l_x + (l_drift_x * 0.100000001)), (l_y + (l_drift_y * 0.100000001)), (61.0 * l_k), 0.0799999982, (1.10000002 * l_k), l_t, 13.0) * 1.20000005);
  return ((l_far + l_mid) + l_near);
}
fn verse_sun_disc(l_d: f32, l_r: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  var l_rr: f32 = coax_p_stars_max(l_r, 0.75);
  coax_t0 = (l_d - l_rr);
  coax_t1 = coax_p_stars_max((l_rr * 0.0799999982), 0.75);
  var l_x: f32 = (coax_t0 / coax_t1);
  if ((l_x <= (-1.0)))
  {
    return 1.0;
  }
  if ((l_x >= 1.0))
  {
    return 0.0;
  }
  return (0.5 - (0.5 * l_x));
}
fn coax_p_stars_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn verse_sun_corona(l_d: f32, l_r: f32, l_dpr: f32) -> f32 {
  var l_k: f32 = coax_p_stars_max(l_dpr, 0.5);
  var l_rr: f32 = coax_p_stars_max(l_r, 0.75);
  var l_out: f32 = coax_p_stars_max((l_d - l_rr), 0.0);
  var l_corona: f32 = (exp(((-l_out) / ((l_rr * 0.600000024) + (3.0 * l_k)))) * 0.899999976);
  var l_halo: f32 = (exp(((-l_out) / ((l_rr * 3.0) + (40.0 * l_k)))) * 0.219999999);
  var l_glare: f32 = (0.0599999987 / (1.0 + (l_out / (120.0 * l_k))));
  return coax_p_stars_min(((l_corona + l_halo) + l_glare), 1.5);
}
// ---- end generated

const GREY_LIGHT = vec3f(0.78);
const ACCENT = vec3f(0.216, 0.851, 0.851);
const SUN_CORE = vec3f(1.0, 0.97, 0.9);
const SUN_GLOW = vec3f(1.0, 0.9, 0.66);
const DUST = vec3f(0.6, 0.6, 0.62);

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}

@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let stars = verse_starfield(pos.x, pos.y, u.misc.x, u.misc.y, u.time, u.sun.w);
  var c = vec3f(0.006, 0.008, 0.014) + vec3f(0.85, 0.9, 1.0) * stars * 0.8;
  let d = distance(pos.xy, u.sun.xy);
  let disc = verse_sun_disc(d, u.sun.z);
  let corona = verse_sun_corona(d, u.sun.z, u.sun.w);
  c = mix(c + SUN_GLOW * corona * 0.9, SUN_CORE, disc);
  c = c + DUST * belt_dust(pos.xy);
  let worlds = i32(u.misc.w);
  for (var k = 0; k < 12; k++) {
    if (k >= worlds) { break; }
    c = world(pos.xy, c, u.discs[k * 2], u.discs[k * 2 + 1]);
  }
  c = c * u.fade;
  let n = i32(u.misc.z);
  for (var k = 0; k < 8; k++) {
    if (k >= n) { break; }
    c = panel(pos.xy, c, u.panels[k * 2], u.panels[k * 2 + 1]);
  }
  return vec4f(c, 1.0);
}

// ---- noise for surfaces and dust
fn hash2(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn vnoise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), s.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn fbm(p: vec2f) -> f32 {
  var v = 0.0; var a = 0.5; var q = p;
  for (var i = 0; i < 3; i++) { v += a * vnoise(q); q = q * 2.07 + vec2f(1.7, 9.2); a *= 0.5; }
  return v / 0.875;
}

// The Belt as dust: a band from 2.1 to 3.3 AU about the Sun with the Kirkwood gaps (Jupiter's
// resonances at 2.50, 2.82 and 2.95 AU), clumped by noise that turns with the rocks. It fades
// out when an AU is wider than the screen many times over (close in, there is nothing to see).
fn belt_dust(p: vec2f) -> f32 {
  let au = u.belt.z;
  if (au <= 0.0) { return 0.0; }
  let rel = (p - u.belt.xy) / au;
  let r = length(rel);
  if (r < 2.0 || r > 3.5) { return 0.0; }
  let prof = smoothstep(2.06, 2.4, r) * (1.0 - smoothstep(2.95, 3.4, r));
  let g1 = (r - 2.5) / 0.022; let g2 = (r - 2.82) / 0.018; let g3 = (r - 2.95) / 0.016;
  let gaps = 1.0 - 0.6 * exp(-g1 * g1) - 0.5 * exp(-g2 * g2) - 0.4 * exp(-g3 * g3);
  let c = cos(u.belt.w); let s = sin(u.belt.w);
  let w = vec2f(c * rel.x - s * rel.y, s * rel.x + c * rel.y);
  let n = fbm(w * 7.0) * 0.65 + fbm(w * 31.0) * 0.35;
  let fade = 1.0 - smoothstep(20000.0, 60000.0, au);
  return prof * gaps * (0.12 + 1.1 * n * n * n) * 0.16 * fade;
}

// ---- the worlds
// Kinds: 1 Mercury, 2 Venus, 3 the Earth, 4 the Moon, 5 Mars, 6 Jupiter, 7 Saturn, 8 Ceres,
// 9 Vesta, 10 an ice rock. b is the way to the Sun on screen, a seed and a spin (radians).
fn surface(kind: i32, n: vec3f, seed: f32, spin: f32) -> vec3f {
  let lon = atan2(n.x, n.z) + spin;
  let uv = vec2f(lon, n.y * 1.6) + vec2f(seed * 3.1, seed);
  if (kind == 1) { return vec3f(0.56, 0.52, 0.49) * (0.7 + 0.55 * fbm(uv * 4.0)); }
  if (kind == 2) { return mix(vec3f(0.93, 0.87, 0.7), vec3f(0.98, 0.95, 0.86), fbm(uv * vec2f(1.5, 4.0))); }
  if (kind == 3) {
    let land = smoothstep(0.52, 0.58, fbm(uv * 2.2));
    var col = mix(vec3f(0.05, 0.16, 0.42), mix(vec3f(0.2, 0.38, 0.17), vec3f(0.55, 0.48, 0.34), fbm(uv * 5.0)), land);
    let cap = smoothstep(0.82, 0.9, abs(n.y));
    let cloud = smoothstep(0.5, 0.72, fbm(uv * vec2f(3.0, 6.0) + vec2f(u.time * 0.01, 0.0)));
    return mix(col, vec3f(0.95), max(cap, cloud * 0.85));
  }
  if (kind == 4) { return vec3f(0.62, 0.62, 0.6) * (0.65 + 0.5 * smoothstep(0.35, 0.6, fbm(uv * 3.0))); }
  if (kind == 5) {
    let dark = smoothstep(0.45, 0.65, fbm(uv * 3.0));
    let col = mix(vec3f(0.72, 0.34, 0.25), vec3f(0.42, 0.22, 0.17), dark);
    return mix(col, vec3f(0.93, 0.92, 0.9), smoothstep(0.86, 0.93, abs(n.y)));
  }
  if (kind == 6 || kind == 7) {
    let wob = fbm(vec2f(lon * 2.0, n.y * 14.0)) * 2.2;
    let band = 0.5 + 0.5 * sin(n.y * select(17.0, 12.0, kind == 7) + wob);
    if (kind == 6) { return mix(vec3f(0.62, 0.48, 0.38), vec3f(0.9, 0.85, 0.75), band); }
    return mix(vec3f(0.76, 0.68, 0.5), vec3f(0.92, 0.86, 0.68), band);
  }
  if (kind == 8) { return vec3f(0.4, 0.4, 0.39) * (0.75 + 0.45 * fbm(uv * 5.0)) + vec3f(0.5) * smoothstep(0.9, 0.95, fbm(uv * 9.0)); }
  if (kind == 9) { return vec3f(0.55, 0.53, 0.5) * (0.7 + 0.5 * fbm(uv * 4.0)); }
  let crater = smoothstep(0.55, 0.7, fbm(uv * 5.0));
  return mix(vec3f(0.66, 0.72, 0.78), vec3f(0.36, 0.38, 0.42), crater) * (0.85 + 0.3 * fbm(uv * 13.0));
}
fn air(kind: i32) -> vec4f {   // the colour at the limb and how much
  if (kind == 2) { return vec4f(1.0, 0.95, 0.82, 0.55); }
  if (kind == 3) { return vec4f(0.35, 0.62, 1.0, 0.85); }
  if (kind == 5) { return vec4f(0.95, 0.62, 0.55, 0.3); }
  if (kind == 6) { return vec4f(0.9, 0.86, 0.78, 0.2); }
  if (kind == 7) { return vec4f(0.95, 0.9, 0.75, 0.18); }
  return vec4f(0.0);
}
// Saturn's rings, seen tilted: an ellipse from 1.24 to 2.27 radii with the Cassini Division,
// lit except in the planet's shadow.
fn rings(q: vec2f, l: vec2f) -> vec4f {
  let ca = 0.94; let sa = -0.342;
  let r2 = vec2f(ca * q.x - sa * q.y, sa * q.x + ca * q.y);
  let e = vec2f(r2.x, r2.y / 0.42);
  let rr = length(e);
  if (rr < 1.24 || rr > 2.27) { return vec4f(0.0); }
  var dens = 0.55 + 0.25 * sin(rr * 46.0) + 0.2 * sin(rr * 131.0);
  dens *= 1.0 - 0.92 * (smoothstep(1.9, 1.93, rr) - smoothstep(1.98, 2.01, rr));
  dens *= smoothstep(1.24, 1.32, rr) * (1.0 - smoothstep(2.18, 2.27, rr));
  dens *= select(0.55, 1.0, rr > 1.55);
  // the planet's shadow, worked out in the ring's own plane (the squash undone), soft at its edge
  let lr = vec2f(ca * l.x - sa * l.y, sa * l.x + ca * l.y);
  let lp = normalize(vec2f(lr.x, lr.y / 0.42));
  let along = dot(e, lp);
  let side = length(e - along * lp);
  let shade = 1.0 - 0.88 * (1.0 - smoothstep(0.95, 1.05, side)) * smoothstep(0.0, -0.1, along);
  return vec4f(vec3f(0.86, 0.8, 0.66) * shade, clamp(dens, 0.0, 1.0));
}
fn world(p: vec2f, under: vec3f, a: vec4f, b: vec4f) -> vec3f {
  let rad = max(a.z, 0.5);
  let rel = p - a.xy;
  let d = length(rel);
  if (d > rad * 2.4 + 6.0) { return under; }
  let kind = i32(a.w + 0.5);
  let q = rel / rad;
  let l2 = normalize(b.xy + vec2f(0.0001, 0.0));
  let l = normalize(vec3f(l2, 0.35));
  let aa = 1.2 / rad;
  var c = under;
  // the rings' far half, under the planet
  var ring = vec4f(0.0);
  var ring_front = false;
  if (kind == 7) {
    ring = rings(q, l2);
    let ca = 0.94; let sa = -0.342;
    ring_front = (sa * q.x + ca * q.y) > 0.0;
    if (!ring_front) { c = mix(c, ring.rgb, ring.a * 0.85); }
  }
  // the outline: rocks are lumpy
  var edge = 1.0;
  if (kind >= 8) {
    let dir = q / max(length(q), 0.0001);
    let lump = select(0.06, 0.3, kind == 10);
    edge = 1.0 + lump * (fbm(dir * 1.7 + vec2f(b.z * 7.0, b.z * 3.0)) - 0.5) * 2.0;
  }
  let qe = d / rad / edge;
  let atm = air(kind);
  if (atm.w > 0.0 && qe > 1.0) {
    let lit = 0.25 + 0.75 * max(dot(q / max(length(q), 0.0001), l2), 0.0);
    let glow = exp(-(qe - 1.0) * rad / (0.06 * rad + 1.5)) * atm.w * lit;
    c = c + atm.rgb * glow * 0.8;
  }
  let cover = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, qe);
  if (cover > 0.0) {
    let qq = q / edge;
    let z = sqrt(max(1.0 - dot(qq, qq), 0.0));
    let n = vec3f(qq.x, -qq.y, z);
    let ln = vec3f(l.x, -l.y, l.z);
    let lam = max(dot(n, ln), 0.0);
    var col = surface(kind, n, b.z, b.w);
    var light = 0.035 + 1.15 * pow(lam, 0.85);
    if (kind == 6 || kind == 7) { light *= 0.65 + 0.35 * z; }
    var lit_c = col * light;
    if (atm.w > 0.0) { lit_c += atm.rgb * pow(1.0 - z, 3.0) * atm.w * 0.6 * smoothstep(-0.2, 0.4, dot(n, ln)); }
    c = mix(c, lit_c, cover);
  }
  if (kind == 7 && ring_front) { c = mix(c, ring.rgb, ring.a * 0.85); }
  return c;
}

// A HUD panel's glass: dark, its plain corners (top right, bottom left) clipped at 45°, faint
// scanlines and a glow inside its edge (newland.wgsl's), blue when it is warm (selected).
fn panel(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  let alpha = s.y * u.fade;
  if (alpha <= 0.0) { return under; }
  let x = p.x - r.x; let y = p.y - r.y; let w = r.z - r.x; let h = r.w - r.y;
  let side = min(min(x, w - x), min(y, h - y));
  let corner = min((w - x) + y, x + (h - y)) - s.x;
  let inside = min(side, corner * 0.7071);
  if (inside <= 0.0) { return under; }
  let warm = select(0.0, 1.0, s.z > 0.5);
  let tint = mix(GREY_LIGHT, ACCENT, warm);
  let shade = mix(vec3f(0.022), vec3f(0.013), clamp(y / max(h, 1.0), 0.0, 1.0));
  let line = select(0.0, 0.018, fract(p.y / 3.0) < 0.34);
  let edge = exp(-inside / 22.0) * (0.07 + 0.1 * warm) + 0.03 * warm;
  let glass = mix(under * 0.25, shade, 0.85) + vec3f(line) + tint * edge;
  return mix(under, glass, clamp(alpha, 0.0, 1.0) * smoothstep(0.0, 1.5, inside));
}

// ---- the strokes, one quad each
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f) -> V {
  var o: V;
  let ndc = vec2f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0);
  o.pos = vec4f(ndc, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
