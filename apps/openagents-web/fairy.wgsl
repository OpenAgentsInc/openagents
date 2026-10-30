// Fairy Forest's passes in the painterly style (docs/verse/art-styles.md):
// the painted sky, the scene (the ground of cells, every shape, the
// wind-swept grass), the stream in the painted water preset, then the post
// pass (gentle bloom, the soft tone curve, lifted blacks) and the HUD. Every
// number comes from fairy.coax; the shading curves are verse/paint.coax's,
// generated below. Light is linear until the post pass encodes it.
//
// The fade (verse/fairy-forest/forest.coax): colour is drained most of the
// way to a cool grey of the same lightness (desaturated, never whitened)
// everywhere but inside the bloom circles, which spread out from what you
// restore; a circle's young front glows a moment.
//
// The palette is a clear, saturated daytime: yellow-green grass, rich
// mid-green crowns, a teal-blue sky warming to cream round the sun, and a
// haze only in the distance, teal in the middle distance shifting to
// blue-violet far off. Its numbers are Fairy Forest's own, not the shared
// painterly presets'.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, you_z: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,       // the eye, the haze's density
  sun: vec4f,       // toward the sun, phone
  ground: vec4f,    // cells a side, cell size, west and south edge
  grass: vec4f,     // the first patch's x, z, patches a side, blades a patch
  grass2: vec4f,    // the grass's reach (m), the water's level, a trail's half width, your x
  shadow: array<vec4f, 16>,
  path: array<vec4f, 32>,
  path_info: vec4f,
  bloom: array<vec4f, 24>,
  bloom_info: vec4f,
  plate: array<vec4f, 32>,
  water: WaterU,
  night: vec4f,     // how dark (0 day, 1 night), dawn or dusk's warmth, the light she carries, its height
}
@group(0) @binding(0) var<uniform> u: U;

// The painterly day key (art-styles.md 3 to 5): key, ambient, shadow tint, haze, sky.
const KEY = vec3f(1.0, 0.95, 0.84);
const SKY_AMB = vec3f(0.50, 0.68, 0.82);
const GROUND_AMB = vec3f(0.42, 0.42, 0.24);
const SHADOW_TINT = vec3f(0.46, 0.60, 0.86);
const HAZE = vec3f(0.50, 0.78, 0.80);       // the horizon and the middle distance's haze: teal
const HAZE_FAR = vec3f(0.52, 0.56, 0.86);   // the far haze: blue-violet
const HAZE_KEY = vec3f(1.0, 0.90, 0.68);    // toward the sun: warm cream
const ZENITH = vec3f(0.14, 0.46, 0.74);     // teal-blue overhead
const HAZE_FROM = 28.0;                     // m: nothing nearer is hazed
const RIM_DAY = vec3f(1.0, 0.84, 0.60);

// Through the day (fairy.coax U_NIGHT, verse/fairy-forest/night.coax): the
// day's numbers above at noon, warm and low at dawn and dusk, and by night a
// moonlit blue: a cool key from the moon, a deep blue sky with stars, the
// haze a dark blue so the lights carry.
fn by_time(day: vec3f, dusk: vec3f, night: vec3f) -> vec3f { return mix(mix(day, dusk, u.night.y), night, u.night.x); }
fn key_c() -> vec3f { return by_time(KEY, vec3f(1.0, 0.66, 0.40) * 0.9, vec3f(0.45, 0.56, 1.0) * 0.3); }
fn sky_amb() -> vec3f { return by_time(SKY_AMB, vec3f(0.58, 0.52, 0.60), vec3f(0.12, 0.17, 0.38)); }
fn ground_amb() -> vec3f { return by_time(GROUND_AMB, vec3f(0.42, 0.34, 0.26), vec3f(0.05, 0.06, 0.10)); }
fn shadow_tint() -> vec3f { return by_time(SHADOW_TINT, vec3f(0.36, 0.56, 0.66), vec3f(0.34, 0.42, 0.74)); }
fn haze_c() -> vec3f { return by_time(HAZE, vec3f(0.90, 0.60, 0.38), vec3f(0.05, 0.08, 0.20)); }
fn haze_far() -> vec3f { return by_time(HAZE_FAR, vec3f(0.62, 0.46, 0.62), vec3f(0.05, 0.06, 0.17)); }
fn haze_key() -> vec3f { return by_time(HAZE_KEY, vec3f(1.0, 0.62, 0.34), vec3f(0.36, 0.42, 0.66)); }
fn zenith() -> vec3f { return by_time(ZENITH, vec3f(0.22, 0.30, 0.58), vec3f(0.015, 0.03, 0.10)); }
fn rim_c() -> vec3f { return by_time(RIM_DAY, vec3f(1.0, 0.62, 0.40), vec3f(0.50, 0.62, 1.0)); }
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

