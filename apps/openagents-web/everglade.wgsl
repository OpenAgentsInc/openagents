// Everglade's three passes: the sky, the scene (with the river's water in
// two blended halves after it) and the HUD. Every number
// comes from everglade.coax: the view, the eye and the fog, the sun, the
// ground's heights (store 0) and each object's place, size, colour and shape
// (store 1), the shadows, the streets, the paved squares and the HUD's
// plates and strokes. The kit's meshes come from the realm's pack: its
// vertices (pack_realm.py: 48 bytes, position, normal, uv, colour with its
// layer in the top byte) and its texture layers.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, horizon: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
  sun: vec4f,
  ground: vec4f,
  shadow: array<vec4f, 16>,
  path: array<vec4f, 32>,
  path_info: vec4f,
  square: array<vec4f, 4>,
  plate: array<vec4f, 64>,
  water: WaterU,   // the river's waves, look and mesh (verse/water-look.coax W_* rows)
}
@group(0) @binding(0) var<uniform> u: U;

const HORIZON = vec3f(0.80, 0.86, 0.90);
const ZENITH = vec3f(0.36, 0.56, 0.82);

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 6.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.y >= 2.0;
  let fill = select(vec3f(0.055, 0.075, 0.06), vec3f(0.09, 0.12, 0.09), over);
  let line = select(vec3f(0.62, 0.52, 0.30), vec3f(0.90, 0.76, 0.45), over || s.y >= 1.0);
  let w = 1.25 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1) * 0.8);
}
fn plates(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 32; k++) {
    if (f32(k) >= u.plates) { break; }
    c = plate(p, c, u.plate[k * 2], u.plate[k * 2 + 1]);
  }
  return c;
}
fn encode(c: vec3f) -> vec3f { return pow(clamp(c, vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2)); }

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let up = clamp((u.horizon - pos.y) / max(u.size.y, 1.0) * 1.6, 0.0, 1.0);
  let sky = mix(HORIZON, ZENITH, pow(up, 0.7));
  return vec4f(plates(pos.xy, encode(sky)), 1.0);
}

struct Obj { r0: vec4f, r1: vec4f, r2: vec4f, colour: vec4f }
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> heights: array<f32>;
@group(1) @binding(2) var<storage, read> objects: array<Obj>;
@group(1) @binding(3) var<storage, read> pose: array<vec4f>;   // the Ranger's joints: three rows each
@group(1) @binding(4) var<storage, read> water_nodes: array<f32>;   // the river's baked nodes (verse/water-look.coax)
@group(1) @binding(5) var sheets: texture_2d_array<f32>;
@group(1) @binding(6) var sheet: sampler;
fn word(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
  @location(3) @interpolate(flat) look: u32,
  @location(4) uv: vec2f,
  @location(5) @interpolate(flat) slice: u32,
}

fn quad(v: u32) -> vec2f {
  var q = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  return q[v];
}
fn height(x: i32, z: i32) -> f32 {
  let n = i32(u.ground.x);
  let cx = clamp(x, 0, n);
  let cz = clamp(z, 0, n);
  return heights[u32(cz * (n + 1) + cx)];
}

struct Vert { p: vec3f, n: vec3f }
const TAU = 6.2831853;

fn box_vertex(id: u32) -> Vert {
  let f = id / 6u;
  let c = quad(id % 6u) - vec2f(0.5);
  var n = vec3f(0.0); var a = vec3f(0.0); var b = vec3f(0.0);
  switch f {
    case 0u: { n = vec3f(1.0, 0.0, 0.0); a = vec3f(0.0, 0.0, 1.0); b = vec3f(0.0, 1.0, 0.0); }
    case 1u: { n = vec3f(-1.0, 0.0, 0.0); a = vec3f(0.0, 0.0, 1.0); b = vec3f(0.0, 1.0, 0.0); }
    case 2u: { n = vec3f(0.0, 1.0, 0.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 0.0, 1.0); }
    case 3u: { n = vec3f(0.0, -1.0, 0.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 0.0, 1.0); }
    case 4u: { n = vec3f(0.0, 0.0, 1.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 1.0, 0.0); }
    default: { n = vec3f(0.0, 0.0, -1.0); a = vec3f(1.0, 0.0, 0.0); b = vec3f(0.0, 1.0, 0.0); }
  }
  return Vert(n * 0.5 + a * c.x + b * c.y, n);
}
fn sphere_vertex(id: u32) -> Vert {
  let q = id / 6u;
  let c = quad(id % 6u);
  let lon = (f32(q % 16u) + c.x) / 16.0 * TAU;
  let lat = (f32(q / 16u) + c.y) / 10.0 * 3.14159265;
  let n = vec3f(sin(lat) * cos(lon), cos(lat), sin(lat) * sin(lon));
  return Vert(n * 0.5, n);
}
fn cylinder_vertex(id: u32) -> Vert {
  if (id < 96u) {
    let c = quad(id % 6u);
    let a = (f32(id / 6u) + c.x) / 16.0 * TAU;
    let n = vec3f(cos(a), 0.0, sin(a));
    return Vert(vec3f(n.x * 0.5, c.y - 0.5, n.z * 0.5), n);
  }
  let t = (id - 96u) / 3u;
  let k = (id - 96u) % 3u;
  let top = t < 16u;
  let y = select(-0.5, 0.5, top);
  let n = vec3f(0.0, select(-1.0, 1.0, top), 0.0);
  if (k == 0u) { return Vert(vec3f(0.0, y, 0.0), n); }
  let a = (f32(t % 16u) + f32(k - 1u)) / 16.0 * TAU;
  return Vert(vec3f(cos(a) * 0.5, y, sin(a) * 0.5), n);
}
fn cone_vertex(id: u32) -> Vert {
  let t = (id % 48u) / 3u;
  let k = id % 3u;
  if (id < 48u) {
    let mid = (f32(t) + 0.5) / 16.0 * TAU;
    if (k == 0u) { return Vert(vec3f(0.0, 0.5, 0.0), normalize(vec3f(cos(mid), 0.5, sin(mid)))); }
    let a = (f32(t) + f32(k - 1u)) / 16.0 * TAU;
    return Vert(vec3f(cos(a) * 0.5, -0.5, sin(a) * 0.5), normalize(vec3f(cos(a), 0.5, sin(a))));
  }
  let n = vec3f(0.0, -1.0, 0.0);
  if (k == 0u) { return Vert(vec3f(0.0, -0.5, 0.0), n); }
  let a = (f32(t) + f32(k - 1u)) / 16.0 * TAU;
  return Vert(vec3f(cos(a) * 0.5, -0.5, sin(a) * 0.5), n);
}

