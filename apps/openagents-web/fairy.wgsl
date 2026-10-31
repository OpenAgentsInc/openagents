// Fairy Forest's passes in the flat style (docs/verse/art-styles.md, Flat):
// a flat sky over a soft horizon band, the scene in three flat tones (two for
// characters) with flat translucent shadows, the stream and the lakes in flat
// bands, then a post pass that blooms only what glows, and the HUD. No specular, no rim, no outline, no tone curve. Every
// number of the scene comes from fairy.coax; the shading curves and the
// palette (FLAT_GARDEN: olive and lime, cattail orange, pale mint, lavender,
// cream) are verse/style.coax's, generated below. Light is linear until the
// post pass encodes it.
//
// The fade (verse/fairy-forest/forest.coax): colour is drained most of the
// way (four tenths) to a pale grey-green of the same lightness everywhere but inside the
// bloom circles, which spread out from what you restore; a circle's young
// front glows a moment.
//
// Every shape's colour is drawn toward the palette's nearest (snap, in the
// vertex stage, once a vertex): the realm keeps its shapes and the palette
// decides their colours.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, you_z: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,       // the eye, the haze's density
  sun: vec4f,       // toward the sun, phone
  ground: vec4f,    // cells west to east, cell size, west and south edge
  overview: vec4f,  // how far the haze is pushed back while the map is open (m; fairymap.coax)
  here: vec4f,      // unused, the water's level, unused, your x
  shadow: array<vec4f, 16>,
  path: array<vec4f, 32>,   // the fade's circles (x, z, radius, edge; forest.coax FF_FADE): the paths are in the ground's planes now
  path_info: vec4f,         // 0, a footpath's half width, the ground's rows south to north, its points (a plane's length)
  bloom: array<vec4f, 24>,
  bloom_info: vec4f,
  plate: array<vec4f, 32>,
  water: WaterU,
  night: vec4f,     // how dark (0 day, 1 night), dawn or dusk's warmth, the light she carries, its height
}
@group(0) @binding(0) var<uniform> u: U;

// The palette's slots (verse/style.coax FLAT_*), linear.
const PALETTE = 1.0;   // FLAT_GARDEN
const P_SKY = 0.0; const P_HORIZON = 1.0; const P_GROUND = 2.0; const P_GROUND2 = 3.0; const P_PATH = 4.0;
const P_LIT = 5.0; const P_SHADE = 6.0; const P_SHADOW = 7.0; const P_ACCENT = 8.0; const P_DARK = 9.0;
const P_LIGHT = 10.0; const P_FOLIAGE = 11.0; const P_FOLIAGE2 = 12.0; const P_WATER = 13.0; const P_WOOD = 14.0;
const P_WARM = 15.0; const P_MINT = 16.0; const P_ROCK = 17.0; const P_LAVENDER = 18.0; const P_SAGE = 19.0;
fn pal(slot: f32) -> vec3f {
  return vec3f(verse_style_flat_colour(PALETTE, slot, 0.0), verse_style_flat_colour(PALETTE, slot, 1.0), verse_style_flat_colour(PALETTE, slot, 2.0));
}
// Through the day (fairy.coax U_NIGHT, verse/fairy-forest/night.coax): the palette at noon, warmer at
// dawn and dusk, a blue dusk by night (verse_style_flat_time).
fn by_time() -> vec3f {
  return vec3f(verse_style_flat_time(PALETTE, 0.0, u.night.y, u.night.x), verse_style_flat_time(PALETTE, 1.0, u.night.y, u.night.x), verse_style_flat_time(PALETTE, 2.0, u.night.y, u.night.x));
}
fn tp(slot: f32) -> vec3f { return pal(slot) * by_time(); }
// The flat preset's numbers (verse/style-preset.coax verse_style_flat): the far fade, the hem, a tall
// shape's dithered darkening toward its foot and its steps.
const FADE_FROM = 40.0; const FADE_OVER = 80.0; const FADE_CAP = 0.5;
const HEM = 0.06; const DITHER = 0.25; const DITHER_STEPS = 4.0;
const TAU = 6.2831853;

