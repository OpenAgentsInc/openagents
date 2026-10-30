// Orbit League's two passes (orbitleague.coax): bg_fs traces the whole
// world per pixel (the arena's floor and glass, the goal frames and nets,
// the ball, both roadsters, the boost flames, and the space round the
// station: stars, a planet and the sun), in the flat style with ink lines
// (docs/verse/art-styles.md), with the HUD's plates over it; fx draws the
// HUD's strokes and text. Every place and size comes from orbitleague.coax.
struct Car { p: vec4f, x: vec4f, y: vec4f, z: vec4f }   // place (w boost), right (w team), up, forward (w steer)
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
  misc: vec4f,
  panel: array<vec4f, 32>,
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
const SUN = vec3f(0.48, 0.42, 0.77);   // normalised in sun()
fn sun() -> vec3f { return normalize(SUN); }
fn team(t: f32) -> vec3f { return select(BLUE, MAGENTA, t > 0.5); }
// Flat light: three bands of the sun, and a sky fill.
fn flat_light(n: vec3f) -> f32 {
  let l = max(0.0, dot(n, sun()));
  let band = select(select(0.55, 0.8, l > 0.25), 1.0, l > 0.6);
  return 0.35 + 0.65 * band * (0.7 + 0.3 * max(0.0, n.y));
}

// ---- space
const PLANET_C = vec3f(-2600.0, -1500.0, 2200.0);
const PLANET_R = 1100.0;
fn space(d: vec3f) -> vec3f {
  var c = mix(vec3f(0.006, 0.008, 0.02), vec3f(0.03, 0.02, 0.06), clamp(0.5 + 0.5 * d.y, 0.0, 1.0));
  // stars: one at most in a cell of the direction's grid
  let g = d * 180.0;
  let cell = floor(g);
  let h = hash3(cell);
  if (h > 0.965) {
    let at = cell + vec3f(hash3(cell + 1.3), hash3(cell + 7.1), hash3(cell + 3.7));
    let r = length(g - at);
    let tw = select(0.8 + 0.2 * sin(u.time * (0.5 + h * 3.0) + h * 50.0), 1.0, u.still > 0.5);
    c += vec3f(0.9, 0.95, 1.0) * (1.0 - smoothstep(0.05, 0.28, r)) * select(0.5, 1.0, h > 0.993) * tw;
  }
  // the sun
  let sd = dot(d, sun());
  c += vec3f(1.0, 0.9, 0.7) * (smoothstep(0.9995, 0.9998, sd) * 3.0 + pow(max(sd, 0.0), 400.0) * 0.6);
  // the planet: a big blue world below and ahead, its night side dark, a thin air glow
  let m = -PLANET_C;
  let b = dot(m, d);
  let q = dot(m, m) - PLANET_R * PLANET_R;
  let disc = b * b - q;
  if (disc > 0.0) {
    let t = -b - sqrt(disc);
    if (t > 0.0) {
      let n = normalize(d * t - PLANET_C);
      let l = dot(n, sun());
      let lat = n.y * 6.0 + sin(n.x * 9.0 + n.z * 4.0) * 0.8;
      var ground = mix(vec3f(0.06, 0.22, 0.55), vec3f(0.12, 0.38, 0.30), step(0.55, fract(lat * 0.37 + sin(n.z * 13.0) * 0.2)));
      ground = mix(ground, vec3f(0.92, 0.95, 1.0), step(0.78, fract(n.x * 5.0 + n.y * 11.0 + sin(n.z * 20.0) * 0.4)) * 0.8);
      let band = select(select(0.08, 0.55, l > 0.0), 1.0, l > 0.35);
      c = ground * band + vec3f(0.3, 0.55, 1.0) * pow(1.0 - max(0.0, dot(n, -d)), 3.0) * 0.6 * max(l + 0.3, 0.0);
    }
  }
  // the air's halo round the limb
  let miss = length(m - d * b) - PLANET_R;
  if (b < 0.0 && miss > 0.0) { c += vec3f(0.3, 0.55, 1.0) * exp(-miss / 40.0) * 0.5; }
  return c;
}

// ---- shapes
struct Hit { t: f32, n: vec3f, col: vec3f, ink: f32 }
fn miss() -> Hit { return Hit(1e9, vec3f(0.0, 1.0, 0.0), vec3f(0.0), 0.0); }

