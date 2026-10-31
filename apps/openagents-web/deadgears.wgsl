struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, horizon: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
  sun: vec4f,
  sunc: vec4f,
  sky: vec4f,
  gnd: vec4f,
  zen: vec4f,
  hor: vec4f,
  fog: vec4f,
  info: vec4f,
  light: array<vec4f, 80>,
  shadow: array<vec4f, 24>,
  occ: array<vec4f, 20>,
  plate: array<vec4f, 32>,
}
@group(0) @binding(0) var<uniform> u: U;

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 4.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.y >= 2.0;
  let fill = select(vec3f(0.025, 0.032, 0.042), vec3f(0.07, 0.09, 0.11), over);
  let line = select(vec3f(0.36, 0.42, 0.5), vec3f(0.82, 0.88, 0.96), over || s.y >= 1.0);
  let w = 1.0 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1) * 0.85);
}
fn plates(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
    if (f32(k) >= u.plates) { break; }
    c = plate(p, c, u.plate[k * 2], u.plate[k * 2 + 1]);
  }
  return c;
}
fn tone(c: vec3f) -> vec3f {
  let x = max(c, vec3f(0.0));
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}
fn encode(c: vec3f) -> vec3f { return pow(clamp(c, vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2)); }
fn hurt(p: vec2f, c: vec3f) -> vec3f {
  if (u.info.w <= 0.0) { return c; }
  let q = (p / u.size - 0.5) * 2.0;
  let edge = smoothstep(0.55, 1.35, length(q));
  return mix(c, vec3f(0.55, 0.04, 0.03), edge * u.info.w * 0.6);
}

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let up = clamp((u.horizon - pos.y) / max(u.size.y, 1.0) * 1.8, 0.0, 1.0);
  var sky = mix(u.hor.rgb, u.zen.rgb, pow(up, 0.6));
  sky = sky * sky * 1.4;
  let cloud = noise(vec2f(pos.x / max(u.size.y, 1.0) * 3.0 + u.time * 0.01, up * 7.0)) * noise(vec2f(pos.x / max(u.size.y, 1.0) * 9.0, up * 19.0));
  sky = mix(sky, u.hor.rgb * 1.1, cloud * 0.35 * (1.0 - up * 0.5));
  return vec4f(plates(pos.xy, encode(tone(sky * 0.9))), 1.0);
}

struct Obj { r0: vec4f, r1: vec4f, r2: vec4f, colour: vec4f }
@group(1) @binding(0) var<storage, read> objects: array<Obj>;

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
  @location(3) glow: f32,
}

fn quad(v: u32) -> vec2f {
  var q = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  return q[v];
}

// The figures (deadgears.coax ROUNDS, the first objects): each a bone of a
// verse/figure.coax rig, a round cone from end a (r0: its place, its radius)
// to end b (r1), as Hecate's Cemetery draws its figures' bones: two half
// balls and the side between, eight round, two rings to a half
// (ROUND_VERTICES, 240).
struct Vert { p: vec3f, n: vec3f }
const ROUNDS: u32 = 96u;
const AROUND: u32 = 8u;
const RINGS: u32 = 2u;
fn round_vertex(m: Obj, id: u32) -> Vert {
  let q = id / 6u;
  let c = quad(id % 6u);
  let row = q / AROUND + u32(c.y);
  let k = f32(q % AROUND + u32(c.x)) / f32(AROUND) * 6.2831853;
  let a = m.r0.xyz; let b = m.r1.xyz;
  let axis = normalize(b - a + vec3f(0.0, 0.00001, 0.0));
  let side = normalize(cross(axis, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(axis.y) < 0.9)));
  let other = cross(axis, side);
  let out = side * cos(k) + other * sin(k);
  let top = row > RINGS;
  let step = f32(select(row, row - RINGS - 1u, top)) / f32(RINGS) * 1.5707963;
  let phi = select(step - 1.5707963, step, top);
  let n = out * cos(phi) + axis * sin(phi);
  let r = select(m.r0.w, m.r1.w, top);
  return Vert(select(a, b, top) + n * r, n);
}
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  var o: S;
  let m = objects[instance];
  if (instance < ROUNDS) {
    let v = round_vertex(m, id);
    o.pos = vec4f(dot(u.r0, vec4f(v.p, 1.0)), dot(u.r1, vec4f(v.p, 1.0)), dot(u.r2, vec4f(v.p, 1.0)), dot(u.r3, vec4f(v.p, 1.0)));
    o.world = v.p;
    o.normal = v.n;
    o.tint = pow(max(m.colour.rgb, vec3f(0.0)), vec3f(2.2));
    o.glow = m.colour.w;
    return o;
  }
  let f = id / 6u;
  let c = quad(id % 6u) - vec2f(0.5);
  var n = vec3f(0.0); var a = vec3f(0.0); var b = vec3f(0.0);
  switch f {
    case 0u: { n = vec3f(1.0, 0.0, 0.0); a = vec3f(0.0, 0.0, 1.0); b = vec3f(0.0, 1.0, 0.0); }
    case 1u: { n = vec3f(-1.0, 0.0, 0.0); a = vec3f(0.0, 1.0, 0.0); b = vec3f(0.0, 0.0, 1.0); }
    case 2u: { n = vec3f(0.0, 1.0, 0.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 0.0, 1.0); }
    case 3u: { n = vec3f(0.0, -1.0, 0.0); a = vec3f(0.0, 0.0, 1.0); b = vec3f(1.0, 0.0, 0.0); }
    case 4u: { n = vec3f(0.0, 0.0, 1.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 1.0, 0.0); }
    default: { n = vec3f(0.0, 0.0, -1.0); a = vec3f(0.0, 1.0, 0.0); b = vec3f(1.0, 0.0, 0.0); }
  }
  let p = vec4f(n * 0.5 + a * c.x + b * c.y, 1.0);
  let w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let c0 = vec3f(m.r0.x, m.r1.x, m.r2.x);
  let c1 = vec3f(m.r0.y, m.r1.y, m.r2.y);
  let c2 = vec3f(m.r0.z, m.r1.z, m.r2.z);
  let normal = cross(c1, c2) * n.x + cross(c2, c0) * n.y + cross(c0, c1) * n.z;
  let wp = vec4f(w, 1.0);
  o.pos = vec4f(dot(u.r0, wp), dot(u.r1, wp), dot(u.r2, wp), dot(u.r3, wp));
  o.world = w;
  o.normal = normalize(normal + vec3f(0.0, 0.00001, 0.0));
  o.tint = pow(max(m.colour.rgb, vec3f(0.0)), vec3f(2.2));
  o.glow = m.colour.w;
  return o;
}

