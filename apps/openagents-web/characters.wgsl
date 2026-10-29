// The characters realm (characters.coax): a night garden and the figures
// standing in it, each a few dozen round cones (verse/figure.coax) melted
// into one body and found by marching each pixel's ray through it, then the
// HUD's strokes and plates. A figure is lit (its coat under the moon) or a
// soft silhouette: black, its edge a fuzzy fringe that thins into the night,
// lit by nothing but a rim of moonlight.

struct U {
  size: vec2f, time: f32, fade: f32,
  figures: f32, panels: f32, dpr: f32, selected: f32,
  eye: vec4f,      // the eye, and the tangent of half the view's height
  right: vec4f, up: vec4f, ahead: vec4f,
  moon: vec4f,     // toward the moon
  pad: vec4f,
  panel: array<vec4f, 56>,   // per plate: x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
  fig: array<vec4f, 30>,     // per figure (characters.coax FIG_FLOATS): bound; coat, look; marks, pattern; x, y, z, yaw; bones, blend, size, glow
  bone: array<vec4f, 384>,   // per figure 32 bones: end a, radius + 1000 × region; end b, radius
}
@group(0) @binding(0) var<uniform> u: U;

const BONES: i32 = 32;
const STEPS: i32 = 56;

fn tone(c: vec3f) -> vec3f { return vec3f(1.0) - exp(-c * 1.35); }

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
  for (var k = 0; k < 28; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}

fn hash3(p: vec3f) -> f32 {
  let q = fract(p * vec3f(0.1031, 0.1030, 0.0973));
  let r = q + dot(q, q.yxz + 33.33);
  return fract((r.x + r.y) * r.z);
}
fn noise3(p: vec3f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let w = f * f * (3.0 - 2.0 * f);
  let a = mix(mix(hash3(i), hash3(i + vec3f(1, 0, 0)), w.x), mix(hash3(i + vec3f(0, 1, 0)), hash3(i + vec3f(1, 1, 0)), w.x), w.y);
  let b = mix(mix(hash3(i + vec3f(0, 0, 1)), hash3(i + vec3f(1, 0, 1)), w.x), mix(hash3(i + vec3f(0, 1, 1)), hash3(i + vec3f(1, 1, 1)), w.x), w.y);
  return mix(a, b, w.z);
}