fn clip(w: vec3f) -> vec4f {
  let p = vec4f(w, 1.0);
  return vec4f(dot(u.r0, p), dot(u.r1, p), dot(u.r2, p), dot(u.r3, p));
}

@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  var o: S;
  let n = u32(u.ground.x);
  if (instance < n * n) {
    let c = quad(id % 6u);
    let gx = i32(instance % n) + i32(c.x);
    let gz = i32(instance / n) + i32(c.y);
    let w = vec3f(u.ground.z + f32(gx) * u.ground.y, height(gx, gz), u.ground.w + f32(gz) * u.ground.y);
    let normal = normalize(vec3f(height(gx - 1, gz) - height(gx + 1, gz), 2.0 * u.ground.y, height(gx, gz - 1) - height(gx, gz + 1)));
    o.pos = clip(w);
    o.world = w;
    o.normal = normal;
    o.tint = vec3f(0.30, 0.50, 0.20);
    o.look = 99u;
    o.uv = vec2f(0.0);
    o.slice = 0u;
    return o;
  }
  let m = objects[instance - n * n];
  let kind = u32(m.colour.w) % 8u;
  var v: Vert;
  var tint = m.colour.rgb;
  o.uv = vec2f(0.0);
  o.slice = 0u;
  switch kind {
    case 1u: { v = box_vertex(id); }
    case 2u: { v = sphere_vertex(id); }
    case 3u: { v = cylinder_vertex(id); }
    case 5u: {
      let b = id * 12u;
      v = Vert(vec3f(word(b), word(b + 1u), word(b + 2u)), vec3f(word(b + 3u), word(b + 4u), word(b + 5u)));
      o.uv = vec2f(word(b + 6u), word(b + 7u));
      o.slice = vertices[b + 8u] >> 24u;
      tint = unpack4x8unorm(vertices[b + 8u]).rgb * m.colour.rgb;
    }
    case 6u: {
      // The Ranger: the pack's 32-byte character vertex, skinned by up to four joints of the pose.
      let b = u32(m.colour.x) + id * 8u;
      let at = vec4f(word(b), word(b + 1u), word(b + 2u), 1.0);
      let nw = unpack4x8snorm(vertices[b + 3u]).xyz;
      let j = vec4u(unpack4x8unorm(vertices[b + 6u]) * 255.0 + 0.5);
      let wt = unpack4x8unorm(vertices[b + 7u]);
      var sp = vec3f(0.0);
      var sn = vec3f(0.0);
      for (var k = 0u; k < 4u; k++) {
        let r = j[k] * 3u;
        let a = pose[r]; let c = pose[r + 1u]; let e = pose[r + 2u];
        sp += wt[k] * vec3f(dot(a, at), dot(c, at), dot(e, at));
        sn += wt[k] * vec3f(dot(a.xyz, nw), dot(c.xyz, nw), dot(e.xyz, nw));
      }
      v = Vert(sp, sn);
      o.uv = vec2f(word(b + 4u), word(b + 5u));
      o.slice = vertices[b + 3u] >> 24u;
      tint = vec3f(1.0);
    }
    default: { v = cone_vertex(id); }
  }
  let p = vec4f(v.p, 1.0);
  let w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let c0 = vec3f(m.r0.x, m.r1.x, m.r2.x);
  let c1 = vec3f(m.r0.y, m.r1.y, m.r2.y);
  let c2 = vec3f(m.r0.z, m.r1.z, m.r2.z);
  let normal = cross(c1, c2) * v.n.x + cross(c2, c0) * v.n.y + cross(c0, c1) * v.n.z;
  o.pos = clip(w);
  o.world = w;
  o.normal = normalize(normal + vec3f(0.0, 0.00001, 0.0));
  o.tint = tint;
  o.look = u32(m.colour.w) / 8u;
  return o;
}

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn path_distance(p: vec2f) -> f32 {
  var d = 1000.0;
  let count = i32(u.path_info.x);
  for (var i = 0; i < 32; i++) {
    if (i >= count) { break; }
    let seg = u.path[i];
    let a = seg.xy;
    let ab = seg.zw - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    d = min(d, length(p - a - ab * t));
  }
  return d;
}
// A shadow is x, z, radius and its strength with the floor it falls on
// packed in (everglade.coax shadow: strength + 2 * centimetres above -100 m),
// so one cast on a bench's top darkens the bench, not the ground under it.
// level: only surfaces within SHADOW_LEVEL of that floor take it (the water takes any).
const SHADOW_LEVEL = 0.35;
fn shade_shadows_at(p: vec2f, y: f32, level: bool) -> f32 {
  var s = 1.0;
  for (var i = 0; i < 16; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    let cm = floor(b.w * 0.5);
    let strength = b.w - cm * 2.0;
    if (level && abs(y - (cm * 0.01 - 100.0)) > SHADOW_LEVEL) { continue; }
    let d = length(p - b.xy) / b.z;
    s *= 1.0 - strength * (1.0 - smoothstep(0.35, 1.0, d));
  }
  return s;
}
fn shade_shadows(p: vec2f) -> f32 { return shade_shadows_at(p, 0.0, false); }

