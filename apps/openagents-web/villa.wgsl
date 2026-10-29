// Hecate's Cemetery's passes (villa.coax): the night sky with the moon in
// its real phase (or space, in the flight's first take), the scene, and a
// post pass that draws a pale outline along edges (verse/look.coax), blooms
// what glows and tones it, under the HUD's plates; then the HUD's strokes.
// Every number comes from villa.coax: the view, the eye, the moon, the
// ground's heights (store 0), each object's place, size, colour and shape
// (store 1), the shadows, the paths and the plates.
//
// The look is Kiki's (verse/planetary-villa/README.md): simple low-poly
// shapes, each face lit flat; the moon the only key light, not very bright,
// cold blue; outlines so shapes and paths read in the dark. The dog glows
// white; you are a shadow, black, your edges fuzzy.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, horizon: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,      // the eye, and 1 while space shows
  flash: vec4f,    // the cut's white flash, 0, 0, 0
  plate: array<vec4f, 32>,   // per plate: x0, y0, x1, y1, then scale, alpha, look, 0
  moon: vec4f,     // toward the moon, and its phase (0 new, 0.5 full)
  sky: vec4f,      // the moonlight's strength, the lit share of the disc
  ground: vec4f,
  shadow: array<vec4f, 16>,
  path: array<vec4f, 48>,
  path_info: vec4f,
}
@group(0) @binding(0) var<uniform> u: U;

const NIGHT_TOP = vec3f(0.006, 0.009, 0.026);
const NIGHT_LOW = vec3f(0.028, 0.038, 0.075);
const FOG = vec3f(0.022, 0.03, 0.06);
const MOONLIGHT = vec3f(0.42, 0.52, 0.8);
const INK = vec3f(0.36, 0.43, 0.62);