struct Lit { @location(0) light: vec4f, @location(1) aux: vec4f }

fn ray(pos: vec2f) -> vec3f {
  let nx = pos.x / u.size.x * 2.0 - 1.0;
  let ny = 1.0 - pos.y / u.size.y * 2.0;
  let fx2 = dot(u.r0.xyz, u.r0.xyz);
  let fy2 = dot(u.r1.xyz, u.r1.xyz);
  return normalize(u.r3.xyz + u.r0.xyz * nx / fx2 + u.r1.xyz * ny / fy2);
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn sun_dir() -> vec3f { return normalize(u.sun.xyz); }

// The flat sky along d: one colour over a soft band of the horizon's, the sun (or by night the
// moon) a flat disc, and by night a few flat stars.
fn sky_colour(d: vec3f) -> vec3f {
  let up = d.y;
  var c = mix(tp(P_SKY), tp(P_HORIZON), verse_style_flat_sky(up));
  let cs = dot(d, sun_dir());
  if (cs > 0.99966) { c = mix(pal(P_LIGHT) * 1.1, pal(P_LIGHT) * 0.8, u.night.x); }
  if (u.night.x > 0.0 && up > 0.02) {
    let q = d / max(abs(d.x), max(abs(d.y), abs(d.z))) * 160.0;
    let cell = floor(q);
    let h = hash(cell.xy + cell.yz * 7.13 + cell.zx * 3.71);
    let point = 1.0 - smoothstep(0.12, 0.2, length(fract(q) - vec3f(0.5)));
    c = mix(c, pal(P_LIGHT) * 0.8, point * step(0.99, h) * u.night.x);
  }
  return c;
}

// How much colour is back at xz (forest.coax ff_restored), and the glow of a young front there.
fn restored(xz: vec2f) -> vec2f {
  var front = 0.0;
  let count = i32(u.bloom_info.x);
  // outside the fade's circles (forest.coax FF_FADE) the park keeps its colour
  var faded = 0.0;
  for (var i = 0; i < 8; i++) {
    if (i >= i32(u.bloom_info.y)) { break; }
    let z = u.path[i];
    faded = max(faded, 1.0 - clamp((distance(xz, z.xy) - (z.z - z.w)) / max(z.w, 0.001), 0.0, 1.0));
  }
  var best = 1.0 - faded;
  for (var i = 0; i < 24; i++) {
    if (i >= count) { break; }
    let b = u.bloom[i];
    if (b.z <= 0.0) { continue; }
    let d = distance(xz, b.xy);
    let edge = min(5.0, b.z);
    best = max(best, 1.0 - clamp((d - (b.z - edge)) / max(edge, 0.001), 0.0, 1.0));
    front = max(front, b.w * exp(-pow((d - b.z) / 1.6, 2.0)));
  }
  return vec2f(best, front);
}
// The faded world: the colour drained to a pale grey-green of its lightness.
fn faded(c: vec3f, r: f32) -> vec3f {
  if (r >= 1.0) { return c; }
  let l = dot(c, vec3f(0.3, 0.55, 0.15));
  let grey = vec3f(l) * vec3f(0.97, 1.0, 0.93) * 0.92;
  return mix(mix(grey, c, 0.6), c, r);
}

// Far off, a surface goes over to the horizon colour: one soft fade, no fog layers.
fn hazed(c: vec3f, world: vec3f) -> vec3f {
  return mix(c, tp(P_HORIZON), verse_style_flat_fade(max(length(world - u.eye.xyz) - u.overview.x, 0.0), FADE_FROM, FADE_OVER, FADE_CAP));
}

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 8.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.y >= 2.0;
  let fill = select(vec3f(0.16, 0.2, 0.27), vec3f(0.22, 0.27, 0.36), over);
  let line = select(vec3f(0.78, 0.7, 0.52), vec3f(0.98, 0.84, 0.52), over || s.y >= 1.0);
  let w = 1.25 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0) * 0.9;
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1) * 0.7);
}
fn plates(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
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
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Lit {
  let d = ray(pos.xy);
  let c = sky_colour(d);
  let share = select(0.0, 0.3, dot(d, sun_dir()) > 0.99966);
  return Lit(vec4f(c, share), vec4f(0.0));
}

struct Obj { r0: vec4f, r1: vec4f, r2: vec4f, colour: vec4f }
@group(1) @binding(0) var<storage, read> heights: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<Obj>;
@group(1) @binding(2) var<storage, read> water_nodes: array<f32>;

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
  @location(3) @interpolate(flat) look: u32,
  @location(4) local: vec3f,     // where on its shape (a unit shape, -0.5 to 0.5): a wing's veins
  @location(5) @interpolate(flat) size: vec2f,   // its height (m), how tall it reads (0 to 1)
}

