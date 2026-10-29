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
  style: array<vec4f, 32>,  // the painterly preset at the clock (verse/style-preset.coax SU_*: row = SU / 4)
  solid: array<vec4f, 32>,  // where no grass grows: the buildings' and props' footprints (x0, z0, x1, z1)
}
@group(0) @binding(0) var<uniform> u: U;

// The painterly preset at the clock (art-styles.md; everglade.coax writes u.style each frame).
fn st(i: u32) -> vec4f { return u.style[i]; }
// The haze over a surface d metres off at height y, looking along v (spec 4): the preset's curve from
// where it starts (u.sun.w) and its density (u.eye.w), both the preset's own but thinned over the
// map's lift, teal where thin and the preset's blue far off, warm toward the key light.
fn haze_share(d: f32, y: f32) -> f32 { return verse_style_haze(d, y, u.eye.y - 2.5, u.sun.w, u.eye.w, st(9u).z, st(9u).w); }
fn haze_colour(f: f32, v: vec3f) -> vec3f {
  let hz = st(10u).xyz; let cap = st(9u).z;
  let h = vec3f(verse_style_haze_hue(hz.x, 0.0, f, cap), verse_style_haze_hue(hz.y, 1.0, f, cap), verse_style_haze_hue(hz.z, 2.0, f, cap));
  return mix(h, st(11u).xyz, verse_style_haze_glow(dot(v, st(1u).xyz)));
}

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
// Everything draws light (rgba16float, its alpha the share that blooms) and aux (r: the sky's glow
// the light shafts blur); post_fs blooms, tones, encodes and lays the plates over it (host.js post).
struct Lit { @location(0) light: vec4f, @location(1) aux: vec4f }
// The view's inverse (the clip rows', by cofactors), so the sky is painted along each pixel's ray.
fn unview() -> mat4x4<f32> {
  let m = transpose(mat4x4<f32>(u.r0, u.r1, u.r2, u.r3));
  let a00 = m[0][0]; let a01 = m[0][1]; let a02 = m[0][2]; let a03 = m[0][3];
  let a10 = m[1][0]; let a11 = m[1][1]; let a12 = m[1][2]; let a13 = m[1][3];
  let a20 = m[2][0]; let a21 = m[2][1]; let a22 = m[2][2]; let a23 = m[2][3];
  let a30 = m[3][0]; let a31 = m[3][1]; let a32 = m[3][2]; let a33 = m[3][3];
  let b00 = a00 * a11 - a01 * a10; let b01 = a00 * a12 - a02 * a10; let b02 = a00 * a13 - a03 * a10;
  let b03 = a01 * a12 - a02 * a11; let b04 = a01 * a13 - a03 * a11; let b05 = a02 * a13 - a03 * a12;
  let b06 = a20 * a31 - a21 * a30; let b07 = a20 * a32 - a22 * a30; let b08 = a20 * a33 - a23 * a30;
  let b09 = a21 * a32 - a22 * a31; let b10 = a21 * a33 - a23 * a31; let b11 = a22 * a33 - a23 * a32;
  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
  let k = 1.0 / select(det, 1e-9, abs(det) < 1e-12);
  return mat4x4<f32>(
    vec4f(a11 * b11 - a12 * b10 + a13 * b09, a02 * b10 - a01 * b11 - a03 * b09, a31 * b05 - a32 * b04 + a33 * b03, a22 * b04 - a21 * b05 - a23 * b03) * k,
    vec4f(a12 * b08 - a10 * b11 - a13 * b07, a00 * b11 - a02 * b08 + a03 * b07, a32 * b02 - a30 * b05 - a33 * b01, a20 * b05 - a22 * b02 + a23 * b01) * k,
    vec4f(a10 * b10 - a11 * b08 + a13 * b06, a01 * b08 - a00 * b10 - a03 * b06, a30 * b04 - a31 * b02 + a33 * b00, a21 * b02 - a20 * b04 - a23 * b00) * k,
    vec4f(a11 * b07 - a10 * b09 - a12 * b06, a00 * b09 - a01 * b07 + a02 * b06, a31 * b01 - a30 * b03 - a32 * b00, a20 * b03 - a21 * b01 + a22 * b00) * k);
}
fn pixel_ray(p: vec2f) -> vec3f {
  let ndc = vec4f(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.9, 1.0);
  let w = unview() * ndc;
  return normalize(w.xyz / w.w - u.eye.xyz);
}
fn hash2(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
// Stars along a ray at night: one at most per cell of the sky's own grid, a few brighter.
fn stars(d: vec3f) -> f32 {
  if (d.y <= 0.0) { return 0.0; }
  let q = d.xz / (d.y + 0.35) * 260.0;
  let g = floor(q);
  let h = hash2(g);
  if (h < 0.93) { return 0.0; }
  let at = g + vec2f(hash2(g + 1.7), hash2(g + 9.1)) * 0.8 + 0.1;
  return select(0.32, 0.75, h > 0.992) * (1.0 - smoothstep(0.12, 0.42, length(q - at)));
}
// The painted sky along d (spec 5): the gradient by elevation, the horizon band lifted toward the
// haze, the sun or moon with its two-term halo, and soft two-tone clouds drifting with the wind.
fn sky(d: vec3f, l: vec3f) -> vec3f {
  let up = d.y;
  var c = mix(st(13u).xyz, st(12u).xyz, verse_style_sky_t(up));
  c = mix(c, st(10u).xyz, verse_style_sky_band(up, st(12u).w));
  let ct = dot(d, l);
  let moon = st(1u).w > 0.5;
  let disc = smoothstep(cos(st(14u).x * 1.15), cos(st(14u).x * 0.85), ct);
  let halo = verse_style_halo(ct, st(14u).y) * 0.5 + verse_style_halo(ct, st(14u).z) * 0.25;
  c += select(st(11u).xyz, st(0u).xyz, moon) * halo * select(1.0, 0.5, moon);
  c = mix(c, select(vec3f(1.0, 0.97, 0.88) * 4.0, vec3f(0.85, 0.88, 1.0) * 1.6, moon), disc);
  let cl = st(15u); let c2 = st(16u);
  if (up > 0.01 && cl.x > 0.0) {
    let t = select(u.time, 0.0, u.still > 0.5);
    let p = d.xz / up * cl.y + st(25u).xy * cl.z * t;
    let n = verse_style_cloud_field(p.x, p.y, c2.x, c2.y, cl.w);
    let cover = verse_style_cloud_cover(n, cl.x, c2.z, c2.w);
    let toward = normalize(l.xz + vec2f(0.0001)) * 260.0;
    let lit = verse_style_cloud_lit(n, verse_style_cloud_field(p.x + toward.x, p.y + toward.y, c2.x, c2.y, cl.w));
    let night = st(14u).w;
    let cloud = mix(st(4u).xyz * st(17u).w * 0.9, st(17u).xyz, lit) * max(st(0u).w, 0.3) * (1.0 - 0.75 * night);
    c = mix(c, cloud, cover * verse_style_cloud_fade(up) * (1.0 - 0.5 * night));
  }
  return c + vec3f(stars(d)) * st(14u).w * 0.6;
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Lit {
  let d = pixel_ray(pos.xy);
  let l = st(1u).xyz;
  let c = sky(d, l);
  let b = st(19u);
  return Lit(vec4f(c, verse_style_bloom_share(verse_style_luma(c.x, c.y, c.z), b.x, b.y)), vec4f(verse_style_shaft_source(dot(d, l)), 0.0, 0.0, 0.0));
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

// ---- blob canopies (art-styles.md 7, verse/style.coax): the painterly preset's numbers until
// eg-paint-light gives Everglade the preset's uniform block
const CANOPY_MIX = 0.85;
const CANOPY_SWAY = 0.05;
const WIND = vec4f(0.30, 12.0, 3.0, 1.0 / 25.0);   // amplitude (rad), wavelength (m), speed (m/s), gust scale
const WIND_DIR = vec2f(0.8, 0.6);
const LOOK_FAR_CANOPY = 6u;                        // a far tree's cone, lit as a blob
const LOOK_CANOPY = 7u;                            // a kit tree's leaves, lit as a blob
const LOOK_CHARACTER = 8u;                         // the Ranger: textured, two bands and the stronger rim (spec 11)
const LOOK_ROCK = 10u;                             // a kit rock (everglade.coax LOOK_ROCK): darker, mottled, mossy on top
fn canopy_n(n: vec3f, blob: vec3f) -> vec3f {
  return normalize(vec3f(verse_style_canopy_normal(n.x, blob.x, CANOPY_MIX), verse_style_canopy_normal(n.y, blob.y, CANOPY_MIX), verse_style_canopy_normal(n.z, blob.z, CANOPY_MIX)));
}
// How far the wind moves a canopy point `share` of its tree's height up, the tree standing at `at`:
// the grass's wind (frozen at 0 under still), 0.05 m at the top.
fn canopy_sway(at: vec3f, share: f32) -> vec3f {
  let t = select(u.time, 0.0, u.still > 0.5);
  let theta = verse_style_wind(dot(at.xz, WIND_DIR), verse_style_gust(at.x, at.z, t, WIND_DIR.x, WIND_DIR.y, WIND.z, WIND.w), t, WIND.x, WIND.y, WIND.z);
  let d = verse_style_canopy_sway(theta, WIND.x, CANOPY_SWAY, share);
  return vec3f(WIND_DIR.x * d, 0.0, WIND_DIR.y * d);
}

@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S { return place(id, instance); }
// Where a vertex of a draw lands (the scene and the shadow map both draw it).
fn place(id: u32, instance: u32) -> S {
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
  var canopy = false;
  var cone = false;
  var ranger = false;
  var share = 0.0;
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
      // a tree's leaves (pack_realm.py: the blob in the vertex's spare word): one soft blob round the
      // canopy's centre, breathing with the wind
      let blob = vertices[b + 9u];
      if (blob != 0u) {
        v.n = canopy_n(v.n, unpack4x8snorm(blob).xyz);
        share = f32(blob >> 24u) / 255.0;
        canopy = true;
      }
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
      ranger = true;
    }
    default: {
      v = cone_vertex(id);
      // a far tree's cone shades as the kit's canopy does: its normal leans to the blob round its middle
      if (u32(m.colour.w) / 8u == 1u) { v.n = canopy_n(v.n, normalize(v.p + vec3f(0.0, 0.0001, 0.0))); cone = true; }
    }
  }
  let p = vec4f(v.p, 1.0);
  var w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  if (share > 0.0) { w += canopy_sway(vec3f(m.r0.w, m.r1.w, m.r2.w), share); }
  let c0 = vec3f(m.r0.x, m.r1.x, m.r2.x);
  let c1 = vec3f(m.r0.y, m.r1.y, m.r2.y);
  let c2 = vec3f(m.r0.z, m.r1.z, m.r2.z);
  let normal = cross(c1, c2) * v.n.x + cross(c2, c0) * v.n.y + cross(c0, c1) * v.n.z;
  o.pos = clip(w);
  o.world = w;
  o.normal = normalize(normal + vec3f(0.0, 0.00001, 0.0));
  o.tint = tint;
  o.look = select(select(select(u32(m.colour.w) / 8u, LOOK_CANOPY, canopy), LOOK_FAR_CANOPY, cone), LOOK_CHARACTER, ranger);
  return o;
}

// ---- the key light's shadow map (spec 3; host.js shadow_vs): every draw but the ground drawn from the
// light into a depth map round the eye (verse_style_shadow_reach: 60 m, 40 on a phone), its middle
// snapped to whole texels so shadows hold still, read back with the preset's Poisson taps. A leaf
// card's clear texels cast nothing, so the canopies throw dappled shade.
struct Light { ax: vec3f, ay: vec3f, l: vec3f, mx: f32, my: f32, reach: f32 }
fn light_space() -> Light {
  let l = st(1u).xyz;
  let ax = select(vec3f(1.0, 0.0, 0.0), normalize(cross(vec3f(0.0, 1.0, 0.0), l)), abs(l.y) < 0.999);
  let ay = cross(l, ax);
  let reach = verse_style_shadow_reach(st(31u).z);
  let size = max(st(21u).x, 1.0);
  let c = vec3f(u.eye.x, 0.0, u.eye.z);
  return Light(ax, ay, l, verse_style_shadow_snap(dot(c, ax), reach, size), verse_style_shadow_snap(dot(c, ay), reach, size), reach);
}
fn shadow_place(w: vec3f, k: Light) -> vec3f {
  return vec3f(verse_style_shadow_across(dot(w, k.ax) - k.mx, k.reach), verse_style_shadow_across(dot(w, k.ay) - k.my, k.reach),
    verse_style_shadow_depth(dot(w - vec3f(k.mx * k.ax + k.my * k.ay), k.l), k.reach));
}
struct Shade { @builtin(position) pos: vec4f, @location(0) uv: vec2f, @location(1) @interpolate(flat) slice: u32, @location(2) @interpolate(flat) cut: u32 }
@vertex fn shadow_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> Shade {
  var o: Shade;
  let n = u32(u.ground.x);
  if (instance < n * n || st(21u).x < 1.0) { o.pos = vec4f(0.0, 0.0, -1.0, 1.0); return o; }   // the ground casts nothing
  let v = place(id, instance);
  let q = shadow_place(v.world, light_space());
  o.pos = vec4f(q.xy, q.z, 1.0);
  o.uv = v.uv; o.slice = v.slice;
  o.cut = select(0u, 1u, v.look == 5u || v.look == LOOK_CANOPY || v.look == LOOK_CHARACTER || v.look == LOOK_ROCK);
  return o;
}
@fragment fn shadow_fs(v: Shade) {
  if (v.cut == 1u && textureSampleLevel(sheets, sheet, v.uv, v.slice, 0.0).a < 0.35) { discard; }
}
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
// How much of the key reaches p (normal n): 1 lit, 0 in shadow, the map's taps averaged; outside it, lit.
fn map_lit(p: vec3f, n: vec3f) -> f32 {
  if (st(21u).x < 1.0) { return 1.0; }
  let q = shadow_place(p + n * 0.08, light_space());
  let uv = vec2f(0.5 + 0.5 * q.x, 0.5 - 0.5 * q.y);
  let texel = 1.0 / f32(textureDimensions(shadow_map).x);
  let phone = st(31u).z; let taps = st(21u).y; let r = st(21u).z;
  // the taps' disc 1.6 times the preset's: the penumbra spans several texels, so the map's stair-steps
  // melt into a soft edge; the average then firmed (smoothstep) so a thin caster's shade keeps its depth
  var lit = 0.0;
  for (var k = 0.0; k < 9.0; k += 1.0) {
    if (k >= taps) { break; }
    let o = vec2f(verse_style_shadow_tap(k, phone, 0.0), verse_style_shadow_tap(k, phone, 1.0));
    lit += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + o * r * 1.6 * texel, q.z - 0.0006);
  }
  let inside = all(uv > vec2f(0.0)) && all(uv < vec2f(1.0));
  return select(1.0, smoothstep(0.1, 0.75, lit / max(taps, 1.0)), inside);
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

@fragment fn scene_fs(s: S, @builtin(front_facing) front: bool) -> Lit {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  var n = normalize(s.normal);
  var base = s.tint;
  var shadow = 1.0;
  if (s.look == 5u || s.look == LOOK_CANOPY || s.look == LOOK_CHARACTER || s.look == LOOK_ROCK) {
    // A kit mesh: its texture, cut out where the leaves' layer is clear; both sides of a leaf lit
    // (a canopy's blob normal already faces out, so it keeps it).
    if (tex.a < 0.35) { discard; }
    base = tex.rgb * s.tint;
    if (!front && s.look != LOOK_CANOPY) { n = -n; }
    // a kit piece's top (a bench, a crate) takes the shadow of whoever stands on it
    if (n.y > 0.7) { shadow = shade_shadows_at(s.world.xz, s.world.y, true); }
  }
  if (s.look == LOOK_ROCK) {
    // a rock: its pale texture taken down to a mid grey-brown, mottled in broad patches, and moss
    // where it faces up (broken by noise), so in full sun it sits under the grass's value, not over it
    let p = s.world;
    let mottle = noise(p.xz * 0.9 + p.y * 0.7) * 0.6 + noise(p.xz * 4.1 - p.y * 3.3) * 0.4;
    base = base * vec3f(0.62, 0.6, 0.55) * (0.78 + 0.34 * mottle);
    let moss = smoothstep(0.35, 0.8, n.y + 0.35 * (noise(p.xz * 2.3 + p.y) - 0.5));
    base = mix(base, vec3f(0.11, 0.2, 0.07) * (0.8 + 0.4 * mottle), moss * 0.85);
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
  // the shapes' and the ground's colours are written as they show on screen (sRGB); the light works in
  // linear, as the kit's textures (sampled sRGB) already are
  if (s.look != 5u && s.look != LOOK_CANOPY && s.look != LOOK_CHARACTER && s.look != LOOK_ROCK) { base = pow(max(base, vec3f(0.0)), vec3f(2.2)); }
  let lit = verse_style_contact(map_lit(s.world, n), 1.0 - shadow, st(4u).w);
  return paint(base, n, s.world, lit, s.look == LOOK_CHARACTER || s.look == 3u, true);
}
// The painterly light (spec 1 to 4): the wrapped key times what of it reaches (lit: the shadow map and
// the movers' blobs) in three soft bands (two for a character), the shadow band the albedo under the
// cool tinted ambient, the warm rim along silhouettes (not on a blade: a field edge on would all be
// rim), then the haze; with the share that blooms and the glow a hazed surface passes the shafts.
fn paint(base: vec3f, n: vec3f, world: vec3f, lit: f32, character: bool, rim: bool) -> Lit {
  let l = st(1u).xyz;
  let v = normalize(world - u.eye.xyz);
  let w = verse_style_wrap(dot(n, l)) * lit;
  let band = select(verse_style_bands(w, st(7u).x, st(7u).y, st(7u).z, st(7u).w), verse_style_bands2(w, st(8u).x, st(8u).y), character);
  let key = st(0u).xyz * st(0u).w;
  let amb = vec3f(verse_style_ambient(n.y, st(5u).x, st(6u).x, st(5u).w), verse_style_ambient(n.y, st(5u).y, st(6u).y, st(5u).w), verse_style_ambient(n.y, st(5u).z, st(6u).z, st(5u).w));
  var c = banded(base, st(4u).xyz * amb, key + amb, band);
  let rs = select(st(2u).w, st(3u).x, character);
  if (rim) { c += st(2u).xyz * verse_style_rim(dot(n, -v), dot(n, l), st(3u).y, st(3u).z, rs) * lit; }
  c += water_caustics(world, n) * base * key * lit;   // light the water focuses on what lies under it
  let d = length(u.eye.xyz - world);
  let fog = haze_share(d, world.y);
  c = mix(c, haze_colour(fog, v), fog);
  let b = st(19u);
  return Lit(vec4f(c, verse_style_bloom_share(verse_style_luma(c.x, c.y, c.z), b.x, b.y)), vec4f(verse_style_shaft_source(dot(v, l)) * fog, 0.0, 0.0, 0.0));
}
// A banded surface (spec 1 and 3): the shadow band takes the tint's hue once more at the preset's
// amount with its value kept (painterly shadows are cool, warm albedos too); the lit band the key.
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

// The water's hooks take the painted sky, the preset's key and its ambient (eg-paint-light).
fn water_host_sky(dir: vec3f, level: f32) -> vec3f {
  let l = st(1u).xyz;
  let d = normalize(vec3f(dir.x, max(dir.y, 0.0), dir.z));
  let sharp = mix(st(10u).xyz, st(12u).xyz, verse_style_sky_t(d.y));
  let blurred = mix(st(10u).xyz, st(12u).xyz, 0.55);
  return mix(sharp, blurred, level * level) + select(st(11u).xyz, st(0u).xyz, st(1u).w > 0.5) * verse_style_halo(dot(d, l), st(14u).z) * 0.25;
}
fn water_host_sun() -> vec4f { return vec4f(st(1u).xyz, select(1.0, 0.0, st(0u).w <= 0.0)); }
fn water_host_sun_light() -> vec3f { return st(0u).xyz * st(0u).w * WATER_SUN_E; }
fn water_host_light(world: vec3f, n: vec3f) -> vec3f {
  let amb = vec3f(verse_style_ambient(n.y, st(5u).x, st(6u).x, st(5u).w), verse_style_ambient(n.y, st(5u).y, st(6u).y, st(5u).w), verse_style_ambient(n.y, st(5u).z, st(6u).z, st(5u).w));
  return amb + st(0u).xyz * st(0u).w * max(dot(n, st(1u).xyz), 0.0) * map_lit(world, n) * shade_shadows(world.xz);
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

// How deep (m) a point lies under the river at rest, or a negative number when it is not under it.
fn water_depth_at(p: vec3f) -> f32 {
  if (u.water.grid.x < 2.0 || u.water.frame.w <= 0.0) { return -1.0; }
  let uv = water_uv(p.xz);
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > u.water.grid.x - 1.0 || uv.y > u.water.grid.y - 1.0) { return -1.0; }
  let r = water_rest(uv);
  if (r.depth <= 0.0) { return -1.0; }
  return r.pos.y - p.y;
}
// Caustics (W7, on what lies under the water): the ridges of two drifting noise fields, the
// light the surface's bumps focus onto the bed, faded in over the first 0.1 m and out with depth,
// on what faces up. Its share of the key light.
fn water_caustic_pattern(p: vec2f, t: f32) -> f32 {
  let a = water_noise(vec3f(p * 1.6, t * 0.45));
  let b = water_noise(vec3f(p * 2.9 + vec2f(3.1, 7.7), t * 0.6 + 4.0));
  let ra = 1.0 - abs(2.0 * a - 1.0);
  let rb = 1.0 - abs(2.0 * b - 1.0);
  return pow(ra, 7.0) * 0.8 + pow(rb, 7.0) * 0.55;
}
fn water_caustics(p: vec3f, n: vec3f) -> f32 {
  let depth = water_depth_at(p);
  if (depth <= 0.0) { return 0.0; }
  let up = smoothstep(-0.2, 0.6, n.y);
  return water_caustic_pattern(p.xz, u.water.params.x) * smoothstep(0.0, 0.1, depth) * exp(-depth * 0.7) * up * 1.4;
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

// ---- the painted look (art-styles.md 12; verse/water-look.coax water_paint_*, the `painted` preset,
// W_ABSORB + 3): colour bands by depth in place of Beer-Lambert, the painted sky's gradient in
// Schlick's Fresnel with no roughness blur, sparkles where a cell's tilt mirrors the key light, a
// clean white shore line with a broken inner edge, W6's ripples as white rings, and faint streaks
// carried along the current. The bed shows through the shallows (its caustics, water_caustics).

const W_SHALLOW = vec3f(0.30, 0.72, 0.74);
const W_MID = vec3f(0.10, 0.45, 0.55);
const W_DEEP = vec3f(0.05, 0.22, 0.36);
fn water_paint_ramp(edge: f32, x: f32) -> f32 { return smoothstep(edge - 0.075, edge + 0.075, x); }
fn water_paint_sparkle(rdl: f32) -> f32 { return smoothstep(0.99756, 0.99939, rdl); }

fn water_painted(world: vec3f, rest: vec2f, flow: vec2f, depth: f32, shore: f32, white: f32, n: vec3f, v: vec3f, footprint: f32) -> WaterShade {
  var o: WaterShade;
  let l = water_host_sun().xyz;
  // the bands, in the light the land has (its shadows fall on the water too)
  let band = mix(mix(W_SHALLOW, W_MID, water_paint_ramp(0.4, depth)), W_DEEP, water_paint_ramp(1.5, depth));
  let light = water_host_light(world, vec3f(0.0, 1.0, 0.0));
  var body = pow(band, vec3f(2.2)) * (light * 0.82 + vec3f(0.05));
  // a painted surface is calmer than the optics': the waves bend its reflection a third as much,
  // and the sky never takes more than half of it, so the bands still show at a grazing look
  let calm = normalize(mix(vec3f(0.0, 1.0, 0.0), n, 0.35));
  let fresnel = min(water_fresnel(max(dot(calm, v), 0.001)), 0.5);
  let refl = water_host_sky(reflect(-v, calm), 0.0);
  // sparkles: a cell of the grid sparkles when its hashed tilt mirrors the key
  let grid = select(0.15, 0.25, u.water.control.x < 8.0);
  let cell = floor(rest / grid);
  let jitter = vec2f(water_hash(vec3f(cell, 1.0)), water_hash(vec3f(cell, 2.0))) - vec2f(0.5);
  let tilt = normalize(n + vec3f(jitter.x, 0.0, jitter.y) * 0.35);
  let rdl = dot(reflect(-v, tilt), l);
  let twinkle = 0.5 + 0.5 * sin(u.time * (1.0 - u.still) * 21.0 + water_hash(vec3f(cell, 3.0)) * 6.28);
  let centre = (cell + vec2f(0.5) + jitter * 0.5) * grid;
  let dot_ = 1.0 - smoothstep(footprint * 1.2, footprint * 2.2, distance(rest, centre));
  let sparkle = water_paint_sparkle(rdl) * twinkle * dot_ * water_host_sun().w * shade_shadows(world.xz);
  // the shore line (0.25 m, its inner edge broken), white water, the rings, streaks along the current
  let broken = (water_noise(vec3f(rest * 3.0, u.time * 0.3)) - 0.5) * 0.12;
  let line = (1.0 - smoothstep(0.17 + broken, 0.27 + broken, shore)) * smoothstep(-0.02, 0.04, depth);
  let noise = water_foam_noise(rest, flow);
  let churn = clamp(white, 0.0, 1.0) * smoothstep(0.3, 0.6, noise + 0.3 * white);
  let rings = clamp(water_ripple_foam(rest) * 1.6, 0.0, 1.0);
  // streaks: thin lines stretched along the current, carried down it at its speed
  let speed = length(flow);
  let along = flow / max(speed, 1e-3);
  let q = vec2f(dot(rest, vec2f(-along.y, along.x)) * 2.4, dot(rest, along) * 0.3 - u.water.params.x * speed * 0.3);
  let ridge = 1.0 - abs(2.0 * water_noise(vec3f(q, u.water.params.x * 0.15)) - 1.0);
  let broken_by = smoothstep(0.4, 0.7, water_noise(vec3f(rest * 0.6, u.water.params.x * 0.1)));
  let streak = smoothstep(0.95, 0.995, ridge) * broken_by * smoothstep(0.05, 0.4, speed) * 0.35 * smoothstep(0.1, 0.5, depth)
    * (1.0 - smoothstep(0.08, 0.25, footprint));
  let foam = clamp(max(max(line, churn), rings) + streak, 0.0, 1.0);
  let alpha = (0.5 + 0.42 * smoothstep(0.05, 1.2, depth)) * smoothstep(-0.05, 0.1, depth);
  let edge = smoothstep(-0.05, 0.1, depth);
  let foam_rgb = vec3f(0.95, 0.97, 0.96) * (light * 0.9 + vec3f(0.06));
  var emit = body * alpha * (1.0 - fresnel) + refl * fresnel * edge + vec3f(1.0, 0.97, 0.88) * sparkle * water_host_sun_light() * 1.2;
  emit = mix(emit, foam_rgb, foam);
  o.emit = emit;
  o.transmit = vec3f((1.0 - alpha) * (1.0 - fresnel * edge) * (1.0 - foam));
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
  if (u.water.absorb.w > 0.5) { return water_painted(world, rest, flow, depth, i.look.w, i.look.x, n, v, footprint); }
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
  let fog = haze_share(d, i.world_depth.y);
  let emit = s.emit * (1.0 - fog) + haze_colour(fog, (i.world_depth.xyz - u.eye.xyz) / max(d, 0.001)) * fog * (vec3f(1.0) - s.transmit);
  // linear light now (the post pass encodes): what lies behind times the transmittance, plus the emitted
  var o: WaterShade;
  o.transmit = s.transmit;
  o.emit = max(emit, vec3f(0.0));
  return o;
}
@fragment fn water_transmit_fs(i: WaterOut) -> @location(0) vec4f {
  return vec4f(clamp(water_halves(i).transmit, vec3f(0.0), vec3f(1.0)), 1.0);
}
@fragment fn water_emit_fs(i: WaterOut) -> @location(0) vec4f {
  return vec4f(water_halves(i).emit, 0.0);
}

// ---- the post pass (host.js postLayouts, art-styles.md 6 and 9): the bright share at a quarter and an
// eighth of the size, blurred, mixed back at the preset's bloom mix; the light shafts; then the tone,
// the encoding and the plates over it.
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
fn tone(c: vec3f) -> vec3f {
  let e = st(18u).x;
  var t = vec3f(verse_style_tone(c.x, e), verse_style_tone(c.y, e), verse_style_tone(c.z, e));
  let y = verse_style_luma(t.x, t.y, t.z);
  t = vec3f(verse_style_saturation(t.x, y, st(18u).y), verse_style_saturation(t.y, y, st(18u).y), verse_style_saturation(t.z, y, st(18u).y));
  return vec3f(verse_style_lift(t.x, st(18u).z), verse_style_lift(t.y, st(18u).z), verse_style_lift(t.z, st(18u).z));
}
@fragment fn post_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let light = textureLoad(post_light, vec2i(pos.xy), 0).rgb;
  let uv = pos.xy / u.size;
  let b = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb * 0.55 + textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb * 0.45;
  let m = st(19u).z;
  var c = vec3f(verse_style_bloom_mix(light.x, b.x, m), verse_style_bloom_mix(light.y, b.y, m), verse_style_bloom_mix(light.z, b.z, m));
  c += shafts(pos.xy, uv) * st(11u).xyz * st(0u).w;
  return vec4f(plates(pos.xy, encode(tone(c) * u.fade)), 1.0);
}
// Light shafts (spec 6): march from the pixel toward the key light's place on screen, gathering the
// sky's glow (aux.r) with each tap weighted decay^i; the canopies and roofs block it, so light streams
// through the gaps. Faded as the light leaves the frame; the march starts a hashed fraction in.
fn shafts(p: vec2f, uv: vec2f) -> f32 {
  let sh = st(20u);
  let k = clip(u.eye.xyz + st(1u).xyz * 1000.0);
  if (k.w <= 0.01 || sh.z <= 0.0) { return 0.0; }
  let at = vec2f(0.5 + 0.5 * k.x / k.w, 0.5 - 0.5 * k.y / k.w);
  let fade = verse_style_shaft_fade(at.x, at.y, sh.w);
  if (fade <= 0.0) { return 0.0; }
  let n = sh.x;
  let step = (at - uv) / n;
  let jitter = fract(52.9829189 * fract(dot(p, vec2f(0.06711056, 0.00583715))));
  var sum = 0.0;
  for (var i = 0.0; i < 24.0; i += 1.0) {
    if (i >= n) { break; }
    let q = uv + step * (i + jitter);
    if (any(q < vec2f(0.0)) || any(q > vec2f(1.0))) { break; }
    sum += textureSampleLevel(post_aux, post_lin, q, 0.0).r * verse_style_shaft_tap(i, sh.y);
  }
  return verse_style_shaft(sum, verse_style_shaft_total(n, sh.y), sh.z, fade);
}

// ---- the grass (art-styles.md 8): one tapered triangle a blade, instanced from a hash of its cell
// (no buffer of blades), in a square two radii across round the eye, world-aligned so blades hold
// still as you walk; behind the eye or past the radius (shrinking over the last 6 m) none. Its colour
// is the meadow's under it, the root in the blades' shade and the tip lighter and warmer; its normal
// the ground's up, tilted toward the light as the wind bends it. None on the streets, the squares,
// the river, the buildings' and props' footprints; the movers (their blob shadows) part it.
const ROOT = 0.78;
fn ground_y(q: vec2f) -> f32 {
  let g = (q - u.ground.zw) / u.ground.y;
  let i = vec2i(floor(g)); let f = fract(g);
  return mix(mix(height(i.x, i.y), height(i.x + 1, i.y), f.x), mix(height(i.x, i.y + 1), height(i.x + 1, i.y + 1), f.x), f.y);
}
fn meadow(q: vec2f) -> vec3f {
  return pow(mix(vec3f(0.24, 0.43, 0.15), vec3f(0.38, 0.58, 0.22), noise(q * 0.13)), vec3f(2.2));
}
fn grass_mask(q: vec2f) -> f32 {
  var m = verse_style_grass_clear(path_distance(q) - u.path_info.y);
  for (var k = 0; k < 4; k++) {
    let sq = u.square[k];
    if (sq.z <= 0.0) { break; }
    m *= verse_style_grass_clear(length(q - sq.xy) - sq.z);
  }
  for (var k = 0; k < 32; k++) {
    let b = u.solid[k];
    if (b.z <= b.x) { continue; }
    let d = max(max(b.x - q.x, q.x - b.z), max(b.y - q.y, q.y - b.w));
    m *= verse_style_grass_clear(d - 0.3);
  }
  return m * verse_style_grass_clear(0.25 - water_over(q) * 5.0);
}
struct G {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) n: vec3f,
  @location(2) albedo: vec3f,
}
@vertex fn grass_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> G {
  var o: G;
  o.pos = vec4f(0.0, 0.0, -1.0, 1.0); o.world = vec3f(0.0); o.n = vec3f(0.0, 1.0, 0.0); o.albedo = vec3f(0.0);
  let g = st(22u); let g2 = st(23u); let w = st(24u); let w2 = st(25u);
  let per = max(u32(g.x), 1u); let r = g.y; let side = u32(2.0 * r);
  // past the far field's blades the near ring's (verse_style_near_*): a square 2 reach across, hashed
  // apart from the far field's by an offset seed, widened toward the eye
  let reach = verse_style_near_reach(st(31u).z);
  let far_n = per * side * side;
  let near = i >= far_n;
  let nper = max(u32(st(26u).w), 1u); let nside = u32(2.0 * reach);
  let j = select(i, i - far_n, near);
  let bper = select(per, nper, near); let bside = select(side, nside, near); let half = select(r, reach, near);
  let cell = j / bper; let b = f32(j % bper) + select(0.0, 101.0, near);
  let c = floor(u.eye.xz) - vec2f(half) + vec2f(f32(cell % bside), f32(cell / bside));
  let k = c + vec2f(b * 7.13, b * -3.71);
  let q = c + vec2f(hash(k), hash(k + vec2f(19.1, 7.7)));
  let rel = q - u.eye.xz;
  let dist = length(rel);
  // behind the eye nothing: the clip test on the root
  let root_clip = clip(vec3f(q.x, ground_y(q), q.y));
  if (root_clip.w < -1.0 || abs(root_clip.x) > root_clip.w * 1.2 + 1.0) { return o; }
  let tall = verse_style_blade_height(hash(k + vec2f(3.3, 41.9)), g.w, g2.x) * verse_style_grass_fade(dist, r, g.z) * grass_mask(q);
  if (tall < 0.02) { return o; }
  let t = select(u.time, 0.0, u.still > 0.5);
  let wd = vec2f(w2.x, w2.y);
  let theta = verse_style_wind(dot(q, wd), verse_style_gust(q.x, q.y, t, wd.x, wd.y, w.z, w.w), t, w.x, w.y, w.z);
  // the movers (their blob shadows: you, the people, the cats) part the blades within 0.7 m
  var lean = wd * verse_style_bend(theta, tall, 1.0);
  for (var m = 0; m < 8; m++) {
    let sh = u.shadow[m];
    if (sh.z <= 0.0) { break; }
    let away = q - sh.xy;
    lean += normalize(away + vec2f(0.0001)) * tall * 0.85 * verse_style_part(length(away), w2.w);
  }
  let ang = hash(k + vec2f(5.9, 13.3)) * 6.2831853;
  let across = vec3f(cos(ang), 0.0, sin(ang)) * g2.y * 0.5 * verse_style_near_width(dist, reach);
  var world = vec3f(q.x, ground_y(q), q.y);
  let base = meadow(q);
  let hj = hash(k + vec2f(31.7, 2.9));
  let a = vec3f(verse_style_jitter(base.x, 0.0, hj, g2.w), verse_style_jitter(base.y, 1.0, hj, g2.w), verse_style_jitter(base.z, 2.0, hj, g2.w));
  if (v == 0u) { world -= across; o.albedo = a * ROOT; }
  else if (v == 1u) { world += across; o.albedo = a * ROOT; }
  else {
    world += vec3f(lean.x, sqrt(max(tall * tall - dot(lean, lean), tall * tall * 0.15)), lean.y);
    o.albedo = vec3f(verse_style_tip(a.x, 0.0, g2.z), verse_style_tip(a.y, 1.0, g2.z), verse_style_tip(a.z, 2.0, g2.z));
  }
  let l = st(1u).xyz;
  let lx = normalize(vec2f(l.x, l.z) + vec2f(0.0001));
  let tilt = verse_style_grass_tilt(theta, w.x, w2.z);
  o.n = normalize(vec3f(lx.x * sin(tilt), cos(tilt), lx.y * sin(tilt)));
  o.pos = clip(world);
  o.world = world;
  return o;
}
@fragment fn grass_fs(g: G) -> Lit {
  return paint(g.albedo, normalize(g.n), g.world, map_lit(g.world, vec3f(0.0, 1.0, 0.0)), false, false);
}

// The art style's shading (verse/style.coax; docs/verse/art-styles.md), fed by the preset's STYLE_FLOATS.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter, verse_style_canopy_axis, verse_style_canopy_normal, verse_style_canopy_sway, verse_style_canopy_open, verse_style_near_reach, verse_style_near_blades, verse_style_near_width) by coax/web/wgsl.py; do not edit
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
fn verse_style_canopy_axis(l_p: f32, l_c: f32, l_half: f32) -> f32 {
  var l_h: f32 = coax_p_st_max(l_half, 0.00100000005);
  return ((l_p - l_c) / (l_h * l_h));
}
fn verse_style_canopy_normal(l_mesh_n: f32, l_blob_n: f32, l_k: f32) -> f32 {
  return (l_mesh_n + ((l_blob_n - l_mesh_n) * l_k));
}
fn verse_style_canopy_sway(l_theta: f32, l_amp: f32, l_reach: f32, l_share: f32) -> f32 {
  var coax_t50: f32;
  var coax_t51: f32;
  var coax_t52: f32;
  var coax_t53: f32;
  coax_t50 = (l_reach * coax_p_st_sat(l_share));
  coax_t52 = (-1.0);
  coax_t53 = coax_p_st_min(1.0, (l_theta / coax_p_st_max(l_amp, 9.99999975e-05)));
  coax_t51 = coax_p_st_max(coax_t52, coax_t53);
  return (coax_t50 * coax_t51);
}
fn verse_style_canopy_open(l_n: f32, l_gap: f32) -> f32 {
  return select(0.0, 1.0, (l_n < l_gap));
}
fn verse_style_near_reach(l_on_phone: f32) -> f32 {
  return select(7.0, 6.0, (l_on_phone > 0.5));
}
fn verse_style_near_blades(l_reach: f32, l_density: f32) -> f32 {
  return (((4.0 * l_reach) * l_reach) * l_density);
}
fn verse_style_near_width(l_d: f32, l_reach: f32) -> f32 {
  return (1.0 + (1.39999998 * (1.0 - coax_p_st_sat((l_d / coax_p_st_max(l_reach, 0.00100000005))))));
}
// ---- end generated