struct Lit { @location(0) light: vec4f, @location(1) aux: vec4f }

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn hash3(p: vec3f) -> f32 { return fract(sin(dot(p, vec3f(127.1, 311.7, 74.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}

// ---- the sky

// The ray through a pixel, from the view's rows (villa.coax view).
fn ray(pos: vec2f) -> vec3f {
  let nx = pos.x / u.size.x * 2.0 - 1.0;
  let ny = 1.0 - pos.y / u.size.y * 2.0;
  let fx2 = dot(u.r0.xyz, u.r0.xyz);
  let fy2 = dot(u.r1.xyz, u.r1.xyz);
  return normalize(u.r3.xyz + u.r0.xyz * nx / fx2 + u.r1.xyz * ny / fy2);
}
// Stars: a few bright, many faint, a band of them across space.
fn stars(d: vec3f, many: f32) -> f32 {
  var c = 0.0;
  for (var layer = 0; layer < 2; layer++) {
    let scale = select(90.0, 220.0, layer == 1);
    let q = d * scale;
    let cell = floor(q);
    let h = hash3(cell);
    let keep = select(0.985, 0.97 - 0.05 * many, layer == 1);
    if (h > keep) {
      let at = cell + vec3f(hash3(cell + 1.3), hash3(cell + 2.7), hash3(cell + 4.1));
      let r = length(q - at);
      let twinkle = 0.75 + 0.25 * sin(u.time * (1.0 + 3.0 * h) + h * 40.0) * (1.0 - u.still);
      c += (1.0 - smoothstep(0.0, 0.35, r)) * (h - keep) / (1.0 - keep) * twinkle * select(1.0, 0.55, layer == 1);
    }
  }
  return c;
}
// The moon: a disc lit as the real one is now (the sun's side from its
// phase), a faint earthshine on the dark side, and a halo.
fn moon(d: vec3f) -> vec4f {
  let m = normalize(u.moon.xyz);
  let size = 0.045;
  let c = dot(d, m);
  let side = normalize(cross(vec3f(0.0, 1.0, 0.0), m));
  let up = cross(m, side);
  let x = dot(d, side) / size;
  let y = dot(d, up) / size;
  let r = length(vec2f(x, y));
  let halo = exp(-max(1.0 - c, 0.0) * 160.0) * 0.12 * (0.4 + 0.6 * u.sky.y) + exp(-max(1.0 - c, 0.0) * 1400.0) * 0.18 * u.sky.y;
  if (r >= 1.0 || c < 0.0) { return vec4f(MOONLIGHT * halo, 0.0); }
  let z = sqrt(max(0.0, 1.0 - r * r));
  let a = u.moon.w * 6.2831853;
  let toward_sun = vec3f(-sin(a), 0.0, -cos(a));
  let lambert = dot(vec3f(x, y, z), toward_sun);
  let lit = smoothstep(-0.04, 0.08, lambert);
  let mare = 0.82 + 0.18 * noise(vec2f(x, y) * 3.2 + vec2f(7.0, 1.0));
  let disc = vec3f(0.92, 0.94, 0.88) * mare * (0.05 + 1.85 * lit) + vec3f(0.02, 0.025, 0.04);
  let edge = 1.0 - smoothstep(0.96, 1.0, r);
  return vec4f(mix(MOONLIGHT * halo, disc, edge), edge * lit);
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Lit {
  let d = ray(pos.xy);
  let space = u.eye.w;
  let up = clamp(d.y * 2.5, 0.0, 1.0);
  var sky = mix(NIGHT_LOW, NIGHT_TOP, pow(up, 0.6));
  sky = mix(sky, vec3f(0.004, 0.003, 0.012) + vec3f(0.02, 0.012, 0.04) * pow(1.0 - abs(dot(d, normalize(vec3f(0.3, 0.2, 1.0)))), 6.0), space);
  let s = stars(d, space) * select(clamp(d.y * 6.0, 0.0, 1.0), 1.0, space > 0.5);
  var c = sky + vec3f(0.85, 0.9, 1.0) * s * 1.2;
  let mo = moon(d);
  c = select(c + mo.rgb, mo.rgb, mo.a > 0.0);
  c += vec3f(u.flash.x);
  return Lit(vec4f(c, select(0.0, 1.0, mo.a > 0.2) + u.flash.x), vec4f(0.0));
}

// ---- the scene

struct Obj { r0: vec4f, r1: vec4f, r2: vec4f, colour: vec4f }
@group(1) @binding(0) var<storage, read> heights: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<Obj>;

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
  @location(3) @interpolate(flat) look: u32,
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
fn globe_vertex(id: u32, lons: u32, lats: u32) -> Vert {
  let q = id / 6u;
  let c = quad(id % 6u);
  let lon = (f32(q % lons) + c.x) / f32(lons) * TAU;
  let lat = (f32(q / lons) + c.y) / f32(lats) * 3.14159265;
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
    o.pos = clip(w);
    o.world = w;
    o.normal = normalize(vec3f(height(gx - 1, gz) - height(gx + 1, gz), 2.0 * u.ground.y, height(gx, gz - 1) - height(gx, gz + 1)));
    o.tint = vec3f(0.1, 0.16, 0.12);
    o.look = 99u;
    return o;
  }
  let m = objects[instance - n * n];
  let kind = u32(m.colour.w) % 8u;
  var v: Vert;
  switch kind {
    case 1u: { v = box_vertex(id); }
    case 2u: { v = globe_vertex(id, 16u, 10u); }
    case 3u: { v = cylinder_vertex(id); }
    case 5u: { v = globe_vertex(id, 8u, 6u); }
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
  o.tint = m.colour.rgb;
  o.look = u32(m.colour.w) / 8u;
  return o;
}

fn path_distance(p: vec2f) -> f32 {
  var d = 1000.0;
  let count = i32(u.path_info.x);
  for (var i = 0; i < 48; i++) {
    if (i >= count) { break; }
    let seg = u.path[i];
    let a = seg.xy;
    let ab = seg.zw - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    d = min(d, length(p - a - ab * t));
  }
  return d;
}
fn shade_shadows(p: vec2f) -> f32 {
  var s = 1.0;
  for (var i = 0; i < 16; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    let d = length(p - b.xy) / b.z;
    s *= 1.0 - b.w * (1.0 - smoothstep(0.35, 1.0, d));
  }
  return s;
}

// Each look's material number (an outline is drawn where it changes) and
// its outline's strength, 0 to 15.
fn material(look: u32) -> u32 { return min(look, 14u) + 1u; }
fn ink_of(look: u32) -> u32 {
  if (look == 5u || look == 7u || look == 8u || look == 6u) { return 0u; }   // what glows, the shadow, the planet
  if (look == 99u) { return 6u; }
  return 12u;
}

@fragment fn scene_fs(s: S) -> Lit {
  // Flat faces: the low-poly look comes from the face's own normal.
  let dx = dpdx(s.world);
  let dy = dpdy(s.world);
  let to_eye = normalize(u.eye.xyz - s.world);
  var facet = normalize(cross(dy, dx));
  if (dot(facet, to_eye) < 0.0) { facet = -facet; }
  let smooth_n = normalize(s.normal);
  let moon_d = normalize(u.moon.xyz);
  let key = u.sky.x;
  let d = length(u.eye.xyz - s.world);
  let fog = clamp(1.0 - exp(-max(d - 18.0, 0.0) * 0.022), 0.0, 0.92);

  // You: a shadow, black all through, fuzzy at the edge, a thin rim of moonlight.
  if (s.look == 6u) {
    let facing = abs(dot(smooth_n, to_eye));
    let grain = hash(floor(s.pos.xy) + vec2f(floor(u.time * 24.0) * (1.0 - u.still) * 0.37, 0.0));
    if (grain > smoothstep(0.02, 0.5, facing) * 1.25) { discard; }
    let rim = pow(1.0 - facing, 3.0) * max(dot(smooth_n, moon_d), 0.0) * 0.25 * key;
    let c = mix(vec3f(0.004, 0.005, 0.009) + MOONLIGHT * rim, FOG, fog * 0.6);
    return Lit(vec4f(c, 0.0), vec4f(smooth_n, f32(material(6u) * 16u)));
  }
  // What glows: the dog's white light, softly brighter where it faces you.
  if (s.look == 5u || s.look == 7u) {
    let facing = abs(dot(smooth_n, to_eye));
    let shade = 0.55 + 0.35 * facing + 0.2 * max(dot(smooth_n, moon_d), 0.0);
    let c = s.tint * select(shade, 1.6, s.look == 7u);
    return Lit(vec4f(mix(c, FOG, fog * 0.35), select(0.35, 0.9, s.look == 7u)), vec4f(smooth_n, f32(material(s.look) * 16u)));
  }
  if (s.look == 8u) {
    let n = smooth_n;
    let lambert = max(dot(n, moon_d), 0.0);
    let rim = pow(1.0 - abs(dot(n, to_eye)), 3.0);
    let land = noise(vec2f(atan2(n.z, n.x) * 3.0, n.y * 4.0));
    let base = mix(s.tint, vec3f(0.09, 0.16, 0.12), smoothstep(0.45, 0.6, land));
    let c = base * (0.05 + lambert * 1.3) + vec3f(0.25, 0.35, 0.7) * rim * 0.9;
    return Lit(vec4f(c, rim * 0.5), vec4f(n, f32(material(8u) * 16u)));
  }

  var n = facet;
  var base = s.tint;
  let xz = s.world.xz;
  var shadow = 1.0;
  if (s.look == 99u) {
    let broad = noise(xz * 0.11);
    let fine = noise(xz * 2.3);
    base = mix(vec3f(0.07, 0.12, 0.09), vec3f(0.11, 0.17, 0.12), broad) * (0.9 + 0.15 * fine);
    let pd = path_distance(xz);
    let half = u.path_info.y;
    let on_path = 1.0 - smoothstep(half - 0.08, half + 0.08, pd);
    if (on_path > 0.0) {
      let grit = noise(xz * 4.0) * 0.5 + noise(xz * 11.0) * 0.5;
      base = mix(base, mix(vec3f(0.2, 0.2, 0.22), vec3f(0.27, 0.27, 0.29), grit), on_path);
    }
    // the path's edge: a pale line, so the way reads in the dark
    let edge = 1.0 - smoothstep(0.0, 0.07, abs(pd - half));
    base = mix(base, INK * 0.9, edge * 0.7);
    shadow = shade_shadows(xz);
  } else if (s.look == 1u) {
    base *= 0.85 + 0.3 * noise(s.world.xy * 2.1 + s.world.zx * 1.7);
  } else if (s.look == 2u) {
    base *= 0.9 + 0.12 * noise(s.world.xz * 7.0 + s.world.y * 5.0);
  } else if (s.look == 4u) {
    n = smooth_n;
    base = mix(base, vec3f(0.5, 0.6, 0.8), 0.3 * noise(xz * 2.0 + vec2f(u.time * 0.25 * (1.0 - u.still), 0.0)));
  } else if (s.look == 10u) {
    n = smooth_n;
  }
  let lambert = max(dot(n, moon_d), 0.0);
  let ambient = mix(vec3f(0.035, 0.04, 0.05), vec3f(0.07, 0.09, 0.15), n.y * 0.5 + 0.5);
  var c = base * (ambient + MOONLIGHT * lambert * key * 0.95 * shadow) * mix(0.7, 1.0, shadow);
  if (s.look == 4u) {   // the moon on the water
    let r = reflect(-to_eye, n);
    c += MOONLIGHT * pow(max(dot(r, moon_d), 0.0), 60.0) * 1.2 * key;
  }
  c = mix(c, FOG, fog);
  return Lit(vec4f(c, 0.0), vec4f(n, f32(material(s.look) * 16u + ink_of(s.look))));
}

// ---- the post pass (host.js postLayouts): the bright share, blurred at a quarter and an
// eighth of the size (bloom_fs), added back; the outlines; toned onto the screen under the plates.
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
      c += t.rgb * min(t.a, 1.0);
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

// The outline (verse/look.coax): where the depth jumps, the surface creases
// or the material changes, drawn as a pale moonlit line rather than ink, as
// strongly as the nearer surface's material asks.
fn outlined(p: vec2i, light: vec3f) -> vec3f {
  let a = textureLoad(post_aux, p, 0);
  let tag = u32(round(a.a));
  let strength = f32(tag & 15u) / 15.0;
  if (strength <= 0.0) { return light; }
  let o = i32(max(1.0, floor(u.size.y / 800.0)));
  let top = vec2i(textureDimensions(post_depth)) - 1;
  let at = array(vec2i(-o, 0), vec2i(o, 0), vec2i(0, -o), vec2i(0, o));
  let dc = textureLoad(post_depth, p, 0);
  var d: array<f32, 4>;
  var edge = 0.0;
  for (var k = 0; k < 4; k++) {
    let q = clamp(p + at[k], vec2i(0), top);
    d[k] = textureLoad(post_depth, q, 0);
    let b = textureLoad(post_aux, q, 0);
    if (d[k] < 1.0) {
      edge = max(edge, verse_outline_crease(dot(a.rgb, b.rgb)));
      let other = u32(round(b.a)) >> 4u;
      if (other != tag >> 4u && dc <= d[k] + 0.00001 && (tag >> 4u) > other) { edge = max(edge, 0.7); }
    } else {
      edge = 1.0;   // against the sky
    }
  }
  let lap = max(abs(d[0] + d[1] - 2.0 * dc), abs(d[2] + d[3] - 2.0 * dc));
  edge = max(edge, verse_outline_depth(lap, dc));
  let fade = 1.0 - smoothstep(0.985, 0.999, dc);   // far edges fade into the night
  return mix(light, max(light, INK * (0.35 + 0.65 * u.sky.x)), clamp(edge * strength * fade, 0.0, 1.0) * 0.55);
}

fn tone(c: vec3f) -> vec3f { return vec3f(1.0) - exp(-c * 2.1); }
fn encode(c: vec3f) -> vec3f { return pow(clamp(c, vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2)); }

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 6.0 * s.x;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let dd = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (dd > 1.0) { return under; }
  let over = s.z >= 2.0;
  let fill = select(vec3f(0.02, 0.025, 0.045), vec3f(0.045, 0.055, 0.09), over);
  let line = select(vec3f(0.36, 0.42, 0.58), vec3f(0.62, 0.7, 0.88), over || s.z >= 1.0);
  let w = 1.25 * s.x;
  let inside = clamp(0.5 - dd, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(dd + w * 0.5), 0.0, 1.0);
  let a = clamp(s.y, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1) * 0.8);
}
fn plates(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
    if (f32(k) >= u.plates) { break; }
    c = plate(p, c, u.plate[k * 2], u.plate[k * 2 + 1]);
  }
  return c;
}

@fragment fn post_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy);
  let uv = pos.xy / u.size;
  let light = outlined(p, textureLoad(post_light, p, 0).rgb);
  let near = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb;
  let far = textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb;
  let c = light + near * 0.55 + far * 0.45;
  return vec4f(plates(pos.xy, encode(tone(c))), 1.0);
}