fn quad(v: u32) -> vec2f {
  var q = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  return q[v];
}
fn height(x: i32, z: i32) -> f32 {
  let n = i32(u.ground.x);
  let cx = clamp(x, 0, n);
  let cz = clamp(z, 0, i32(u.path_info.z));
  return heights[u32(cz * (n + 1) + cx)];
}
// How far a ground point is past a footpath's edge (x) and a drive's or the street's (y): the heights' second and third planes (fairy.coax path_planes).
fn path_planes(x: i32, z: i32) -> vec2f {
  let i = u32(z * (i32(u.ground.x) + 1) + x);
  let p = u32(u.path_info.w);
  return vec2f(heights[p + i], heights[2u * p + i]);
}
// The ground's height at xz, the same two triangles a cell as it is drawn (terrain.coax).
fn ground_h(xz: vec2f) -> f32 {
  let n = u.ground.x;
  let g = clamp((xz - u.ground.zw) / u.ground.y, vec2f(0.0), vec2f(n, u.path_info.z) - vec2f(0.0001));
  let i = i32(floor(g.x)); let j = i32(floor(g.y));
  let f = g - vec2f(f32(i), f32(j));
  let h00 = height(i, j); let h10 = height(i + 1, j); let h01 = height(i, j + 1); let h11 = height(i + 1, j + 1);
  if (f.x >= f.y) { return h00 + (h10 - h00) * f.x + (h11 - h10) * f.y; }
  return h00 + (h11 - h01) * f.x + (h01 - h00) * f.y;
}

struct Vert { p: vec3f, n: vec3f }

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
// Smooth sides (painterly: no facets); flat caps.
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
    if (k == 0u) { return Vert(vec3f(0.0, 0.5, 0.0), vec3f(0.0, 1.0, 0.0)); }
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

// The cast shadows, flat translucent shapes: the light left (1 none, 0 all), the deepest where they overlap.
fn blob_shadows(p: vec2f) -> f32 {
  var s = 0.0;
  for (var i = 0; i < 16; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    s = max(s, verse_style_flat_shadow(length(p - b.xy) / b.z) * min(b.w * 1.4, 1.0));
  }
  return 1.0 - s;
}

// A shape's colour drawn `k` of the way to the palette's nearest (by sRGB-ish distance).
fn near(q: vec3f, slot: f32, best: vec4f) -> vec4f {
  let p = pal(slot);
  let e = sqrt(p) - q;
  let dd = dot(e, e);
  return select(best, vec4f(slot, dd, 0.0, 0.0), dd < best.y);
}
fn snap(c: vec3f, k: f32) -> vec3f {
  let q = sqrt(max(c, vec3f(0.0)));
  var b = vec4f(P_GROUND, 1e9, 0.0, 0.0);
  b = near(q, P_GROUND, b); b = near(q, P_GROUND2, b); b = near(q, P_PATH, b); b = near(q, P_DARK, b);
  b = near(q, P_LIGHT, b); b = near(q, P_FOLIAGE, b); b = near(q, P_FOLIAGE2, b); b = near(q, P_WATER, b);
  b = near(q, P_WOOD, b); b = near(q, P_WARM, b); b = near(q, P_MINT, b); b = near(q, P_ROCK, b);
  b = near(q, P_LAVENDER, b); b = near(q, P_SAGE, b);
  return mix(c, pal(b.x), k);
}