@fragment fn scene_fs(s: S, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  var n = normalize(s.normal);
  var base = s.tint;
  var shadow = 1.0;
  if (s.look == 5u) {
    // A kit mesh: its texture, cut out where the leaves' layer is clear; both sides of a leaf lit.
    if (tex.a < 0.35) { discard; }
    base = tex.rgb * s.tint;
    if (!front) { n = -n; }
    // a kit piece's top (a bench, a crate) takes the shadow of whoever stands on it
    if (n.y > 0.7) { shadow = shade_shadows_at(s.world.xz, s.world.y, true); }
  }
  let xz = s.world.xz;
  if (s.look == 99u) {
    let broad = noise(xz * 0.13);
    let fine = noise(xz * 2.7);
    base = mix(vec3f(0.24, 0.43, 0.15), vec3f(0.38, 0.58, 0.22), broad) * (0.88 + 0.2 * fine);
    for (var q = 0; q < 4; q++) {
      let sq = u.square[q];
      if (sq.z <= 0.0) { break; }
      let paved = 1.0 - smoothstep(sq.z - 0.3, sq.z + 0.3, length(xz - sq.xy));
      let stone = mix(vec3f(0.56, 0.53, 0.47), vec3f(0.66, 0.63, 0.56), noise(xz * 1.7)) * (0.9 + 0.12 * noise(xz * 9.0));
      base = mix(base, stone, paved);
    }
    let d = path_distance(xz);
    let half = u.path_info.y;
    let edge = 1.0 - smoothstep(half - 0.12, half + 0.12, d);
    if (edge > 0.0) {
      let grit = noise(xz * 5.0) * 0.5 + noise(xz * 13.0) * 0.5;
      let dirt = mix(vec3f(0.55, 0.46, 0.33), vec3f(0.68, 0.6, 0.46), grit);
      base = mix(base, dirt, edge);
    }
    // the river's bed: silt and stones under the water, darker the deeper,
    // and wet earth where the bank meets it
    let under = water_over(xz);
    let mud = mix(vec3f(0.27, 0.22, 0.15), vec3f(0.36, 0.3, 0.2), noise(xz * 2.3)) * (0.85 + 0.25 * noise(xz * 11.0));
    let wet = smoothstep(-0.02, 0.25, under);
    base = mix(base, mud * (1.0 - 0.45 * smoothstep(0.3, 2.0, under)), wet);
    shadow = shade_shadows_at(xz, s.world.y, true);
  } else if (s.look == 1u) {
    base *= 0.8 + 0.35 * noise(s.world.xy * 3.1 + s.world.zx * 2.3);
  } else if (s.look == 2u) {
    base *= 0.9 + 0.15 * noise(s.world.xz * 9.0 + s.world.y * 7.0);
  } else if (s.look == 4u) {
    base = mix(base, vec3f(0.85, 0.92, 0.95), 0.25 * noise(xz * 1.5 + vec2f(u.time * 0.2, 0.0)));
  }
  let sun = normalize(u.sun.xyz);
  let lambert = max(dot(n, sun), 0.0);
  let sky = mix(vec3f(0.34, 0.31, 0.24), vec3f(0.56, 0.66, 0.80), n.y * 0.5 + 0.5);
  var c = base * (sky * 0.62 + vec3f(1.0, 0.94, 0.82) * lambert * 0.9 * shadow) * mix(0.75, 1.0, shadow);
  let d = length(u.eye.xyz - s.world);
  let fog = 1.0 - exp(-max(d - u.sun.w, 0.0) * u.eye.w);
  c = mix(c, HORIZON, clamp(fog, 0.0, 0.9));
  return vec4f(plates(s.pos.xy, encode(c)), 1.0);
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

// ---- water (phase W2 of docs/verse/water.md): a port of the previous
// engine's shared water.wgsl (git show 32e4d59fc2:crates/verse-pbr/src/water/
// water.wgsl), with this realm's sky, sun, light and fog as its hooks (the
// old imported renderer's, imported.wgsl). It implements public techniques:
//
// - Gerstner waves (Fournier and Reeves 1986, in Finch's GPU Gems 2004 form)
//   from verse/water.coax's own terms: each angle's constant part comes from
//   the integer phase counter in f64 on the CPU (water_look_angles).
// - Schlick's Fresnel (1994) at an index of 1.33, with Snell's window and
//   total internal reflection from below.
// - Sky reflection blurred by roughness and the pixel's footprint; sun
//   glints from GGX (Walter et al. 2007) with the height-correlated Smith
//   visibility (Heitz 2014), faded with distance by tier.
// - Crest scattering with the Henyey-Greenstein phase function (1941).
// - Per-channel Beer-Lambert absorption over the baked depth, from Jerlov's
//   water types (Marine Optics, 1976).
// - Shore foam from the baked distance transform; two-phase flow-map
//   advection of the detail and the foam (Vlachos, Portal 2, 2010).
//
// The mesh is the river's baked node grid (store 3, WATER_NODE floats a
// node: x, level, z, depth, shore, flow x, flow z, white water), a patch an
// instance, its quads sampled between the nodes. Each patch draws twice
// after the scene, depth-tested and not written (host.js): water_transmit_fs
// multiplies what lies behind by the surface's per-channel transmittance,
// then water_emit_fs adds what it reflects, scatters and foams.
//
// This realm draws into a gamma-encoded target (encode), so the transmitted
// half multiplies by the encoded transmittance, which is exact for a
// product, and the emitted half adds the encoded difference it makes over a
// dim bed (WATER_BED), which is close where a bed shows and exact where the
// water hides it.

struct WaterU {
  waves: array<vec4f, 16>,
  detail: array<vec4f, 32>,
  ripples: array<vec4f, 24>,
  params: vec4f,    // time, detail count, ripple count, wave count
  absorb: vec4f,
  scatter: vec4f,   // in-scatter rgb, foam
  body: vec4f,      // roughness, swell gain, eye inside, crest scattering
  rest: vec4f,      // shore band, pixel angle, sun disc, metres along a node
  control: vec4f,   // detail waves, glint fade, foam octaves, 1
  grid: vec4f,      // nodes across, nodes along, quads across a patch, quads along a patch
  grid2: vec4f,     // nodes along a patch, patches
  frame: vec4f,     // the first node's x, z, the way along (rad about +Y from +z), metres across a node
}

const WATER_IOR: f32 = 1.33;
const WATER_F0: f32 = 0.0200593;          // ((n - 1) / (n + 1))^2
const WATER_RIPPLE_SPEED: f32 = 1.1;
const WATER_FLOW_PERIOD: f32 = 1.6;
const WATER_DRY: f32 = 0.6;
const WATER_SUN = vec3f(1.0, 0.94, 0.82);
const WATER_SUN_E: f32 = 2.8274;          // the sun's illuminance: scene_fs's 0.9 times PI
const WATER_BED: f32 = 0.15;              // the linear radiance a lit bed under the water has, about

fn water_host_sky(dir: vec3f, level: f32) -> vec3f {
  let up = clamp(dir.y, 0.0, 1.0);
  let sharp = mix(HORIZON, ZENITH, smoothstep(0.0, 0.55, up));
  let blurred = mix(HORIZON, ZENITH, 0.55);
  return mix(sharp, blurred, level * level);
}
fn water_host_sun() -> vec4f { return vec4f(normalize(u.sun.xyz), 1.0); }
fn water_host_sun_light() -> vec3f { return WATER_SUN * WATER_SUN_E; }
fn water_host_light(world: vec3f, n: vec3f) -> vec3f {
  let sky = mix(vec3f(0.34, 0.31, 0.24), vec3f(0.56, 0.66, 0.80), n.y * 0.5 + 0.5);
  return sky * 0.62 + WATER_SUN * 0.9 * max(dot(n, normalize(u.sun.xyz)), 0.0) * shade_shadows(world.xz);
}

fn water_node(k: u32, f: u32) -> f32 { return water_nodes[k * 8u + f]; }
// A point's place among the nodes (u across, v along), for a straight
// course: the frame row says where the first node is and which way the rows run.
fn water_uv(xz: vec2f) -> vec2f {
  let t = vec2f(sin(u.water.frame.z), cos(u.water.frame.z));
  let side = vec2f(-t.y, t.x);
  let d = xz - u.water.frame.xy;
  return vec2f(dot(d, side) / max(u.water.frame.w, 1e-3), dot(d, t) / max(u.water.rest.w, 1e-3));
}
// How deep the water stands over ground at xz (m; 0 where there is none).
fn water_over(xz: vec2f) -> f32 {
  if (u.water.grid.x < 2.0 || u.water.frame.w <= 0.0) { return 0.0; }
  let uv = water_uv(xz);
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > u.water.grid.x - 1.0 || uv.y > u.water.grid.y - 1.0) { return 0.0; }
  return max(water_rest(uv).depth, 0.0);
}
struct WaterRest { pos: vec3f, depth: f32, shore: f32, flow: vec2f, foam: f32 }
// The baked grid between its nodes: u across, v along, in node units.
fn water_rest(uv: vec2f) -> WaterRest {
  let cols = u32(u.water.grid.x);
  let rows = u32(u.water.grid.y);
  let a = clamp(uv, vec2f(0.0), vec2f(f32(cols - 1u), f32(rows - 1u)));
  let i = min(u32(floor(a.x)), cols - 2u);
  let j = min(u32(floor(a.y)), rows - 2u);
  let f = a - vec2f(f32(i), f32(j));
  let k00 = j * cols + i;
  let k10 = k00 + 1u;
  let k01 = k00 + cols;
  let k11 = k01 + 1u;
  var o: WaterRest;
  var v: array<f32, 8>;
  for (var c = 0u; c < 8u; c++) {
    v[c] = mix(mix(water_node(k00, c), water_node(k10, c), f.x), mix(water_node(k01, c), water_node(k11, c), f.x), f.y);
  }
  o.pos = vec3f(v[0], v[1], v[2]);
  o.depth = v[3];
  o.shore = v[4];
  o.flow = vec2f(v[5], v[6]);
  o.foam = v[7];
  return o;
}