// A box (centre c, half h) in a frame; o and d already in it. Its normal in that frame, and ink near its edges.
fn box_hit(o: vec3f, d: vec3f, c: vec3f, h: vec3f, col: vec3f) -> Hit {
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
  return Hit(tnear, n, col, 1.0 - smoothstep(0.025, 0.05, mid));
}
// A wheel: a cylinder along x (centre c, radius r, half width w).
fn wheel_hit(o: vec3f, d: vec3f, c: vec3f, r: f32, w: f32) -> Hit {
  let q = o - c;
  let a = d.y * d.y + d.z * d.z;
  let b = q.y * d.y + q.z * d.z;
  let cc = q.y * q.y + q.z * q.z - r * r;
  var best = miss();
  let disc = b * b - a * cc;
  if (disc >= 0.0 && a > 1e-6) {
    let t = (-b - sqrt(disc)) / a;
    let p = q + d * t;
    if (t > 0.0 && abs(p.x) <= w) { best = Hit(t, normalize(vec3f(0.0, p.y, p.z)), vec3f(0.07, 0.07, 0.08), 1.0 - smoothstep(0.03, 0.06, w - abs(p.x))); }
  }
  // the hubs
  for (var s = -1.0; s <= 1.0; s += 2.0) {
    if (abs(d.x) > 1e-6) {
      let t = (s * w - q.x) / d.x;
      let p = q + d * t;
      let rr = length(p.yz);
      if (t > 0.0 && t < best.t && rr <= r) {
        let rim = select(vec3f(0.07, 0.07, 0.08), vec3f(0.75, 0.78, 0.82), rr < r * 0.62);
        best = Hit(t, vec3f(s, 0.0, 0.0), rim, 1.0 - smoothstep(0.02, 0.04, abs(rr - r * 0.62)) + 1.0 - smoothstep(0.02, 0.04, r - rr));
      }
    }
  }
  return best;
}
fn closer(a: Hit, b: Hit) -> Hit { if (b.t < a.t) { return b; } return a; }

// A roadster of our own: a low wedge body in its team's colour, a dark
// screen and cabin, a white stripe, a rear wing and four wheels.
fn car_hit(ro: vec3f, rd: vec3f, c: Car) -> Hit {
  let q = ro - c.p.xyz;
  let o = vec3f(dot(q, c.x.xyz), dot(q, c.y.xyz), dot(q, c.z.xyz));
  let d = vec3f(dot(rd, c.x.xyz), dot(rd, c.y.xyz), dot(rd, c.z.xyz));
  // the bounding sphere first
  let bq = dot(o, d);
  if (dot(o, o) - bq * bq > 6.5 && bq > 0.0) { return miss(); }
  let paint = team(c.x.w);
  var h = box_hit(o, d, vec3f(0.0, -0.12, 0.0), vec3f(0.95, 0.22, 2.1), paint);
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.14, -0.75), vec3f(0.9, 0.12, 1.15), paint));
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.33, -0.15), vec3f(0.72, 0.15, 0.5), vec3f(0.04, 0.06, 0.1)));
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.11, 1.4), vec3f(0.22, 0.015, 0.7), vec3f(0.95, 0.96, 1.0)));
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.42, -1.95), vec3f(0.92, 0.035, 0.16), vec3f(0.1, 0.1, 0.12)));
  h = closer(h, box_hit(o, d, vec3f(0.0, 0.0, 2.08), vec3f(0.7, 0.06, 0.04), vec3f(1.0, 0.95, 0.8)));
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
    var w = wheel_hit(wo, wd, vec3f(0.0), 0.4, 0.14);
    if (w.t < h.t && i < 2) {
      let cs = cos(steer); let sn = sin(steer);
      w.n = vec3f(cs * w.n.x - sn * w.n.z, w.n.y, sn * w.n.x + cs * w.n.z);
    }
    h = closer(h, w);
  }
  if (h.t < 1e8) { h.n = h.n.x * c.x.xyz + h.n.y * c.y.xyz + h.n.z * c.z.xyz; }
  return h;
}
// The boost: a glow along a short line behind the car.
fn flame(ro: vec3f, rd: vec3f, c: Car, tmax: f32) -> vec3f {
  if (c.p.w < 0.5) { return vec3f(0.0); }
  let a = c.p.xyz - c.z.xyz * 2.15;
  let b = c.p.xyz - c.z.xyz * 3.6;
  // the closest approach of the ray and the segment
  let ab = b - a;
  let w0 = ro - a;
  let aa = dot(rd, rd); let bb = dot(rd, ab); let cc = dot(ab, ab); let dd = dot(rd, w0); let ee = dot(ab, w0);
  let den = aa * cc - bb * bb;
  var s = 0.0; var t = 0.0;
  if (den > 1e-6) { s = clamp((bb * ee - cc * dd) / den, 0.0, tmax); t = clamp((aa * ee - bb * dd) / den, 0.0, 1.0); }
  let dist = length(ro + rd * s - (a + ab * t));
  let flick = select(0.85 + 0.15 * sin(u.time * 60.0), 1.0, u.still > 0.5);
  let r = 0.28 * (1.0 - t * 0.7);
  return mix(vec3f(0.85, 0.95, 1.0), vec3f(0.3, 0.55, 1.0), t) * exp(-dist * dist / (r * r)) * 1.6 * flick;
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
  // its seams in its own frame: three great circles, and a dot at each pole
  let l = qrot(vec4f(-u.ballq.xyz, u.ballq.w), n);
  let a = abs(l);
  let seam = min(min(a.x, a.y), a.z);
  let pole = max(max(a.x, a.y), a.z);
  var col = vec3f(0.93, 0.94, 0.96);
  if (pole > 0.93) { col = vec3f(0.12, 0.14, 0.2); }
  return Hit(t, n, col, 1.0 - smoothstep(0.035, 0.06, seam));
}

