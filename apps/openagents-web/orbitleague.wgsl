// Orbit League's two passes (orbitleague.coax): bg_fs traces the whole
// world per pixel (the arena's floor and glass, the goal frames and nets,
// the ball, both roadsters, the boost flames, the station's ribs and the
// space round it: stars, a ringed planet, a moon and the sun), in the flat
// style with ink lines (docs/verse/art-styles.md), with the HUD's plates
// over it; fx draws the HUD's strokes and text. Every place and size comes
// from orbitleague.coax.
//
// The roadster is our own design: low-poly convex hulls (a wedge body,
// four fender pods, a raked windscreen, two speedster humps), a driver's
// helmet, a rear wing and four wheels, with headlights and a tail bar that
// glow. The floor and the glass reflect the ball and the cars.
struct Car { p: vec4f, x: vec4f, y: vec4f, z: vec4f }   // place (w boost), right (w team), up (w the wheels' turn), forward (w steer)
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, zerog: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,                   // the eye; w the goal's flash
  fwd: vec4f, rx: vec4f, uy: vec4f,   // a pixel's ray: fwd + ndc.x rx + ndc.y uy; fwd.w the flash's team
  ball: vec4f,                  // centre, radius
  ballq: vec4f,                 // its attitude
  cars: array<Car, 2>,
  arena: vec4f,                 // half width, half length, roof, the goal mouth's half width
  goal: vec4f,                  // crossbar, net depth
  misc: vec4f,                  // the ball's speed (m/s)
  panel: array<vec4f, 32>,
  trail: array<vec4f, 27>,      // the ball's trail, then each car's boost: a bound (centre, radius) and 8 points (place, strength)
  blast: vec4f,                 // the goal's blast: centre, age (< 0 none)
  blast2: vec4f,                // its team
  pads: array<vec4f, 16>,       // the boost pads: x, z, big (1), how ready (1 ready)
}
@group(0) @binding(0) var<uniform> u: U;

// ---- the plates (droprealm.wgsl's)
fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 6.0 * s.x;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.z >= 2.0;
  let lit = over || (s.z >= 1.0 && s.z < 2.0);
  let fill = select(vec3f(0.05, 0.07, 0.12), vec3f(0.10, 0.15, 0.25), over);
  let line = select(vec3f(0.35, 0.55, 0.85), vec3f(0.6, 0.85, 1.0), lit);
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

fn hash3(p: vec3f) -> f32 { return fract(sin(dot(p, vec3f(127.1, 311.7, 74.7))) * 43758.5453); }
const BLUE = vec3f(0.18, 0.48, 1.0);
const MAGENTA = vec3f(1.0, 0.22, 0.72);
const SUN = vec3f(-0.5, 0.62, -0.45);   // normalised in sun(): high, behind the blue end
fn sun() -> vec3f { return normalize(SUN); }
fn team(t: f32) -> vec3f { return select(BLUE, MAGENTA, t > 0.5); }
fn moving() -> f32 { return select(1.0, 0.0, u.still > 0.5); }
// Flat light: three bands of the sun, and a sky fill.
fn flat_light(n: vec3f) -> f32 {
  let l = max(0.0, dot(n, sun()));
  let band = select(select(0.55, 0.8, l > 0.25), 1.0, l > 0.6);
  return 0.35 + 0.65 * band * (0.7 + 0.3 * max(0.0, n.y));
}

// ---- space: a ringed ice giant ahead of the blue end, a moon behind, the sun high over the blue end
const PLANET_D = vec3f(-0.5, 0.3, 1.0);
const PLANET_DIST = 3000.0;
const PLANET_R = 700.0;
const RING_N = vec3f(0.22, 1.0, -0.3);
const MOON_D = vec3f(0.5, 0.32, -1.0);
const MOON_DIST = 2000.0;
const MOON_R = 70.0;
fn planet_c() -> vec3f { return normalize(PLANET_D) * PLANET_DIST; }
fn sphere_t(d: vec3f, c: vec3f, r: f32) -> f32 {
  let b = dot(c, d);
  let disc = b * b - dot(c, c) + r * r;
  if (disc < 0.0) { return -1.0; }
  return b - sqrt(disc);
}
fn planet_col(n: vec3f) -> vec3f {
  // cool bands along its own pole (the rings'), warped, and one pale storm
  let pole = normalize(RING_N);
  let la = dot(n, pole);
  let b = la + sin(dot(n, vec3f(7.0, 1.0, 5.0))) * 0.035 + sin(la * 31.0 + n.x * 5.0) * 0.012;
  var col = mix(vec3f(0.16, 0.36, 0.50), vec3f(0.58, 0.74, 0.86), 0.5 + 0.5 * sin(b * 21.0));
  col = mix(col, vec3f(0.40, 0.34, 0.62), (0.5 + 0.5 * sin(b * 53.0 + 1.3)) * 0.35);
  let storm = (n - normalize(vec3f(0.3, -0.05, -0.95))) * vec3f(1.0, 2.2, 1.0);
  col = mix(col, vec3f(0.86, 0.93, 0.98), (1.0 - smoothstep(0.05, 0.085, length(storm))) * 0.85);
  return col;
}
fn space(d: vec3f) -> vec3f {
  var c = mix(vec3f(0.006, 0.008, 0.02), vec3f(0.025, 0.02, 0.055), clamp(0.5 + 0.5 * d.y, 0.0, 1.0));
  // a faint band of the galaxy
  let band = dot(d, normalize(vec3f(0.3, 0.55, -0.78)));
  let wave = 0.55 + 0.45 * sin(d.x * 7.0 + sin(d.z * 5.0) * 2.0) * sin(d.y * 9.0 + d.z * 3.0);
  c += mix(vec3f(0.09, 0.03, 0.15), vec3f(0.02, 0.09, 0.13), 0.5 + 0.5 * sin(d.x * 3.0 - d.z * 2.0)) * exp(-band * band * 14.0) * wave * 0.55;
  // stars: one at most in a cell of the direction's grid
  let g = d * 180.0;
  let cell = floor(g);
  let h = hash3(cell);
  if (h > 0.96) {
    let at = cell + vec3f(hash3(cell + 1.3), hash3(cell + 7.1), hash3(cell + 3.7));
    let r = length(g - at);
    let tw = 1.0 - 0.2 * moving() * (0.5 + 0.5 * sin(u.time * (0.5 + h * 3.0) + h * 50.0));
    let tint = mix(vec3f(0.75, 0.85, 1.0), vec3f(1.0, 0.95, 1.0), fract(h * 37.0));
    c += tint * (1.0 - smoothstep(0.05, 0.28, r)) * select(0.5, 1.0, h > 0.993) * tw;
  }
  // the sun: a white disc and its glare
  let sd = dot(d, sun());
  c += vec3f(1.0, 0.97, 0.93) * (smoothstep(0.9995, 0.9998, sd) * 3.0 + pow(max(sd, 0.0), 400.0) * 0.7 + pow(max(sd, 0.0), 24.0) * 0.06);
  // the moon: grey, cratered, lit from the sun's side
  let mc = normalize(MOON_D) * MOON_DIST;
  let mt = sphere_t(d, mc, MOON_R);
  if (mt > 0.0) {
    let n = normalize(d * mt - mc);
    let cr = hash3(floor(n * 9.0));
    var mcol = vec3f(0.55, 0.57, 0.62) * (0.85 + 0.15 * cr);
    mcol *= 1.0 - 0.25 * (1.0 - smoothstep(0.1, 0.3, length(fract(n * 9.0) - 0.5))) * step(0.6, cr);
    c = mcol * (0.05 + 0.95 * smoothstep(-0.05, 0.2, dot(n, sun())));
  }
  // the planet and its rings
  let pc = planet_c();
  let pt = sphere_t(d, pc, PLANET_R);
  if (pt > 0.0) {
    let n = normalize(d * pt - pc);
    let l = dot(n, sun());
    let lit = 0.05 + 0.95 * smoothstep(-0.08, 0.22, l);
    c = planet_col(n) * lit + vec3f(0.3, 0.6, 1.0) * pow(1.0 - max(0.0, dot(n, -d)), 3.0) * 0.8 * smoothstep(-0.35, 0.3, l);
  } else {
    // the air's halo round the limb
    let b = dot(pc, d);
    let miss = length(pc - d * b) - PLANET_R;
    if (b > 0.0 && miss > 0.0) { c += vec3f(0.3, 0.6, 1.0) * exp(-miss / 25.0) * 0.45; }
  }
  let rn = normalize(RING_N);
  let den = dot(d, rn);
  if (abs(den) > 1e-4) {
    let rt = dot(pc, rn) / den;
    let rp = d * rt - pc;
    let r = length(rp) / PLANET_R;
    if (rt > 0.0 && r > 1.35 && r < 2.25 && (pt < 0.0 || rt < pt)) {
      var a = 0.35 + 0.35 * sin(r * 34.0) * sin(r * 11.0 + 0.7);
      a *= smoothstep(1.35, 1.42, r) * (1.0 - smoothstep(2.15, 2.25, r)) * (1.0 - 0.8 * (1.0 - smoothstep(0.012, 0.03, abs(r - 1.78))));
      // the planet's shadow on the rings
      let sb = dot(rp, sun());
      let shadowed = sb < 0.0 && dot(rp, rp) - sb * sb < PLANET_R * PLANET_R;
      let rl = select(0.25 + 0.75 * abs(dot(rn, sun())), 0.06, shadowed);
      c = mix(c, vec3f(0.74, 0.78, 0.90) * rl, clamp(a, 0.0, 0.85));
    }
  }
  return c;
}
// A cheap reflection of the station for gloss: the floor's dark below, space above, the planet and the sun.
fn env(r: vec3f) -> vec3f {
  var c = mix(vec3f(0.03, 0.035, 0.05), vec3f(0.05, 0.07, 0.13), smoothstep(-0.15, 0.25, r.y));
  c += vec3f(0.30, 0.50, 0.65) * pow(max(dot(r, normalize(PLANET_D)), 0.0), 10.0) * 0.7;
  c += vec3f(1.0, 0.97, 0.93) * pow(max(dot(r, sun()), 0.0), 60.0) * 1.2;
  // the arena's edge lights, a line at the horizon
  c += vec3f(0.35, 0.55, 1.0) * exp(-r.y * r.y * 900.0) * 0.5;
  return c;
}