fn water_shoal(k: f32, depth: f32) -> f32 { return sqrt(tanh(clamp(k * depth, 0.0, 10.0))); }

// The Gerstner displacement (xyz, m) of rest point p0 and how tightly the
// waves squeeze the surface there (w, crests near one).
fn water_gerstner(p0: vec2f, depth: f32, scale: f32) -> vec4f {
  var d = vec3f(0.0);
  var squeeze = 0.0;
  let count = i32(u.water.params.w);
  let gain = u.water.body.y * scale;
  for (var i = 0; i < 8; i++) {
    if (i >= count) { break; }
    let a = u.water.waves[i * 2];
    let c = u.water.waves[i * 2 + 1];
    let k = a.z;
    let amp = a.w * gain * water_shoal(k, depth);
    let theta = k * dot(a.xy, p0) + c.y;
    let co = cos(theta);
    let s = sin(theta);
    d.x += c.x * amp * a.x * co;
    d.z += c.x * amp * a.y * co;
    d.y += amp * s;
    squeeze += c.x * amp * k * s;
  }
  return vec4f(d, squeeze);
}
// The waves' slope at p0 (dh/dx, dh/dz) and the squeeze's change of the normal's y (Finch 2004, eq. 12).
fn water_gerstner_slope(p0: vec2f, depth: f32, scale: f32) -> vec3f {
  var g = vec3f(0.0);
  let count = i32(u.water.params.w);
  let gain = u.water.body.y * scale;
  for (var i = 0; i < 8; i++) {
    if (i >= count) { break; }
    let a = u.water.waves[i * 2];
    let c = u.water.waves[i * 2 + 1];
    let k = a.z;
    let wa = k * a.w * gain * water_shoal(k, depth);
    let theta = k * dot(a.xy, p0) + c.y;
    let co = cos(theta);
    g.x += a.x * wa * co;
    g.y += a.y * wa * co;
    g.z += c.x * wa * sin(theta);
  }
  return g;
}