// The wind (art-styles.md 8): a 12 m wave at 3 m/s along the wind, with gusts.
const WIND = vec2f(0.8, 0.6);
fn wind_bend(xz: vec2f) -> f32 {
  let t = u.time * (1.0 - u.still);
  let k = TAU / 12.0;
  let along = dot(xz, WIND);
  let gust = noise(xz / 25.0 - WIND * t * 3.0 / 25.0);
  return 0.30 * (0.6 * sin(k * along - k * 3.0 * t) + 0.4 * (gust * 2.0 - 1.0));
}

@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  var o: S;
  let n = u32(u.ground.x);
  let cells = n * u32(u.path_info.z);
  if (instance < cells) {
    let c = quad(id % 6u);
    let gx = i32(instance % n) + i32(c.x);
    let gz = i32(instance / n) + i32(c.y);
    let w = vec3f(u.ground.z + f32(gx) * u.ground.y, height(gx, gz), u.ground.w + f32(gz) * u.ground.y);
    // the height field's normal over three cells, so the terrain's facets don't show
    let normal = normalize(vec3f(height(gx - 2, gz) - height(gx + 2, gz), 4.0 * u.ground.y, height(gx, gz - 2) - height(gx, gz + 2)));
    o.pos = clip(w);
    o.world = w;
    o.normal = normal;
    o.tint = vec3f(path_planes(gx, gz), 0.0);
    o.look = 99u;
    o.local = vec3f(0.0);
    o.size = vec2f(0.0);
    return o;
  }
  let objects_n = arrayLength(&objects);
  let m = objects[instance - cells];
  let kind = u32(m.colour.w) % 8u;
  var v: Vert;
  switch kind {
    case 1u: { v = box_vertex(id); }
    case 2u: { v = sphere_vertex(id); }
    case 3u: { v = cylinder_vertex(id); }
    default: { v = cone_vertex(id); }
  }
  let p = vec4f(v.p, 1.0);
  var w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let c0 = vec3f(m.r0.x, m.r1.x, m.r2.x);
  let c1 = vec3f(m.r0.y, m.r1.y, m.r2.y);
  let c2 = vec3f(m.r0.z, m.r1.z, m.r2.z);
  let normal = cross(c1, c2) * v.n.x + cross(c2, c0) * v.n.y + cross(c0, c1) * v.n.z;
  let code = u32(m.colour.w);
  let look = (code % 128u) / 8u;
  // canopies breathe in the wind, the top more than the bottom
  if (look == 1u && kind == 2u && length(c1) > 1.0) {
    w = vec3f(w.x + WIND.x * wind_bend(w.xz) * 0.25 * (v.p.y + 0.5), w.y, w.z + WIND.y * wind_bend(w.xz) * 0.25 * (v.p.y + 0.5));
  }
  o.pos = clip(w);
  o.world = w;
  o.normal = normalize(normal + vec3f(0.0, 0.00001, 0.0));
  // the palette: what glows, her wings and her skin and clothes keep more of their own colour
  let own = look == 3u || look == 4u || look == 5u || look == 6u || look == 9u;
  o.tint = snap(m.colour.rgb, select(0.8, 0.5, own));
  o.look = look + select(0u, 32u, code >= 128u);   // + 32: hers (fairy.coax MINE)
  o.local = v.p;
  o.size = vec2f(length(c1), smoothstep(1.6, 3.2, length(c1)));
  return o;
}

// The ground's colour at xz: broad flat fills of lime and olive, the footpaths a flat tan band,
// the drives and avenues the same tan greyed. paths: how far past a footpath's and a drive's
// edge (the ground's planes, fairy.coax path_planes).
fn ground_colour(xz: vec2f, aa: f32, paths: vec2f) -> vec3f {
  var base = mix(pal(P_GROUND), pal(P_GROUND2), step(0.58, noise(xz * 0.035)));
  base = mix(base, pal(P_PATH), 1.0 - smoothstep(-aa, aa, paths.x));
  let grey = vec3f(dot(pal(P_PATH), vec3f(0.3, 0.55, 0.15))) * 0.78;
  base = mix(base, grey, 1.0 - smoothstep(-aa, aa, paths.y));
  return base;
}