// ---- shapes
// mat: 0 matte, 1 paint, 2 glass, 3 light (emits its colour), 4 the ball, 5 the floor
struct Hit { t: f32, n: vec3f, col: vec3f, ink: f32, mat: i32 }
fn miss() -> Hit { return Hit(1e9, vec3f(0.0, 1.0, 0.0), vec3f(0.0), 0.0, 0); }
fn closer(a: Hit, b: Hit) -> Hit { if (b.t < a.t) { return b; } return a; }

// A box (centre c, half h) in a frame; o and d already in it. Its normal in that frame, and ink near its edges.
fn box_hit(o: vec3f, d: vec3f, c: vec3f, h: vec3f, col: vec3f, mat: i32) -> Hit {
  let inv = 1.0 / d;
  let t0 = (c - h - o) * inv;
  let t1 = (c + h - o) * inv;
  let tn = min(t0, t1);
  let tf = max(t0, t1);
  let tnear = max(max(tn.x, tn.y), tn.z);
  let tfar = min(min(tf.x, tf.y), tf.z);
  if (tnear > tfar || tfar < 0.0 || tnear < 0.0) { return miss(); }
  let p = o + d * tnear - c;
  let e = h - abs(p);
  var n = vec3f(0.0);
  if (tnear == tn.x) { n = vec3f(-sign(d.x), 0.0, 0.0); }
  else if (tnear == tn.y) { n = vec3f(0.0, -sign(d.y), 0.0); }
  else { n = vec3f(0.0, 0.0, -sign(d.z)); }
  // the edge: the middle of the three distances to the faces
  let mid = e.x + e.y + e.z - min(min(e.x, e.y), e.z) - max(max(e.x, e.y), e.z);
  return Hit(tnear, n, col, 1.0 - smoothstep(0.025, 0.05, mid), mat);
}

// The roadster's hulls, planes as (normal, offset); from a small script of points and normals.
var<private> HULL: array<vec4f, 45> = array<vec4f, 45>(
  vec4f(0.0000, -1.0000, 0.0000, 0.3200),
  vec4f(0.0000, 1.0000, 0.0000, 0.1200),
  vec4f(1.0000, 0.0000, 0.0000, 0.9500),
  vec4f(-1.0000, 0.0000, 0.0000, 0.9500),
  vec4f(0.5812, 0.8137, 0.0000, 0.5115),
  vec4f(-0.5812, 0.8137, 0.0000, 0.5115),
  vec4f(0.6402, -0.7682, 0.0000, 0.7234),
  vec4f(-0.6402, -0.7682, 0.0000, 0.7234),
  vec4f(0.0000, 0.0000, 1.0000, 2.1500),
  vec4f(0.0000, 0.9821, 0.1886, 0.2875),
  vec4f(0.0000, -0.6097, 0.7926, 1.7407),
  vec4f(0.4500, 0.0000, 0.8930, 2.1243),
  vec4f(-0.4500, 0.0000, 0.8930, 2.1243),
  vec4f(0.0000, 0.0000, -1.0000, 2.0800),
  vec4f(0.0000, 0.9439, -0.3304, 0.7244),
  vec4f(0.3998, 0.0000, -0.9166, 2.1672),
  vec4f(-0.3998, 0.0000, -0.9166, 2.1672),
  vec4f(0.0000, -0.5547, -0.8321, 1.8000),
  vec4f(1.0000, 0.0000, 0.0000, 0.1900),
  vec4f(-1.0000, 0.0000, 0.0000, 0.3000),
  vec4f(0.0000, -1.0000, 0.0000, 0.3000),
  vec4f(0.0000, 1.0000, 0.0000, 0.0500),
  vec4f(0.6727, 0.7399, 0.0000, 0.0242),
  vec4f(0.0000, 0.6727, 0.7399, 0.2408),
  vec4f(0.0000, 0.6727, -0.7399, 0.2408),
  vec4f(0.0000, 0.0000, 1.0000, 0.6600),
  vec4f(0.0000, 0.0000, -1.0000, 0.6600),
  vec4f(0.0000, -0.6402, 0.7682, 0.5762),
  vec4f(0.0000, -0.6402, -0.7682, 0.5762),
  vec4f(0.0000, 0.7276, 0.6860, 0.4852),
  vec4f(0.0000, -0.7276, -0.6860, -0.4452),
  vec4f(0.0000, 1.0000, 0.0000, 0.4600),
  vec4f(0.0000, -1.0000, 0.0000, -0.0800),
  vec4f(0.9929, 0.1191, 0.0000, 0.6672),
  vec4f(-0.9929, 0.1191, 0.0000, 0.6672),
  vec4f(0.7071, 0.7071, 0.0000, 0.6788),
  vec4f(-0.7071, 0.7071, 0.0000, 0.6788),
  vec4f(1.0000, 0.0000, 0.0000, 0.2500),
  vec4f(-1.0000, 0.0000, 0.0000, 0.2500),
  vec4f(0.0000, -1.0000, 0.0000, 0.0000),
  vec4f(0.0000, 0.9789, -0.2043, 0.4741),
  vec4f(0.0000, 0.3304, 0.9439, -0.3728),
  vec4f(0.0000, 0.0000, -1.0000, 1.7500),
  vec4f(0.6727, 0.7399, 0.0000, 0.3458),
  vec4f(-0.6727, 0.7399, 0.0000, 0.3458)
);