// The painted sky along d: the gradient by the ray's elevation, the horizon
// band lifted toward the haze, the sun with its halo and soft two-tone clouds.
fn sky_colour(d: vec3f, clouds: bool) -> vec3f {
  let up = d.y;
  var c = mix(haze_c(), zenith(), verse_paint_sky_mix(max(up, 0.0)));
  c = mix(c, haze_c() * 1.08, verse_paint_sky_band(up));
  let l = sun_dir();
  let cs = dot(d, l);
  // the horizon warms to cream toward the sun, and a warm bloom rings it
  c = mix(c, haze_key() * 1.1, pow(max(cs, 0.0), 6.0) * (1.0 - smoothstep(0.0, 0.5, up)) * 0.8);
  c += mix(haze_key(), vec3f(1.0, 0.97, 0.9), 0.5) * verse_paint_halo(cs) * 0.8 * (1.0 - 0.7 * u.night.x);
  // the disc, drawn at 2 degrees: the sun, or by night the pale moon
  if (cs > 0.99939) { c = mix(vec3f(1.0, 0.97, 0.9) * 4.0, vec3f(0.86, 0.9, 1.0) * 1.8, u.night.x); }
  // the stars, by night: a few bright points on a fine grid of the sky, twinkling
  if (u.night.x > 0.0 && up > 0.02) {
    let q = d / max(abs(d.x), max(abs(d.y), abs(d.z))) * 260.0;
    let cell = floor(q);
    let h = hash(cell.xy + cell.yz * 7.13 + cell.zx * 3.71);
    let f = fract(q) - vec3f(0.5);
    let point = 1.0 - smoothstep(0.08, 0.3, length(f));
    let twinkle = 0.6 + 0.4 * sin(u.time * (1.0 + h * 3.0) + h * 40.0);
    c += vec3f(0.9, 0.93, 1.0) * point * step(0.985, h) * twinkle * u.night.x * smoothstep(0.02, 0.25, up) * 1.4;
  }
  if (clouds && up > 0.01) {
    let drift = vec2f(u.time * 3.0, u.time * 1.2) * (1.0 - u.still);
    let p = d.xz / up * 1500.0 + drift;
    // the engine's clouds (verse/style.coax): two octaves, one on a phone
    let octaves = select(1.0, 2.0, u.sun.w < 0.5);
    let n = verse_style_cloud_field(p.x, p.y, 1.0 / 2000.0, 1.0 / 700.0, octaves);
    let cover = verse_style_cloud_cover(n, 0.35, 0.48, 0.62);
    let toward = normalize(l.xz + vec2f(0.0001)) * 260.0;
    let lit = verse_style_cloud_lit(n, verse_style_cloud_field(p.x + toward.x, p.y + toward.y, 1.0 / 2000.0, 1.0 / 700.0, 1.0));
    let cloud = mix(shadow_tint() * 1.4 * 0.9, by_time(vec3f(1.0, 0.98, 0.95) * 1.15, vec3f(1.0, 0.72, 0.56), vec3f(0.2, 0.24, 0.4)), lit) * (1.0 - 0.55 * u.night.x);
    c = mix(c, cloud, cover * verse_style_cloud_fade(up));
  }
  return c;
}