// ---- the HUD's strokes

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

// ---- generated: verse/look.coax (verse_outline_depth, verse_outline_crease) by coax/web/wgsl.py; do not edit
fn coax_p_lk_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_ramp(l_a: f32, l_b: f32, l_x: f32) -> f32 {
  var l_t: f32 = coax_p_lk_min(1.0, coax_p_lk_max(0.0, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_outline_depth(l_lap: f32, l_depth: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  coax_t0 = abs(l_lap);
  coax_t1 = coax_p_lk_max((1.0 - l_depth), 9.99999997e-07);
  var l_jump: f32 = (coax_t0 / coax_t1);
  return coax_p_lk_ramp(0.0399999991, 0.119999997, l_jump);
}
fn verse_outline_crease(l_facing: f32) -> f32 {
  return coax_p_lk_ramp(0.75, 0.550000012, l_facing);
}
// ---- end generated

// The art style's shading (verse/style.coax; docs/verse/art-styles.md), fed by the preset's STYLE_FLOATS.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact) by coax/web/wgsl.py; do not edit
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
  var coax_t28: f32;
  var coax_t29: f32;
  if ((l_lum <= 0.0))
  {
    return 0.0;
  }
  var l_k: f32 = coax_p_st_max(l_knee, 9.99999975e-06);
  coax_t28 = coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0);
  coax_t29 = (2.0 * l_k);
  var l_q: f32 = coax_p_st_min(coax_t28, coax_t29);
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
// ---- end generated
