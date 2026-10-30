// Belt 2200's two passes (belt.coax decides every number): the sky behind
// space (verse/stars.coax: the starfield, the Sun's disc and corona; the
// block below is generated from it) with the HUD's glass, then the
// strokes: glowing line art as the homepage draws it (newland.wgsl).

struct U {
  size: vec2f, time: f32, fade: f32,
  sun: vec4f,                 // x, y, radius (device pixels), dpr
  misc: vec4f,                // the stars' drift x, y, the panel count, 0
  panels: array<vec4f, 16>,   // per panel: x0, y0, x1, y1 (device pixels), then cut, alpha, warm, 0
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
const SUN_GLOW = vec3f(1.0, 0.62, 0.28);

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
  c = c * u.fade;
  let n = i32(u.misc.z);
  for (var k = 0; k < 8; k++) {
    if (k >= n) { break; }
    c = panel(pos.xy, c, u.panels[k * 2], u.panels[k * 2 + 1]);
  }
  return vec4f(c, 1.0);
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