// A convex hull: planes n.p <= w from HULL[first..first + count]. The ray's
// way in, the face's normal, and ink where the face meets another.
struct HullHit { t: f32, n: vec3f, edge: f32 }
fn hull_cast(o: vec3f, d: vec3f, first: i32, count: i32) -> HullHit {
  var tn = -1e9; var tf = 1e9; var nn = vec3f(0.0); var k = -1;
  for (var i = first; i < first + count; i++) {
    let pl = HULL[i];
    let den = dot(pl.xyz, d);
    let num = pl.w - dot(pl.xyz, o);
    if (abs(den) < 1e-7) {
      if (num < 0.0) { return HullHit(1e9, nn, 0.0); }
      continue;
    }
    let t = num / den;
    if (den < 0.0) { if (t > tn) { tn = t; nn = pl.xyz; k = i; } } else { tf = min(tf, t); }
  }
  if (k < 0 || tn > tf || tn < 0.0) { return HullHit(1e9, nn, 0.0); }
  let p = o + d * tn;
  var e = 1e9;
  for (var i = first; i < first + count; i++) {
    if (i != k) { let pl = HULL[i]; e = min(e, pl.w - dot(pl.xyz, p)); }
  }
  return HullHit(tn, nn, 1.0 - smoothstep(0.012, 0.028, e));
}

// A wheel: a cylinder along x (centre c, radius r, half width w): a dark tyre and a bright rim.
fn wheel_hit(o: vec3f, d: vec3f, c: vec3f, r: f32, w: f32, spin: f32) -> Hit {
  let q = o - c;
  let a = d.y * d.y + d.z * d.z;
  let b = q.y * d.y + q.z * d.z;
  let cc = q.y * q.y + q.z * q.z - r * r;
  var best = miss();
  let disc = b * b - a * cc;
  if (disc >= 0.0 && a > 1e-6) {
    let t = (-b - sqrt(disc)) / a;
    let p = q + d * t;
    if (t > 0.0 && abs(p.x) <= w) { best = Hit(t, normalize(vec3f(0.0, p.y, p.z)), vec3f(0.05, 0.05, 0.06), 1.0 - smoothstep(0.03, 0.06, w - abs(p.x)), 0); }
  }
  // the hubs: five spokes in a bright rim
  for (var s = -1.0; s <= 1.0; s += 2.0) {
    if (abs(d.x) > 1e-6) {
      let t = (s * w - q.x) / d.x;
      let p = q + d * t;
      let rr = length(p.yz);
      if (t > 0.0 && t < best.t && rr <= r) {
        let ang = (atan2(p.y, p.z) + spin) * 5.0 / 6.2831853;
        let spoke = 1.0 - smoothstep(0.12, 0.2, abs(fract(ang) - 0.5));
        let rim = rr < r * 0.66 && (spoke > 0.5 || rr < r * 0.16);
        let col = select(vec3f(0.05, 0.05, 0.06), vec3f(0.78, 0.82, 0.88), rim);
        best = Hit(t, vec3f(s, 0.0, 0.0), col, 1.0 - smoothstep(0.02, 0.04, abs(rr - r * 0.66)) + 1.0 - smoothstep(0.02, 0.04, r - rr), select(0, 1, rim));
      }
    }
  }
  return best;
}

// A roadster of our own.
fn car_hit(ro: vec3f, rd: vec3f, c: Car) -> Hit {
  let q = ro - c.p.xyz;
  let o = vec3f(dot(q, c.x.xyz), dot(q, c.y.xyz), dot(q, c.z.xyz));
  let d = vec3f(dot(rd, c.x.xyz), dot(rd, c.y.xyz), dot(rd, c.z.xyz));
  // the bounding sphere first
  let bq = dot(o, d);
  let oo = dot(o, o);
  if (oo - bq * bq > 6.8 || (bq > 0.0 && oo > 6.8)) { return miss(); }
  let paint = team(c.x.w);
  var h = miss();
  // the body: a wedge with a dark sill, a cockpit, a stripe, headlights and a tail bar
  let b = hull_cast(o, d, 0, 18);
  if (b.t < h.t) {
    let p = o + d * b.t;
    let n = b.n;
    var col = paint; var mat = 1;
    if (p.y < -0.2 && n.y < 0.5) { col = vec3f(0.05, 0.055, 0.07); mat = 0; }
    if (n.y > 0.9 && abs(p.x) < 0.6 && p.z > -1.0 && p.z < 0.55) { col = vec3f(0.035, 0.04, 0.055); mat = 0; }
    if (n.y > 0.5 && abs(p.x) < 0.13 && p.z > 0.6) { col = vec3f(0.92, 0.94, 1.0); }
    if (n.z > 0.3 && p.z > 1.7 && p.y > -0.17 && p.y < -0.05 && abs(p.x) > 0.2) { col = vec3f(0.85, 0.93, 1.0) * 1.8; mat = 3; }
    if (n.z < -0.8 && abs(p.y) < 0.035 && abs(p.x) < 0.8) { col = mix(paint, vec3f(1.0), 0.45) * 1.8; mat = 3; }
    h = Hit(b.t, n, col, b.edge, mat);
  }
  // the fender pods over the wheels
  for (var i = 0; i < 4; i++) {
    let sx = select(-1.0, 1.0, (i & 1) == 1);
    let wz = select(1.4, -1.4, i >= 2);
    let f = hull_cast(vec3f(o.x * sx - 0.86, o.y, o.z - wz), vec3f(d.x * sx, d.y, d.z), 18, 11);
    if (f.t < h.t) { h = Hit(f.t, vec3f(f.n.x * sx, f.n.y, f.n.z), paint, f.edge, 1); }
  }
  // the windscreen
  let w = hull_cast(o, d, 29, 8);
  if (w.t < h.t) { h = Hit(w.t, w.n, vec3f(0.02, 0.035, 0.07), w.edge, 2); }
  // the humps behind the seats
  for (var s = -1.0; s <= 1.0; s += 2.0) {
    let k = hull_cast(vec3f(o.x * s - 0.32, o.y, o.z), vec3f(d.x * s, d.y, d.z), 37, 8);
    if (k.t < h.t) { h = Hit(k.t, vec3f(k.n.x * s, k.n.y, k.n.z), paint, k.edge, 1); }
  }
  // the driver's helmet: white, a dark visor, a stripe in the team's colour
  let hc = vec3f(0.32, 0.4, -0.22);
  let ht = sphere_t(d, hc - o, 0.2);
  if (ht > 0.0 && ht < h.t) {
    let n = normalize(o + d * ht - hc);
    var col = vec3f(0.92, 0.93, 0.95); var mat = 0;
    if (n.z > 0.3 && n.y < 0.5 && n.y > -0.35) { col = vec3f(0.03, 0.06, 0.12); mat = 2; }
    else if (abs(n.x) < 0.12 && n.y > 0.0) { col = paint; mat = 1; }
    h = Hit(ht, n, col, 0.0, mat);
  }
  // the rear wing on two struts
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.43, -1.86), vec3f(0.9, 0.03, 0.17), vec3f(0.07, 0.075, 0.09), 1));
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.405, -1.86), vec3f(0.92, 0.01, 0.175), paint, 1));
  h = closer(h, box_hit(o, d, vec3f(-0.5, 0.26, -1.86), vec3f(0.03, 0.15, 0.07), vec3f(0.07, 0.075, 0.09), 0));
  h = closer(h, box_hit(o, d, vec3f(0.5, 0.26, -1.86), vec3f(0.03, 0.15, 0.07), vec3f(0.07, 0.075, 0.09), 0));
  let steer = c.z.w;
  for (var i = 0; i < 4; i++) {
    let x = select(-0.86, 0.86, (i & 1) == 1);
    let z = select(1.4, -1.4, i >= 2);
    var wo = o - vec3f(x, -0.58, z);
    var wd = d;
    if (i < 2) {
      // a front wheel turns with the steering
      let cs = cos(-steer); let sn = sin(-steer);
      wo = vec3f(cs * wo.x - sn * wo.z, wo.y, sn * wo.x + cs * wo.z);
      wd = vec3f(cs * wd.x - sn * wd.z, wd.y, sn * wd.x + cs * wd.z);
    }
    var wh = wheel_hit(wo, wd, vec3f(0.0), 0.4, 0.15, c.y.w);
    if (wh.t < h.t && i < 2) {
      let cs = cos(steer); let sn = sin(steer);
      wh.n = vec3f(cs * wh.n.x - sn * wh.n.z, wh.n.y, sn * wh.n.x + cs * wh.n.z);
    }
    h = closer(h, wh);
  }
  if (h.t < 1e8) { h.n = h.n.x * c.x.xyz + h.n.y * c.y.xyz + h.n.z * c.z.xyz; }
  return h;
}

