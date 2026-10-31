// Hecate's Cemetery's passes (villa.coax) in the flat style (docs/verse/art-styles.md,
// Flat): the night sky, one flat dusty blue over a soft horizon band, with
// the moon a flat ivory disc in its real phase (or space, in the flight's
// first take), the scene in three flat tones, and a post pass that blooms only
// what glows, under the HUD's plates; then the HUD's strokes. No outline, no
// specular, no rim: the palette (verse/style.coax FLAT_CEMETERY, dusty blue,
// near-black, ivory and a touch of gold) does the work.
// Every number comes from villa.coax: the view, the eye, the moon, the
// ground's heights (store 0), each object's place, size, colour and shape
// (store 1), the shadows, the paths and the plates.
//
// The look is Kiki's (verse/planetary-villa/README.md): simple low-poly
// shapes, each face lit flat; the moon the only key light. The dog glows
// ivory; you are a shadow, near-black, your edges fuzzy.

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

const MOONLIGHT = vec3f(0.42, 0.52, 0.8);   // the planet's rim in space only

// The palette's slots (verse/style.coax FLAT_*), linear.
const PALETTE = 0.0;   // FLAT_CEMETERY
const P_SKY = 0.0; const P_HORIZON = 1.0; const P_GROUND = 2.0; const P_GROUND2 = 3.0; const P_PATH = 4.0;
const P_LIT = 5.0; const P_SHADE = 6.0; const P_SHADOW = 7.0; const P_ACCENT = 8.0; const P_DARK = 9.0;
const P_LIGHT = 10.0; const P_FOLIAGE = 11.0; const P_FOLIAGE2 = 12.0; const P_WATER = 13.0; const P_WOOD = 14.0;
const P_ROCK = 17.0; const P_SLATE = 18.0;
fn pal(slot: f32) -> vec3f {
  return vec3f(verse_style_flat_colour(PALETTE, slot, 0.0), verse_style_flat_colour(PALETTE, slot, 1.0), verse_style_flat_colour(PALETTE, slot, 2.0));
}
// The flat preset's numbers (verse/style-preset.coax verse_style_flat): the far fade, the hem's
// width, a tall shape's dithered darkening toward its foot and its steps, a flat shadow's strength.
const FADE_FROM = 40.0; const FADE_OVER = 80.0; const FADE_CAP = 0.5;
const HEM = 0.06; const DITHER = 0.25; const DITHER_STEPS = 4.0;

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
// The moon: a flat ivory disc lit as the real one is now (the sun's side from
// its phase), the dark side a flat step of the sky; no halo, no texture.
fn moon(d: vec3f, sky: vec3f) -> vec4f {
  let m = normalize(u.moon.xyz);
  let size = 0.045;
  let c = dot(d, m);
  let side = normalize(cross(vec3f(0.0, 1.0, 0.0), m));
  let up = cross(m, side);
  let x = dot(d, side) / size;
  let y = dot(d, up) / size;
  let r = length(vec2f(x, y));
  if (r >= 1.0 || c < 0.0) { return vec4f(sky, 0.0); }
  let z = sqrt(max(0.0, 1.0 - r * r));
  let a = u.moon.w * 6.2831853;
  let toward_sun = vec3f(-sin(a), 0.0, -cos(a));
  let lit = smoothstep(-0.02, 0.02, dot(vec3f(x, y, z), toward_sun));
  let disc = mix(mix(sky, pal(P_SLATE), 0.5), pal(P_LIGHT), lit);
  let edge = 1.0 - smoothstep(0.97, 1.0, r);
  return vec4f(mix(sky, disc, edge), edge * lit);
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Lit {
  let d = ray(pos.xy);
  let space = u.eye.w;
  // the sky: one flat dusty blue, a soft paler band at the horizon; space (the flight) keeps its stars
  var sky = mix(pal(P_SKY), pal(P_HORIZON), verse_style_flat_sky(d.y));
  let deep = vec3f(0.004, 0.003, 0.012) + vec3f(0.02, 0.012, 0.04) * pow(1.0 - abs(dot(d, normalize(vec3f(0.3, 0.2, 1.0)))), 6.0);
  sky = mix(sky, deep + vec3f(0.85, 0.9, 1.0) * stars(d, space) * 1.2, space);
  let mo = moon(d, sky);
  let c = mo.rgb + vec3f(u.flash.x);
  return Lit(vec4f(c, select(0.0, 0.3, mo.a > 0.2) + u.flash.x), vec4f(0.0));
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
  @location(4) local: vec3f,                       // where on its shape (a unit shape, -0.5 to 0.5)
  @location(5) @interpolate(flat) size: vec2f,     // its height (m), how tall it reads (0 to 1)
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
    o.local = vec3f(0.0);
    o.size = vec2f(0.0);
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
  o.local = v.p;
  o.size = vec2f(length(c1), smoothstep(1.6, 3.2, length(c1)));
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
// The cast shadows: flat translucent shapes, the deepest one where they overlap (0 none, 1 all).
fn shade_shadows(p: vec2f) -> f32 {
  var s = 0.0;
  for (var i = 0; i < 16; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    s = max(s, verse_style_flat_shadow(length(p - b.xy) / b.z) * min(b.w * 1.4, 1.0));
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

// The light from the moon held at least 0.45 up, so a low or set moon never puts the whole
// garden in its dark tone: the cemetery is drawn at night, not in the dark.
fn key_dir() -> vec3f {
  let m = normalize(u.moon.xyz);
  return normalize(vec3f(m.x, max(m.y, 0.45), m.z));
}
// A surface's colour in the palette by its look: the realm keeps its shapes, the palette
// decides their colours.
fn albedo(look: u32, tint: vec3f) -> vec3f {
  let hi = max(tint.r, max(tint.g, tint.b));
  if (look == 1u) { return mix(pal(P_FOLIAGE), pal(P_FOLIAGE2), smoothstep(0.17, 0.22, hi)); }   // cypresses, trees, hedges
  if (look == 2u) {                                                                             // stone: ivory, plaques slate
    if (hi < 0.3) { return pal(P_SLATE); }
    return mix(pal(P_ROCK), pal(P_LIGHT), smoothstep(0.38, 0.5, hi));
  }
  if (look == 3u) { return pal(P_DARK); }                                                        // iron
  if (look == 4u) { return pal(P_WATER); }
  if (look == 9u) { return pal(P_FOLIAGE2); }                                                    // turned soil
  if (look == 10u) { return pal(P_DARK); }
  return pal(P_WOOD);                                                                            // trunks, benches
}

@fragment fn scene_fs(s: S) -> Lit {
  // Flat faces: the low-poly look comes from the face's own normal.
  let dx = dpdx(s.world);
  let dy = dpdy(s.world);
  let ground_aa = max(length(fwidth(s.world.xz)), 0.002);   // derivatives first, while control flow is uniform
  let local_aa = fwidth(s.local.y);
  let to_eye = normalize(u.eye.xyz - s.world);
  var facet = normalize(cross(dy, dx));
  if (dot(facet, to_eye) < 0.0) { facet = -facet; }
  let smooth_n = normalize(s.normal);
  let d = length(u.eye.xyz - s.world);
  let fade = verse_style_flat_fade(d, FADE_FROM, FADE_OVER, FADE_CAP);

  // You: a shadow, near-black all through, fuzzy at the edge.
  if (s.look == 6u) {
    let facing = abs(dot(smooth_n, to_eye));
    let grain = hash(floor(s.pos.xy) + vec2f(floor(u.time * 24.0) * (1.0 - u.still) * 0.37, 0.0));
    if (grain > smoothstep(0.02, 0.5, facing) * 1.25) { discard; }
    return Lit(vec4f(mix(pal(P_DARK) * 0.6, pal(P_HORIZON), fade * 0.6), 0.0), vec4f(smooth_n, f32(material(6u) * 16u)));
  }
  // What glows: the dog's light, one flat ivory.
  if (s.look == 5u || s.look == 7u) {
    let c = mix(pal(P_LIGHT), s.tint, 0.25) * select(1.15, 1.6, s.look == 7u);
    return Lit(vec4f(mix(c, pal(P_HORIZON), fade * 0.35), select(0.35, 0.9, s.look == 7u)), vec4f(smooth_n, f32(material(s.look) * 16u)));
  }
  // The planet, seen in the flight: two flat tones and its land in flat patches.
  if (s.look == 8u) {
    let n = smooth_n;
    let lit = smoothstep(-0.02, 0.02, dot(n, normalize(u.moon.xyz)));
    let land = step(0.52, noise(vec2f(atan2(n.z, n.x) * 3.0, n.y * 4.0)));
    let base = mix(pal(P_SLATE), pal(P_GROUND2), land);
    let c = base * mix(0.25, 1.0, lit);
    return Lit(vec4f(c, 0.0), vec4f(n, f32(material(8u) * 16u)));
  }

  var n = facet;
  let xz = s.world.xz;
  var base = albedo(s.look, s.tint);
  var cover = 0.0;
  if (s.look == 99u) {
    // the ground: one flat dusty blue in broad patches of two, the paths a flat pale band
    // with a thin ivory trim along their edges
    base = mix(pal(P_GROUND), pal(P_GROUND2), step(0.6, noise(xz * 0.045)));
    let pd = path_distance(xz);
    let half = u.path_info.y;
    let aa = ground_aa;
    base = mix(base, pal(P_PATH), 1.0 - smoothstep(half - aa, half + aa, pd));
    base = mix(base, pal(P_LIGHT), verse_style_flat_hem(abs(pd - half), 0.035, aa) * 0.7);
    cover = shade_shadows(xz);
  } else if (s.look == 4u || s.look == 10u) {
    n = smooth_n;
  }
  // a tall shape steps a little darker toward its foot, dithered where the steps meet
  if (s.size.y > 0.0 && s.look != 99u) {
    let t = verse_style_flat_dither(0.5 - s.local.y, s.pos.x, s.pos.y, DITHER_STEPS);
    base = mix(base, base * pal(P_SHADE) * 0.8, t * DITHER * s.size.y);
  }
  // trim: a thin gold line along the top of the iron (the gate's bar, the railings' tips)
  if (s.look == 3u) {
    let from_top = (0.5 - s.local.y) * s.size.x;
    base = mix(base, pal(P_ACCENT), verse_style_flat_hem(from_top, 0.03, max(local_aa * s.size.x, 0.001)));
  }
  // three flat tones, lit, shade and dark; no specular; the cast shadow a flat translucent shape
  let band = verse_style_flat_band(verse_style_flat_light(dot(n, key_dir()), 1.0));
  var c = base * mix(pal(P_SHADE), pal(P_LIT), band);
  c *= mix(vec3f(1.0), pal(P_SHADOW), cover);
  c = mix(c, pal(P_HORIZON), fade);
  return Lit(vec4f(c, 0.0), vec4f(n, f32(material(s.look) * 16u + ink_of(s.look))));
}

// ---- the post pass (host.js postLayouts): the bright share, blurred at a quarter and an
// eighth of the size (bloom_fs), added back gently (only what glows has a share); no outline and
// no tone curve, so the palette shows as it is; under the plates.
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
  let light = textureLoad(post_light, p, 0).rgb;
  let near = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb;
  let far = textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb;
  let c = light + (near * 0.55 + far * 0.45) * 0.6;
  return vec4f(plates(pos.xy, encode(c)), 1.0);
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
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter, verse_style_flat_light, verse_style_flat_band, verse_style_flat_band2, verse_style_flat_sky, verse_style_flat_shadow, verse_style_flat_fade, verse_style_bayer, verse_style_flat_dither, verse_style_flat_hem, verse_style_flat_srgb, verse_style_flat_colour, verse_style_flat_time) by coax/web/wgsl.py; do not edit
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
fn verse_style_flat_light(l_fl_ndl: f32, l_fl_lit: f32) -> f32 {
  var coax_t54: f32;
  var coax_t55: f32;
  coax_t54 = coax_p_st_sat(((l_fl_ndl * 0.5) + 0.5));
  coax_t55 = coax_p_st_sat(l_fl_lit);
  return (coax_t54 * coax_t55);
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