struct WaterOut {
  @builtin(position) clip: vec4f,
  @location(0) world_depth: vec4f,   // the displaced position, the depth under it at rest
  @location(1) rest_flow: vec4f,     // the rest position on the plane, the flow (m/s)
  @location(2) look: vec4f,          // white water, 0, crest squeeze, shore distance (m)
  @location(3) extra: vec4f,         // 0, height over the level (m)
}

// A patch an instance; its quads are sampled between the baked nodes, and a
// quad whose four corners stand above the water is dry ground and not drawn.
@vertex fn water_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) piece: u32) -> WaterOut {
  var o: WaterOut;
  let qa = u32(u.water.grid.z);
  let ql = u32(u.water.grid.w);
  let quad_id = id / 6u;
  let c = quad(id % 6u);
  let qx = quad_id % qa;
  let qz = quad_id / qa;
  let across = (u.water.grid.x - 1.0) / f32(qa);
  let along = u.water.grid2.x / f32(ql);
  let base = f32(piece) * u.water.grid2.x;
  let uv = vec2f((f32(qx) + c.x) * across, base + (f32(qz) + c.y) * along);
  var low = 1000.0;
  for (var k = 0u; k < 4u; k++) {
    let corner = vec2f(f32(qx + (k & 1u)) * across, base + f32(qz + (k >> 1u)) * along);
    low = min(low, -water_rest(corner).depth);
  }
  let r = water_rest(uv);
  if (low > WATER_DRY || quad_id >= qa * ql) {
    o.clip = vec4f(0.0, 0.0, 2.0, 1.0);
    return o;
  }
  let wave = water_gerstner(r.pos.xz, r.depth, 1.0);
  let world = r.pos + wave.xyz;
  o.clip = clip(world);
  o.world_depth = vec4f(world, r.depth);
  o.rest_flow = vec4f(r.pos.xz, r.flow);
  o.look = vec4f(r.foam, 0.0, wave.w, r.shore);
  o.extra = vec4f(0.0, world.y - r.pos.y, 0.0, 0.0);
  return o;
}

// ---- detail: what bends the normal but never moves the surface

