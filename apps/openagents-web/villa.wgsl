// Hecate's Cemetery's passes (villa.coax) in the flat style, faceted (docs/verse/art-styles.md,
// Flat and Flat: facets): a plain periwinkle night sky over a soft lilac horizon band, with
// the moon a flat cream disc in its real phase (or space, in the flight's first take), the
// scene flat-shaded low poly (each face one tone from its own normal, round shapes coarse and
// cut), soft lavender cast shadows stretched away from the moon, and a post pass that draws a
// crisp light line along each shape's outline and blooms only what glows, under the HUD's
// plates; then the HUD's strokes. No specular, no tone curve: the palette (verse/style.coax
// FLAT_CEMETERY, moonlit pastels) and each shape's own pastel (LOOK_PAINT) do the work.
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
  shadow: array<vec4f, 32>,
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
// Low poly (art-styles.md, Flat: facets): a sphere is coarse, eight faces round and five down, the
// rest of its lons x lats grid collapsed to nothing; `nudge` moves each corner in or out by up to that
// share of the radius (verse_style_facet_jitter, seeded per shape), so a crown or a stone reads as cut.
const ROUND = 8.0; const DOWN = 5.0;
fn nothing() -> Vert { return Vert(vec3f(0.0), vec3f(0.0, 1.0, 0.0)); }
fn globe_vertex(id: u32, lons: u32, lats: u32, nudge: f32, seed: f32) -> Vert {
  let q = id / 6u;
  let c = quad(id % 6u);
  if (q % lons >= 8u || q / lons >= lats) { return nothing(); }
  let i = f32(q % lons) + c.x;
  let j = f32(q / lons) + c.y;
  let lon = i / ROUND * TAU;
  let lat = j / DOWN * 3.14159265;
  let n = vec3f(sin(lat) * cos(lon), cos(lat), sin(lat) * sin(lon));
  return Vert(n * 0.5 * (1.0 + nudge * verse_style_facet_jitter(i, j, ROUND, DOWN, seed)), n);
}
// Eight flat sides and flat caps: 48 side vertices, then 8 triangles a cap, top then bottom.
fn cylinder_vertex(id: u32) -> Vert {
  if (id < 48u) {
    let c = quad(id % 6u);
    let a = (f32(id / 6u) + c.x) / ROUND * TAU;
    let n = vec3f(cos(a), 0.0, sin(a));
    return Vert(vec3f(n.x * 0.5, c.y - 0.5, n.z * 0.5), n);
  }
  let t = (id - 48u) / 3u;
  let k = (id - 48u) % 3u;
  let top = t < 8u;
  let y = select(-0.5, 0.5, top);
  let n = vec3f(0.0, select(-1.0, 1.0, top), 0.0);
  if (k == 0u) { return Vert(vec3f(0.0, y, 0.0), n); }
  let a = (f32(t % 8u) + f32(k - 1u)) / ROUND * TAU;
  return Vert(vec3f(cos(a) * 0.5, y, sin(a) * 0.5), n);
}
// Eight sides to its point (24 vertices), then its base (24).
fn cone_vertex(id: u32) -> Vert {
  let t = (id % 24u) / 3u;
  let k = id % 3u;
  if (id < 24u) {
    let mid = (f32(t) + 0.5) / ROUND * TAU;
    if (k == 0u) { return Vert(vec3f(0.0, 0.5, 0.0), normalize(vec3f(cos(mid), 0.5, sin(mid)))); }
    let a = (f32(t) + f32(k - 1u)) / ROUND * TAU;
    return Vert(vec3f(cos(a) * 0.5, -0.5, sin(a) * 0.5), normalize(vec3f(cos(a), 0.5, sin(a))));
  }
  let n = vec3f(0.0, -1.0, 0.0);
  if (k == 0u) { return Vert(vec3f(0.0, -0.5, 0.0), n); }
  let a = (f32(t) + f32(k - 1u)) / ROUND * TAU;
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
  let shape_look = u32(m.colour.w) / 8u;
  // leaves, stones, soil and cut shapes are nudged irregular; figures, what glows and the paint keep even faces
  let nudge = select(0.0, 0.16, shape_look == 1u || shape_look == 2u || shape_look == 9u || shape_look == 12u);
  let seed = fract(m.r0.w * 0.137 + m.r2.w * 0.071 + m.r1.w * 0.29);
  var v: Vert;
  switch kind {
    case 1u: { v = box_vertex(id); }
    case 2u: { v = globe_vertex(id, 8u, 5u, nudge, seed); }
    case 3u: { v = cylinder_vertex(id); }
    case 5u: { v = globe_vertex(id, 8u, 5u, nudge, seed); }
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
// The cast shadows: soft flat shapes stretched away from the moon, the deepest one where they
// overlap (0 none, 1 all). Each is a foot (x, z, radius) and, packed in w, its length away from
// the moon in tenths of a metre (the whole part) and its strength (the fraction).
fn shade_shadows(p: vec2f) -> f32 {
  var s = 0.0;
  let k = key_dir();
  let away = -normalize(vec2f(k.x, k.z) + vec2f(0.0001, 0.0));
  for (var i = 0; i < 32; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    let reach = floor(b.w) * 0.1;
    let strength = fract(b.w);
    let t = clamp(dot(p - b.xy, away), 0.0, reach);
    let q = p - b.xy - away * t;
    // narrowing a little toward its far end, as a crown's shadow does
    let r = b.z * (1.0 - 0.35 * t / max(reach, 0.001));
    s = max(s, verse_style_flat_shadow(length(q) / r) * min(strength * 1.4, 1.0));
  }
  return s;
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
  if (look == 11u || look == 12u || look == 13u) { return pow(clamp(tint, vec3f(0.0), vec3f(1.0)), vec3f(2.2)); }   // a shape's own pastel (sRGB)
  return pal(P_WOOD);                                                                            // trunks, benches
}

@fragment fn scene_fs(s: S) -> Lit {
  // Flat faces: the low-poly look comes from the face's own normal.
  let dx = dpdx(s.world);
  let dy = dpdy(s.world);
  let ground_aa = max(length(fwidth(s.world.xz)), 0.002);   // derivatives first, while control flow is uniform
  let local_aa = fwidth(s.local.y);
  let to_eye = normalize(u.eye.xyz - s.world);
  var facet = cross(dy, dx);
  facet = normalize(select(facet, vec3f(0.0, 1.0, 0.0), dot(facet, facet) < 1e-14));
  if (dot(facet, to_eye) < 0.0) { facet = -facet; }
  let smooth_n = normalize(s.normal);
  let d = length(u.eye.xyz - s.world);
  let fade = verse_style_flat_fade(d, FADE_FROM, FADE_OVER, FADE_CAP);

  // You: a spirit visiting from another realm, pale lavender light, solid at its heart and
  // dissolving toward its rim in a fine ordered dither that drifts, so the bloom and the wisps
  // rising off you (villa.coax particles) make it soft and fuzzy.
  if (s.look == 6u) {
    let facing = abs(dot(smooth_n, to_eye));
    let drift = floor(u.time * 12.0) * (1.0 - u.still);
    let body = smoothstep(0.0, 0.8, facing);
    let cell = floor(s.pos.xy / max(u.k, 1.0)) + vec2f(drift, drift * 2.0);
    // a third of its heart and nearly all of its rim let what is behind it show
    if (verse_style_bayer(cell.x, cell.y) > body * 0.68) { discard; }
    let core = mix(vec3f(0.5, 0.55, 0.92), vec3f(0.8, 0.82, 1.0), body);
    return Lit(vec4f(mix(core, pal(P_HORIZON), fade * 0.5), 0.22), vec4f(smooth_n, -d));
  }
  // What glows: the dog's light, one flat cream.
  if (s.look == 5u || s.look == 7u) {
    // the dog's own colours (its coat, markings and collar) show through its light
    let c = mix(pal(P_LIGHT), s.tint, select(0.8, 0.25, s.look == 7u)) * select(1.15, 1.6, s.look == 7u);
    return Lit(vec4f(mix(c, pal(P_HORIZON), fade * 0.35), select(0.35, 0.9, s.look == 7u)), vec4f(smooth_n, -d));
  }
  // The planet, seen in the flight: two flat tones and its land in flat patches.
  if (s.look == 8u) {
    let n = smooth_n;
    let lit = smoothstep(-0.02, 0.02, dot(n, normalize(u.moon.xyz)));
    let land = step(0.52, noise(vec2f(atan2(n.z, n.x) * 3.0, n.y * 4.0)));
    let base = mix(pal(P_SLATE), pal(P_GROUND2), land);
    let c = base * mix(0.25, 1.0, lit);
    return Lit(vec4f(c, 0.0), vec4f(n, -d));
  }

  var n = facet;
  let xz = s.world.xz;
  var base = albedo(s.look, s.tint);
  var cover = 0.0;
  var edge = 1.0;   // a light outline where this surface stands in front of something further (the post pass)
  if (s.look == 99u) {
    // the ground: soft sage in broad patches of two, its own faces showing; the paths a flat
    // cream band with a thin light trim along their edges
    base = mix(pal(P_GROUND), pal(P_GROUND2), step(0.6, noise(xz * 0.045)));
    let pd = path_distance(xz);
    let half = u.path_info.y;
    let aa = ground_aa;
    base = mix(base, pal(P_PATH), 1.0 - smoothstep(half - aa, half + aa, pd));
    base = mix(base, pal(P_LIGHT), verse_style_flat_hem(abs(pd - half), 0.035, aa) * 0.7);
    cover = shade_shadows(xz);
    edge = 0.0;
  } else if (s.look == 4u) {
    edge = 0.0;
  } else if (s.look == 10u) {
    n = smooth_n;
  }
  // trim: a thin cream line along the top of the iron (the gate's bar, the railings' tips)
  if (s.look == 3u) {
    let from_top = (0.5 - s.local.y) * s.size.x;
    base = mix(base, pal(P_ACCENT), verse_style_flat_hem(from_top, 0.03, max(local_aa * s.size.x, 0.001)));
  }
  // one tone a face (art-styles.md, Flat: facets), between the palette's shade and lit; no specular;
  // the cast shadow a soft lavender shape on the ground
  let band = verse_style_facet_tone(dot(n, key_dir()), 1.0);
  var c = base * mix(pal(P_SHADE), pal(P_LIT), band);
  c *= mix(vec3f(1.0), pal(P_SHADOW), cover);
  c = mix(c, pal(P_HORIZON), fade);
  // the outline only where the shape reads at its distance: tiny far things (a tuft, a petal) get none
  let reads = max(s.size.x, 0.0) / max(d, 0.1) > 0.035 || s.look == 99u;
  // your dog (look 13): its own coat, markings and collar, faceted like the rest, with a soft
  // moonlit rim round its outline and a little of it in the bloom, so it is still a spirit's dog
  var share = 0.0;
  if (s.look == 13u) {
    edge = 0.0;   // its bones meet inside one soft coat: no line between them
    let rim = pow(1.0 - abs(dot(smooth_n, to_eye)), 2.5);
    c = mix(c, vec3f(0.92, 0.94, 1.0), rim * 0.55);
    share = 0.08 + 0.3 * rim;
  }
  return Lit(vec4f(c, share), vec4f(n, select(-d, d, edge > 0.5 && reads)));
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
  var c = light + (near * 0.55 + far * 0.45) * 0.6;
  // the crisp light line along a shape's outline (art-styles.md, Flat: facets): on the near side of
  // an edge, where a neighbour lies clearly further off (or is sky), fading out with distance
  let own = textureLoad(post_aux, p, 0).w;
  if (own > 0.0 && own < 140.0 && u.eye.w < 0.5) {
    let hi = vec2i(u.size) - vec2i(1);
    var e = verse_style_facet_edge(own, textureLoad(post_aux, clamp(p + vec2i(1, 0), vec2i(0), hi), 0).w);
    e = max(e, verse_style_facet_edge(own, textureLoad(post_aux, clamp(p - vec2i(1, 0), vec2i(0), hi), 0).w));
    e = max(e, verse_style_facet_edge(own, textureLoad(post_aux, clamp(p + vec2i(0, 1), vec2i(0), hi), 0).w));
    e = max(e, verse_style_facet_edge(own, textureLoad(post_aux, clamp(p - vec2i(0, 1), vec2i(0), hi), 0).w));
    c = mix(c, pal(P_ACCENT), e * 0.8 * (1.0 - smoothstep(60.0, 140.0, own)));
  }
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

// ---- generated: verse/look.coax (verse_outline_depth, verse_outline_crease) by coax wgsl; do not edit
fn verse_outline_depth(p_lap: f32, p_depth: f32) -> f32 {
  var l_lap: f32 = p_lap;
  var l_depth: f32 = p_depth;
  var l_jump: f32;
  l_jump = (abs(l_lap) / coax_p_lk_max((0x1.000000p0f - l_depth), 0x1.0c6f7ap-20f));
  return coax_p_lk_ramp(0x1.47ae14p-5f, 0x1.eb851ep-4f, l_jump);
}
fn verse_outline_crease(p_facing: f32) -> f32 {
  var l_facing: f32 = p_facing;
  return coax_p_lk_ramp(0x1.800000p-1f, 0x1.19999ap-1f, l_facing);
}
fn coax_p_lk_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_lk_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
// ---- end generated

// The art style's shading (verse/style.coax; docs/verse/art-styles.md), fed by the preset's STYLE_FLOATS.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter, verse_style_flat_light, verse_style_flat_band, verse_style_flat_band2, verse_style_flat_sky, verse_style_flat_shadow, verse_style_flat_fade, verse_style_bayer, verse_style_flat_dither, verse_style_flat_hem, verse_style_flat_srgb, verse_style_flat_colour, verse_style_flat_time, verse_style_facet_tone, verse_style_facet_jitter, verse_style_facet_edge) by coax wgsl; do not edit
fn verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn verse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e0: f32 = p_e0;
  var l_e1: f32 = p_e1;
  var l_s: f32 = p_s;
  var l_mid: f32 = p_mid;
  return ((l_mid * coax_p_verse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_verse_style_soft_step(l_e1, l_s, l_w)));
}
fn verse_style_bands2(p_w: f32, p_e: f32, p_s: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  return coax_p_verse_style_soft_step(l_e, l_s, l_w);
}
fn verse_style_shade(p_albedo: f32, p_shadow_light: f32, p_lit_light: f32, p_band: f32) -> f32 {
  var l_albedo: f32 = p_albedo;
  var l_shadow_light: f32 = p_shadow_light;
  var l_lit_light: f32 = p_lit_light;
  var l_band: f32 = p_band;
  return (l_albedo * (l_shadow_light + ((l_lit_light - l_shadow_light) * coax_p_st_sat(l_band))));
}
fn verse_style_cool(p_c: f32, p_tint: f32, p_tint_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_tint: f32 = p_tint;
  var l_tint_lum: f32 = p_tint_lum;
  var l_amount: f32 = p_amount;
  return (l_c * coax_pow_verse_style_wrap((coax_p_st_max(l_tint, 0x1.0c6f7ap-20f) / coax_p_st_max(l_tint_lum, 0x1.0c6f7ap-20f)), coax_p_st_max(l_amount, 0.0f)));
}
fn verse_style_keep_value(p_c: f32, p_before: f32, p_after: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_before: f32 = p_before;
  var l_after: f32 = p_after;
  return ((l_c * l_before) / coax_p_st_max(l_after, 0x1.0c6f7ap-20f));
}
fn verse_style_banded(p_shadow_c: f32, p_lit_c: f32, p_band: f32) -> f32 {
  var l_shadow_c: f32 = p_shadow_c;
  var l_lit_c: f32 = p_lit_c;
  var l_band: f32 = p_band;
  return (l_shadow_c + ((l_lit_c - l_shadow_c) * coax_p_st_sat(l_band)));
}
fn verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn verse_style_dab(p_rdl: f32, p_cut: f32, p_strength: f32) -> f32 {
  var l_rdl: f32 = p_rdl;
  var l_cut: f32 = p_cut;
  var l_strength: f32 = p_strength;
  return select(0.0f, l_strength, (l_rdl >= l_cut));
}
fn verse_style_rim(p_ndv: f32, p_ndl: f32, p_start: f32, p_full: f32, p_strength: f32) -> f32 {
  var l_ndv: f32 = p_ndv;
  var l_ndl: f32 = p_ndl;
  var l_start: f32 = p_start;
  var l_full: f32 = p_full;
  var l_strength: f32 = p_strength;
  var l_t: f32;
  var l_edge: f32;
  l_t = coax_p_st_sat(((l_ndv - l_start) / (l_full - l_start)));
  l_edge = ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
  return ((l_edge * (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_st_sat((l_ndl + 0x1.333334p-2f))))) * l_strength);
}
fn verse_style_haze(p_d: f32, p_y: f32, p_base_y: f32, p_start: f32, p_density: f32, p_cap: f32, p_falloff: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_y: f32 = p_y;
  var l_base_y: f32 = p_base_y;
  var l_start: f32 = p_start;
  var l_density: f32 = p_density;
  var l_cap: f32 = p_cap;
  var l_falloff: f32 = p_falloff;
  var l_dist: f32;
  var l_height: f32;
  l_dist = (0x1.000000p0f - exp(((-l_density) * coax_p_st_max((l_d - l_start), 0.0f))));
  l_height = exp(((-coax_p_st_max((l_y - l_base_y), 0.0f)) / coax_p_st_max(l_falloff, 0x1.0624dep-10f)));
  return coax_p_st_min((l_dist * (0x1.000000p0f + ((l_height - 0x1.000000p0f) * 0x1.333334p-1f))), l_cap);
}
fn verse_style_haze_glow(p_vdl: f32) -> f32 {
  var l_vdl: f32 = p_vdl;
  var l_a: f32;
  var l_a2: f32;
  var l_a4: f32;
  l_a = coax_p_st_sat(l_vdl);
  l_a2 = (l_a * l_a);
  l_a4 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0x1.ccccccp-2f);
}
fn verse_style_haze_teal(p_ch: f32) -> f32 {
  var l_ch: f32 = p_ch;
  if ((l_ch < 0x1.000000p-1f))
  {
    return 0x1.666666p-1f;
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return 0x1.000000p0f;
  }
  return 0x1.ae147ap-1f;
}
fn verse_style_haze_hue(p_h: f32, p_ch: f32, p_f: f32, p_cap: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_ch: f32 = p_ch;
  var l_f: f32 = p_f;
  var l_cap: f32 = p_cap;
  var l_t: f32;
  var l_far: f32;
  l_t = verse_style_haze_teal(l_ch);
  l_far = coax_p_verse_style_soft_step(((0x1.333334p-3f + l_cap) * 0x1.000000p-1f), ((l_cap - 0x1.333334p-3f) * 0x1.000000p-1f), l_f);
  return (l_h * (l_t + ((0x1.000000p0f - l_t) * l_far)));
}
fn verse_style_fog(p_c: f32, p_haze_c: f32, p_f: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_haze_c: f32 = p_haze_c;
  var l_f: f32 = p_f;
  return (l_c + ((l_haze_c - l_c) * coax_p_st_sat(l_f)));
}
fn verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_verse_style_wrap(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn verse_style_sky_band(p_up: f32, p_lift: f32) -> f32 {
  var l_up: f32 = p_up;
  var l_lift: f32 = p_lift;
  return (l_lift * (0x1.000000p0f - coax_p_st_sat((abs(l_up) / 0x1.ac0832p-4f))));
}
fn verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn verse_style_noise(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = verse_style_hash(l_ix, l_iy);
  l_b = verse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = verse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = verse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn verse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_s1: f32 = p_s1;
  var l_s2: f32 = p_s2;
  var l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (verse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn verse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_cover: f32 = p_cover;
  var l_cut_lo: f32 = p_cut_lo;
  var l_cut_hi: f32 = p_cut_hi;
  return (coax_p_verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_verse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn verse_style_cloud_lit(p_n: f32, p_toward: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_toward: f32 = p_toward;
  return coax_p_st_sat((((l_n - l_toward) * 0x1.400000p2f) + 0x1.19999ap-1f));
}
fn verse_style_cloud_fade(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return (coax_p_verse_style_soft_step(0x1.5c28f6p-4f, 0x1.333334p-4f, l_up) * 0x1.d70a3ep-1f);
}
fn verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn verse_style_lift(p_c: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lift: f32 = p_lift;
  return (l_lift + ((0x1.000000p0f - l_lift) * l_c));
}
fn verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn verse_style_bloom_share(p_lum: f32, p_threshold: f32, p_knee: f32) -> f32 {
  var l_lum: f32 = p_lum;
  var l_threshold: f32 = p_threshold;
  var l_knee: f32 = p_knee;
  var l_k: f32;
  var l_q: f32;
  if ((l_lum <= 0.0f))
  {
    return 0.0f;
  }
  l_k = coax_p_st_max(l_knee, 0x1.4f8b58p-17f);
  l_q = coax_p_st_min(coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0f), (0x1.000000p1f * l_k));
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (0x1.000000p2f * l_k)), (l_lum - l_threshold)) / l_lum), 0x1.000000p0f);
}
fn verse_style_bloom_mix(p_c: f32, p_bloom: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_bloom: f32 = p_bloom;
  var l_amount: f32 = p_amount;
  return (l_c + (l_bloom * l_amount));
}
fn verse_style_shadow_reach(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    return 0x1.400000p5f;
  }
  return 0x1.e00000p5f;
}
fn verse_style_shadow_snap(p_x: f32, p_reach: f32, p_size: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  var l_size: f32 = p_size;
  var l_texel: f32;
  l_texel = ((0x1.000000p1f * l_reach) / coax_p_st_max(l_size, 0x1.000000p0f));
  return (floor(((l_x / l_texel) + 0x1.000000p-1f)) * l_texel);
}
fn verse_style_shadow_across(p_x: f32, p_reach: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  return (l_x / coax_p_st_max(l_reach, 0x1.0624dep-10f));
}
fn verse_style_shadow_depth(p_along: f32, p_reach: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_reach: f32 = p_reach;
  return (0x1.000000p-1f - (l_along / (0x1.000000p2f * coax_p_st_max(l_reach, 0x1.0624dep-10f))));
}
fn verse_style_shadow_taps(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    return 0x1.000000p2f;
  }
  return 0x1.200000p3f;
}
fn verse_style_shadow_radius(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  return (0x1.400000p1f + (0.0f * l_on_phone));
}
fn verse_style_shadow_tap(p_k: f32, p_on_phone: f32, p_which: f32) -> f32 {
  var l_k: f32 = p_k;
  var l_on_phone: f32 = p_on_phone;
  var l_which: f32 = p_which;
  var l_x: f32;
  var l_y: f32;
  l_x = 0.0f;
  l_y = 0.0f;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = (-0x1.333334p-1f);
      l_y = (-0x1.000000p-2f);
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = 0x1.000000p-2f;
        l_y = (-0x1.333334p-1f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = 0x1.333334p-1f;
          l_y = 0x1.000000p-2f;
        }
        else
        {
          l_x = (-0x1.000000p-2f);
          l_y = 0x1.333334p-1f;
        }
      }
    }
  }
  else
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = 0.0f;
      l_y = 0.0f;
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = (-0x1.4dd2f2p-2f);
        l_y = (-0x1.9fbe76p-2f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = (-0x1.ae147ap-1f);
          l_y = (-0x1.2f1aa0p-4f);
        }
        else
        {
          if ((l_k < 0x1.c00000p1f))
          {
            l_x = (-0x1.645a1cp-1f);
            l_y = 0x1.d3f7cep-2f;
          }
          else
          {
            if ((l_k < 0x1.200000p2f))
            {
              l_x = (-0x1.9fbe76p-3f);
              l_y = 0x1.3df3b6p-1f;
            }
            else
            {
              if ((l_k < 0x1.600000p2f))
              {
                l_x = 0x1.ec8b44p-1f;
                l_y = (-0x1.8f5c28p-3f);
              }
              else
              {
                if ((l_k < 0x1.a00000p2f))
                {
                  l_x = 0x1.e45a1cp-2f;
                  l_y = (-0x1.eb851ep-2f);
                }
                else
                {
                  if ((l_k < 0x1.e00000p2f))
                  {
                    l_x = 0x1.09ba5ep-1f;
                    l_y = 0x1.88b43ap-1f;
                  }
                  else
                  {
                    l_x = 0x1.7ae148p-3f;
                    l_y = (-0x1.c9374cp-1f);
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if ((l_which > 0x1.000000p-1f))
  {
    return l_y;
  }
  return l_x;
}
fn verse_style_contact(p_cast_lit: f32, p_blob: f32, p_strength: f32) -> f32 {
  var l_cast_lit: f32 = p_cast_lit;
  var l_blob: f32 = p_blob;
  var l_strength: f32 = p_strength;
  return coax_p_st_min(l_cast_lit, (0x1.000000p0f - (l_strength * coax_p_st_sat(l_blob))));
}
fn verse_style_shaft_source(p_cos_t: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  return exp(((-0x1.800000p4f) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn verse_style_shaft_fade(p_x: f32, p_y: f32, p_margin: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_margin: f32 = p_margin;
  var l_out: f32;
  l_out = coax_p_st_max(coax_p_st_max(coax_p_st_max((-l_x), (l_x - 0x1.000000p0f)), coax_p_st_max((-l_y), (l_y - 0x1.000000p0f))), 0.0f);
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_margin * 0x1.000000p-1f), (l_margin * 0x1.000000p-1f), l_out));
}
fn verse_style_shaft_tap(p_i: f32, p_decay: f32) -> f32 {
  var l_i: f32 = p_i;
  var l_decay: f32 = p_decay;
  return coax_pow_verse_style_wrap(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_i);
}
fn verse_style_shaft_total(p_n: f32, p_decay: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_decay: f32 = p_decay;
  if ((abs((0x1.000000p0f - l_decay)) < 0x1.0c6f7ap-20f))
  {
    return l_n;
  }
  return ((0x1.000000p0f - coax_pow_verse_style_wrap(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_n)) / (0x1.000000p0f - l_decay));
}
fn verse_style_shaft(p_sum: f32, p_total: f32, p_weight: f32, p_fade: f32) -> f32 {
  var l_sum: f32 = p_sum;
  var l_total: f32 = p_total;
  var l_weight: f32 = p_weight;
  var l_fade: f32 = p_fade;
  return (((l_sum / coax_p_st_max(l_total, 0x1.0c6f7ap-20f)) * l_weight) * coax_p_st_sat(l_fade));
}
fn verse_style_gust(p_x: f32, p_z: f32, p_t: f32, p_wx: f32, p_wz: f32, p_speed: f32, p_scale: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: f32 = p_t;
  var l_wx: f32 = p_wx;
  var l_wz: f32 = p_wz;
  var l_speed: f32 = p_speed;
  var l_scale: f32 = p_scale;
  return ((0x1.000000p1f * verse_style_noise(((l_x - ((l_wx * l_speed) * l_t)) * l_scale), ((l_z - ((l_wz * l_speed) * l_t)) * l_scale))) - 0x1.000000p0f);
}
fn verse_style_wind(p_along: f32, p_gust: f32, p_t: f32, p_amp: f32, p_wavelength: f32, p_speed: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_gust: f32 = p_gust;
  var l_t: f32 = p_t;
  var l_amp: f32 = p_amp;
  var l_wavelength: f32 = p_wavelength;
  var l_speed: f32 = p_speed;
  var l_k: f32;
  l_k = (0x1.921fb6p2f / coax_p_st_max(l_wavelength, 0x1.0624dep-10f));
  return (l_amp * ((0x1.333334p-1f * sin(((l_k * l_along) - ((l_k * l_speed) * l_t)))) + (0x1.99999ap-2f * l_gust)));
}
fn verse_style_bend(p_theta: f32, p_height: f32, p_share: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_height: f32 = p_height;
  var l_share: f32 = p_share;
  return (((sin(l_theta) * l_height) * l_share) * l_share);
}
fn verse_style_grass_tilt(p_theta: f32, p_amp: f32, p_tilt_most: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_amp: f32 = p_amp;
  var l_tilt_most: f32 = p_tilt_most;
  return (l_tilt_most * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, (l_theta / coax_p_st_max(l_amp, 0x1.a36e2ep-14f)))));
}
fn verse_style_blade_height(p_h: f32, p_low: f32, p_high: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return (l_low + ((l_high - l_low) * coax_p_st_sat(l_h)));
}
fn verse_style_grass_fade(p_d: f32, p_radius: f32, p_fade: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  var l_fade: f32 = p_fade;
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_radius - (l_fade * 0x1.000000p-1f)), (l_fade * 0x1.000000p-1f), l_d));
}
fn verse_style_grass_blades(p_radius: f32, p_density: f32) -> f32 {
  var l_radius: f32 = p_radius;
  var l_density: f32 = p_density;
  return (((0x1.000000p1f * l_radius) * l_radius) * l_density);
}
fn verse_style_grass_clear(p_past: f32) -> f32 {
  var l_past: f32 = p_past;
  return coax_p_verse_style_soft_step(0x1.000000p-3f, 0x1.000000p-3f, l_past);
}
fn verse_style_segment(p_x: f32, p_z: f32, p_ax: f32, p_az: f32, p_bx: f32, p_bz: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ax: f32 = p_ax;
  var l_az: f32 = p_az;
  var l_bx: f32 = p_bx;
  var l_bz: f32 = p_bz;
  var l_ux: f32;
  var l_uz: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_h: f32;
  var l_dx: f32;
  var l_dz: f32;
  l_ux = (l_bx - l_ax);
  l_uz = (l_bz - l_az);
  l_px = (l_x - l_ax);
  l_pz = (l_z - l_az);
  l_h = coax_p_st_sat((((l_px * l_ux) + (l_pz * l_uz)) / coax_p_st_max(((l_ux * l_ux) + (l_uz * l_uz)), 0x1.0c6f7ap-20f)));
  l_dx = (l_px - (l_ux * l_h));
  l_dz = (l_pz - (l_uz * l_h));
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn verse_style_part(p_d: f32, p_radius: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  return coax_p_st_sat((0x1.000000p0f - (l_d / coax_p_st_max(l_radius, 0x1.0624dep-10f))));
}
fn verse_style_tip(p_c: f32, p_ch: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_lift: f32 = p_lift;
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + (l_lift * 0x1.4cccccp0f)));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return (l_c * (0x1.000000p0f + l_lift));
  }
  return l_c;
}
fn verse_style_jitter(p_c: f32, p_ch: f32, p_h: f32, p_jitter: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_h: f32 = p_h;
  var l_jitter: f32 = p_jitter;
  var l_j: f32;
  l_j = (((l_h * 0x1.000000p1f) - 0x1.000000p0f) * l_jitter);
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + l_j));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return l_c;
  }
  return (l_c * (0x1.000000p0f - l_j));
}
fn verse_style_flat_light(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  return (coax_p_st_sat(((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f)) * coax_p_st_sat(l_fl_lit));
}
fn verse_style_flat_band(p_fl_w: f32) -> f32 {
  var l_fl_w: f32 = p_fl_w;
  return verse_style_bands(l_fl_w, 0x1.851eb8p-2f, 0x1.3d70a4p-1f, 0x1.47ae14p-7f, 0x1.000000p-1f);
}
fn verse_style_flat_band2(p_fl_w: f32) -> f32 {
  var l_fl_w: f32 = p_fl_w;
  return coax_p_verse_style_soft_step(0x1.ccccccp-2f, 0x1.47ae14p-7f, l_fl_w);
}
fn verse_style_flat_sky(p_fl_up: f32) -> f32 {
  var l_fl_up: f32 = p_fl_up;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-4f, l_fl_up));
}
fn verse_style_flat_shadow(p_fl_d: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.b33334p-1f, 0x1.333334p-3f, l_fl_d));
}
fn verse_style_flat_fade(p_fl_d: f32, p_fl_start: f32, p_fl_reach: f32, p_fl_cap: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  var l_fl_start: f32 = p_fl_start;
  var l_fl_reach: f32 = p_fl_reach;
  var l_fl_cap: f32 = p_fl_cap;
  return (l_fl_cap * coax_p_verse_style_soft_step((l_fl_start + (l_fl_reach * 0x1.000000p-1f)), coax_p_st_max((l_fl_reach * 0x1.000000p-1f), 0x1.0624dep-10f), l_fl_d));
}
fn verse_style_bayer(p_fl_px: f32, p_fl_py: f32) -> f32 {
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_x: f32;
  var l_fl_y: f32;
  var l_fl_x0: f32;
  var l_fl_y0: f32;
  var l_fl_x1: f32;
  var l_fl_y1: f32;
  l_fl_x = (l_fl_px - (0x1.000000p2f * floor((l_fl_px * 0x1.000000p-2f))));
  l_fl_y = (l_fl_py - (0x1.000000p2f * floor((l_fl_py * 0x1.000000p-2f))));
  l_fl_x0 = (l_fl_x - (0x1.000000p1f * floor((l_fl_x * 0x1.000000p-1f))));
  l_fl_y0 = (l_fl_y - (0x1.000000p1f * floor((l_fl_y * 0x1.000000p-1f))));
  l_fl_x1 = floor((l_fl_x * 0x1.000000p-1f));
  l_fl_y1 = floor((l_fl_y * 0x1.000000p-1f));
  return ((((0x1.000000p2f * coax_p_st_bayer2(l_fl_x0, l_fl_y0)) + coax_p_st_bayer2(l_fl_x1, l_fl_y1)) + 0x1.000000p-1f) / 0x1.000000p4f);
}
fn verse_style_flat_dither(p_fl_t: f32, p_fl_px: f32, p_fl_py: f32, p_fl_steps: f32) -> f32 {
  var l_fl_t: f32 = p_fl_t;
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_steps: f32 = p_fl_steps;
  var l_fl_n: f32;
  l_fl_n = coax_p_st_max(l_fl_steps, 0x1.000000p0f);
  return coax_p_st_sat((floor(((coax_p_st_sat(l_fl_t) * l_fl_n) + verse_style_bayer(l_fl_px, l_fl_py))) / l_fl_n));
}
fn verse_style_flat_hem(p_fl_y: f32, p_fl_width: f32, p_fl_aa: f32) -> f32 {
  var l_fl_y: f32 = p_fl_y;
  var l_fl_width: f32 = p_fl_width;
  var l_fl_aa: f32 = p_fl_aa;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 0x1.0c6f7ap-20f), l_fl_y));
}
fn verse_style_flat_srgb(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.99999ap-2f, 0x1.da1cacp-2f, 0x1.6147aep-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7d70a4p-1f, 0x1.6978d4p-1f, 0x1.b1a9fcp-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c5a1cap-2f, 0x1.2b020cp-1f, 0x1.12f1aap-1f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f9db22p-2f, 0x1.43126ep-1f, 0x1.2147aep-1f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c00000p-1f, 0x1.a3d70ap-1f, 0x1.6b851ep-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.fae148p-1f, 0x1.f0a3d8p-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-1f, 0x1.51eb86p-1f, 0x1.b851ecp-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.eb851ep-1f, 0x1.ccccccp-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a1cac0p-3f, 0x1.b22d0ep-3f, 0x1.5d2f1ap-2f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ee147ap-1f, 0x1.dfbe76p-1f, 0x1.bdf3b6p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.158106p-2f, 0x1.c9ba5ep-2f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-2f, 0x1.3d70a4p-1f, 0x1.f5c290p-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.6147aep-1f, 0x1.170a3ep-1f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.c00000p-1f, 0x1.28f5c2p-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8f5c28p-1f, 0x1.ccccccp-1f, 0x1.b33334p-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8d4fe0p-1f, 0x1.87ae14p-1f, 0x1.bbe76cp-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.10e560p-1f, 0x1.10e560p-1f, 0x1.71a9fcp-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.666666p-1f, 0x1.3d70a4p-1f);
  }
  if ((l_fl_realm > 0x1.800000p0f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.6353f8p-1f, 0x1.a76c8cp-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ab851ep-1f, 0x1.b3b646p-1f, 0x1.d81062p-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.839582p-1f, 0x1.f9db22p-2f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.170a3ep-1f, 0x1.6147aep-1f, 0x1.d60418p-2f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c7ae14p-1f, 0x1.a5e354p-1f, 0x1.795810p-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f7cedap-1f, 0x1.e7ef9ep-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.2d0e56p-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.fbe76cp-1f, 0x1.f7cedap-1f, 0x1.ee147ap-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-3f, 0x1.51eb86p-3f, 0x1.f9db22p-3f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ec0832p-1f, 0x1.e5e354p-1f, 0x1.d1eb86p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7df3b6p-2f, 0x1.49374cp-1f, 0x1.03126ep-1f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.3126eap-1f, 0x1.9fbe76p-1f, 0x1.f1a9fcp-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.916872p-1f, 0x1.a5e354p-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.5d2f1ap-1f, 0x1.0cccccp-1f, 0x1.d1eb86p-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.e9fbe8p-1f, 0x1.bdf3b6p-1f, 0x1.3126eap-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.916872p-1f, 0x1.d3f7cep-1f, 0x1.b7cedap-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.656042p-1f, 0x1.558106p-1f, 0x1.ab851ep-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a5e354p-1f, 0x1.5d2f1ap-1f, 0x1.c9ba5ep-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.d60418p-1f, 0x1.472b02p-1f, 0x1.753f7cp-1f);
  }
  if ((l_fl_slot < 0x1.000000p-1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.a76c8cp-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.800000p0f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-1f, 0x1.dfbe76p-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.400000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.3f7ceep-1f, 0x1.6b851ep-1f, 0x1.28f5c2p-2f);
  }
  if ((l_fl_slot < 0x1.c00000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.333334p-1f, 0x1.f1a9fcp-3f);
  }
  if ((l_fl_slot < 0x1.200000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.b3b646p-1f, 0x1.818938p-1f, 0x1.14fdf4p-1f);
  }
  if ((l_fl_slot < 0x1.600000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f5c290p-1f, 0x1.d70a3ep-1f);
  }
  if ((l_fl_slot < 0x1.a00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.666666p-1f, 0x1.666666p-1f, 0x1.ae147ap-1f);
  }
  if ((l_fl_slot < 0x1.e00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.851eb8p-1f, 0x1.800000p-1f, 0x1.c28f5cp-1f);
  }
  if ((l_fl_slot < 0x1.100000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.ee147ap-1f, 0x1.c18938p-1f);
  }
  if ((l_fl_slot < 0x1.300000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d2f1aap-4f, 0x1.f3b646p-4f, 0x1.51eb86p-3f);
  }
  if ((l_fl_slot < 0x1.500000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.e5e354p-1f, 0x1.d60418p-1f, 0x1.a5e354p-1f);
  }
  if ((l_fl_slot < 0x1.700000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.bd70a4p-2f, 0x1.14fdf4p-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.900000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.6b851ep-1f, 0x1.9fbe76p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.b00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.1f3b64p-1f, 0x1.9fbe76p-1f, 0x1.818938p-1f);
  }
  if ((l_fl_slot < 0x1.d00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.516872p-1f, 0x1.0ed916p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.f00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.c18938p-1f, 0x1.09374cp-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.080000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.89ba5ep-1f, 0x1.d9999ap-1f, 0x1.ad9168p-1f);
  }
  if ((l_fl_slot < 0x1.180000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.472b02p-1f, 0x1.89ba5ep-1f);
  }
  if ((l_fl_slot < 0x1.280000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.9374bcp-1f, 0x1.676c8cp-1f, 0x1.c18938p-1f);
  }
  return coax_p_st_pick(l_fl_ch, 0x1.4f5c28p-1f, 0x1.6b851ep-1f, 0x1.353f7cp-1f);
}
fn verse_style_flat_colour(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  return coax_pow_verse_style_wrap(verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 0x1.19999ap1f);
}
fn verse_style_flat_time(p_fl_realm: f32, p_fl_ch: f32, p_fl_dusk: f32, p_fl_night: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_dusk: f32 = p_fl_dusk;
  var l_fl_night: f32 = p_fl_night;
  var l_fl_warm: f32;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_fl_warm = (0x1.000000p0f + ((coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.99999ap-1f, 0x1.5c28f6p-1f) - 0x1.000000p0f) * coax_p_st_sat(l_fl_dusk)));
  return (l_fl_warm + ((coax_p_st_pick(l_fl_ch, 0x1.99999ap-3f, 0x1.000000p-2f, 0x1.c28f5cp-2f) - l_fl_warm) * coax_p_st_sat(l_fl_night)));
}
fn verse_style_facet_tone(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  var l_fl_t: f32;
  l_fl_t = coax_p_st_sat((((((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f) * coax_p_st_sat(l_fl_lit)) - 0x1.70a3d8p-3f) / 0x1.666666p-1f));
  return ((l_fl_t * l_fl_t) * (0x1.800000p1f - (0x1.000000p1f * l_fl_t)));
}
fn verse_style_facet_jitter(p_fl_i: f32, p_fl_j: f32, p_fl_round: f32, p_fl_down: f32, p_fl_seed: f32) -> f32 {
  var l_fl_i: f32 = p_fl_i;
  var l_fl_j: f32 = p_fl_j;
  var l_fl_round: f32 = p_fl_round;
  var l_fl_down: f32 = p_fl_down;
  var l_fl_seed: f32 = p_fl_seed;
  var l_fl_a: f32;
  l_fl_a = (l_fl_i - (l_fl_round * floor((l_fl_i / coax_p_st_max(l_fl_round, 0x1.000000p0f)))));
  if (((l_fl_j < 0x1.000000p-1f) || (l_fl_j > (l_fl_down - 0x1.000000p-1f))))
  {
    l_fl_a = 0.0f;
  }
  return ((verse_style_hash((l_fl_a + (l_fl_seed * 0x1.a00000p3f)), (l_fl_j + (l_fl_seed * 0x1.c00000p2f))) * 0x1.000000p1f) - 0x1.000000p0f);
}
fn verse_style_facet_edge(p_fl_own: f32, p_fl_other: f32) -> f32 {
  var l_fl_own: f32 = p_fl_own;
  var l_fl_other: f32 = p_fl_other;
  var l_fl_gap: f32;
  var l_fl_at: f32;
  if ((l_fl_own <= 0.0f))
  {
    return 0.0f;
  }
  if ((l_fl_other <= 0.0f))
  {
    return 0x1.000000p0f;
  }
  l_fl_gap = (coax_p_st_max(l_fl_other, (-l_fl_other)) - l_fl_own);
  l_fl_at = ((l_fl_own * 0x1.47ae14p-5f) + 0x1.000000p-2f);
  return coax_p_verse_style_soft_step((l_fl_at * 0x1.800000p0f), (l_fl_at * 0x1.000000p-1f), l_fl_gap);
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_bayer2(p_fl_a: f32, p_fl_b: f32) -> f32 {
  var l_fl_a: f32 = p_fl_a;
  var l_fl_b: f32 = p_fl_b;
  return (((0x1.000000p1f * l_fl_a) + (0x1.800000p1f * l_fl_b)) - ((0x1.000000p2f * l_fl_a) * l_fl_b));
}
fn coax_p_st_pick(p_fl_ch: f32, p_fl_r: f32, p_fl_g: f32, p_fl_b: f32) -> f32 {
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_r: f32 = p_fl_r;
  var l_fl_g: f32 = p_fl_g;
  var l_fl_b: f32 = p_fl_b;
  if ((l_fl_ch < 0x1.000000p-1f))
  {
    return l_fl_r;
  }
  if ((l_fl_ch < 0x1.800000p0f))
  {
    return l_fl_g;
  }
  return l_fl_b;
}
fn coax_bits_verse_style_wrap(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_verse_style_wrap(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_verse_style_wrap(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_verse_style_wrap(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}
// ---- end generated