fn slab(o: vec3f, d: vec3f, lo: vec3f, hi: vec3f) -> bool {
  let inv = 1.0 / d;
  let t0 = (lo - o) * inv;
  let t1 = (hi - o) * inv;
  let a = min(t0, t1);
  let b = max(t0, t1);
  let near = max(max(a.x, a.y), a.z);
  let far = min(min(b.x, b.y), b.z);
  return far > max(near, 0.02);
}

fn sunlight(p: vec3f, n: vec3f) -> f32 {
  let s = normalize(u.sun.xyz);
  if (s.y <= 0.01) { return 0.0; }
  let o = p + n * 0.03;
  var v = 1.0;
  if (abs(s.x) > 0.001) {
    let t = (sign(s.x) * 15.8 - o.x) / s.x;
    if (t > 0.02 && o.y + s.y * t < 5.6) { return 0.0; }
  }
  if (abs(s.z) > 0.001) {
    let wall = select(-23.8, 13.8, s.z > 0.0);
    let t = (wall - o.z) / s.z;
    if (t > 0.02 && o.y + s.y * t < 6.0) { return 0.0; }
  }
  if (o.y < 5.6) {
    let t = (5.8 - o.y) / s.y;
    let q = o + s * t;
    let rib = abs(fract((q.z + 22.0) / 4.0 + 0.5) - 0.5) * 4.0;
    v *= smoothstep(0.1, 0.2, rib);
  }
  let count = i32(u.info.z);
  for (var i = 0; i < 10; i++) {
    if (i >= count) { break; }
    if (slab(o, s, u.occ[i * 2].xyz, u.occ[i * 2 + 1].xyz)) { return 0.0; }
  }
  return v;
}

fn blobs(p: vec3f) -> f32 {
  if (p.y > 0.12) { return 1.0; }
  var s = 1.0;
  let count = i32(u.info.y);
  for (var i = 0; i < 24; i++) {
    if (i >= count) { break; }
    let b = u.shadow[i];
    let d = length(p.xz - b.xy) / b.z;
    s *= 1.0 - b.w * (1.0 - smoothstep(0.3, 1.0, d));
  }
  return s;
}

@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  let n = normalize(s.normal);
  var base = s.tint;
  if (s.glow < 0.0) {
    let c = base * 1.6;
    return vec4f(plates(s.pos.xy, hurt(s.pos.xy, encode(tone(c)))), 1.0);
  }
  if (s.world.y < 0.02 && n.y > 0.5) {
    base *= 0.85 + 0.25 * noise(s.world.xz * 1.3) + 0.08 * noise(s.world.xz * 11.0);
  }
  let sun = normalize(u.sun.xyz);
  let lit = sunlight(s.world, n) * max(dot(n, sun), 0.0) * u.sun.w;
  let ambient = mix(u.gnd.rgb, u.sky.rgb, n.y * 0.5 + 0.5);
  let occlusion = blobs(s.world);
  var light = ambient * occlusion + u.sunc.rgb * lit * mix(0.35, 1.0, occlusion);
  let count = i32(u.info.x);
  for (var i = 0; i < 40; i++) {
    if (i >= count) { break; }
    let l = u.light[i * 2];
    let to = l.xyz - s.world;
    let d = length(to);
    if (d >= l.w) { continue; }
    let fall = 1.0 - d / l.w;
    let wrap = max(dot(n, to / max(d, 0.001)), 0.0) * 0.85 + 0.15;
    light += u.light[i * 2 + 1].rgb * fall * fall * wrap * 1.4;
  }
  var c = base * light + base * s.glow * 1.3;
  let d = length(u.eye.xyz - s.world);
  let fog = 1.0 - exp(-max(d - 6.0, 0.0) * u.eye.w);
  c = mix(c, u.fog.rgb, clamp(fog, 0.0, 0.85));
  return vec4f(plates(s.pos.xy, hurt(s.pos.xy, encode(tone(c * 1.15)))), 1.0);
}

struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,
  @location(1) shape: vec4f,
  @location(2) tint: vec4f,
  @location(3) bar: vec4f,
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
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