// ---- the arena
fn floor_col(p: vec3f) -> vec3f {
  let hx = u.arena.x; let hz = u.arena.y; let gw = u.arena.w;
  var c = select(vec3f(0.10, 0.12, 0.17), vec3f(0.14, 0.11, 0.10), p.z > 0.0);
  // the deck's plates, five metres a side
  let g = abs(fract(p.xz / 5.0 + 0.5) - 0.5) * 5.0;
  c *= 1.0 - 0.18 * (1.0 - smoothstep(0.03, 0.07, min(g.x, g.y)));
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
  c += side * (1.0 - smoothstep(0.0, 0.6, edge)) * 0.5 * step(gw, abs(p.x) + step(abs(p.z), hz) * 99.0);
  return c;
}
// Shadows straight down from the sun's side: soft discs under the ball and the cars.
fn shade(p: vec3f) -> f32 {
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

// The glass the ray leaves the arena through, and the nets it passes: their
// colour over what is behind, and where they are.
struct Glass { t: f32, col: vec3f, a: f32 }
fn glass(ro: vec3f, rd: vec3f, tmax: f32) -> Glass {
  let hx = u.arena.x; let hz = u.arena.y; let hh = u.arena.z; let gw = u.arena.w; let gh = u.goal.x; let gd = u.goal.y;
  var out = Glass(1e9, vec3f(0.0), 0.0);
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
    // glass in a frame of struts every 5 metres
    var uv = p.xz;
    if (n.x != 0.0) { uv = p.zy; } else if (n.z != 0.0) { uv = p.xy; }
    let g = abs(fract(uv / 5.0 + 0.5) - 0.5) * 5.0;
    let strut = 1.0 - smoothstep(0.05, 0.12, min(g.x, g.y));
    // the frame round each pane's edge (not the face's own axis)
    var e = min(hx - abs(p.x), hz - abs(p.z));
    if (n.x != 0.0) { e = min(hz - abs(p.z), min(p.y, hh - p.y)); }
    if (n.z != 0.0) { e = min(hx - abs(p.x), min(p.y, hh - p.y)); }
    let rim = 1.0 - smoothstep(0.0, 0.25, e);
    let side = select(BLUE, MAGENTA, p.z > 0.0);
    out = Glass(t, mix(vec3f(0.35, 0.5, 0.7), side, 0.4), clamp(0.035 + strut * 0.28 + rim * 0.55, 0.0, 1.0));
    if (n.z != 0.0) {
      // the end wall reads as its team's
      out.col = mix(out.col, side, 0.4); out.a = clamp(out.a + 0.03, 0.0, 1.0);
    }
    return out;
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
  return Glass(tn, col * 1.2, clamp(0.12 + mesh * 0.65, 0.0, 1.0));
}

// The goal frames: posts and a crossbar at each mouth, lit in the team's colour.
fn frames(ro: vec3f, rd: vec3f) -> Hit {
  let hz = u.arena.y; let gw = u.arena.w; let gh = u.goal.x;
  var h = miss();
  for (var e = -1.0; e <= 1.0; e += 2.0) {
    let col = select(BLUE, MAGENTA, e > 0.0) * 1.3;
    let z = e * (hz + 0.15);
    h = closer(h, box_hit(ro, rd, vec3f(-gw - 0.2, gh * 0.5, z), vec3f(0.2, gh * 0.5 + 0.2, 0.2), col));
    h = closer(h, box_hit(ro, rd, vec3f(gw + 0.2, gh * 0.5, z), vec3f(0.2, gh * 0.5 + 0.2, 0.2), col));
    h = closer(h, box_hit(ro, rd, vec3f(0.0, gh + 0.2, z), vec3f(gw + 0.4, 0.2, 0.2), col));
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
    if (on) { h = Hit(t, vec3f(0.0, 1.0, 0.0), floor_col(p), 0.0); }
  }
  h = closer(h, ball_hit(ro, rd));
  h = closer(h, car_hit(ro, rd, u.cars[0]));
  h = closer(h, car_hit(ro, rd, u.cars[1]));
  h = closer(h, frames(ro, rd));
  var c = vec3f(0.0);
  if (h.t < 1e8) {
    let p = ro + rd * h.t;
    var light = flat_light(h.n);
    if (h.n.y > 0.99 && p.y < 0.01) { light = 0.85 * shade(p); }
    c = h.col * light;
    // a rim of light on the ball and the cars from the planet below
    c += vec3f(0.15, 0.3, 0.6) * pow(1.0 - abs(dot(h.n, -rd)), 3.0) * 0.5 * step(0.02, p.y);
    c = mix(c, vec3f(0.01, 0.015, 0.03), clamp(h.ink, 0.0, 1.0) * 0.85);
    // far things fade a little into the station's haze
    c = mix(c, vec3f(0.05, 0.07, 0.12), clamp(h.t / 220.0, 0.0, 0.5));
  } else {
    c = space(rd);
  }
  // glass and nets in front of it
  let gl = glass(ro, rd, h.t);
  if (gl.a > 0.0) { c = mix(c, gl.col, gl.a); }
  // the boost flames
  c += flame(ro, rd, u.cars[0], h.t) + flame(ro, rd, u.cars[1], h.t);
  // zero-g: a cold tint; a goal: its team's flash
  c = mix(c, c * vec3f(0.85, 0.95, 1.15), u.zerog * 0.5);
  c += team(u.fwd.w) * u.eye.w * 0.35;
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
      return coax_p_st_pick(l_fl_ch, 0.36500001, 0.442999989, 0.575999975);
    }
    if ((l_fl_slot < 1.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.540000021, 0.603999972, 0.698000014);
    }
    if ((l_fl_slot < 2.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.254999995, 0.317999989, 0.419999987);
    }
    if ((l_fl_slot < 3.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.289999992, 0.356999993, 0.455000013);
    }
    if ((l_fl_slot < 4.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.699999988, 0.699999988, 0.689999998);
    }
    if ((l_fl_slot < 5.5))
    {
      return coax_p_st_pick(l_fl_ch, 1.0, 1.0, 0.980000019);
    }
    if ((l_fl_slot < 6.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.600000024, 0.649999976, 0.800000012);
    }
    if ((l_fl_slot < 7.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.639999986, 0.680000007, 0.819999993);
    }
    if ((l_fl_slot < 8.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.851000011, 0.70599997, 0.352999985);
    }
    if ((l_fl_slot < 9.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.0820000023, 0.0939999968, 0.122000001);
    }
    if ((l_fl_slot < 10.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.925000012, 0.893999994, 0.811999977);
    }
    if ((l_fl_slot < 11.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.118000001, 0.157000005, 0.200000003);
    }
    if ((l_fl_slot < 12.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.165000007, 0.216000006, 0.270999998);
    }
    if ((l_fl_slot < 13.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.497999996, 0.583999991, 0.702000022);
    }
    if ((l_fl_slot < 14.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.165000007, 0.141000003, 0.152999997);
    }
    if ((l_fl_slot < 15.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.851000011, 0.70599997, 0.352999985);
    }
    if ((l_fl_slot < 16.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.779999971, 0.800000012, 0.779999971);
    }
    if ((l_fl_slot < 17.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.787999988, 0.768999994, 0.713999987);
    }
    if ((l_fl_slot < 18.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.419999987, 0.460000008, 0.600000024);
    }
    return coax_p_st_pick(l_fl_ch, 0.379999995, 0.430000007, 0.469999999);
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