// pcg3d (Jarzynski and Olano, "Hash Functions for GPU Rendering", JCGT 2020) on the cell's corner.
fn water_hash(p: vec3f) -> f32 {
  var v = bitcast<vec3u>(vec3i(p)) * 1664525u + 1013904223u;
  v.x += v.y * v.z;
  v.y += v.z * v.x;
  v.z += v.x * v.y;
  v = v ^ (v >> vec3u(16u));
  v.x += v.y * v.z;
  return f32(v.x >> 8u) / 16777215.0;
}
fn water_noise(p: vec3f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  let a = water_hash(i);
  let b = water_hash(i + vec3f(1.0, 0.0, 0.0));
  let c = water_hash(i + vec3f(0.0, 1.0, 0.0));
  let d = water_hash(i + vec3f(1.0, 1.0, 0.0));
  let e = water_hash(i + vec3f(0.0, 0.0, 1.0));
  let g = water_hash(i + vec3f(1.0, 0.0, 1.0));
  let h = water_hash(i + vec3f(0.0, 1.0, 1.0));
  let k = water_hash(i + vec3f(1.0, 1.0, 1.0));
  return mix(mix(mix(a, b, s.x), mix(c, d, s.x), s.y), mix(mix(e, g, s.x), mix(h, k, s.x), s.y), s.z);
}

// The analytic detail waves' slope at p, each faded once it is shorter than a
// few of the pixel's footprint (m), and the slope variance the faded waves
// would have added (z), which becomes roughness (after Toksvig 2005).
fn water_detail_waves(p: vec2f, footprint: f32, gain: f32) -> vec3f {
  var g = vec2f(0.0);
  var lost = 0.0;
  let t = u.water.params.x;
  let n = min(i32(u.water.control.x), i32(u.water.params.y));
  for (var i = 0; i < 16; i++) {
    if (i >= n) { break; }
    let a = u.water.detail[i * 2];
    let b = u.water.detail[i * 2 + 1];
    let k = a.z;
    let slope = a.w * k * gain;
    let keep = clamp(b.z / (6.0 * max(footprint, 1e-4)) - 0.5, 0.0, 1.0);
    let theta = k * dot(a.xy, p) - b.x * t + b.y;
    g += a.xy * (slope * keep * cos(theta));
    lost += 0.5 * slope * slope * (1.0 - keep * keep);
  }
  return vec3f(g, lost);
}

// A pixel's footprint on the surface (m): the eye's distance times the
// pixel's angle, stretched along the view at grazing angles.
fn water_footprint(v: vec3f, distance: f32) -> f32 {
  let size = max(distance * u.water.rest.y, 1e-4);
  return size * sqrt(1.0 / max(abs(v.y), 0.08));
}

// Detail carried along the flow in two phases half a cycle apart, blended
// so neither phase's reset shows (Vlachos 2010). Still water stands.
fn water_flow_detail(p: vec2f, flow: vec2f, footprint: f32) -> vec3f {
  if (length(flow) <= 0.02) { return water_detail_waves(p, footprint, 1.0); }
  let t = u.water.params.x;
  let phase_a = fract(t / WATER_FLOW_PERIOD);
  let phase_b = fract(t / WATER_FLOW_PERIOD + 0.5);
  let da = water_detail_waves(p - flow * phase_a * WATER_FLOW_PERIOD, footprint, 1.4);
  let db = water_detail_waves(p - flow * phase_b * WATER_FLOW_PERIOD + vec2f(3.7, 1.3), footprint, 1.4);
  let wa = 1.0 - abs(2.0 * phase_a - 1.0);
  return da * wa + db * (1.0 - wa);
}

// The ripples' slope at p: damped wave packets riding expanding rings.
fn water_ripples(p: vec2f) -> vec2f {
  var g = vec2f(0.0);
  let t = u.water.params.x;
  let count = i32(u.water.params.z);
  let k = 6.2831853 / 0.45;
  for (var i = 0; i < 24; i++) {
    if (i >= count) { break; }
    let r = u.water.ripples[i];
    let age = t - r.z;
    let to = p - r.xy;
    let dist = length(to);
    let x = dist - WATER_RIPPLE_SPEED * age;
    let envelope = exp(-x * x / 0.18) * exp(-age * 1.1) / (1.0 + 1.5 * dist);
    g += to / max(dist, 1e-3) * (r.w * envelope * k * cos(k * x));
  }
  return g;
}
// Foam riding the rings of young ripples.
fn water_ripple_foam(p: vec2f) -> f32 {
  var foam = 0.0;
  let t = u.water.params.x;
  let count = i32(u.water.params.z);
  for (var i = 0; i < 24; i++) {
    if (i >= count) { break; }
    let r = u.water.ripples[i];
    let age = t - r.z;
    let x = distance(p, r.xy) - WATER_RIPPLE_SPEED * age;
    foam = max(foam, exp(-x * x / 0.08) * exp(-age * 1.6) * clamp(r.w * 25.0, 0.0, 1.0));
  }
  return foam;
}

