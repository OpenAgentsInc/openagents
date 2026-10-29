// The fixture realm's three passes: the night behind (stars, a glow where
// the floor lies, and the plates fixture.coax lays out), the pack's meshes
// (scene_vs, scene_fs: lit by a key light and the lamp, the plates kept over
// them) and the strokes over all of it (the floor's rings, the lamp, the
// words). Every place and size comes from fixture.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, painted: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,  // the view: clip = rows · world
  lamp: vec4f,                                 // the lamp: where (metres), strength
  panel: array<vec4f, 32>,  // per plate (fixture.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
  style: array<vec4f, 32>,  // the style's preset at the clock (verse/style-preset.coax SU_*: row = SU / 4), when painted
}
@group(0) @binding(0) var<uniform> u: U;
// A plate (verse.wgsl's): a dark wood fill with one thin bronze line round
// it and small rounded corners. Look 1 lights the line; 2 (under the mouse
// or pressed) adds a step lighter fill and the bright line.
fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 4.0 * s.x;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.z >= 2.0;
  let lit = over || (s.z >= 1.0 && s.z < 2.0);
  let fill = select(vec3f(0.165, 0.114, 0.078), vec3f(0.235, 0.168, 0.115), over);
  let line = select(vec3f(0.69, 0.541, 0.29), vec3f(0.86, 0.70, 0.42), lit);
  let w = 1.25 * s.x;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.y, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1));
}
fn panels(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
// Stars: one at most per cell of 3 CSS pixels' grid, a few of them brighter, twinkling slowly.
fn stars(p: vec2f) -> f32 {
  let cell = 3.0 * max(u.k, 1.0) * 6.0;
  let g = floor(p / cell);
  let h = hash(g);
  if (h < 0.93) { return 0.0; }
  let at = (g + vec2f(hash(g + 1.7), hash(g + 9.1)) * 0.8 + 0.1) * cell;
  let d = length(p - at) / max(u.k, 1.0);
  let size = select(0.55, 0.95, h > 0.992);
  let twinkle = select(0.75 + 0.25 * sin(u.time * (0.6 + h * 2.0) + h * 40.0), 1.0, u.still > 0.5);
  let peak = select(0.32, 0.75, h > 0.992) * twinkle;
  return peak * (1.0 - smoothstep(size * 0.4, size + 0.6, d));
}
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  if (u.painted > 0.5) { return vec4f(panels(p, paint(p)), 1.0); }
  let c = vec2f(0.5, 0.58) * u.size;
  let r = length((p - c) / max(u.size.y, 1.0));
  var sky = vec3f(0.012, 0.013, 0.02) + vec3f(0.035, 0.036, 0.045) * exp(-r * r * 6.0);
  sky += vec3f(stars(p)) * u.fade;
  return vec4f(panels(p, sky), 1.0);
}
// ---- the style fixture (fixture.coax, ?style=painterly): a fixed scene traced
// here and shaded with the style's own functions (verse/style.coax, the
// generated block below) and numbers (u.style). Everything in metres; the
// ground is y = 0 and the eye looks down -z.
const EYE = vec3f(0.0, 1.75, 8.0);
const SPHERE = vec4f(-1.7, 0.85, 0.0, 0.85);
const CUBE = vec4f(1.5, 0.6, 0.3, 0.6);              // centre, half size; turned 0.5 rad about y
const TRUNK = vec4f(3.9, 0.0, -2.6, 0.18);           // base, radius; 1.9 m tall
fn canopy(i: i32) -> vec4f {
  var c = array<vec4f, 3>(vec4f(3.9, 2.7, -2.6, 1.15), vec4f(3.25, 2.3, -2.3, 0.8), vec4f(4.5, 2.35, -2.9, 0.85));
  return c[i];
}
const CANOPY_C = vec3f(3.9, 2.5, -2.6);
const BODY = vec4f(-0.3, 0.0, -1.8, 0.24);           // a figure: capsule from y 0.24 to 1.30, head above
const HEAD = vec4f(-0.3, 1.62, -1.8, 0.17);
const POND = vec4f(0.7, 0.0, 2.7, 1.5);              // centre x, -, z, radius; 1.9 m deep in the middle
const GRASS = vec4f(-3.4, 0.0, 2.2, 1.6);
const RIDGE_Z = vec3f(60.0, 140.0, 260.0);
struct Hit { t: f32, n: vec3f, albedo: vec3f, kind: f32, id: f32 }   // kind 0 none, 1 matte, 2 water, 3 character; id: which thing (outlines)
fn st(i: u32) -> vec4f { return u.style[i]; }
fn sphere_t(o: vec3f, d: vec3f, s: vec4f) -> f32 {
  let oc = o - s.xyz; let b = dot(oc, d); let c = dot(oc, oc) - s.w * s.w; let h = b * b - c;
  if (h < 0.0) { return -1.0; }
  let t = -b - sqrt(h);
  return select(-1.0, t, t > 0.001);
}
fn turn(v: vec3f, a: f32) -> vec3f { let c = cos(a); let s = sin(a); return vec3f(c * v.x - s * v.z, v.y, s * v.x + c * v.z); }
fn cube_hit(o: vec3f, d: vec3f) -> vec4f {        // t and the normal
  let lo = turn(o - CUBE.xyz, -0.5); let ld = turn(d, -0.5);
  let inv = 1.0 / ld;
  let a = (-vec3f(CUBE.w) - lo) * inv; let b = (vec3f(CUBE.w) - lo) * inv;
  let t0 = min(a, b); let t1 = max(a, b);
  let tn = max(max(t0.x, t0.y), t0.z); let tf = min(min(t1.x, t1.y), t1.z);
  if (tn > tf || tn < 0.001) { return vec4f(-1.0); }
  var n = vec3f(0.0);
  if (tn == t0.x) { n = vec3f(-sign(ld.x), 0.0, 0.0); } else if (tn == t0.y) { n = vec3f(0.0, -sign(ld.y), 0.0); } else { n = vec3f(0.0, 0.0, -sign(ld.z)); }
  return vec4f(tn, turn(n, 0.5));
}
fn cylinder_t(o: vec3f, d: vec3f, c: vec4f, top: f32) -> f32 {   // a vertical cylinder from c.y to top
  let oc = o.xz - c.xz; let a = dot(d.xz, d.xz); let b = dot(oc, d.xz); let k = dot(oc, oc) - c.w * c.w;
  let h = b * b - a * k;
  if (h < 0.0 || a < 1e-6) { return -1.0; }
  let t = (-b - sqrt(h)) / a; let y = o.y + d.y * t;
  return select(-1.0, t, t > 0.001 && y >= c.y && y <= top);
}
fn capsule_t(o: vec3f, d: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {   // Inigo Quilez's
  let ba = b - a; let oa = o - a;
  let baba = dot(ba, ba); let bard = dot(ba, d); let baoa = dot(ba, oa); let rdoa = dot(d, oa); let oaoa = dot(oa, oa);
  let qa = baba - bard * bard; let qb = baba * rdoa - baoa * bard; let qc = baba * oaoa - baoa * baoa - r * r * baba;
  let h = qb * qb - qa * qc;
  if (h >= 0.0) {
    let t = (-qb - sqrt(h)) / qa; let y = baoa + t * bard;
    if (y > 0.0 && y < baba && t > 0.001) { return t; }
    let oc = select(o - b, oa, y <= 0.0);
    let bb = dot(d, oc); let cc = dot(oc, oc) - r * r; let hh = bb * bb - cc;
    if (hh > 0.0) { let t2 = -bb - sqrt(hh); if (t2 > 0.001) { return t2; } }
  }
  return -1.0;
}
fn capsule_n(p: vec3f, a: vec3f, b: vec3f) -> vec3f {
  let ba = b - a; let h = clamp(dot(p - a, ba) / dot(ba, ba), 0.0, 1.0);
  return normalize(p - (a + ba * h));
}
fn ridge_h(x: f32, i: i32) -> f32 {
  let f = f32(i);
  var bases = array<f32, 3>(9.0, 22.0, 44.0);
  let base = bases[i];
  let s = 12.0 * (1.0 + f * f);   // nearer ridges are narrower in metres, so all three read alike on screen
  return base * (0.72 + 0.18 * sin(x / s + f * 1.7) + 0.08 * sin(x / (0.37 * s) + 0.6 + f));
}
fn ridge_slope(x: f32, i: i32) -> f32 { return (ridge_h(x + 0.5, i) - ridge_h(x - 0.5, i)); }
fn ridge_albedo(i: i32) -> vec3f {
  var a = array<vec3f, 3>(vec3f(0.09, 0.20, 0.06), vec3f(0.08, 0.18, 0.08), vec3f(0.08, 0.16, 0.10));
  return a[i];
}
// The scene, nearest first. shadow: only whether anything is in the way, and how near the miss (soft edges).
fn trace(o: vec3f, d: vec3f) -> Hit {
  var h = Hit(1e9, vec3f(0.0, 1.0, 0.0), vec3f(0.0), 0.0, 0.0);
  var t = sphere_t(o, d, SPHERE);
  if (t > 0.0 && t < h.t) { h = Hit(t, normalize(o + d * t - SPHERE.xyz), vec3f(0.46, 0.13, 0.08), 1.0, 1.0); }
  let c = cube_hit(o, d);
  if (c.x > 0.0 && c.x < h.t) { h = Hit(c.x, c.yzw, vec3f(0.11, 0.19, 0.46), 1.0, 2.0); }
  t = cylinder_t(o, d, TRUNK, 1.9);
  if (t > 0.0 && t < h.t) { let p = o + d * t; h = Hit(t, normalize(vec3f(p.x - TRUNK.x, 0.0, p.z - TRUNK.z)), vec3f(0.17, 0.10, 0.05), 1.0, 3.0); }
  for (var i = 0; i < 3; i++) {
    t = sphere_t(o, d, canopy(i));
    if (t > 0.0 && t < h.t) {
      // a blob: the leaf mass shades as one soft shape round the canopy's centre (spec 7)
      let p = o + d * t;
      h = Hit(t, normalize(mix(normalize(p - canopy(i).xyz), normalize(p - CANOPY_C), 0.85)), vec3f(0.11, 0.27, 0.05), 1.0, 4.0);
    }
  }
  t = capsule_t(o, d, BODY.xyz + vec3f(0.0, 0.24, 0.0), BODY.xyz + vec3f(0.0, 1.30, 0.0), BODY.w);
  if (t > 0.0 && t < h.t) { h = Hit(t, capsule_n(o + d * t, BODY.xyz + vec3f(0.0, 0.24, 0.0), BODY.xyz + vec3f(0.0, 1.30, 0.0)), vec3f(0.36, 0.24, 0.11), 3.0, 5.0); }
  t = sphere_t(o, d, HEAD);
  if (t > 0.0 && t < h.t) { h = Hit(t, normalize(o + d * t - HEAD.xyz), vec3f(0.62, 0.38, 0.26), 3.0, 6.0); }
  // the ridges: painted flats standing across the view, each up to its silhouette
  for (var i = 0; i < 3; i++) {
    var zs = RIDGE_Z;
    let tz = (-zs[i] - o.z) / d.z;
    if (d.z < 0.0 && tz > 0.0 && tz < h.t) {
      let p = o + d * tz;
      if (p.y < ridge_h(p.x, i)) {
        h = Hit(tz, normalize(vec3f(-ridge_slope(p.x, i), 0.7 + 0.3 * clamp(p.y / ridge_h(p.x, i), 0.0, 1.0), 0.5)), ridge_albedo(i), 1.0, 7.0 + f32(i));
      }
    }
  }
  // the ground, with the pond and the grass in it
  if (d.y < 0.0) {
    let tg = -o.y / d.y;
    if (tg > 0.0 && tg < h.t) {
      let p = o + d * tg;
      let pond = length(p.xz - POND.xz) / POND.w;
      let grass = length(p.xz - GRASS.xz) / GRASS.w;
      if (pond < 1.0) { h = Hit(tg, vec3f(0.0, 1.0, 0.0), vec3f(1.9 * (1.0 - pond * pond)), 2.0, 10.0); }
      else if (grass < 1.0) { h = Hit(tg, vec3f(0.0, 1.0, 0.0), vec3f(0.15, 0.33, 0.05), 1.0, 10.0); }
      else { h = Hit(tg, vec3f(0.0, 1.0, 0.0), mix(vec3f(0.21, 0.29, 0.08), vec3f(0.27, 0.22, 0.10), 0.5 + 0.5 * sin(p.x * 0.21 + p.z * 0.13)), 1.0, 10.0); }
    }
  }
  return h;
}
// How much of the key reaches p: 1 lit, 0 in shadow, soft over how near the ray passes a sphere.
fn sphere_shade(o: vec3f, d: vec3f, s: vec4f) -> f32 {
  let oc = s.xyz - o; let b = dot(oc, d);
  if (b <= 0.0 || dot(oc, oc) < s.w * s.w) { return 1.0; }   // behind, or a proxy's own surface
  let miss = sqrt(max(dot(oc, oc) - b * b, 0.0)) - s.w;
  return clamp(miss / (0.08 * b + 0.02) + 0.5, 0.0, 1.0);
}
fn shadow(p: vec3f, l: vec3f) -> f32 {
  var s = sphere_shade(p, l, SPHERE);
  for (var i = 0; i < 3; i++) { s = min(s, sphere_shade(p, l, canopy(i))); }
  s = min(s, sphere_shade(p, l, HEAD));
  s = min(s, sphere_shade(p, l, vec4f(BODY.x, 0.55, BODY.z, BODY.w)));
  s = min(s, sphere_shade(p, l, vec4f(BODY.x, 1.0, BODY.z, BODY.w)));
  s = min(s, sphere_shade(p, l, vec4f(TRUNK.x, 0.6, TRUNK.z, TRUNK.w)));
  s = min(s, sphere_shade(p, l, vec4f(TRUNK.x, 1.4, TRUNK.z, TRUNK.w)));
  if (cube_hit(p, l).x > 0.0) { s = 0.0; }
  return s;
}
fn sky(d: vec3f, l: vec3f) -> vec3f {
  let up = d.y;
  var c = mix(st(13u).xyz, st(12u).xyz, verse_style_sky_t(up));
  c = mix(c, st(10u).xyz, verse_style_sky_band(up, st(12u).w));
  let ct = dot(d, l);
  let moon = st(1u).w > 0.5;
  let disc = smoothstep(cos(st(14u).x * 1.15), cos(st(14u).x * 0.85), ct);
  let halo = verse_style_halo(ct, st(14u).y) * 0.5 + verse_style_halo(ct, st(14u).z) * 0.25;
  let glow = select(st(11u).xyz, st(0u).xyz, moon);
  c += glow * halo * select(1.0, 0.5, moon);
  c = mix(c, select(vec3f(1.0, 0.97, 0.88) * 4.0, vec3f(0.85, 0.88, 1.0) * 1.6, moon), disc);
  return c;
}
fn shade(h: Hit, o: vec3f, d: vec3f) -> vec3f {
  let p = o + d * h.t;
  let l = st(1u).xyz;
  let n = h.n;
  let ndl = dot(n, l);
  let lit = shadow(p + n * 0.01, l);
  let character = h.kind > 2.5;
  let w = verse_style_wrap(ndl) * lit;
  let band = select(verse_style_bands(w, st(7u).x, st(7u).y, st(7u).z, st(7u).w), verse_style_bands2(w, st(8u).x, st(8u).y), character);
  let key = st(0u).xyz * st(0u).w;
  let amb = vec3f(verse_style_ambient(n.y, st(5u).x, st(6u).x, st(5u).w), verse_style_ambient(n.y, st(5u).y, st(6u).y, st(5u).w), verse_style_ambient(n.y, st(5u).z, st(6u).z, st(5u).w));
  let dark = st(4u).xyz * amb;
  let bright = key + amb;
  var a = h.albedo;
  var c = vec3f(0.0);
  if (h.kind > 1.5 && h.kind < 2.5) {
    // the pond: colour bands by depth (spec 12), the sky's colour in it by Fresnel
    let depth = h.albedo.x;
    let sh = st(27u); let md = st(28u); let dp = st(29u);
    let s1 = smoothstep(sh.w - dp.w, sh.w + dp.w, depth); let s2 = smoothstep(md.w - dp.w, md.w + dp.w, depth);
    a = mix(mix(sh.xyz, md.xyz, s1), dp.xyz, s2);
    c = vec3f(verse_style_shade(a.x, dark.x, bright.x, band), verse_style_shade(a.y, dark.y, bright.y, band), verse_style_shade(a.z, dark.z, bright.z, band));
    let r = reflect(d, n);
    let f = 0.02 + 0.98 * pow(1.0 - max(dot(-d, n), 0.0), 5.0);
    c = mix(c, sky(r, l), f);
  } else {
    c = vec3f(verse_style_shade(a.x, dark.x, bright.x, band), verse_style_shade(a.y, dark.y, bright.y, band), verse_style_shade(a.z, dark.z, bright.z, band));
    let rs = select(st(2u).w, st(3u).x, character);
    c += st(2u).xyz * verse_style_rim(dot(n, -d), ndl, st(3u).y, st(3u).z, rs);
  }
  // the haze, the same for everything (spec 4)
  let hz = st(9u);
  let f = verse_style_haze(h.t, p.y, 0.0, hz.x, hz.y, hz.z, hz.w);
  let hc = mix(st(10u).xyz, st(11u).xyz, verse_style_haze_glow(dot(d, l)));
  return vec3f(verse_style_fog(c.x, hc.x, f), verse_style_fog(c.y, hc.y, f), verse_style_fog(c.z, hc.z, f));
}
fn tone(c: vec3f) -> vec3f {
  if (st(18u).w < 0.5) { return c; }
  let e = st(18u).x;
  var t = vec3f(verse_style_tone(c.x, e), verse_style_tone(c.y, e), verse_style_tone(c.z, e));
  let y = verse_style_luma(t.x, t.y, t.z);
  t = vec3f(verse_style_saturation(t.x, y, st(18u).y), verse_style_saturation(t.y, y, st(18u).y), verse_style_saturation(t.z, y, st(18u).y));
  return vec3f(verse_style_lift(t.x, st(18u).z), verse_style_lift(t.y, st(18u).z), verse_style_lift(t.z, st(18u).z));
}
// The eye's ray through pixel p, from a fixed camera.
fn ray(p: vec2f) -> vec3f {
  let aspect = u.size.x / max(u.size.y, 1.0);
  let tv = max(0.36, 0.5 / aspect);        // a phone's tall canvas still holds the whole scene across
  let q = vec2f((p.x / u.size.x * 2.0 - 1.0) * tv * aspect, (1.0 - p.y / u.size.y * 2.0) * tv);
  let fwd = normalize(vec3f(0.0, -0.12, -1.0));
  let right = normalize(cross(fwd, vec3f(0.0, 1.0, 0.0)));
  let up = cross(right, fwd);
  return normalize(fwd + right * q.x + up * q.y);
}
fn paint(p: vec2f) -> vec3f {
  let d = ray(p);
  let h = trace(EYE, d);
  var c = vec3f(0.0);
  if (h.kind == 0.0) {
    c = sky(d, st(1u).xyz);
    c += vec3f(stars(p)) * st(14u).w * 0.6;
  } else {
    c = shade(h, EYE, d);
    // the outline in the style's form (verse/look.coax): where the thing changes a pixel over or
    // the surface turns sharply; painterly makes it a pale edge, faded with the haze
    let w = max(u.k, 1.0);
    let hx = trace(EYE, ray(p + vec2f(w, 0.0)));
    let hy = trace(EYE, ray(p + vec2f(0.0, w)));
    var edge = select(0.0, 1.0, hx.id != h.id || hy.id != h.id);
    edge = max(edge, max(verse_outline_crease(dot(h.n, hx.n)), verse_outline_crease(dot(h.n, hy.n))));
    let wp = EYE + d * h.t;
    let hz = st(9u);
    let f = verse_style_haze(h.t, wp.y, 0.0, hz.x, hz.y, hz.z, hz.w);
    let style = st(31u).y; let cap = st(31u).x;
    c = vec3f(verse_outline_styled(style, c.x, edge, cap, f), verse_outline_styled(style, c.y, edge, cap, f), verse_outline_styled(style, c.z, edge, cap, f));
  }
  return pow(clamp(tone(c), vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2));
}
// ---- the pack's meshes: its vertices (pack_realm.py: the Haunted House's 48
// bytes, position, normal, uv, colour with its layer in the top byte), a
// transform and tint a draw, its texture layers
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
struct Object { r0: vec4f, r1: vec4f, r2: vec4f, material: vec4f }
@group(1) @binding(1) var<storage, read> objects: array<Object>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
fn word(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }
struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) tint: vec3f,
  @location(4) @interpolate(flat) slice: u32,
}
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  let b = id * 12u;
  let p = vec4f(word(b), word(b + 1u), word(b + 2u), 1.0);
  let n = vec3f(word(b + 3u), word(b + 4u), word(b + 5u));
  let m = objects[instance];
  let w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let wp = vec4f(w, 1.0);
  var o: S;
  o.pos = vec4f(dot(u.r0, wp), dot(u.r1, wp), dot(u.r2, wp), dot(u.r3, wp));
  o.world = w;
  o.normal = vec3f(dot(m.r0.xyz, n), dot(m.r1.xyz, n), dot(m.r2.xyz, n));
  o.uv = vec2f(word(b + 6u), word(b + 7u));
  o.tint = unpack4x8unorm(vertices[b + 8u]).rgb * m.material.rgb;
  o.slice = vertices[b + 8u] >> 24u;
  return o;
}
@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  if (tex.a < 0.35) { discard; }
  let n = normalize(s.normal);
  let key = max(0.0, dot(n, normalize(vec3f(-0.4, 0.8, 0.45))));
  var lit = vec3f(0.10, 0.11, 0.16) * (0.6 + 0.4 * n.y) + vec3f(0.42, 0.44, 0.55) * key;
  let to = u.lamp.xyz - s.world;
  let d = length(to);
  let near = max(0.0, dot(n, to / max(d, 0.001))) * u.lamp.w / (1.0 + 0.18 * d * d);
  lit += vec3f(1.0, 0.62, 0.28) * near * 1.6;
  let c = tex.rgb * s.tint * lit * u.fade;
  return vec4f(panels(s.pos.xy, c), 1.0);
}
// ---- the strokes: one quad per stroke or light (fixture.coax lit, tinted_dot)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode
  @location(3) bar: vec4f,
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f) -> V {
  var o: V;
  let ndc = vec2f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0);
  o.pos = vec4f(ndc, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  o.bar = d;
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

// The outlines in the style's form (verse/look.coax).
// ---- generated: verse/look.coax (verse_outline_styled, verse_outline_crease) by coax/web/wgsl.py; do not edit
fn coax_p_lk_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn verse_outline_ink(l_light: f32, l_edge: f32) -> f32 {
  return (l_light * (1.0 - (0.819999993 * coax_p_lk_min(1.0, coax_p_lk_max(0.0, l_edge)))));
}
fn verse_outline_pale(l_light: f32, l_edge: f32, l_cap: f32, l_haze: f32) -> f32 {
  var coax_t4: f32;
  var coax_t5: f32;
  var coax_t6: f32;
  var coax_t7: f32;
  coax_t5 = coax_p_lk_min(coax_p_lk_min(1.0, coax_p_lk_max(0.0, l_edge)), 1.0);
  coax_t6 = coax_p_lk_min(coax_p_lk_max(l_cap, 0.0), 0.349999994);
  coax_t4 = (coax_t5 * coax_t6);
  coax_t7 = (1.0 - coax_p_lk_min(1.0, coax_p_lk_max(0.0, l_haze)));
  var l_w: f32 = (coax_t4 * coax_t7);
  return (l_light + (l_w * (coax_p_lk_max(l_light, 0.0) + 0.25)));
}
fn verse_outline_styled(l_style: f32, l_light: f32, l_edge: f32, l_cap: f32, l_haze: f32) -> f32 {
  if ((l_style < 0.5))
  {
    return verse_outline_ink(l_light, l_edge);
  }
  if ((l_style < 1.5))
  {
    return verse_outline_pale(l_light, l_edge, l_cap, l_haze);
  }
  return l_light;
}
fn coax_p_lk_ramp(l_a: f32, l_b: f32, l_x: f32) -> f32 {
  var l_t: f32 = coax_p_lk_min(1.0, coax_p_lk_max(0.0, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_outline_crease(l_facing: f32) -> f32 {
  return coax_p_lk_ramp(0.75, 0.550000012, l_facing);
}
// ---- end generated
// The art style's shading (verse/style.coax; docs/verse/art-styles.md), fed by the preset's STYLE_FLOATS.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma) by coax/web/wgsl.py; do not edit
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
fn verse_style_ambient(l_ny: f32, l_sky: f32, l_ground: f32, l_scale: f32) -> f32 {
  var coax_t8: f32;
  var coax_t9: f32;
  coax_t8 = (l_sky - l_ground);
  coax_t9 = coax_p_st_sat(((l_ny * 0.5) + 0.5));
  return ((l_ground + (coax_t8 * coax_t9)) * l_scale);
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
  var coax_t10: f32;
  var coax_t11: f32;
  var coax_t12: f32;
  var coax_t13: f32;
  coax_t10 = (-l_density);
  coax_t11 = coax_p_st_max((l_d - l_start), 0.0);
  var l_dist: f32 = (1.0 - exp((coax_t10 * coax_t11)));
  coax_t12 = (-coax_p_st_max((l_y - l_base_y), 0.0));
  coax_t13 = coax_p_st_max(l_falloff, 0.00100000005);
  var l_height: f32 = exp((coax_t12 / coax_t13));
  return coax_p_st_min((l_dist * (1.0 + ((l_height - 1.0) * 0.600000024))), l_cap);
}
fn verse_style_haze_glow(l_vdl: f32) -> f32 {
  var l_a: f32 = coax_p_st_sat(l_vdl);
  var l_a2: f32 = (l_a * l_a);
  var l_a4: f32 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0.449999988);
}
fn verse_style_fog(l_c: f32, l_haze_c: f32, l_f: f32) -> f32 {
  var coax_t14: f32;
  var coax_t15: f32;
  coax_t14 = (l_haze_c - l_c);
  coax_t15 = coax_p_st_sat(l_f);
  return (l_c + (coax_t14 * coax_t15));
}
fn verse_style_sky_t(l_up: f32) -> f32 {
  return pow(coax_p_st_max(l_up, 9.99999997e-07), 0.550000012);
}
fn verse_style_sky_band(l_up: f32, l_lift: f32) -> f32 {
  return (l_lift * (1.0 - coax_p_st_sat((abs(l_up) / 0.104500003))));
}
fn verse_style_halo(l_cos_t: f32, l_k: f32) -> f32 {
  var coax_t16: f32;
  var coax_t17: f32;
  coax_t16 = (-l_k);
  coax_t17 = coax_p_st_max((1.0 - l_cos_t), 0.0);
  return exp((coax_t16 * coax_t17));
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
// ---- end generated
