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
// With the post pass (host.js postLayouts) everything draws light and, in alpha, the share
// of it that blooms; lowpoly blooms nothing and post_fs passes it through as drawn.
struct Lit { @location(0) light: vec4f, @location(1) aux: vec4f }
// bg_fs also writes the traced scene's depth (host.js: a bg_fs with frag_depth), so the grass drawn
// after it hides behind the sphere, the cube and the tree; everything else is at the far plane.
struct Bg { @location(0) light: vec4f, @location(1) aux: vec4f, @builtin(frag_depth) depth: f32 }
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Bg {
  let p = pos.xy;
  if (u.painted > 0.5) {
    let c = paint(p);
    return Bg(c, vec4f(shaft_glow, 0.0, 0.0, 0.0), paint_depth);   // aux.r: the sky's glow the light shafts blur
  }
  let c = vec2f(0.5, 0.58) * u.size;
  let r = length((p - c) / max(u.size.y, 1.0));
  var sky = vec3f(0.012, 0.013, 0.02) + vec3f(0.035, 0.036, 0.045) * exp(-r * r * 6.0);
  sky += vec3f(stars(p)) * u.fade;
  return Bg(vec4f(panels(p, sky), 0.0), vec4f(0.0), 1.0);
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
const LANTERN = vec4f(-2.3, 0.16, 1.9, 0.16);         // a glowing orb on the ground: all its light blooms (spec 6)
const POND = vec4f(0.7, 0.0, 2.7, 1.5);              // centre x, -, z, radius; 1.9 m deep in the middle
const GRASS = vec4f(-3.4, 0.0, 2.2, 1.6);             // a lusher clump in the meadow
// The dirt path across the meadow behind the shapes, two segments (x, z each end) and its half width:
// the grass's mask (spec 8), with the pond. No blade grows on either.
const PATH0 = vec4f(-16.0, -3.6, 2.0, -5.2);
const PATH1 = vec4f(2.0, -5.2, 16.0, -8.4);
const PATH_W = 0.6;
const RIDGE_Z = vec3f(60.0, 115.0, 260.0);
struct Hit { t: f32, n: vec3f, albedo: vec3f, kind: f32, id: f32 }   // kind 0 none, 1 matte, 2 water, 3 character, 4 emissive (albedo is its light); id: which thing (outlines)
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
  var bases = array<f32, 3>(9.0, 21.0, 44.0);
  let base = bases[i];
  let s = 12.0 * (1.0 + f * f);   // nearer ridges are narrower in metres, so all three read alike on screen
  // the two near ridges open in a valley ahead, so the horizon there is the far ridge and the
  // far ground: the band the style check reads the haze from (art-styles.md 15)
  var gaps = array<vec2f, 3>(vec2f(14.0, 30.0), vec2f(24.0, 48.0), vec2f(0.0, 0.0));
  let g = gaps[i];
  let open = select(1.0, smoothstep(g.x, g.y, abs(x)), g.y > 0.0);
  return open * base * (0.72 + 0.18 * sin(x / s + f * 1.7) + 0.08 * sin(x / (0.37 * s) + 0.6 + f));
}
fn ridge_slope(x: f32, i: i32) -> f32 { return (ridge_h(x + 0.5, i) - ridge_h(x - 0.5, i)); }
fn ridge_albedo(i: i32) -> vec3f {
  // each farther flat painted a little bluer and paler under its haze, as the eye expects of a stack
  var a = array<vec3f, 3>(vec3f(0.09, 0.20, 0.06), vec3f(0.08, 0.17, 0.12), vec3f(0.34, 0.42, 0.50));
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
  t = sphere_t(o, d, LANTERN);
  if (t > 0.0 && t < h.t) { h = Hit(t, normalize(o + d * t - LANTERN.xyz), vec3f(3.2, 1.9, 0.8), 4.0, 11.0); }
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
      else {
        // the soil between the blades is the meadow's colour in their shade; the path is bare earth
        let bare = 1.0 - grass_mask(p.xz);
        h = Hit(tg, vec3f(0.0, 1.0, 0.0), mix(meadow(p.xz) * ROOT, path_albedo(p.xz), bare), 1.0, 10.0);
      }
    }
  }
  return h;
}
// ---- the key light's shadow map (spec 3; host.js shadowLayout, caster_vs): the scene's shapes
// drawn from the key light into a depth map round the eye (verse_style_shadow_reach: 60 m, 40 on a
// phone), its middle snapped to whole texels so shadows hold still, and read back here with the
// style's Poisson taps (9 at 2.5 texels, 4 on a phone), so a cast shadow has the terminator's soft edge.
const CASTER_SEGMENTS = 24u;
const CASTER_RINGS = 13u;                            // odd: no ring on a capsule's equator
fn caster(i: u32) -> vec4f {                         // centre, radius (the cube: its half size)
  var c = array<vec4f, 8>(SPHERE, CUBE, vec4f(3.9, 2.7, -2.6, 1.15), vec4f(3.25, 2.3, -2.3, 0.8), vec4f(4.5, 2.35, -2.9, 0.85),
    vec4f(TRUNK.x, 0.95, TRUNK.z, TRUNK.w), vec4f(BODY.x, 0.77, BODY.z, BODY.w), HEAD);
  return c[i];
}
fn caster_stretch(i: u32) -> f32 {                   // a capsule's straight half length (trunk, body)
  var h = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.77, 0.53, 0.0);
  return h[i];
}
struct Light { ax: vec3f, ay: vec3f, l: vec3f, mx: f32, my: f32, reach: f32 }
fn light_space() -> Light {
  let l = st(1u).xyz;
  let ax = select(vec3f(1.0, 0.0, 0.0), normalize(cross(vec3f(0.0, 1.0, 0.0), l)), abs(l.y) < 0.999);
  let ay = cross(l, ax);
  let reach = verse_style_shadow_reach(st(31u).z);
  let size = max(st(21u).x, 1.0);
  let c = vec3f(EYE.x, 0.0, EYE.z);
  return Light(ax, ay, l, verse_style_shadow_snap(dot(c, ax), reach, size), verse_style_shadow_snap(dot(c, ay), reach, size), reach);
}
fn shadow_place(w: vec3f, k: Light) -> vec3f {       // -1..1 across the map, and depth
  return vec3f(verse_style_shadow_across(dot(w, k.ax) - k.mx, k.reach), verse_style_shadow_across(dot(w, k.ay) - k.my, k.reach),
    verse_style_shadow_depth(dot(w - vec3f(k.mx * k.ax + k.my * k.ay), k.l), k.reach));
}
@vertex fn caster_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4f {
  var corner = array(vec2u(0u, 0u), vec2u(1u, 0u), vec2u(1u, 1u), vec2u(0u, 0u), vec2u(1u, 1u), vec2u(0u, 1u));
  let s = caster(i);
  var w = s.xyz;
  if (i == 1u) {
    // the cube: six faces of two triangles, turned as it is traced
    if (v < 36u) {
      let f = v / 6u; let a = f / 2u; let k = corner[v % 6u];
      var q = vec3f(0.0);
      q[a] = select(-1.0, 1.0, (f & 1u) == 1u);
      q[(a + 1u) % 3u] = f32(k.x) * 2.0 - 1.0;
      q[(a + 2u) % 3u] = f32(k.y) * 2.0 - 1.0;
      w = s.xyz + turn(q * s.w, 0.5);
    }
  } else {
    let quad = v / 6u; let k = corner[v % 6u];
    let ring = min(quad / CASTER_SEGMENTS + k.y, CASTER_RINGS);
    let th = 3.14159265 * f32(ring) / f32(CASTER_RINGS);
    let ph = 6.2831853 * f32(quad % CASTER_SEGMENTS + k.x) / f32(CASTER_SEGMENTS);
    w = s.xyz + vec3f(sin(th) * cos(ph), cos(th), sin(th) * sin(ph)) * s.w;
    w.y += select(-1.0, 1.0, ring * 2u < CASTER_RINGS) * caster_stretch(i);
  }
  let q = shadow_place(w, light_space());
  return vec4f(q.xy, q.z, 1.0);
}
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
// How much of the key reaches p (normal n): 1 lit, 0 in shadow, the map's taps averaged.
fn map_lit(p: vec3f, n: vec3f) -> f32 {
  if (st(21u).x < 1.0) { return 1.0; }               // a style without a map (lowpoly)
  let q = shadow_place(p + n * 0.05, light_space());
  let uv = vec2f(0.5 + 0.5 * q.x, 0.5 - 0.5 * q.y);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0))) { return 1.0; }
  let texel = 1.0 / f32(textureDimensions(shadow_map).x);
  let phone = st(31u).z; let taps = st(21u).y; let r = st(21u).z;
  var lit = 0.0;
  for (var k = 0.0; k < taps; k += 1.0) {
    let o = vec2f(verse_style_shadow_tap(k, phone, 0.0), verse_style_shadow_tap(k, phone, 1.0));
    lit += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + o * r * texel, q.z - 0.0004);
  }
  return lit / max(taps, 1.0);
}
fn shadow(p: vec3f, n: vec3f) -> f32 {
  // the figure moves in a realm, so it keeps a contact blob under it (spec 3), under the cast shadow
  let blob = 1.0 - smoothstep(0.15, 0.55, length(p.xz - BODY.xz)) * step(p.y, 0.02);
  return verse_style_contact(map_lit(p, n), blob, st(4u).w);
}
// The painted sky along d (spec 5): from the ray, not the screen, so it holds still as the eye
// pitches. The gradient by elevation, the horizon band lifted toward the haze, the sun or moon
// with its two-term halo, and one layer of soft two-tone clouds at 1,500 m drifting with the wind.
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
  let cl = st(15u); let c2 = st(16u);
  if (up > 0.01 && cl.x > 0.0) {
    let t = select(u.time, 0.0, u.still > 0.5);           // still: the clouds stand where they were at 0
    let p = d.xz / up * cl.y + st(25u).xy * cl.z * t;
    let n = verse_style_cloud_field(p.x, p.y, c2.x, c2.y, cl.w);
    let cover = verse_style_cloud_cover(n, cl.x, c2.z, c2.w);
    let toward = normalize(l.xz + vec2f(0.0001)) * 260.0;
    let lit = verse_style_cloud_lit(n, verse_style_cloud_field(p.x + toward.x, p.y + toward.y, c2.x, c2.y, cl.w));
    // at night (the stars out) the clouds go dim and thin, so the moonlit sky stays dark
    let night = st(14u).w;
    let cloud = mix(st(4u).xyz * st(17u).w * 0.9, st(17u).xyz, lit) * max(st(0u).w, 0.3) * (1.0 - 0.75 * night);
    c = mix(c, cloud, cover * verse_style_cloud_fade(up) * (1.0 - 0.5 * night));
  }
  return c;
}
// A banded surface (spec 1 and 3): the albedo under the tinted ambient in the shadow band, given the
// tint's hue once more at the preset's amount with its value kept (painterly shadows are cool, warm
// albedos too), and under the key plus ambient in the lit one, mixed by the band.
fn banded(a: vec3f, dark: vec3f, bright: vec3f, band: f32) -> vec3f {
  let s = a * dark;
  let t = st(4u).xyz;
  let tl = verse_style_luma(t.x, t.y, t.z);
  let k = st(21u).w;
  let m = vec3f(verse_style_cool(s.x, t.x, tl, k), verse_style_cool(s.y, t.y, tl, k), verse_style_cool(s.z, t.z, tl, k));
  let y = verse_style_luma(s.x, s.y, s.z); let ym = verse_style_luma(m.x, m.y, m.z);
  let cool = vec3f(verse_style_keep_value(m.x, y, ym), verse_style_keep_value(m.y, y, ym), verse_style_keep_value(m.z, y, ym));
  let l = a * bright;
  return vec3f(verse_style_banded(cool.x, l.x, band), verse_style_banded(cool.y, l.y, band), verse_style_banded(cool.z, l.z, band));
}
// Where a surface sits in the bands: the wrapped key times the shadow map, three bands (two for a character).
fn surface_band(h: Hit, lit: f32) -> f32 {
  let w = verse_style_wrap(dot(h.n, st(1u).xyz)) * lit;
  return select(verse_style_bands(w, st(7u).x, st(7u).y, st(7u).z, st(7u).w), verse_style_bands2(w, st(8u).x, st(8u).y), h.kind > 2.5);
}
fn shade(h: Hit, o: vec3f, d: vec3f) -> vec3f {
  if (h.kind > 3.5) { return h.albedo; }
  let p = o + d * h.t;
  let l = st(1u).xyz;
  let n = h.n;
  let ndl = dot(n, l);
  let character = h.kind > 2.5;
  let lit = shadow(p, n);
  let band = surface_band(h, lit);
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
    c = banded(a, dark, bright, band);
    let r = reflect(d, n);
    let f = 0.02 + 0.98 * pow(1.0 - max(dot(-d, n), 0.0), 5.0);
    c = mix(c, sky(r, l), f);
  } else {
    c = banded(a, dark, bright, band);
    let rs = select(st(2u).w, st(3u).x, character);
    // the rim is the key's light: a cast shadow takes it too
    // (not on a blade: a field seen edge on would all be rim)
    c += st(2u).xyz * verse_style_rim(dot(n, -d), ndl, st(3u).y, st(3u).z, rs) * lit * select(1.0, 0.0, h.id == 12.0);
  }
  // the haze, the same for everything (spec 4)
  let hz = st(9u);
  let f = verse_style_haze(h.t, p.y, 0.0, hz.x, hz.y, hz.z, hz.w);
  let hz3 = st(10u).xyz;
  let hue = vec3f(verse_style_haze_hue(hz3.x, 0.0, f, hz.z), verse_style_haze_hue(hz3.y, 1.0, f, hz.z), verse_style_haze_hue(hz3.z, 2.0, f, hz.z));
  let hc = mix(hue, st(11u).xyz, verse_style_haze_glow(dot(d, l)));
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
  let a = -0.11942 + u.r0.x;                 // pitch: the fixture's own, plus ?pitch= (radians, fixture.coax)
  let fwd = vec3f(0.0, sin(a), -cos(a));
  let right = normalize(cross(fwd, vec3f(0.0, 1.0, 0.0)));
  let up = cross(right, fwd);
  return normalize(fwd + right * q.x + up * q.y);
}
// Where a direction lands on screen, 0 to 1 across and down (ray's inverse); z 1 when it lies ahead of the eye.
fn screen_at(d: vec3f) -> vec3f {
  let aspect = u.size.x / max(u.size.y, 1.0);
  let tv = max(0.36, 0.5 / aspect);
  let a = -0.11942 + u.r0.x;
  let fwd = vec3f(0.0, sin(a), -cos(a));
  let right = normalize(cross(fwd, vec3f(0.0, 1.0, 0.0)));
  let up = cross(right, fwd);
  let z = dot(d, fwd);
  let q = vec2f(dot(d, right), dot(d, up)) / max(z, 0.0001);
  return vec3f((q.x / (tv * aspect) + 1.0) * 0.5, (1.0 - q.y / tv) * 0.5, select(0.0, 1.0, z > 0.01));
}
// The light shafts' source (art-styles.md 6): the sky's glow toward the key light, set where paint draws sky.
var<private> shaft_glow: f32 = 0.0;
// The traced hit's depth for bg_fs (1, the far plane, for the sky): the same projection grass_vs draws with.
var<private> paint_depth: f32 = 1.0;
const NEAR = 0.1;
const FAR = 2000.0;
fn eye_basis() -> mat3x3<f32> {             // right, up, forward: ray's camera
  let a = -0.11942 + u.r0.x;
  let fwd = vec3f(0.0, sin(a), -cos(a));
  let right = normalize(cross(fwd, vec3f(0.0, 1.0, 0.0)));
  return mat3x3<f32>(right, cross(right, fwd), fwd);
}
fn depth_at(z: f32) -> f32 { return FAR / (FAR - NEAR) * (1.0 - NEAR / max(z, NEAR)); }
fn paint(p: vec2f) -> vec4f {
  let d = ray(p);
  let h = trace(EYE, d);
  if (h.kind > 0.5) { paint_depth = depth_at(h.t * dot(d, eye_basis()[2])); }
  if (u.painted > 1.5) {
    // ?probe=shadow, for the style check: red where a lit surface (matte, water, character) is in the
    // shadow band, green its id over 16, blue 1 on every such surface the haze has taken under 0.35 of;
    // black for sky, what glows, and what the haze has taken
    let wp = EYE + d * h.t; let hz = st(9u);
    if (h.kind < 0.5 || h.kind > 3.5 || verse_style_haze(h.t, wp.y, 0.0, hz.x, hz.y, hz.z, hz.w) >= 0.35) { return vec4f(0.0); }
    return vec4f(select(0.0, 1.0, surface_band(h, shadow(wp, h.n)) < 0.5), h.id / 16.0, 1.0, 0.0);
  }
  var c = vec3f(0.0);
  if (h.kind == 0.0) {
    c = sky(d, st(1u).xyz);
    shaft_glow = verse_style_shaft_source(dot(d, st(1u).xyz));
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
    shaft_glow = verse_style_shaft_source(dot(d, st(1u).xyz)) * f;   // far, hazed things let the glow through
    let style = st(31u).y; let cap = st(31u).x;
    c = vec3f(verse_outline_styled(style, c.x, edge, cap, f), verse_outline_styled(style, c.y, edge, cap, f), verse_outline_styled(style, c.z, edge, cap, f));
  }
  // linear light and the share of it that blooms; post_fs blooms, tones and encodes it
  let b = st(19u);
  return vec4f(c, select(verse_style_bloom_share(verse_style_luma(c.x, c.y, c.z), b.x, b.y), 1.0, h.kind > 3.5));
}
// ---- the grass (spec 8): one tapered triangle a blade, instanced from a hash of its cell (no buffer of
// blades), 24 a square metre to 32 m ahead of the eye (8 to 18 m on a phone), shrinking over the last
// 6 m. Its colour is the meadow's under it, the root in the blades' own shade and the tip 15% lighter
// and warmer; its normal the ground's up, tilted toward the light as the wind bends it.
const ROOT = 0.78;                             // a blade's root, and the soil between, in the blades' shade
fn meadow(q: vec2f) -> vec3f {               // the meadow's painted colour: yellow-green to mid-green, broad strokes
  let lush = 1.0 - smoothstep(0.6, 1.0, length(q - GRASS.xz) / GRASS.w);
  let t = 0.5 + 0.5 * sin(q.x * 0.21 + q.y * 0.13);
  return mix(mix(vec3f(0.22, 0.38, 0.04), vec3f(0.10, 0.25, 0.04), t * 0.7), vec3f(0.09, 0.26, 0.03), lush * 0.7);
}
fn path_albedo(q: vec2f) -> vec3f { return mix(vec3f(0.30, 0.21, 0.10), vec3f(0.26, 0.20, 0.11), 0.5 + 0.5 * sin(q.x * 0.17)); }
fn grass_mask(q: vec2f) -> f32 {           // 1 where grass grows: none on the path or in the pond
  let path = min(verse_style_segment(q.x, q.y, PATH0.x, PATH0.y, PATH0.z, PATH0.w), verse_style_segment(q.x, q.y, PATH1.x, PATH1.y, PATH1.z, PATH1.w)) - PATH_W;
  let pond = length(q - POND.xz) - POND.w;
  return verse_style_grass_clear(path) * verse_style_grass_clear(pond);
}
struct G {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) n: vec3f,
  @location(2) albedo: vec3f,
}
@vertex fn grass_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> G {
  var o: G;
  o.pos = vec4f(0.0); o.world = vec3f(0.0); o.n = vec3f(0.0, 1.0, 0.0); o.albedo = vec3f(0.0);
  let g = st(22u); let g2 = st(23u); let w = st(24u); let w2 = st(25u);
  let per = max(u32(g.x), 1u); let r = g.y; let side = u32(2.0 * r);
  let cell = i / per; let b = f32(i % per);
  let c = vec2f(f32(cell % side) - r + EYE.x, EYE.z - 1.0 - f32(cell / side));
  let k = c + vec2f(b * 7.13, b * -3.71);
  let q = c + vec2f(hash(k), hash(k + vec2f(19.1, 7.7)));
  let dist = length(q - EYE.xz);
  let tall = verse_style_blade_height(hash(k + vec2f(3.3, 41.9)), g.w, g2.x) * verse_style_grass_fade(dist, r, g.z) * grass_mask(q);
  if (tall < 0.02) { return o; }                       // none here: a point, nothing drawn
  // the wind, frozen at t = 0 under still (goldens, reduced motion)
  let t = select(u.time, 0.0, u.still > 0.5);
  let wd = vec2f(w2.x, w2.y);
  let theta = verse_style_wind(dot(q, wd), verse_style_gust(q.x, q.y, t, wd.x, wd.y, w.z, w.w), t, w.x, w.y, w.z);
  // a mover (the figure) parts the blades within 0.7 m: they lean away from it
  let away = q - BODY.xz;
  let part = verse_style_part(length(away), w2.w);
  let lean = wd * verse_style_bend(theta, tall, 1.0) + normalize(away + vec2f(0.0001)) * tall * 0.85 * part;
  let ang = hash(k + vec2f(5.9, 13.3)) * 6.2831853;
  let across = vec3f(cos(ang), 0.0, sin(ang)) * g2.y * 0.5;
  var world = vec3f(q.x, 0.0, q.y);
  let base = meadow(q);
  let hj = hash(k + vec2f(31.7, 2.9));
  let a = vec3f(verse_style_jitter(base.x, 0.0, hj, g2.w), verse_style_jitter(base.y, 1.0, hj, g2.w), verse_style_jitter(base.z, 2.0, hj, g2.w));
  if (v == 0u) { world -= across; o.albedo = a * ROOT; }
  else if (v == 1u) { world += across; o.albedo = a * ROOT; }
  else {
    world += vec3f(lean.x, sqrt(max(tall * tall - dot(lean, lean), tall * tall * 0.15)), lean.y);
    o.albedo = vec3f(verse_style_tip(a.x, 0.0, g2.z), verse_style_tip(a.y, 1.0, g2.z), verse_style_tip(a.z, 2.0, g2.z));
  }
  // the normal: up, tilted toward the light by the bend (the bright bands rolling across the field)
  let l = st(1u).xyz;
  let lx = normalize(vec2f(l.x, l.z) + vec2f(0.0001));
  let tilt = verse_style_grass_tilt(theta, w.x, w2.z);
  o.n = normalize(vec3f(lx.x * sin(tilt), cos(tilt), lx.y * sin(tilt)));
  let e = eye_basis(); let rel = world - EYE;
  let aspect = u.size.x / max(u.size.y, 1.0); let tv = max(0.36, 0.5 / aspect);
  let z = dot(rel, e[2]);
  o.pos = vec4f(dot(rel, e[0]) / (tv * aspect), dot(rel, e[1]) / tv, FAR / (FAR - NEAR) * (z - NEAR), z);
  o.world = world;
  return o;
}
@fragment fn grass_fs(g: G) -> Lit {
  let d = normalize(g.world - EYE);
  let h = Hit(length(g.world - EYE), normalize(g.n), g.albedo, 1.0, 12.0);
  let c = shade(h, EYE, d);
  let b = st(19u);
  return Lit(vec4f(c, verse_style_bloom_share(verse_style_luma(c.x, c.y, c.z), b.x, b.y)), vec4f(0.0));
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
@fragment fn scene_fs(s: S) -> Lit {
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
  return Lit(vec4f(panels(s.pos.xy, c), 0.0), vec4f(0.0));
}
// ---- the post pass (host.js postLayouts): the bright share at a quarter and an eighth of
// the size, blurred, mixed back at the style's bloom mix; then the tone, and the plates over it.
struct Step { v: vec4f }
@group(1) @binding(0) var bloom_src: texture_2d<f32>;
@group(1) @binding(1) var bloom_lin: sampler;
@group(1) @binding(2) var<uniform> bloom_step: Step;
@fragment fn bloom_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let texel = 1.0 / vec2f(textureDimensions(bloom_src));
  let uv = pos.xy * bloom_step.v.w * texel;
  if (bloom_step.v.z < 0.5) {
    var c = vec3f(0.0);
    for (var k = 0; k < 4; k++) {
      let t = textureSampleLevel(bloom_src, bloom_lin, uv + (vec2f(f32(k & 1), f32(k >> 1u)) * 2.0 - 1.0) * texel, 0.0);
      c += t.rgb * clamp(t.a, 0.0, 1.0);
    }
    return vec4f(c * 0.25, 1.0);
  }
  let d = bloom_step.v.xy * texel;
  var c = textureSampleLevel(bloom_src, bloom_lin, uv, 0.0).rgb * 0.227;
  c += (textureSampleLevel(bloom_src, bloom_lin, uv + d * 1.385, 0.0).rgb + textureSampleLevel(bloom_src, bloom_lin, uv - d * 1.385, 0.0).rgb) * 0.316;
  c += (textureSampleLevel(bloom_src, bloom_lin, uv + d * 3.231, 0.0).rgb + textureSampleLevel(bloom_src, bloom_lin, uv - d * 3.231, 0.0).rgb) * 0.070;
  return vec4f(c, 1.0);
}
@group(1) @binding(0) var post_light: texture_2d<f32>;
@group(1) @binding(1) var post_quarter: texture_2d<f32>;
@group(1) @binding(2) var post_eighth: texture_2d<f32>;
@group(1) @binding(3) var post_aux: texture_2d<f32>;
@group(1) @binding(4) var post_depth: texture_depth_2d;
@group(1) @binding(5) var post_lin: sampler;
@fragment fn post_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let light = textureLoad(post_light, vec2i(pos.xy), 0).rgb;
  if (u.painted < 0.5 || u.painted > 1.5) { return vec4f(light, 1.0); }   // lowpoly, or the shadow probe's mask as drawn
  let uv = pos.xy / u.size;
  let b = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb * 0.55 + textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb * 0.45;
  let m = st(19u).z;
  var c = vec3f(verse_style_bloom_mix(light.x, b.x, m), verse_style_bloom_mix(light.y, b.y, m), verse_style_bloom_mix(light.z, b.z, m));
  c += shafts(pos.xy, uv) * st(11u).xyz * st(0u).w;
  return vec4f(panels(pos.xy, pow(clamp(tone(c), vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2))), 1.0);
}
// Light shafts (art-styles.md 6): march from the pixel toward the key light's place on screen,
// gathering the sky's glow (aux.r) with each tap weighted decay^i; the tree's crown and trunk block
// it, so light streams through the gaps. Its strength is the preset's weight, faded as the light
// leaves the frame. The march starts a hashed fraction of a step in, so few taps don't step.
fn shafts(p: vec2f, uv: vec2f) -> f32 {
  let sh = st(20u);
  let l = screen_at(st(1u).xyz);
  let fade = verse_style_shaft_fade(l.x, l.y, sh.w) * l.z;
  if (fade <= 0.0 || sh.z <= 0.0) { return 0.0; }
  let n = sh.x;
  let step = (l.xy - uv) / n;
  let jitter = fract(52.9829189 * fract(dot(p, vec2f(0.06711056, 0.00583715))));
  var sum = 0.0;
  for (var i = 0.0; i < n; i += 1.0) {
    let q = uv + step * (i + jitter);
    if (any(q < vec2f(0.0)) || any(q > vec2f(1.0))) { break; }
    sum += textureSampleLevel(post_aux, post_lin, q, 0.0).r * verse_style_shaft_tap(i, sh.y);
  }
  return verse_style_shaft(sum, verse_style_shaft_total(n, sh.y), sh.z, fade);
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
fn verse_style_bloom_mix(l_c: f32, l_bloom: f32, l_mix: f32) -> f32 {
  return (l_c + (l_bloom * l_mix));
}
fn verse_style_shadow_reach(l_phone: f32) -> f32 {
  if ((l_phone > 0.5))
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
fn verse_style_shadow_taps(l_phone: f32) -> f32 {
  if ((l_phone > 0.5))
  {
    return 4.0;
  }
  return 9.0;
}
fn verse_style_shadow_radius(l_phone: f32) -> f32 {
  return (2.5 + (0.0 * l_phone));
}
fn verse_style_shadow_tap(l_k: f32, l_phone: f32, l_axis: f32) -> f32 {
  var l_x: f32 = 0.0;
  var l_y: f32 = 0.0;
  if ((l_phone > 0.5))
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
  if ((l_axis > 0.5))
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