// The boost: a blue-white cone out of the rear bumper, and its glow.
fn flame(ro: vec3f, rd: vec3f, c: Car, tmax: f32) -> vec3f {
  if (c.p.w < 0.5) { return vec3f(0.0); }
  let flick = 1.0 + moving() * 0.12 * sin(u.time * 57.0 + c.x.w * 3.0);
  let a = c.p.xyz - c.z.xyz * 2.12 - c.y.xyz * 0.05;
  let b = a - c.z.xyz * (2.2 * flick);
  // the closest approach of the ray and the segment
  let ab = b - a;
  let w0 = ro - a;
  let aa = dot(rd, rd); let bb = dot(rd, ab); let cc = dot(ab, ab); let dd = dot(rd, w0); let ee = dot(ab, w0);
  let den = aa * cc - bb * bb;
  var s = 0.0; var t = 0.0;
  if (den > 1e-6) { s = clamp((bb * ee - cc * dd) / den, 0.0, tmax); t = clamp((aa * ee - bb * dd) / den, 0.0, 1.0); }
  let dist = length(ro + rd * s - (a + ab * t));
  let r = 0.3 * (1.0 - t * 0.8);
  let core = exp(-dist * dist / (r * r * 0.35));
  let halo = exp(-dist * dist / (r * r * 4.0)) * 0.35;
  let diamonds = 1.0 + 0.5 * pow(0.5 + 0.5 * cos(t * 22.0 - u.time * 30.0 * moving()), 6.0) * (1.0 - t);
  return (vec3f(0.9, 0.97, 1.0) * core * diamonds + vec3f(0.25, 0.5, 1.0) * halo) * (1.6 - t) * flick;
}
// A light's glow where the ray passes it, if the light is in front of what the ray hit.
fn halo(ro: vec3f, rd: vec3f, p: vec3f, tmax: f32, r: f32) -> f32 {
  let s = dot(p - ro, rd);
  if (s < 0.0 || s > tmax + 0.3) { return 0.0; }
  let dd = length(ro + rd * s - p);
  return exp(-dd * dd / (r * r));
}
fn lights(ro: vec3f, rd: vec3f, c: Car, tmax: f32) -> vec3f {
  let f = c.z.xyz;
  let m = c.p.xyz - ro;
  let along = dot(m, rd);
  if (along < 0.0 || dot(m, m) - along * along > 9.0 || along > 90.0) { return vec3f(0.0); }
  let front = max(0.0, -dot(rd, f)); let back = max(0.0, dot(rd, f));
  var o = vec3f(0.0);
  for (var s = -1.0; s <= 1.0; s += 2.0) {
    let hp = c.p.xyz + c.x.xyz * (s * 0.5) - c.y.xyz * 0.11 + f * 2.16;
    o += vec3f(0.75, 0.88, 1.0) * halo(ro, rd, hp, tmax, 0.25) * front * front * 1.2;
  }
  let tp = c.p.xyz - f * 2.1;
  o += mix(team(c.x.w), vec3f(1.0), 0.3) * halo(ro, rd, tp, tmax, 0.35) * back * 0.5;
  return o;
}

// A trail of light through its points, strongest and widest at its head.
fn trail(ro: vec3f, rd: vec3f, k: i32, tmax: f32, width: f32, col: vec3f) -> vec3f {
  let b = u.trail[k * 9];
  if (b.w < 0.0) { return vec3f(0.0); }
  let m = ro - b.xyz;
  let bq = dot(m, rd);
  if (dot(m, m) - bq * bq > b.w * b.w) { return vec3f(0.0); }
  var o = vec3f(0.0);
  for (var i = 0; i < 7; i++) {
    let p0 = u.trail[k * 9 + 1 + i]; let p1 = u.trail[k * 9 + 2 + i];
    if (p0.w <= 0.0 && p1.w <= 0.0) { continue; }
    let ab = p1.xyz - p0.xyz;
    let w0 = ro - p0.xyz;
    let bb = dot(rd, ab); let cc = dot(ab, ab); let dd = dot(rd, w0); let ee = dot(ab, w0);
    let den = cc - bb * bb;
    var t = 0.0;
    if (den > 1e-6) { t = clamp((ee - bb * dd) / den, 0.0, 1.0); }
    let q = p0.xyz + ab * t;
    let s = dot(q - ro, rd);
    if (s < 0.0 || s > tmax) { continue; }
    let dist = length(ro + rd * s - q);
    let age = (f32(i) + t) / 7.0;
    let r = width * (1.0 - 0.7 * age);
    o += col * (exp(-dist * dist / (r * r * 0.2)) * 0.9 + exp(-dist * dist / (r * r)) * 0.3) * mix(p0.w, p1.w, t) * (1.0 - age);
  }
  return o;
}
// The goal's blast: a shell of light in the scorer's colour, and a flash at its heart.
fn blast_r(age: f32) -> f32 { return 1.5 + 28.0 * (1.0 - exp(-age * 2.2)); }
fn blast_fade(age: f32) -> f32 { let k = 1.0 - clamp(age / 1.6, 0.0, 1.0); return k * k; }
fn blast(ro: vec3f, rd: vec3f, tmax: f32) -> vec3f {
  let age = u.blast.w;
  if (age < 0.0) { return vec3f(0.0); }
  let c = u.blast.xyz;
  let r = blast_r(age);
  let s = dot(c - ro, rd);
  let dd = length(ro + rd * s - c);
  if (s < -r || dd > r + 6.0 || s - r > tmax) { return vec3f(0.0); }
  let rim = exp(-pow((r - dd) / (1.0 + r * 0.15), 2.0));
  let col = mix(team(u.blast2.x), vec3f(1.0), 0.3);
  let heart = exp(-dd * dd / (8.0 + age * 60.0)) * max(0.0, 1.0 - age * 1.6) * 2.5;
  return col * (rim * 2.2 + 0.2 * step(dd, r)) * blast_fade(age) + mix(col, vec3f(1.0), 0.6) * heart;
}

fn qrot(q: vec4f, v: vec3f) -> vec3f { let t = 2.0 * cross(q.xyz, v); return v + q.w * t + cross(q.xyz, t); }

fn ball_hit(ro: vec3f, rd: vec3f) -> Hit {
  let m = ro - u.ball.xyz;
  let b = dot(m, rd);
  let c = dot(m, m) - u.ball.w * u.ball.w;
  let disc = b * b - c;
  if (disc < 0.0) { return miss(); }
  let t = -b - sqrt(disc);
  if (t < 0.0) { return miss(); }
  let n = normalize(m + rd * t);
  // its seams in its own frame: three great circles that glow, and a cap at each pole
  let l = qrot(vec4f(-u.ballq.xyz, u.ballq.w), n);
  let a = abs(l);
  let seam = min(min(a.x, a.y), a.z);
  let pole = max(max(a.x, a.y), a.z);
  var col = vec3f(0.90, 0.92, 0.96);
  if (pole > 0.965) { col = vec3f(0.30, 0.36, 0.48); }
  return Hit(t, n, col, 1.0 - smoothstep(0.03, 0.055, seam), 4);
}