// The flat light on albedo c with normal n at world: three flat tones (two for characters),
// lit and shade from the palette, the cast shadow a flat translucent shape (shadow: the light
// it leaves). No rim, no specular.
fn painted(c: vec3f, n: vec3f, world: vec3f, shadow: f32, character: bool) -> vec3f {
  let w = verse_style_flat_light(dot(n, sun_dir()), 1.0);
  var band = verse_style_flat_band(w);
  if (character) { band = verse_style_flat_band2(w); }
  var light = c * mix(tp(P_SHADE), tp(P_LIT), band);
  light *= mix(pal(P_SHADOW), vec3f(1.0), shadow);
  // by night, the light she carries (LIGHT): a warm pool round her, in two flat steps
  if (u.night.z > 0.0) {
    let to = vec3f(u.here.w, u.night.w, u.you_z) - world;
    let d2 = dot(to, to);
    let reach = max(u.night.z, 1.0);   // her lantern (fairytools.coax) sends it twice as far
    let fall = min(u.night.z, 1.0) * (0.5 * step(d2, 36.0 * reach * reach) + 0.5 * step(d2, 9.0 * reach * reach));
    light += c * pal(P_WARM) * 0.8 * fall;
  }
  return light;
}

@fragment fn scene_fs(s: S) -> Lit {
  let ground_aa = max(length(fwidth(s.world.xz)), 0.002);   // derivatives first, while control flow is uniform
  let local_aa = fwidth(s.local.y);
  var n = normalize(s.normal);
  var base = s.tint;
  let xz = s.world.xz;
  var shadow = 1.0;
  var character = false;
  var glow = 0.0;
  var keep = false;
  // hers (look + 32) or not; anything else within about 1.6 m of the lens
  // dissolves (an ordered dither), so nothing near blots out the view
  var look = s.look;
  if (look >= 32u && look < 99u) { look -= 32u; }
  else if (look < 32u) {
    // (and, further out, anything nearer the lens than she is, so a cap or
    // a trunk between you and the camera never blots her or the way out)
    let near = distance(s.world, u.eye.xyz);
    let clear = select(max(1.6, distance(u.eye.xz, vec2f(u.here.w, u.you_z))), 1.6, u.overview.x > 0.0);   // (not over the map)
    if (near < clear) {
      let p = vec2u(s.pos.xy) % 4u;
      let bayer = array(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
      if (smoothstep(clear * 0.8, clear, near) * 16.0 <= bayer[p.y * 4u + p.x] + 0.5) { discard; }
    }
  }
  if (look == 99u) {
    base = ground_colour(xz, ground_aa, s.tint.xy);
    // the banks: a flat tan where the ground dips to the water
    base = mix(base, pal(P_PATH), step(s.world.y, u.here.y + 0.2));
    shadow = blob_shadows(xz);
  } else if (look == 3u || look == 6u) {
    character = true; keep = true;
  } else if (look == 4u) {
    glow = 1.0;
  } else if (look == 9u) {
    glow = 1.0; keep = true;
  } else if (look == 5u) {
    // a wing: veins fanning from its root, a bright rim, and between them a
    // thin membrane the light shows through (every other pixel of it drawn)
    let q = s.local.xy * 2.0;
    // a pointed outline (fairy.coax wing_half): narrow at the root, full past
    // the middle, coming to a point at the tip
    let along = clamp((q.y + 1.0) * 0.5, 0.0, 1.0);
    let half = clamp(1.9 * pow(along, 0.6) * (1.0 - along), 0.0, 1.0);
    if (abs(q.x) > half) { discard; }
    let fan = atan2(q.x, q.y + 1.0);
    let vein = 1.0 - smoothstep(0.035, 0.09, abs(fract(fan * 2.4 + 0.5) - 0.5));
    let rim = smoothstep(0.72, 0.92, abs(q.x) / max(half, 0.001));
    let cross_vein = 1.0 - smoothstep(0.02, 0.06, abs(fract(length(q + vec2f(0.0, 1.0)) * 2.2) - 0.5) - 0.44);
    let solid = max(max(vein, rim), cross_vein * 0.6);
    let cell = vec2u(s.pos.xy) % 2u;
    if (solid < 0.5 && (cell.x + cell.y) % 2u == 0u) { discard; }
    // jewel-like: the tint runs to rose-gold toward the edge, with a glinting rim
    // jewel-like: tint at the root, rose through the middle, gold at the
    // tip, and a glowing rim
    base = mix(base, vec3f(1.0, 0.36, 0.8), smoothstep(0.15, 0.55, along) * 0.85);
    base = mix(base, vec3f(1.0, 0.78, 0.3), smoothstep(0.55, 0.92, along) * 0.8);
    base = mix(base * 0.8, mix(base, vec3f(1.0), 0.3), solid);
    base = mix(base, pal(P_ACCENT), rim * 0.6);
    glow = 0.8; keep = true;
  } else if (look == 10u) {
    // falling water (fairypond.coax): streaks running down it, whiter toward
    // its foot, and between them thin water the light comes through
    let q = s.local;
    let bands = noise(vec2f(q.x * 14.0 + q.z * 6.0, q.y * 4.0 + u.time * 2.4));
    let streak = noise(vec2f(q.x * 31.0, q.y * 9.0 + u.time * 4.0));
    let white = clamp(smoothstep(0.5, 0.85, bands) * 0.8 + smoothstep(0.6, 0.9, streak) * 0.5 + smoothstep(-0.15, -0.5, q.y) * 0.7, 0.0, 1.0);
    let cell = vec2u(s.pos.xy) % 2u;
    if (white < 0.25 && (cell.x + cell.y) % 2u == 0u) { discard; }
    base = mix(pal(P_WATER), pal(P_LIGHT), step(0.5, white));
    glow = 0.0;
  } else if (look == 11u) {
    // a lake (fairypark.coax ffpk_lakes): a few flat tones by its depth over the carved bed, two
    // soft light bands along the shore, and slow broken ripple lines drifting across the open water
    let depth = u.here.y - ground_h(xz);
    let w = pal(P_WATER);
    base = mix(w, w * 0.82, step(0.42, depth));
    base = mix(base, mix(w, pal(P_LIGHT), 0.28), step(depth, 0.2));
    base = mix(base, mix(w, pal(P_LIGHT), 0.55), step(depth, 0.07));
    let drift = vec2f(0.8, 0.6);
    let ph = dot(xz, drift) / 6.0 + noise(xz * 0.04) * 1.5 - u.time * 0.025;
    let thin = max(0.035, ground_aa / 6.0 * 1.5);
    let ripple = step(abs(fract(ph) - 0.5), thin) * step(0.5, noise(xz * vec2f(0.09, 0.13) + drift * u.time * 0.04)) * step(0.14, depth);
    base = mix(base, mix(w, pal(P_LIGHT), 0.45), ripple * (1.0 - smoothstep(30.0, 70.0, distance(s.world, u.eye.xyz))));
    n = vec3f(0.0, 1.0, 0.0);
  } else if (look == 7u) {
    keep = true;
  }
  // trim: a thin cream line along the hem of her clothes
  if (look == 6u) {
    let from_hem = (s.local.y + 0.5) * s.size.x;
    base = mix(base, pal(P_ACCENT), verse_style_flat_hem(from_hem, HEM * 0.5, max(local_aa * s.size.x, 0.001)));
  }
  // a tall shape (a trunk, the heart tree) steps a little darker toward its foot, dithered where the steps meet
  if (s.size.y > 0.0 && look < 99u) {
    let t = verse_style_flat_dither(0.5 - s.local.y, s.pos.x, s.pos.y, DITHER_STEPS);
    base = mix(base, base * pal(P_SHADE) * 0.8, t * DITHER * s.size.y);
  }
  let r = restored(xz);
  if (!keep) { base = faded(base, r.x); }
  var c = vec3f(0.0);
  var share = 0.0;
  if (glow > 0.0) {
    // what glows skips the tones: one flat light colour
    c = base * select(1.3, 1.0, glow < 1.0);
    if (!keep) { c *= mix(0.35, 1.0, r.x); }
    share = select(0.75, 0.45, glow < 1.0);
  } else {
    c = painted(base, n, s.world, shadow, character);
  }
  // a bloom's spreading front sparkles gold
  c += pal(P_ACCENT) * r.y * 0.6;
  share = max(share, r.y * 0.5);
  c = hazed(c, s.world);
  return Lit(vec4f(c, share), vec4f(n, 0.0));
}

// ---- water: the W2 mesh and detail (copied from everglade.wgsl)

const WATER_F0: f32 = 0.0200593;          // ((n - 1) / (n + 1))^2, n = 1.33
const WATER_RIPPLE_SPEED: f32 = 1.1;
const WATER_FLOW_PERIOD: f32 = 1.6;
const WATER_DRY: f32 = 0.6;
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


fn water_node(k: u32, f: u32) -> f32 { return water_nodes[k * 8u + f]; }

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


fn water_fresnel(c: f32) -> f32 {
  let m = clamp(1.0 - c, 0.0, 1.0);
  let m2 = m * m;
  return WATER_F0 + (1.0 - WATER_F0) * m2 * m2 * m;
}

// ---- the flat water: two flat bands of the palette's water by depth (verse_style_flat's
// shallow, mid and deep), a flat cream shore line and rings, no reflection and no sparkle.
// The mesh, the waves and the ripples are the W2 shader's (everglade.wgsl), from
// verse/fairy-forest/stream.coax's nodes (store 2).

struct WaterShade { emit: vec3f, transmit: vec3f }

fn water_fragment(i: WaterOut) -> WaterShade {
  var o: WaterShade;
  let world = i.world_depth.xyz;
  let rest = i.rest_flow.xy;
  let depth = i.world_depth.w;
  let dist = length(u.eye.xyz - world);
  if (u.eye.y < world.y) { o.emit = vec3f(0.0); o.transmit = vec3f(1.0); return o; }
  let w = pal(P_WATER);
  var body = mix(mix(w, w * 0.78, step(0.4, depth)), w * 0.6, step(1.5, depth));
  body *= tp(P_LIT);
  let r = restored(world.xz);
  body = faded(body, r.x);
  let shore = i.look.w;
  let line = step(shore, 0.22) * step(0.0, depth);
  let rings = step(0.5, water_ripple_foam(rest) * 1.6);
  let foam = max(line, rings);
  let alpha = (0.6 + 0.35 * step(0.4, depth)) * smoothstep(-0.05, 0.1, depth);
  let emit = mix(body * alpha, pal(P_LIGHT) * tp(P_LIT), foam);
  let hz = verse_style_flat_fade(max(dist - u.overview.x, 0.0), FADE_FROM, FADE_OVER, FADE_CAP);
  let trans = vec3f((1.0 - alpha) * (1.0 - foam));
  o.emit = emit * (1.0 - hz) + tp(P_HORIZON) * hz * (vec3f(1.0) - trans);
  o.transmit = trans;
  return o;
}

@fragment fn water_transmit_fs(i: WaterOut) -> Lit {
  let s = water_fragment(i);
  return Lit(vec4f(s.transmit, 1.0), vec4f(0.0));
}
@fragment fn water_emit_fs(i: WaterOut) -> Lit {
  let s = water_fragment(i);
  return Lit(vec4f(s.emit, 0.0), vec4f(0.0));
}

// ---- the post pass (host.js postLayouts): the bright share blurred at a quarter
// and an eighth of the size, mixed back gently (only what glows has a share); no tone curve,
// so the palette shows as it is.
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
  if (v.tint.w < -0.5) {
    // An invoice's QR (fairyupgrade.coax ffu_qr_draw): its light modules, flat and square-ended, no glow.
    return vec4f(v.tint.rgb * v.shape.z, 1.0);
  }
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