// How much colour is back at xz (forest.coax ff_restored), and the glow of a young front there.
fn restored(xz: vec2f) -> vec2f {
  var best = 0.0;
  var front = 0.0;
  let count = i32(u.bloom_info.x);
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
// The faded world: the painted colour drained to a pale, cool grey.
fn faded(c: vec3f, r: f32) -> vec3f {
  let l = dot(c, vec3f(0.3, 0.55, 0.15));
  let grey = vec3f(l) * vec3f(0.92, 0.99, 1.02) * 0.84;
  return mix(mix(grey, c, 0.45), c, r);
}

// The faded world at a place: still drained and cool, but in broken washes
// and dabs of value (light straw, blue-grey shadow) instead of one flat tone.
fn faded_at(c: vec3f, r: f32, world: vec3f) -> vec3f {
  if (r >= 1.0) { return c; }
  let q = world.xz + vec2f(world.y * 0.7, -world.y * 0.4);
  let broad = noise(q * 0.07);
  let mid = noise(q * 0.31 + vec2f(7.0, 3.0));
  let dab = noise(q * 1.9 + vec2f(1.0, 5.0));
  let l = dot(c, vec3f(0.3, 0.55, 0.15));
  let value = 0.52 + 0.72 * smoothstep(0.2, 0.8, broad) + 0.4 * (mid - 0.5) + 0.3 * (dab - 0.5);
  let warm = smoothstep(0.3, 0.7, noise(q * 0.045 + vec2f(3.0, 9.0)));
  let tone = mix(vec3f(0.74, 0.86, 1.08), vec3f(1.06, 0.97, 0.76), warm);
  let grey = vec3f(l) * tone * 0.86 * value;
  let kept = mix(grey, c * value, 0.38);
  return mix(kept, c, r);
}

// The haze over a surface at world seen from the eye.
// It starts HAZE_FROM past the shared curve's own start, so the near and
// middle distance stay crisp, and it shifts from teal to blue-violet far off.
fn haze_colour(d: f32, to_sun: f32) -> vec3f {
  let far = mix(haze_c(), haze_far(), smoothstep(60.0, 220.0, d));
  return mix(far, haze_key(), verse_paint_haze_sun(to_sun));
}
fn hazed(c: vec3f, world: vec3f) -> vec3f {
  let to = world - u.eye.xyz;
  let d = length(to);
  let f = verse_paint_haze(max(d - HAZE_FROM, 0.0), world.y - 0.0, u.eye.w) * 0.85;
  return mix(c, haze_colour(d, dot(to / max(d, 0.001), sun_dir())), f);
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
  let c = sky_colour(d, true);
  let share = select(0.0, 0.7, dot(d, sun_dir()) > 0.9993);
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
// The ground's height at xz, the same two triangles a cell as it is drawn (terrain.coax).
fn ground_h(xz: vec2f) -> f32 {
  let n = u.ground.x;
  let g = clamp((xz - u.ground.zw) / u.ground.y, vec2f(0.0), vec2f(n - 0.0001));
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
fn blob_shadows(p: vec2f) -> f32 {
  var s = 1.0;
  for (var i = 0; i < 16; i++) {
    let b = u.shadow[i];
    if (b.z <= 0.0) { break; }
    let d = length(p - b.xy) / b.z;
    s *= 1.0 - b.w * (1.0 - smoothstep(0.35, 1.0, d));
  }
  return s;
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
  if (instance < n * n) {
    let c = quad(id % 6u);
    let gx = i32(instance % n) + i32(c.x);
    let gz = i32(instance / n) + i32(c.y);
    let w = vec3f(u.ground.z + f32(gx) * u.ground.y, height(gx, gz), u.ground.w + f32(gz) * u.ground.y);
    // the height field's normal over three cells, so the terrain's facets don't show
    let normal = normalize(vec3f(height(gx - 2, gz) - height(gx + 2, gz), 4.0 * u.ground.y, height(gx, gz - 2) - height(gx, gz + 2)));
    o.pos = clip(w);
    o.world = w;
    o.normal = normal;
    o.tint = vec3f(0.0);
    o.look = 99u;
    return o;
  }
  let objects_n = arrayLength(&objects);
  if (instance >= n * n + objects_n) { return grass_vertex(id, instance - n * n - objects_n); }
  let m = objects[instance - n * n];
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
  o.tint = m.colour.rgb;
  o.look = look + select(0u, 32u, code >= 128u);   // + 32: hers (fairy.coax MINE)
  o.local = v.p;
  return o;
}

// A blade of grass: one tapered triangle placed from a hash of its patch,
// bent by the wind, leaning away from you, none on trails, water or under the heart tree.
fn grass_vertex(id: u32, cell: u32) -> S {
  var o: S;
  let side = u32(u.grass.z);
  let origin = u.grass.xy + vec2f(f32(cell % side), f32(cell / side)) * 2.0;
  let blade = id / 3u;
  let corner = id % 3u;
  let seed = origin + vec2f(f32(blade) * 0.137, f32(blade) * 0.071);
  let h1 = hash(seed); let h2 = hash(seed + vec2f(5.1, 1.7)); let h3 = hash(seed + vec2f(2.3, 9.4)); let h4 = hash(seed + vec2f(7.7, 3.3));
  let xz = origin + vec2f(h1, h2) * 2.0;
  let centre = u.grass.xy + vec2f(f32(side));
  let reach = u.grass2.x;
  let r = distance(xz, centre);
  var tall = (0.42 + 0.4 * h3) * (1.0 - smoothstep(reach - 6.0, reach, r));
  let gy = ground_h(xz);
  if (path_distance(xz) < u.grass2.z + 0.15 || gy < u.grass2.y + 0.08 || length(xz) < 5.0) { tall = 0.0; }
  if (tall < 0.02) { o.pos = vec4f(0.0, 0.0, 2.0, 1.0); return o; }
  tall *= mix(0.42, 1.25, restored(xz).x);   // restored meadows grow tall
  let yaw = h4 * TAU;
  let across = vec3f(cos(yaw), 0.0, sin(yaw)) * 0.03;
  let base = vec3f(xz.x, gy - 0.02, xz.y);
  var w = base;
  var along = 0.0;
  if (corner == 0u) { w = base - across; }
  if (corner == 1u) { w = base + across; }
  if (corner == 2u) {
    // faded grass sways less, and each blade leans its own way, so a faded
    // meadow never reads as one slant of streaks
    let rr = restored(xz).x;
    var lean = WIND * sin(wind_bend(xz)) * mix(0.4, 1.0, rr) + vec2f(cos(yaw * 1.7), sin(yaw * 1.7)) * 0.14;
    let you = vec2f(u.grass2.w, u.you_z);
    let away = xz - you;
    let dy = length(away);
    if (dy < 0.7) { lean += away / max(dy, 0.01) * (0.7 - dy) * 1.2; }
    w = base + vec3f(lean.x * tall, tall * cos(clamp(length(lean), 0.0, 1.2)), lean.y * tall);
    along = 1.0;
  }
  o.pos = clip(w);
  o.world = w;
  o.normal = vec3f(0.0, 1.0, 0.0);
  o.tint = vec3f(along, h3, 0.0);
  o.look = 98u;
  return o;
}

// The ground's painted colour at xz: broad meadow greens, warm trails, sandy banks.
fn ground_colour(xz: vec2f) -> vec3f {
  let broad = noise(xz * 0.06);
  var base = mix(vec3f(0.10, 0.27, 0.035), vec3f(0.19, 0.35, 0.045), broad);
  base *= 0.9 + 0.2 * noise(xz * 0.21 + vec2f(3.0, 1.0));
  let d = path_distance(xz);
  let half = u.path_info.y;
  let edge = 1.0 - smoothstep(half - 0.25, half + 0.25, d);
  base = mix(base, vec3f(0.52, 0.36, 0.18), edge);
  return base;
}

// The painterly light on albedo c with normal n at world: wrapped Lambert in
// bands (two for characters), the shadow band tinted blue, a warm rim.
fn painted(c: vec3f, n: vec3f, world: vec3f, shadow: f32, character: bool) -> vec3f {
  let l = sun_dir();
  let v = normalize(u.eye.xyz - world);
  let ndl = dot(n, l);
  let w = verse_paint_wrap(ndl, shadow);
  var band = verse_paint_band(w);
  if (character) { band = verse_paint_band2(w); }
  let ambient = mix(ground_amb(), sky_amb(), n.y * 0.5 + 0.5) * 0.55;
  var shade = shadow_tint() * ambient * 1.25;
  // a character's shadow side stays warm and light, so she reads against the greens
  if (character) { shade = mix(shade, key_c() * 0.8, 0.5); }
  var light = c * mix(shade, key_c() + ambient, band);
  let strength = select(0.30, 0.45, character);
  light += rim_c() * verse_paint_rim(abs(dot(n, v)), ndl, strength) * mix(vec3f(0.6), c * 1.6, 0.4) * (1.0 - 0.6 * u.night.x);
  // by night, the light she carries (LIGHT): a warm pool round her
  if (u.night.z > 0.0) {
    let to = vec3f(u.grass2.w, u.night.w, u.you_z) - world;
    let d2 = dot(to, to);
    let fall = u.night.z * 1.3 / (1.0 + d2 * 0.18) * (1.0 - smoothstep(36.0, 100.0, d2));
    let face = 0.45 + 0.55 * max(dot(n, to * inverseSqrt(max(d2, 0.0001))), 0.0);
    light += c * vec3f(1.0, 0.76, 0.44) * fall * face;
  }
  return light;
}

@fragment fn scene_fs(s: S) -> Lit {
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
  if (look >= 32u && look < 98u) { look -= 32u; }
  else if (look < 32u) {
    // (and, further out, anything nearer the lens than she is, so a cap or
    // a trunk between you and the camera never blots her or the way out)
    let near = distance(s.world, u.eye.xyz);
    let clear = max(1.6, distance(u.eye.xz, vec2f(u.grass2.w, u.you_z)));
    if (near < clear) {
      let p = vec2u(s.pos.xy) % 4u;
      let bayer = array(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
      if (smoothstep(clear * 0.8, clear, near) * 16.0 <= bayer[p.y * 4u + p.x] + 0.5) { discard; }
    }
  }
  if (look == 99u) {
    base = ground_colour(xz);
    // the banks: sand where the ground dips to the water
    base = mix(base, vec3f(0.58, 0.46, 0.26), smoothstep(u.grass2.y + 0.5, u.grass2.y - 0.1, s.world.y));
    shadow = blob_shadows(xz);
  } else if (look == 98u) {
    // the root-to-tip shading flattens where the grass is faded, so faded
    // blades read as soft grass rather than streaks
    let rr = restored(xz).x;
    base = ground_colour(xz) * mix(1.0 - 0.22 * (0.35 + 0.65 * rr), 1.0 + 0.2 * (0.35 + 0.65 * rr), s.tint.x) * (0.94 + 0.12 * s.tint.y);
    base = mix(base, base * vec3f(1.2, 1.08, 0.7), s.tint.x * 0.6);
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
    base += vec3f(1.0, 0.92, 0.75) * rim * 0.7;
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
    base = mix(base, vec3f(0.96, 0.99, 1.0), white);
    glow = 0.0;
  } else if (look == 7u) {
    keep = true;
  } else if (look == 8u) {
    // bark: moss in patches up the trunk, darker toward its foot
    let moss = smoothstep(0.35, 0.75, noise(vec2f(s.world.x + s.world.z, s.world.y * 1.3) * 1.7));
    base = mix(base, vec3f(0.26, 0.40, 0.12), moss * 0.7);
    base *= mix(0.5, 1.0, smoothstep(-0.5, 0.05, s.local.y));
  }
  let r = restored(xz);
  if (!keep) { base = faded_at(base, r.x, s.world); }
  var c = vec3f(0.0);
  var share = 0.0;
  if (glow > 0.0) {
    // what glows skips the bands; wings are pale light, brighter at their edges
    let v = normalize(u.eye.xyz - s.world);
    let edge = 1.0 - abs(dot(n, v));
    c = base * (1.3 + 0.9 * edge) * select(1.0, 0.75, glow < 1.0);
    if (!keep) { c *= mix(0.35, 1.0, r.x); }
    share = select(0.75, 0.45, glow < 1.0);
  } else {
    c = painted(base, n, s.world, shadow, character);
  }
  // a bloom's spreading front sparkles gold
  c += vec3f(1.0, 0.86, 0.5) * r.y * 0.6;
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

// ---- the painted water (art-styles.md 12): colour bands by depth, the sky's
// gradient in a Fresnel reflection, sparkles where a cell's tilt catches the
// sun, a clean white shore line and white rings. The mesh, the waves, the
// detail and the ripples are the W2 shader's (everglade.wgsl), from
// verse/fairy-forest/stream.coax's nodes (store 2).

const W_SHALLOW = vec3f(0.30, 0.72, 0.74);
const W_MID = vec3f(0.10, 0.45, 0.55);
const W_DEEP = vec3f(0.05, 0.22, 0.36);

struct WaterShade { emit: vec3f, transmit: vec3f }

fn water_fragment(i: WaterOut) -> WaterShade {
  var o: WaterShade;
  let world = i.world_depth.xyz;
  let rest = i.rest_flow.xy;
  let flow = i.rest_flow.zw;
  let depth = i.world_depth.w;
  let to_eye = u.eye.xyz - world;
  let dist = length(to_eye);
  let v = to_eye / max(dist, 0.001);
  if (u.eye.y < world.y) { o.emit = vec3f(0.0); o.transmit = vec3f(1.0); return o; }
  let footprint = water_footprint(v, dist);
  let swell = water_gerstner_slope(rest, depth, 1.0);
  let detail = water_flow_detail(rest, flow, footprint);
  let slope = swell.xy + detail.xy * 0.6 + water_ripples(rest);
  let n = normalize(vec3f(-slope.x, 1.0 - swell.z, -slope.y));
  let l = sun_dir();
  // the bands, lit softly
  var body = mix(mix(W_SHALLOW, W_MID, verse_paint_water_mid(depth)), W_DEEP, verse_paint_water_deep(depth));
  body *= key_c() * 0.85 + sky_amb() * 0.35;
  let r = restored(world.xz);
  body = faded(body, r.x);
  let fresnel = water_fresnel(max(dot(n, v), 0.001));
  var refl = faded(sky_colour(reflect(-v, n), false), max(r.x, 0.4));
  // sparkles: a cell sparkles when its hashed tilt mirrors the sun
  let grid = select(0.15, 0.25, u.sun.w > 0.5);
  let cell = floor(rest / grid);
  let jitter = vec2f(water_hash(vec3f(cell, 1.0)), water_hash(vec3f(cell, 2.0))) - vec2f(0.5);
  let tilt = normalize(n + vec3f(jitter.x, 0.0, jitter.y) * 0.35);
  let rdl = dot(reflect(-v, tilt), l);
  let twinkle = 0.5 + 0.5 * sin(u.time * (1.0 - u.still) * 21.0 + water_hash(vec3f(cell, 3.0)) * 6.28);
  let centre = (cell + vec2f(0.5)) * grid;
  let dot_ = 1.0 - smoothstep(footprint * 1.2, footprint * 2.2, distance(rest, centre));
  let sparkle = verse_paint_sparkle(rdl) * twinkle * dot_ * r.x;
  // the shore line and the rings
  let shore = i.look.w;
  let broken = noise(rest * 3.0) * 0.08;
  let line = (1.0 - smoothstep(0.18 + broken, 0.25 + broken, shore)) * smoothstep(-0.02, 0.04, depth);
  let rings = clamp(water_ripple_foam(rest) * 1.6, 0.0, 1.0);
  let foam = max(line, rings);
  let alpha = (0.55 + 0.4 * smoothstep(0.05, 0.9, depth)) * smoothstep(-0.05, 0.1, depth);
  let white = vec3f(1.0, 0.98, 0.94) * (key_c() * 0.9 + sky_amb() * 0.3);
  var emit = body * alpha * (1.0 - fresnel) + refl * fresnel * smoothstep(-0.05, 0.1, depth) + vec3f(1.0, 0.97, 0.88) * sparkle * 3.0;
  emit = mix(emit, white, foam);
  // the haze over the water, as over every surface
  let hz = verse_paint_haze(max(dist - HAZE_FROM, 0.0), world.y, u.eye.w) * 0.85;
  let trans = vec3f((1.0 - alpha) * (1.0 - fresnel) * (1.0 - foam));
  o.emit = emit * (1.0 - hz) + mix(haze_c(), haze_key(), verse_paint_haze_sun(dot(-v, l))) * hz * (vec3f(1.0) - trans);
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
// and an eighth of the size, mixed back gently; then the painterly tone.
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

// The tone (art-styles.md 9): 1 - exp(-k c), blacks lifted (verse_paint_tone).
// Fairy Forest's own: saturation up 1.28 and a little contrast round the middle, for a clear day.
fn tone(c: vec3f) -> vec3f {
  var t = vec3f(verse_paint_tone(c.r, 1.35), verse_paint_tone(c.g, 1.35), verse_paint_tone(c.b, 1.35));
  let l = dot(t, vec3f(0.2126, 0.7152, 0.0722));
  t = vec3f(l) + (t - vec3f(l)) * 1.28;
  t = mix(t, t * t * (3.0 - 2.0 * t), 0.25);
  return clamp(t, vec3f(0.0), vec3f(1.0));
}

@fragment fn post_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy);
  let uv = pos.xy / u.size;
  let light = textureLoad(post_light, p, 0).rgb;
  let near = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb;
  let far = textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb;
  let mixback = select(0.26, 0.2, u.sun.w > 0.5);
  var c = light + (near * 0.55 + far * 0.45) * mixback * 3.0;
  c *= mix(vec3f(1.0), vec3f(0.78, 0.84, 1.0), u.night.x);   // the night's exposure, a touch blue
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

// ---- generated: verse/paint.coax (verse_paint_wrap, verse_paint_band, verse_paint_band2, verse_paint_rim, verse_paint_haze, verse_paint_haze_sun, verse_paint_sky_mix, verse_paint_sky_band, verse_paint_halo, verse_paint_cloud, verse_paint_tone, verse_paint_bloom, verse_paint_water_mid, verse_paint_water_deep, verse_paint_sparkle) by coax/web/wgsl.py; do not edit
fn coax_p_pt_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_pt_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_pt_sat(l_x: f32) -> f32 {
  return coax_p_pt_min(1.0, coax_p_pt_max(0.0, l_x));
}
fn verse_paint_wrap(l_ndl: f32, l_shadow: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  coax_t0 = coax_p_pt_sat(((l_ndl + 0.25) / 1.25));
  coax_t1 = coax_p_pt_sat(l_shadow);
  return (coax_t0 * coax_t1);
}
fn coax_p_pt_ramp(l_a: f32, l_b: f32, l_x: f32) -> f32 {
  var l_t: f32 = coax_p_pt_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_paint_band(l_w: f32) -> f32 {
  var coax_t2: f32;
  var coax_t3: f32;
  coax_t2 = (0.550000012 * coax_p_pt_ramp(0.140000001, 0.25999999, l_w));
  coax_t3 = (0.449999988 * coax_p_pt_ramp(0.519999981, 0.639999986, l_w));
  return (coax_t2 + coax_t3);
}
fn verse_paint_band2(l_w: f32) -> f32 {
  return coax_p_pt_ramp(0.319999993, 0.479999989, l_w);
}
fn verse_paint_rim(l_ndv: f32, l_ndl: f32, l_strength: f32) -> f32 {
  var coax_t4: f32;
  var coax_t5: f32;
  coax_t4 = coax_p_pt_ramp(0.379999995, 0.119999997, l_ndv);
  coax_t5 = (0.349999994 + (0.649999976 * coax_p_pt_sat((l_ndl + 0.300000012))));
  return ((coax_t4 * coax_t5) * l_strength);
}
fn verse_paint_haze(l_d: f32, l_above: f32, l_density: f32) -> f32 {
  var coax_t6: f32;
  var coax_t7: f32;
  coax_t6 = (-l_density);
  coax_t7 = coax_p_pt_max((l_d - 20.0), 0.0);
  var l_f_dist: f32 = (1.0 - exp((coax_t6 * coax_t7)));
  var l_f_height: f32 = exp(((-coax_p_pt_max(l_above, 0.0)) / 60.0));
  return coax_p_pt_min((l_f_dist * (1.0 + ((l_f_height - 1.0) * 0.600000024))), 0.879999995);
}
fn verse_paint_haze_sun(l_vdl: f32) -> f32 {
  var l_c: f32 = coax_p_pt_sat(l_vdl);
  var l_c2: f32 = (l_c * l_c);
  var l_c4: f32 = (l_c2 * l_c2);
  return ((l_c4 * l_c4) * 0.449999988);
}
fn verse_paint_sky_mix(l_up: f32) -> f32 {
  if ((l_up <= 0.0))
  {
    return 0.0;
  }
  return pow(l_up, 0.550000012);
}
fn verse_paint_sky_band(l_up: f32) -> f32 {
  return (0.150000006 * (1.0 - coax_p_pt_ramp(0.0, 0.104999997, abs(l_up))));
}
fn verse_paint_halo(l_c: f32) -> f32 {
  var coax_t8: f32;
  var coax_t9: f32;
  coax_t8 = exp(((-900.0) * (1.0 - l_c)));
  coax_t9 = (0.349999994 * exp(((-60.0) * (1.0 - l_c))));
  return (coax_t8 + coax_t9);
}
fn verse_paint_cloud(l_n: f32, l_cover: f32) -> f32 {
  var coax_t10: f32;
  var coax_t11: f32;
  coax_t10 = coax_p_pt_ramp(0.479999989, 0.620000005, (l_n + ((l_cover - 0.349999994) * 0.400000006)));
  coax_t11 = coax_p_pt_ramp(0.0, 0.0500000007, l_cover);
  return (coax_t10 * coax_t11);
}
fn verse_paint_tone(l_c: f32, l_exposure: f32) -> f32 {
  var l_t: f32 = (1.0 - exp(((-coax_p_pt_max(l_c, 0.0)) * l_exposure)));
  return (0.0350000001 + (0.964999974 * l_t));
}
fn verse_paint_bloom(l_l: f32) -> f32 {
  var coax_t12: f32;
  var coax_t13: f32;
  var l_k: f32 = coax_p_pt_sat(((l_l - 0.600000024) / 0.5));
  var l_soft: f32 = ((l_k * l_k) * 0.25);
  coax_t12 = coax_p_pt_max(l_soft, (l_l - 0.850000024));
  coax_t13 = coax_p_pt_max(l_l, 9.99999975e-05);
  return (coax_t12 / coax_t13);
}
fn verse_paint_water_mid(l_depth: f32) -> f32 {
  return coax_p_pt_ramp(0.324999988, 0.474999994, l_depth);
}
fn verse_paint_water_deep(l_depth: f32) -> f32 {
  return coax_p_pt_ramp(1.42499995, 1.57500005, l_depth);
}
fn verse_paint_sparkle(l_rdl: f32) -> f32 {
  return coax_p_pt_ramp(0.997560024, 0.999300003, l_rdl);
}
// ---- end generated

// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter) by coax/web/wgsl.py; do not edit
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
// ---- end generated