// How a hit looks in the light (not the floor).
fn shade_hit(h: Hit, rd: vec3f) -> vec3f {
  let n = h.n;
  if (h.mat == 3) { return h.col; }
  let l = flat_light(n);
  let r = reflect(rd, n);
  let fres = pow(1.0 - abs(dot(n, -rd)), 4.0);
  var c = h.col * l;
  if (h.mat == 1) {
    c = mix(c, env(r), 0.06 + 0.5 * fres);
    c += vec3f(1.0) * smoothstep(0.93, 0.97, dot(r, sun())) * 0.7;
  } else if (h.mat == 2) {
    c = h.col + env(r) * 0.9 + vec3f(1.0) * smoothstep(0.9, 0.95, dot(r, sun())) * 0.6;
  } else if (h.mat == 4) {
    c = mix(c, env(r), 0.05 + 0.3 * fres);
    let glow = clamp(u.misc.x / 25.0, 0.0, 1.0);
    return mix(c, mix(vec3f(0.05, 0.06, 0.09), vec3f(0.6, 0.9, 1.0) * 1.6, 0.2 + 0.8 * glow), h.ink);
  }
  // a rim of light from the planet
  c += vec3f(0.15, 0.3, 0.6) * pow(1.0 - abs(dot(n, -rd)), 3.0) * 0.5;
  return mix(c, vec3f(0.01, 0.015, 0.03), clamp(h.ink, 0.0, 1.0) * 0.85);
}

// ---- the arena
fn floor_col(p: vec3f) -> vec3f {
  let hx = u.arena.x; let hz = u.arena.y; let gw = u.arena.w;
  var c = select(vec3f(0.075, 0.09, 0.135), vec3f(0.11, 0.075, 0.125), p.z > 0.0);
  // the deck's plates, five metres a side
  let g = abs(fract(p.xz / 5.0 + 0.5) - 0.5) * 5.0;
  c *= 1.0 - 0.25 * (1.0 - smoothstep(0.03, 0.07, min(g.x, g.y)));
  // the markings
  var line = 0.0;
  line = max(line, 1.0 - smoothstep(0.1, 0.16, abs(p.z)));
  line = max(line, 1.0 - smoothstep(0.1, 0.16, abs(length(p.xz) - 9.0)));
  let box = vec2f(abs(p.x) - (gw + 5.0), abs(p.z) - (hz - 12.0));
  line = max(line, (1.0 - smoothstep(0.1, 0.16, abs(box.y))) * step(abs(p.x), gw + 5.0) * step(abs(p.z), hz));
  line = max(line, (1.0 - smoothstep(0.1, 0.16, abs(box.x))) * step(hz - 12.0, abs(p.z)) * step(abs(p.z), hz));
  line = max(line, (1.0 - smoothstep(0.1, 0.16, abs(abs(p.z) - hz))) * step(abs(p.x), gw));
  let side = select(BLUE, MAGENTA, p.z > 0.0);
  c = mix(c, mix(vec3f(0.85, 0.9, 1.0), side, 0.35), line * 0.85);
  // a strip of light round the edge
  let edge = min(hx - abs(p.x), hz - abs(p.z));
  c += side * (1.0 - smoothstep(0.0, 0.6, edge)) * 0.6 * step(gw, abs(p.x) + step(abs(p.z), hz) * 99.0);
  return c;
}
// Shadows straight down: soft discs under the ball and the cars.
fn shadow(p: vec3f) -> f32 {
  var k = 1.0;
  let bd = length(p.xz - u.ball.xz);
  let bh = max(u.ball.y - p.y, 0.0);
  k *= 1.0 - 0.55 * (1.0 - smoothstep(u.ball.w * 0.6, u.ball.w * (1.1 + bh * 0.05), bd)) * step(p.y, u.ball.y);
  for (var i = 0; i < 2; i++) {
    let c = u.cars[i];
    let q = p - c.p.xyz;
    let lx = abs(dot(q.xz, c.x.xz)); let lz = abs(dot(q.xz, c.z.xz));
    let h = max(c.p.y - p.y, 0.0);
    let s = (1.0 - smoothstep(0.85, 1.3 + h * 0.05, lx)) * (1.0 - smoothstep(1.9, 2.4 + h * 0.05, lz));
    k *= 1.0 - 0.5 * s * step(p.y, c.p.y);
  }
  return k;
}
// The boost pads: a ring that fills as it gets ready, and a lit disc when it is.
const PAD_COL = vec3f(0.75, 0.9, 1.0);
fn pads(p: vec3f) -> vec3f {
  var o = vec3f(0.0);
  for (var i = 0; i < 16; i++) {
    let pd = u.pads[i];
    let d = length(p.xz - pd.xy);
    let r = select(0.9, 1.5, pd.z > 0.5);
    if (d > r + 0.6) { continue; }
    let ready = pd.w >= 0.999;
    // the ring fills round as it recharges
    let a = atan2(p.x - pd.x, p.z - pd.y) / 6.2831853 + 0.5;
    let lit = select(0.18, 0.9, ready || a < pd.w);
    o += PAD_COL * exp(-pow((d - r) / 0.09, 2.0)) * lit;
    if (ready) {
      let pulse = 0.8 + 0.2 * moving() * sin(u.time * 4.0 + f32(i));
      o += PAD_COL * (1.0 - smoothstep(r * 0.55, r * 0.8, d)) * 0.45 * pulse;
    }
  }
  return o;
}
// A big pad's floating orb, when it is ready.
fn pad_orbs(ro: vec3f, rd: vec3f, tmax: f32) -> vec3f {
  var o = vec3f(0.0);
  for (var i = 0; i < 6; i++) {
    let pd = u.pads[i];
    if (pd.w < 0.999) { continue; }
    let bob = 1.3 + 0.15 * moving() * sin(u.time * 2.0 + f32(i));
    o += PAD_COL * (halo(ro, rd, vec3f(pd.x, bob, pd.y), tmax, 0.35) * 1.4 + halo(ro, rd, vec3f(pd.x, bob, pd.y), tmax, 1.0) * 0.25);
  }
  return o;
}

// Each car's glow on the floor under it, in its team's colour.
fn underglow(p: vec3f) -> vec3f {
  var o = vec3f(0.0);
  for (var i = 0; i < 2; i++) {
    let c = u.cars[i];
    let q = p - c.p.xyz;
    let lx = dot(q, c.x.xyz); let lz = dot(q, c.z.xyz);
    o += team(c.x.w) * exp(-(lx * lx / 1.1 + lz * lz / 5.0)) * clamp(1.8 - c.p.y, 0.0, 1.0) * 0.4;
  }
  return o;
}

// What moves (the ball and the cars), and what moves or stands in the arena (and the goal frames).
fn movers(ro: vec3f, rd: vec3f) -> Hit {
  return closer(closer(ball_hit(ro, rd), car_hit(ro, rd, u.cars[0])), car_hit(ro, rd, u.cars[1]));
}