// Foam's mottled cover: one octave on a phone, two above, carried along the flow in two phases.
fn water_foam_noise(p: vec2f, flow: vec2f) -> f32 {
  let t = u.water.params.x;
  let phase_a = fract(t / WATER_FLOW_PERIOD);
  let phase_b = fract(t / WATER_FLOW_PERIOD + 0.5);
  let wa = 1.0 - abs(2.0 * phase_a - 1.0);
  let pa = p - flow * phase_a * WATER_FLOW_PERIOD;
  let pb = p - flow * phase_b * WATER_FLOW_PERIOD + vec2f(5.3, 2.9);
  var a = water_noise(vec3f(pa * 1.3, t * 0.25));
  var b = water_noise(vec3f(pb * 1.3, t * 0.25));
  if (u.water.control.z > 1.5) {
    a = a * 0.6 + 0.4 * water_noise(vec3f(pa * 4.1 + vec2f(t * 0.07, 0.0), t * 0.5));
    b = b * 0.6 + 0.4 * water_noise(vec3f(pb * 4.1 + vec2f(t * 0.07, 0.0), t * 0.5));
  }
  return a * wa + b * (1.0 - wa);
}

// ---- optics

fn water_fresnel(c: f32) -> f32 {
  let m = clamp(1.0 - c, 0.0, 1.0);
  let m2 = m * m;
  return WATER_F0 + (1.0 - WATER_F0) * m2 * m2 * m;
}
fn water_cos_refracted(ci: f32) -> f32 { return sqrt(max(1.0 - (1.0 - ci * ci) / (WATER_IOR * WATER_IOR), 0.0)); }
// Beer-Lambert through `depth` m seen at cosine ci: the path runs along the refracted ray.
fn water_transmittance(depth: f32, ci: f32) -> vec3f {
  let path = max(depth, 0.0) / max(water_cos_refracted(ci), 0.05);
  return exp(-u.water.absorb.rgb * path);
}
// Light scattered back toward the eye from inside the water, under the sun and sky.
fn water_inscatter() -> vec3f {
  let sun = water_host_sun();
  let sky = water_host_sky(vec3f(0.0, 1.0, 0.0), 1.0);
  let lit = water_host_sun_light() * (sun.w * max(sun.y, 0.0) / 3.14159265);
  return u.water.scatter.rgb * (lit + sky);
}
fn water_sky_roughness(roughness: f32, footprint: f32) -> f32 {
  return clamp(sqrt(roughness) + 0.15 * log2(1.0 + footprint * 4.0), 0.0, 1.0);
}
fn water_ggx(noh: f32, a2: f32) -> f32 {
  let d = noh * noh * (a2 - 1.0) + 1.0;
  return a2 / (3.14159265 * d * d);
}
fn water_smith(nov: f32, nol: f32, a2: f32) -> f32 {
  let gv = nol * sqrt(nov * nov * (1.0 - a2) + a2);
  let gl = nov * sqrt(nol * nol * (1.0 - a2) + a2);
  return 0.5 / max(gv + gl, 1e-6);
}
// The sun's glint per unit illuminance: GGX widened by the sun's disc, fading by the tier's distance.
fn water_glint(n: vec3f, v: vec3f, l: vec3f, roughness: f32, disc: f32, distance: f32) -> f32 {
  let nol = dot(n, l);
  let nov = max(dot(n, v), 1e-3);
  if (nol <= 0.0) { return 0.0; }
  let h = normalize(l + v);
  let fade_at = max(u.water.control.y, 1.0);
  let far = smoothstep(0.4 * fade_at, fade_at, distance);
  let a = min(roughness * roughness + disc * 0.5 + far * 0.08, 1.0);
  let a2 = a * a;
  let spec = water_ggx(max(dot(n, h), 0.0), a2) * water_smith(nov, nol, a2) * water_fresnel(max(dot(v, h), 0.0));
  return spec * nol * (1.0 - 0.75 * far);
}
// Sun through a thin crest toward an eye looking into it (Henyey-Greenstein, g = 0.6).
fn water_crest_scatter(v: vec3f, l: vec3f, n: vec3f, height: f32, squeeze: f32) -> vec3f {
  let strength = u.water.body.w;
  if (strength <= 0.0) { return vec3f(0.0); }
  let g = 0.6;
  let cos_theta = dot(-l, v);
  let hg = (1.0 - g * g) / (4.0 * 3.14159265 * pow(1.0 + g * g - 2.0 * g * cos_theta, 1.5));
  let thin = clamp(height * 2.0, 0.0, 1.0) * clamp(squeeze * 1.5 + 0.2, 0.0, 1.0);
  let side = clamp(1.0 - n.y * 0.6 + dot(n, v) * 0.4, 0.0, 1.0);
  let through = exp(-u.water.absorb.rgb * (1.0 - 0.6 * thin));
  return u.water.scatter.rgb * through * hg * thin * side * strength * 12.0;
}
// The foam: a lace along the shore, the crests the waves squeeze, white water and young ripples.
fn water_foam(rest: vec2f, shore: f32, crest: f32, extra: f32, noise: f32) -> f32 {
  let band = max(u.water.rest.x, 0.05);
  var foam = (1.0 - smoothstep(0.0, band, shore)) * smoothstep(0.3, 0.62, noise + 0.15 * (1.0 - shore / band));
  foam = max(foam, smoothstep(0.62, 0.95, crest) * smoothstep(0.35, 0.7, noise));
  foam = max(foam, clamp(extra, 0.0, 1.0) * smoothstep(0.25, 0.65, noise + 0.25 * extra));
  foam = max(foam, water_ripple_foam(rest) * smoothstep(0.2, 0.6, noise));
  return clamp(foam * u.water.scatter.w, 0.0, 1.0);
}