// One bone: a round cone from a to b, its radius going from ra to rb.
fn cone(p: vec3f, a: vec4f, b: vec4f) -> f32 {
  let ab = b.xyz - a.xyz;
  let ap = p - a.xyz;
  let h = clamp(dot(ap, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
  let ra = a.w - floor(a.w / 1000.0) * 1000.0;
  return length(ap - ab * h) - mix(ra, b.w, h);
}
fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = max(k - abs(a - b), 0.0) / k;
  return min(a, b) - h * h * k * 0.25;
}
// The figure's body: its bones melted together.
fn body(f: i32, p: vec3f) -> f32 {
  let info = u.fig[f * 5 + 4];
  let n = i32(info.x);
  let k = max(info.y, 0.001);
  var d = 1e5;
  for (var i = 0; i < BONES; i++) {
    if (i >= n) { break; }
    let o = (f * BONES + i) * 2;
    d = smin(d, cone(p, u.bone[o], u.bone[o + 1]), k);
  }
  return d;
}
// Which part of the body p is nearest (figure.coax REGION_*).
fn region(f: i32, p: vec3f) -> f32 {
  let n = i32(u.fig[f * 5 + 4].x);
  var d = 1e5;
  var r = 0.0;
  for (var i = 0; i < BONES; i++) {
    if (i >= n) { break; }
    let o = (f * BONES + i) * 2;
    let e = cone(p, u.bone[o], u.bone[o + 1]);
    if (e < d) { d = e; r = floor(u.bone[o].w / 1000.0); }
  }
  return r;
}
fn normal(f: i32, p: vec3f) -> vec3f {
  let e = 0.002;
  let a = vec3f(1.0, -1.0, -1.0); let b = vec3f(-1.0, -1.0, 1.0); let c = vec3f(-1.0, 1.0, -1.0); let d = vec3f(1.0, 1.0, 1.0);
  return normalize(a * body(f, p + a * e) + b * body(f, p + b * e) + c * body(f, p + c * e) + d * body(f, p + d * e));
}

// A coat's colour where p is on figure f, in its own frame.
fn coat(f: i32, p: vec3f, reg: f32) -> vec3f {
  let base = u.fig[f * 5 + 1].rgb;
  let marks = u.fig[f * 5 + 2].rgb;
  let pattern = u.fig[f * 5 + 2].w;
  let at = u.fig[f * 5 + 3];
  let size = max(u.fig[f * 5 + 4].z, 0.1);
  let c = cos(at.w); let s = sin(at.w);
  let w = p - at.xyz;
  let l = vec3f(c * w.x - s * w.z, w.y, s * w.x + c * w.z) / size;
  if (reg > 6.5 && reg < 7.5) { return vec3f(0.012, 0.01, 0.01); }
  if (reg > 7.5) { return vec3f(0.02, 0.016, 0.016); }
  var m = 0.0;
  if (pattern > 0.5 && pattern < 1.5) {
    m = smoothstep(0.52, 0.58, noise3(l * 5.0 + vec3f(f32(f) * 7.0)));
  } else if (pattern > 1.5 && pattern < 2.5) {
    m = select(0.0, 1.0, reg > 1.5 && reg < 4.5) + select(0.0, smoothstep(0.3, 0.12, l.y), reg > 5.5 && reg < 6.5);
  } else if (pattern > 2.5 && pattern < 3.5) {
    m = select(0.0, smoothstep(0.35, 0.6, sin(l.z * 32.0 + noise3(l * 6.0) * 4.0)), reg < 0.5 || (reg > 3.5 && reg < 5.5));
  } else if (pattern > 3.5) {
    m = select(0.0, smoothstep(0.17, 0.12, l.y), reg > 5.5 && reg < 6.5);
  }
  if (reg > 1.5 && reg < 2.5 && pattern < 0.5) { m = 0.35; }
  return mix(base, marks, clamp(m, 0.0, 1.0));
}

// What the ray hits of figure f: how far, and how much of the pixel the
// figure covers with its colour (its soft fringe, if it has one).
struct Hit { t: f32, colour: vec3f, cover: f32 }
fn figure(f: i32, o: vec3f, d: vec3f, pix: f32) -> Hit {
  var h: Hit; h.t = 1e9; h.cover = 0.0;
  let b = u.fig[f * 5];
  let soft = u.fig[f * 5 + 1].w > 0.5;
  let size = max(u.fig[f * 5 + 4].z, 0.1);
  let fuzz = select(0.0, 0.14 * size, soft);
  let oc = o - b.xyz;
  let r = b.w + fuzz;
  let bb = dot(oc, d);
  let disc = bb * bb - dot(oc, oc) + r * r;
  if (disc <= 0.0) { return h; }
  let q = sqrt(disc);
  var t = max(-bb - q, 0.0);
  let end = -bb + q;
  if (end <= 0.0) { return h; }
  var least = 1e5; var at = t;
  var hit = false;
  for (var i = 0; i < STEPS; i++) {
    let p = o + d * t;
    let e = body(f, p);
    let edge = e - pix * t;
    if (edge < least) { least = edge; at = t; }
    if (e < 0.0006 * t + 0.0004) { hit = true; break; }
    t += max(e * 0.9, 0.004);
    if (t > end) { break; }
  }
  let moon = normalize(u.moon.xyz);
  if (soft) {
    // How deep the ray goes: how near it passed (outside), or how square
    // to the surface it met it (inside: grazing is the edge).
    var depth = least;
    var rim = 0.0;
    let reach = fuzz + pix * at;
    if (hit) {
      let n = normal(f, o + d * t);
      let facing = abs(dot(n, d));
      depth = -2.0 * reach * facing;
      rim = pow(1.0 - facing, 2.5) * (0.3 + 0.7 * max(0.0, dot(n, moon) + 0.4));
    }
    let p = o + d * select(at, t, hit);
    let wisp = (noise3(p * 14.0 + vec3f(0.0, u.time * 0.6, u.time * 0.2)) - 0.5) * 0.9 * fuzz;
    let cover = 1.0 - smoothstep(-reach, reach, depth + wisp);
    if (cover <= 0.002) { return h; }
    rim = max(rim, 0.5 * (1.0 - cover) * cover);
    h.t = select(at, t, hit);
    h.colour = vec3f(0.003, 0.004, 0.007) + vec3f(0.42, 0.52, 0.85) * rim * 0.5;
    h.cover = cover * 0.98;
    return h;
  }
  if (!hit) {
    let cover = 1.0 - smoothstep(0.0, pix * at * 1.5, least);
    if (cover <= 0.001) { return h; }
    t = at;
  }
  let p = o + d * t;
  let n = normal(f, p);
  let reg = region(f, p);
  let albedo = coat(f, p, reg);
  let sky = 0.06 + 0.05 * n.y;
  let key = max(0.0, dot(n, moon));
  let rim = pow(1.0 - abs(dot(n, d)), 3.0) * 0.4;
  let glow = u.fig[f * 5 + 4].w;
  let light = vec3f(0.1, 0.11, 0.16) * (sky * 8.0) + vec3f(0.56, 0.64, 0.86) * key * 1.5 + vec3f(0.45, 0.55, 0.85) * rim + vec3f(glow);
  var c = albedo * light;
  if (reg > 6.5 && reg < 8.5) {
    let spec = pow(max(0.0, dot(reflect(d, n), moon)), 24.0);
    c += vec3f(0.6, 0.65, 0.8) * spec;
  }
  h.t = t;
  h.colour = c;
  h.cover = select(1.0 - smoothstep(0.0, pix * t * 1.5, least), 1.0, hit);
  return h;
}

// The ground: dark grass, a pale gravel ring the walkers follow, and under
// each figure the soft dark of its contact shadow.
fn ground(p: vec3f, dist: f32) -> vec3f {
  let r = length(p.xz);
  let grass = vec3f(0.045, 0.07, 0.05) * (0.75 + 0.5 * noise3(vec3f(p.xz * 3.0, 0.0)));
  let ring = smoothstep(0.3, 0.22, abs(r - 2.2)) * (0.7 + 0.5 * noise3(vec3f(p.xz * 14.0, 1.0)));
  var c = mix(grass, vec3f(0.1, 0.096, 0.088), ring);
  c = c * (0.9 + vec3f(0.56, 0.64, 0.86) * max(0.0, u.moon.y) * 1.2);
  var shade = 1.0;
  let n = i32(u.figures);
  for (var f = 0; f < 6; f++) {
    if (f >= n) { break; }
    let b = u.fig[f * 5];
    let size = max(u.fig[f * 5 + 4].z, 0.1);
    if (length(p.xz - b.xz) > b.w + 0.4 * size) { continue; }
    let e = body(f, p + vec3f(0.0, 0.01, 0.0));
    shade = min(shade, mix(0.25, 1.0, smoothstep(0.0, 0.38 * size, e)));
  }
  c *= shade;
  let fog = 1.0 - exp(-dist * dist * 0.0009);
  return mix(c, vec3f(0.06, 0.07, 0.11), fog);
}
fn sky(d: vec3f) -> vec3f {
  let up = clamp(d.y, 0.0, 1.0);
  var c = mix(vec3f(0.06, 0.07, 0.11), vec3f(0.01, 0.012, 0.035), sqrt(up));
  let grid = d * 90.0;
  let star = hash3(floor(grid));
  if (star > 0.993 && d.y > 0.04) {
    let dot = smoothstep(0.16, 0.0, length(fract(grid) - vec3f(0.5)));
    c += vec3f(0.55, 0.6, 0.75) * dot * (star - 0.993) * 120.0 * (0.7 + 0.3 * sin(u.time * 1.3 + star * 90.0));
  }
  let moon = normalize(u.moon.xyz);
  let m = dot(d, moon);
  c += vec3f(0.8, 0.85, 1.0) * smoothstep(0.9993, 0.9996, m) * 1.6 + vec3f(0.12, 0.14, 0.22) * pow(max(m, 0.0), 60.0);
  return c;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let ndc = vec2f(pos.x / u.size.x * 2.0 - 1.0, 1.0 - pos.y / u.size.y * 2.0);
  let aspect = u.size.x / max(u.size.y, 1.0);
  let tv = u.eye.w;
  let d = normalize(u.ahead.xyz + u.right.xyz * ndc.x * aspect * tv + u.up.xyz * ndc.y * tv);
  let o = u.eye.xyz;
  let pix = 2.0 * tv / max(u.size.y, 1.0);
  var c = sky(d);
  var far = 1e9;
  if (d.y < -0.0001) {
    let t = -o.y / d.y;
    far = t;
    c = mix(ground(o + d * t, t), c, smoothstep(55.0, 80.0, t));
  }
  var hits: array<Hit, 6>;
  var count = 0;
  let n = i32(u.figures);
  for (var f = 0; f < 6; f++) {
    if (f >= n) { break; }
    let h = figure(f, o, d, pix);
    if (h.cover > 0.0 && h.t < far) { hits[count] = h; count++; }
  }
  for (var i = 1; i < count; i++) {
    var j = i;
    while (j > 0 && hits[j - 1].t < hits[j].t) { let s = hits[j]; hits[j] = hits[j - 1]; hits[j - 1] = s; j--; }
  }
  for (var i = 0; i < count; i++) { c = mix(c, hits[i].colour, hits[i].cover); }
  return vec4f(panels(pos.xy, tone(c) * u.fade), 1.0);
}

// ---- the HUD: one quad per stroke (characters.coax lit)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,
  @location(1) shape: vec4f,
  @location(2) tint: vec4f,
  @location(3) extra: vec4f,
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  o.extra = d;
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