// The station's ribs: four arches over the glass, outside it, each with a light at its crown.
fn ribs(ro: vec3f, rd: vec3f) -> Hit {
  let hx = u.arena.x; let hh = u.arena.z;
  var h = miss();
  let steel = vec3f(0.11, 0.12, 0.15);
  for (var k = 0; k < 4; k++) {
    let z = -30.0 + f32(k) * 20.0;
    // only a ray that passes the arch's plane near the arch
    let tz = (z - ro.z) / rd.z;
    let q = ro + rd * tz;
    if (abs(rd.z) < 1e-5 || tz < 0.0 || abs(q.x) > hx + 3.0 || q.y > hh + 3.0 || (abs(q.x) < hx - 1.0 && q.y < hh - 1.0)) { continue; }
    h = closer(h, box_hit(ro, rd, vec3f(0.0, hh + 1.0, z), vec3f(hx + 1.4, 0.45, 0.6), steel, 0));
    h = closer(h, box_hit(ro, rd, vec3f(hx + 0.95, hh * 0.5, z), vec3f(0.45, hh * 0.5 + 1.4, 0.6), steel, 0));
    h = closer(h, box_hit(ro, rd, vec3f(-hx - 0.95, hh * 0.5, z), vec3f(0.45, hh * 0.5 + 1.4, 0.6), steel, 0));
  }
  return h;
}
fn rib_lights(ro: vec3f, rd: vec3f, tmax: f32) -> vec3f {
  let hx = u.arena.x; let hh = u.arena.z;
  var o = vec3f(0.0);
  for (var k = 0; k < 4; k++) {
    let z = -30.0 + f32(k) * 20.0;
    let blink = select(1.0, 0.25 + 0.75 * step(0.5, fract(u.time * 0.5 + f32(k) * 0.25)), u.still < 0.5);
    for (var s = -1.0; s <= 1.0; s += 2.0) {
      o += vec3f(0.6, 1.0, 0.95) * halo(ro, rd, vec3f(s * (hx + 0.95), hh + 1.5, z), tmax, 0.35) * blink;
    }
  }
  return o;
}

// The glass the ray leaves the arena through, and the nets it passes: their
// colour over what is behind, how much, and the light they add (their edge
// lights, and the ball and cars reflected in them).
struct Glass { t: f32, col: vec3f, a: f32, add: vec3f }
fn glass(ro: vec3f, rd: vec3f, tmax: f32) -> Glass {
  let hx = u.arena.x; let hz = u.arena.y; let hh = u.arena.z; let gw = u.arena.w; let gh = u.goal.x; let gd = u.goal.y;
  var out = Glass(1e9, vec3f(0.0), 0.0, vec3f(0.0));
  // the exit: the nearest face ahead
  var t = 1e9; var n = vec3f(0.0);
  if (rd.x > 0.0) { let s = (hx - ro.x) / rd.x; if (s < t) { t = s; n = vec3f(1.0, 0.0, 0.0); } }
  if (rd.x < 0.0) { let s = (-hx - ro.x) / rd.x; if (s < t) { t = s; n = vec3f(-1.0, 0.0, 0.0); } }
  if (rd.y > 0.0) { let s = (hh - ro.y) / rd.y; if (s < t) { t = s; n = vec3f(0.0, 1.0, 0.0); } }
  if (rd.z > 0.0) { let s = (hz - ro.z) / rd.z; if (s < t) { t = s; n = vec3f(0.0, 0.0, 1.0); } }
  if (rd.z < 0.0) { let s = (-hz - ro.z) / rd.z; if (s < t) { t = s; n = vec3f(0.0, 0.0, -1.0); } }
  if (t > tmax || t > 1e8) { return out; }
  let p = ro + rd * t;
  let mouth = n.z != 0.0 && abs(p.x) < gw && p.y < gh;
  if (!mouth) {
    var uv = p.xz;
    if (n.x != 0.0) { uv = p.zy; } else if (n.z != 0.0) { uv = p.xy; }
    // glass in a frame of thin struts every 5 metres
    let g = abs(fract(uv / 5.0 + 0.5) - 0.5) * 5.0;
    let strut = 1.0 - smoothstep(0.03, 0.09, min(g.x, g.y));
    // the distance to the face's edge (not along its own axis)
    var e = min(hx - abs(p.x), hz - abs(p.z));
    if (n.x != 0.0) { e = min(hz - abs(p.z), min(p.y, hh - p.y)); }
    if (n.z != 0.0) { e = min(hx - abs(p.x), min(p.y, hh - p.y)); }
    let side = select(BLUE, MAGENTA, p.z > 0.0);
    let fres = pow(1.0 - abs(dot(rd, n)), 4.0);
    var col = mix(vec3f(0.30, 0.45, 0.65), side, 0.35);
    var a = 0.035 + 0.3 * fres + strut * 0.2;
    // broad diagonal streaks, as light lies on glass
    let sh = fract((uv.x * 0.8 + uv.y) * 0.045 + n.x * 0.3 + n.z * 0.6);
    a += 0.07 * (smoothstep(0.0, 0.03, sh) * (1.0 - smoothstep(0.03, 0.12, sh)) + 0.5 * smoothstep(0.2, 0.22, sh) * (1.0 - smoothstep(0.22, 0.26, sh)));
    if (n.z != 0.0) { col = mix(col, side, 0.4); a += 0.03; }
    // light along the arena's edges
    var add = mix(side, vec3f(1.0), 0.25) * exp(-e * e / 0.03) * 0.9;
    // the ball and the cars, reflected
    let rr = reflect(rd, n);
    let rh = movers(p - n * 0.02, rr);
    if (rh.t < 80.0) { add += shade_hit(rh, rr) * (0.16 + 0.4 * fres) * exp(-rh.t * 0.02); }
    return Glass(t, col, clamp(a, 0.0, 1.0), add);
  }
  // into a goal: its net (back, sides, roof)
  let zs = sign(n.z);
  var tn = 1e9; var nn = vec3f(0.0);
  let back = (zs * (hz + gd) - ro.z) / rd.z;
  if (back > t) { tn = back; nn = vec3f(0.0, 0.0, zs); }
  if (rd.x > 0.0) { let s = (gw - ro.x) / rd.x; if (s > t && s < tn) { tn = s; nn = vec3f(1.0, 0.0, 0.0); } }
  if (rd.x < 0.0) { let s = (-gw - ro.x) / rd.x; if (s > t && s < tn) { tn = s; nn = vec3f(-1.0, 0.0, 0.0); } }
  if (rd.y > 0.0) { let s = (gh - ro.y) / rd.y; if (s > t && s < tn) { tn = s; nn = vec3f(0.0, 1.0, 0.0); } }
  if (tn > tmax) { return out; }
  let q = ro + rd * tn;
  var uv = q.xy;
  if (nn.x != 0.0) { uv = q.zy; } else if (nn.y != 0.0) { uv = q.xz; }
  let g = abs(fract(uv / 0.6 + 0.5) - 0.5) * 0.6;
  let mesh = 1.0 - smoothstep(0.015, 0.04, min(g.x, g.y));
  let col = select(BLUE, MAGENTA, zs > 0.0);
  return Glass(tn, col * 1.2, clamp(0.12 + mesh * 0.65, 0.0, 1.0), vec3f(0.0));
}