struct WaterShade { emit: vec3f, transmit: vec3f }

// The surface seen from below: Snell's window, total internal reflection outside it.
fn water_below(n: vec3f, v: vec3f, path: f32, slope: f32) -> WaterShade {
  var o: WaterShade;
  let ci = clamp(dot(v, -n), 0.0, 1.0);
  let st2 = WATER_IOR * WATER_IOR * (1.0 - ci * ci);
  var reflectance = 1.0;
  if (st2 < 1.0) { reflectance = water_fresnel(sqrt(1.0 - st2)); }
  let fog = water_inscatter() * 2.4;
  let shimmer = 1.1 + 1.6 * clamp(slope, 0.0, 0.6);
  let trans = exp(-u.water.absorb.rgb * path);
  o.emit = fog * shimmer * reflectance * trans + fog * (vec3f(1.0) - trans);
  o.transmit = trans * (1.0 - reflectance);
  return o;
}

fn water_fragment(i: WaterOut) -> WaterShade {
  let world = i.world_depth.xyz;
  let rest = i.rest_flow.xy;
  let flow = i.rest_flow.zw;
  let depth = i.world_depth.w;
  let v = normalize(u.eye.xyz - world);
  let eye_distance = distance(u.eye.xyz, world);
  let footprint = water_footprint(v, eye_distance);
  // the normal, and the roughness its lost detail adds
  let swell = water_gerstner_slope(rest, depth, 1.0);
  let detail = water_flow_detail(rest, flow, footprint);
  let slope = swell.xy + detail.xy + water_ripples(rest);
  let n = normalize(vec3f(-slope.x, 1.0 - swell.z, -slope.y));
  let roughness = clamp(sqrt(u.water.body.x * u.water.body.x + detail.z), 0.02, 0.6);
  if (u.eye.y < world.y) {
    if (u.water.body.z > 0.5) { return water_below(n, v, eye_distance, length(slope)); }
    var none: WaterShade;
    none.emit = vec3f(0.0);
    none.transmit = vec3f(1.0);
    return none;
  }
  var o: WaterShade;
  let nov = max(dot(n, v), 1e-3);
  let fresnel = water_fresnel(nov);
  let sky = water_host_sky(reflect(-v, n), water_sky_roughness(roughness, footprint));
  let l = water_host_sun().xyz;
  let light = water_host_sun_light() * shade_shadows(world.xz);
  let glint = water_glint(n, v, l, roughness, u.water.rest.z, eye_distance) * light;
  let crest = water_crest_scatter(v, l, n, i.extra.y, i.look.z) * light;
  let noise = water_foam_noise(rest, flow);
  let foam = water_foam(rest, i.look.w, i.look.z, i.look.x, noise);
  let foam_rgb = vec3f(0.9, 0.94, 0.95) * water_host_light(world, vec3f(0.0, 1.0, 0.0));
  // the column under the surface: its in-scatter, and what of the bed comes through
  let ci = clamp(v.y, 0.0, 1.0);
  let through = water_transmittance(depth, ci);
  let body = water_inscatter() * (vec3f(1.0) - through);
  let clear = sky * fresnel + glint + crest + body * (1.0 - fresnel);
  // thin water at the very edge fades out instead of ending in a line
  let edge = smoothstep(-0.05, 0.12, depth);
  let cover = foam * smoothstep(-0.08, 0.0, depth);
  o.emit = clear * edge * (1.0 - cover) + foam_rgb * cover;
  o.transmit = mix(vec3f(1.0), through * (1.0 - fresnel), edge) * (1.0 - cover);
  return o;
}

// The fogged surface, with the HUD's plates over it, as the two halves blend it.
fn water_halves(i: WaterOut) -> WaterShade {
  var s = water_fragment(i);
  let d = length(u.eye.xyz - i.world_depth.xyz);
  let fog = clamp(1.0 - exp(-max(d - u.sun.w, 0.0) * u.eye.w), 0.0, 0.9);
  let emit = s.emit * (1.0 - fog) + HORIZON * fog * (vec3f(1.0) - s.transmit);
  let bed = WATER_BED * s.transmit;
  // a plate's cover: what of the scene under it a plate keeps
  let open = plates(i.clip.xy, vec3f(1.0)) - plates(i.clip.xy, vec3f(0.0));
  var o: WaterShade;
  o.transmit = mix(vec3f(1.0), encode(s.transmit), open);
  o.emit = max(encode(bed + emit) - encode(bed), vec3f(0.0)) * open;
  return o;
}
@fragment fn water_transmit_fs(i: WaterOut) -> @location(0) vec4f {
  return vec4f(clamp(water_halves(i).transmit, vec3f(0.0), vec3f(1.0)), 1.0);
}
@fragment fn water_emit_fs(i: WaterOut) -> @location(0) vec4f {
  return vec4f(water_halves(i).emit, 0.0);
}

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