// The goal frames: posts and a crossbar at each mouth, lit in the team's colour.
fn frames(ro: vec3f, rd: vec3f) -> Hit {
  let hz = u.arena.y; let gw = u.arena.w; let gh = u.goal.x;
  var h = miss();
  for (var e = -1.0; e <= 1.0; e += 2.0) {
    let col = select(BLUE, MAGENTA, e > 0.0) * 1.3;
    let z = e * (hz + 0.15);
    // only a ray that passes the mouth's frame
    let tz = (z - ro.z) / rd.z;
    let q = ro + rd * tz;
    if (abs(rd.z) < 1e-5 || tz < 0.0 || abs(q.x) > gw + 1.5 || q.y > gh + 1.5 || q.y < -0.5) { continue; }
    h = closer(h, box_hit(ro, rd, vec3f(-gw - 0.2, gh * 0.5, z), vec3f(0.2, gh * 0.5 + 0.2, 0.2), col, 3));
    h = closer(h, box_hit(ro, rd, vec3f(gw + 0.2, gh * 0.5, z), vec3f(0.2, gh * 0.5 + 0.2, 0.2), col, 3));
    h = closer(h, box_hit(ro, rd, vec3f(0.0, gh + 0.2, z), vec3f(gw + 0.4, 0.2, 0.2), col, 3));
  }
  return h;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let px = pos.xy;
  let ndc = vec2f(px.x / u.size.x * 2.0 - 1.0, 1.0 - px.y / u.size.y * 2.0);
  let ro = u.eye.xyz;
  let rd = normalize(u.fwd.xyz + ndc.x * u.rx.xyz + ndc.y * u.uy.xyz);
  let hx = u.arena.x; let hz = u.arena.y; let gw = u.arena.w; let gd = u.goal.y;
  // the nearest solid thing
  var h = miss();
  if (rd.y < 0.0) {
    let t = -ro.y / rd.y;
    let p = ro + rd * t;
    let on = (abs(p.x) <= hx && abs(p.z) <= hz) || (abs(p.x) <= gw && abs(p.z) <= hz + gd);
    if (on) { h = Hit(t, vec3f(0.0, 1.0, 0.0), floor_col(p), 0.0, 5); }
  }
  h = closer(h, movers(ro, rd));
  h = closer(h, frames(ro, rd));
  if (h.t > 1e8) { h = ribs(ro, rd); }
  var c = vec3f(0.0);
  if (h.t < 1e8) {
    let p = ro + rd * h.t;
    if (h.mat == 5) {
      c = h.col * 0.85 * shadow(p) + underglow(p) + pads(p);
      // the blast's ring running out over the floor
      if (u.blast.w >= 0.0) {
        let ring = (length(p.xz - u.blast.xz) - blast_r(u.blast.w) * 1.1) / 1.2;
        c += mix(team(u.blast2.x), vec3f(1.0), 0.3) * exp(-ring * ring) * blast_fade(u.blast.w) * 0.9;
      }
      // the floor is polished: the ball, the cars, the frames and the boost in it
      let rr = vec3f(rd.x, -rd.y, rd.z);
      let q = p + vec3f(0.0, 0.002, 0.0);
      let rh = closer(movers(q, rr), frames(q, rr));
      let k = 0.2 + 0.3 * pow(1.0 - abs(rd.y), 4.0);
      if (rh.t < 70.0) { c = mix(c, shade_hit(rh, rr), k * exp(-rh.t * 0.45)); }
      c += (flame(q, rr, u.cars[0], rh.t) + flame(q, rr, u.cars[1], rh.t)) * k;
    } else {
      c = shade_hit(h, rd);
    }
    // far things fade a little into the station's haze
    c = mix(c, vec3f(0.05, 0.07, 0.12), clamp(h.t / 220.0, 0.0, 0.5));
  } else {
    c = space(rd);
  }
  // glass and nets in front of it
  let gl = glass(ro, rd, h.t);
  if (gl.a > 0.0) { c = mix(c, gl.col, gl.a); }
  c += gl.add;
  // the boost flames, the cars' lights, the ribs' beacons
  c += flame(ro, rd, u.cars[0], h.t) + flame(ro, rd, u.cars[1], h.t);
  c += trail(ro, rd, 0, h.t, u.ball.w * 0.75, vec3f(0.6, 0.85, 1.0));
  c += trail(ro, rd, 1, h.t, 0.32, vec3f(0.45, 0.7, 1.0)) + trail(ro, rd, 2, h.t, 0.32, vec3f(0.45, 0.7, 1.0));
  c += blast(ro, rd, h.t);
  c += pad_orbs(ro, rd, h.t);
  c += lights(ro, rd, u.cars[0], h.t) + lights(ro, rd, u.cars[1], h.t);
  if (rd.y > 0.0 || h.t > 1e8) { c += rib_lights(ro, rd, h.t); }
  // zero-g: a cold tint; a goal: its team's flash
  c = mix(c, c * vec3f(0.85, 0.95, 1.15), u.zerog * 0.5);
  c += team(u.fwd.w) * u.eye.w * 0.12;
  return vec4f(panels(px, c), 1.0);
}

// ---- the strokes: one quad per stroke or light (orbitleague.coax lit, tinted_dot)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,
  @location(1) shape: vec4f,
  @location(2) tint: vec4f,
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

// ---- generated: verse/style.coax (verse_style_flat_light, verse_style_flat_band, verse_style_flat_band2, verse_style_flat_sky, verse_style_flat_shadow, verse_style_flat_fade, verse_style_bayer, verse_style_flat_dither, verse_style_flat_hem, verse_style_flat_srgb, verse_style_flat_colour, verse_style_flat_time) by coax/web/wgsl.py; do not edit
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
fn verse_style_flat_light(l_fl_ndl: f32, l_fl_lit: f32) -> f32 {
  var coax_t54: f32;
  var coax_t55: f32;
  coax_t54 = coax_p_st_sat(((l_fl_ndl * 0.5) + 0.5));
  coax_t55 = coax_p_st_sat(l_fl_lit);
  return (coax_t54 * coax_t55);
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
fn verse_style_flat_band(l_fl_w: f32) -> f32 {
  return verse_style_bands(l_fl_w, 0.379999995, 0.620000005, 0.00999999978, 0.5);
}
fn verse_style_flat_band2(l_fl_w: f32) -> f32 {
  return verse_style_soft_step(0.449999988, 0.00999999978, l_fl_w);
}
fn verse_style_flat_sky(l_fl_up: f32) -> f32 {
  return (1.0 - verse_style_soft_step(0.119999997, 0.119999997, l_fl_up));
}
fn verse_style_flat_shadow(l_fl_d: f32) -> f32 {
  return (1.0 - verse_style_soft_step(0.850000024, 0.150000006, l_fl_d));
}
fn verse_style_flat_fade(l_fl_d: f32, l_fl_start: f32, l_fl_reach: f32, l_fl_cap: f32) -> f32 {
  var coax_t56: f32;
  var coax_t57: f32;
  coax_t56 = (l_fl_start + (l_fl_reach * 0.5));
  coax_t57 = coax_p_st_max((l_fl_reach * 0.5), 0.00100000005);
  return (l_fl_cap * verse_style_soft_step(coax_t56, coax_t57, l_fl_d));
}
fn coax_p_st_bayer2(l_fl_a: f32, l_fl_b: f32) -> f32 {
  return (((2.0 * l_fl_a) + (3.0 * l_fl_b)) - ((4.0 * l_fl_a) * l_fl_b));
}
fn verse_style_bayer(l_fl_px: f32, l_fl_py: f32) -> f32 {
  var coax_t58: f32;
  var coax_t59: f32;
  var l_fl_x: f32 = (l_fl_px - (4.0 * floor((l_fl_px * 0.25))));
  var l_fl_y: f32 = (l_fl_py - (4.0 * floor((l_fl_py * 0.25))));
  var l_fl_x0: f32 = (l_fl_x - (2.0 * floor((l_fl_x * 0.5))));
  var l_fl_y0: f32 = (l_fl_y - (2.0 * floor((l_fl_y * 0.5))));
  var l_fl_x1: f32 = floor((l_fl_x * 0.5));
  var l_fl_y1: f32 = floor((l_fl_y * 0.5));
  coax_t58 = (4.0 * coax_p_st_bayer2(l_fl_x0, l_fl_y0));
  coax_t59 = coax_p_st_bayer2(l_fl_x1, l_fl_y1);
  return (((coax_t58 + coax_t59) + 0.5) / 16.0);
}
fn verse_style_flat_dither(l_fl_t: f32, l_fl_px: f32, l_fl_py: f32, l_fl_steps: f32) -> f32 {
  var coax_t60: f32;
  var coax_t61: f32;
  var l_fl_n: f32 = coax_p_st_max(l_fl_steps, 1.0);
  coax_t60 = (coax_p_st_sat(l_fl_t) * l_fl_n);
  coax_t61 = verse_style_bayer(l_fl_px, l_fl_py);
  return coax_p_st_sat((floor((coax_t60 + coax_t61)) / l_fl_n));
}
fn verse_style_flat_hem(l_fl_y: f32, l_fl_width: f32, l_fl_aa: f32) -> f32 {
  return (1.0 - verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 9.99999997e-07), l_fl_y));
}
fn coax_p_st_pick(l_fl_ch: f32, l_fl_r: f32, l_fl_g: f32, l_fl_b: f32) -> f32 {
  if ((l_fl_ch < 0.5))
  {
    return l_fl_r;
  }
  if ((l_fl_ch < 1.5))
  {
    return l_fl_g;
  }
  return l_fl_b;
}
fn verse_style_flat_srgb(l_fl_realm: f32, l_fl_slot: f32, l_fl_ch: f32) -> f32 {
  if ((l_fl_realm < 0.5))
  {
    if ((l_fl_slot < 0.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.400000006, 0.463, 0.689999998);
    }
    if ((l_fl_slot < 1.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.745000005, 0.70599997, 0.847000003);
    }
    if ((l_fl_slot < 2.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.442999989, 0.583999991, 0.537);
    }
    if ((l_fl_slot < 3.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.493999988, 0.630999982, 0.564999998);
    }
    if ((l_fl_slot < 4.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.875, 0.819999993, 0.709999979);
    }
    if ((l_fl_slot < 5.5))
    {
      return coax_p_st_pick(l_fl_ch, 1.0, 0.99000001, 0.970000029);
    }
    if ((l_fl_slot < 6.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.660000026, 0.660000026, 0.860000014);
    }
    if ((l_fl_slot < 7.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.600000024, 0.600000024, 0.819999993);
    }
    if ((l_fl_slot < 8.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.980000019, 0.959999979, 0.899999976);
    }
    if ((l_fl_slot < 9.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.203999996, 0.211999997, 0.340999991);
    }
    if ((l_fl_slot < 10.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.964999974, 0.936999977, 0.870999992);
    }
    if ((l_fl_slot < 11.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.270999998, 0.446999997, 0.419999987);
    }
    if ((l_fl_slot < 12.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.455000013, 0.620000005, 0.49000001);
    }
    if ((l_fl_slot < 13.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.579999983, 0.819999993, 0.800000012);
    }
    if ((l_fl_slot < 14.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.689999998, 0.545000017, 0.419999987);
    }
    if ((l_fl_slot < 15.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.980000019, 0.875, 0.579999983);
    }
    if ((l_fl_slot < 16.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.779999971, 0.899999976, 0.850000024);
    }
    if ((l_fl_slot < 17.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.776000023, 0.764999986, 0.866999984);
    }
    if ((l_fl_slot < 18.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.532999992, 0.532999992, 0.722000003);
    }
    return coax_p_st_pick(l_fl_ch, 0.600000024, 0.699999988, 0.620000005);
  }
  if ((l_fl_realm > 1.5))
  {
    if ((l_fl_slot < 0.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.626999974, 0.694000006, 0.827000022);
    }
    if ((l_fl_slot < 1.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.866999984, 0.851000011, 0.893999994);
    }
    if ((l_fl_slot < 2.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.658999979, 0.736999989, 0.537);
    }
    if ((l_fl_slot < 3.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.592000008, 0.677999973, 0.493999988);
    }
    if ((l_fl_slot < 4.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.889999986, 0.824000001, 0.736999989);
    }
    if ((l_fl_slot < 5.5))
    {
      return coax_p_st_pick(l_fl_ch, 1.0, 0.984000027, 0.953000009);
    }
    if ((l_fl_slot < 6.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.800000012, 0.796000004, 0.906000018);
    }
    if ((l_fl_slot < 7.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.783999979, 0.776000023, 0.910000026);
    }
    if ((l_fl_slot < 8.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.991999984, 0.984000027, 0.964999974);
    }
    if ((l_fl_slot < 9.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.165000007, 0.165000007, 0.246999994);
    }
    if ((l_fl_slot < 10.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.961000025, 0.949000001, 0.910000026);
    }
    if ((l_fl_slot < 11.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.545000017, 0.662999988, 0.513999999);
    }
    if ((l_fl_slot < 12.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.698000014, 0.791999996, 0.579999983);
    }
    if ((l_fl_slot < 13.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.564999998, 0.768999994, 0.791999996);
    }
    if ((l_fl_slot < 14.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.626999974, 0.532999992, 0.49000001);
    }
    if ((l_fl_slot < 15.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.957000017, 0.870999992, 0.596000016);
    }
    if ((l_fl_slot < 16.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.783999979, 0.913999975, 0.859000027);
    }
    if ((l_fl_slot < 17.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.702000022, 0.694000006, 0.783999979);
    }
    if ((l_fl_slot < 18.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.851000011, 0.757000029, 0.882000029);
    }
    return coax_p_st_pick(l_fl_ch, 0.885999978, 0.70599997, 0.768999994);
  }
  if ((l_fl_slot < 0.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.662999988, 0.827000022, 0.815999985);
  }
  if ((l_fl_slot < 1.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.910000026, 0.936999977, 0.815999985);
  }
  if ((l_fl_slot < 2.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.624000013, 0.709999979, 0.289999992);
  }
  if ((l_fl_slot < 3.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.501999974, 0.600000024, 0.243000001);
  }
  if ((l_fl_slot < 4.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.851000011, 0.753000021, 0.541000009);
  }
  if ((l_fl_slot < 5.5))
  {
    return coax_p_st_pick(l_fl_ch, 1.0, 0.980000019, 0.920000017);
  }
  if ((l_fl_slot < 6.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.699999988, 0.699999988, 0.839999974);
  }
  if ((l_fl_slot < 7.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.75999999, 0.75, 0.879999995);
  }
  if ((l_fl_slot < 8.5))
  {
    return coax_p_st_pick(l_fl_ch, 1.0, 0.964999974, 0.878000021);
  }
  if ((l_fl_slot < 9.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.114, 0.122000001, 0.165000007);
  }
  if ((l_fl_slot < 10.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.949000001, 0.917999983, 0.824000001);
  }
  if ((l_fl_slot < 11.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.435000002, 0.541000009, 0.226999998);
  }
  if ((l_fl_slot < 12.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.709999979, 0.811999977, 0.352999985);
  }
  if ((l_fl_slot < 13.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.56099999, 0.811999977, 0.753000021);
  }
  if ((l_fl_slot < 14.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.658999979, 0.528999984, 0.352999985);
  }
  if ((l_fl_slot < 15.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.878000021, 0.518000007, 0.226999998);
  }
  if ((l_fl_slot < 16.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.768999994, 0.925000012, 0.838999987);
  }
  if ((l_fl_slot < 17.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.662999988, 0.638999999, 0.768999994);
  }
  if ((l_fl_slot < 18.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.787999988, 0.702000022, 0.878000021);
  }
  return coax_p_st_pick(l_fl_ch, 0.654999971, 0.709999979, 0.603999972);
}
fn verse_style_flat_colour(l_fl_realm: f32, l_fl_slot: f32, l_fl_ch: f32) -> f32 {
  return pow(verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 2.20000005);
}
fn verse_style_flat_time(l_fl_realm: f32, l_fl_ch: f32, l_fl_dusk: f32, l_fl_night: f32) -> f32 {
  var coax_t62: f32;
  var coax_t63: f32;
  var coax_t64: f32;
  var coax_t65: f32;
  if ((l_fl_realm < 0.5))
  {
    return 1.0;
  }
  coax_t62 = (coax_p_st_pick(l_fl_ch, 1.0, 0.800000012, 0.680000007) - 1.0);
  coax_t63 = coax_p_st_sat(l_fl_dusk);
  var l_fl_warm: f32 = (1.0 + (coax_t62 * coax_t63));
  coax_t64 = (coax_p_st_pick(l_fl_ch, 0.200000003, 0.25, 0.439999998) - l_fl_warm);
  coax_t65 = coax_p_st_sat(l_fl_night);
  return (l_fl_warm + (coax_t64 * coax_t65));
}
// ---- end generated
