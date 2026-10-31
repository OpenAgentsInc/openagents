// The water lab (waterlab.coax): one traced pass. A sea to the horizon on
// the spectral tiles waterlab.coax packs into the uniforms each frame
// (verse/water-spectrum.coax, phase W4 of docs/verse/water.md), breaking on
// a sloping beach, in the painted water look of docs/verse/art-styles.md 12
// (Everglade's river preset): depth bands, the painted sky in Schlick's
// Fresnel on a calmer normal, sparkles near the sun's reflection, white
// paint for whitecaps, surf and the waterline, and the hazy, cyan-green air
// of the owner's references.
//
// W10 (verse/water-ocean.coax): the sea is a clipmap mesh. surface_vs builds
// each vertex from its index and the levels' numbers (nested grids round the
// eye, each twice the last's spacing, the odd vertices near a level's edge
// sliding onto the next level's grid, the last row out to the horizon), lifts
// it by the sea's height and moves it by cascade 0's choppy displacement;
// surface_fs shades it as the trace did. bg_fs then draws only the sky and
// the beach and writes their depth. The coast's bed depth and surf phase
// come from 8 m cells streamed into the block (shore_at), or are computed
// where no cell is resident.
struct U {
  size: vec4f,    // width, height, time, fade
  mode: vec4f,    // still, phone, the clipmap draws the sea (1), dpr
  eye: vec4f,     // xyz, tan of half the vertical field
  fwd: vec4f, right: vec4f, up: vec4f,
  sun: vec4f,     // toward the sun, strength
  sea: vec4f,     // level, cascade 0's patch, the fine patch, choppiness
  wind: vec4f,    // direction x, z, speed, significant height
  beach: vec4f,   // the still waterline's z, the slope, the peak's deep wavenumber, its omega
  pack: vec4f,    // the packed numbers' scales: height, slope, fine slope, choppy displacement
  clip: vec4f,    // the clipmap: quads a side, levels, morph band (quads), the skirt's reach (m)
  levels: array<vec4f, 12>,  // centre x, z, spacing, hole x + 2 hole z
  cells: vec4f,   // the cell window's first cell x, z, its side (cells), texel spacing (m)
  bases: array<vec4f, 12>,   // each slot's surf phase at its first texel
  words: array<vec4u, 3900>, // the table, the cells, cascade 0, its displacement, the fine slopes
}
@group(0) @binding(0) var<uniform> u: U;

const N = 64u;
const G = 9.81;

const W_TABLE = 0u;
const W_CELLS = 64u;
const W_COARSE = 1264u;
const W_CHOP = 9456u;
const W_FINE = 11504u;
const CELL = 8.0;
const CELL_TEXELS = 5u;
fn word(i: u32) -> u32 { return u.words[i >> 2u][i & 3u]; }
// Cascade 0 at a rest point: height (m), foam, slope x, slope z; bilinear, wrapped.
fn coarse(p: vec2f) -> vec4f {
  let q = p / u.sea.y * f32(N) - 0.5;
  let f = fract(q);
  let i = vec2i(floor(q));
  var acc = vec4f(0.0);
  for (var k = 0; k < 4; k++) {
    let o = vec2i(k & 1, k >> 1);
    let c = vec2u((i + o) & vec2i(i32(N) - 1));
    let t = c.y * N + c.x;
    let a = unpack2x16float(word(W_COARSE + 2u * t));
    let b = unpack2x16float(word(W_COARSE + 2u * t + 1u));
    let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
    acc += w * vec4f(a.x * u.pack.x, a.y, b.x * u.pack.y, b.y * u.pack.y);
  }
  return acc;
}
// The fine tile's slopes (the desktop's second cascade, the phone's baked tile).
fn fine(p: vec2f) -> vec2f {
  let q = p / u.sea.z * f32(N) - 0.5;
  let f = fract(q);
  let i = vec2i(floor(q));
  var acc = vec2f(0.0);
  for (var k = 0; k < 4; k++) {
    let o = vec2i(k & 1, k >> 1);
    let c = vec2u((i + o) & vec2i(i32(N) - 1));
    let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
    acc += w * unpack2x16float(word(W_FINE + c.y * N + c.x)) * u.pack.z;
  }
  return acc;
}
// Cascade 0's choppy displacement at a rest point (m), bilinear, wrapped.
fn chop(p: vec2f) -> vec2f {
  let q = p / u.sea.y * f32(N) - 0.5;
  let f = fract(q);
  let i = vec2i(floor(q));
  var acc = vec2f(0.0);
  for (var k = 0; k < 4; k++) {
    let o = vec2i(k & 1, k >> 1);
    let c = vec2u((i + o) & vec2i(i32(N) - 1));
    let t = c.y * N + c.x;
    let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
    let half = (word(W_CHOP + (t >> 1u)) >> (16u * (t & 1u))) & 0xffffu;
    acc += w * unpack4x8snorm(half).xy;
  }
  return acc * u.pack.w;
}

fn hash2(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), s.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), s.x), s.y);
}

// The beach: the ground's height (m), rising toward +z through the still
// waterline at z = beach.x, gently uneven along x, levelling out at 40 m deep.
fn ground(p: vec2f) -> f32 {
  let wobble = 0.35 * sin(p.x * 0.045) + 0.18 * sin(p.x * 0.11 + 1.3);
  let z = p.y - u.beach.x - wobble / u.beach.y * 0.05;
  // out of the water the beach climbs to a berm, under it the shelf runs out gently
  let land = max(z, 0.0) * 0.09;
  return max(z * u.beach.y + land + 0.03 * noise(p * 0.3), -40.0);
}
fn depth_here(p: vec2f) -> f32 { return max(u.sea.x - ground(p), 0.0); }

// Shallow water (verse/water-spectrum.coax wsp_surf): the peak wave's
// wavenumber over depth h (Eckart's form of w^2 = g k tanh(k h)), the
// shoaling gain sqrt(cg0 / cg), and how far it is past breaking.
fn shallow_k(k0: f32, h: f32) -> f32 { return k0 / sqrt(max(tanh(k0 * max(h, 0.01)), 1e-4)); }
fn shoal_gain(k0: f32, h: f32) -> f32 {
  let k = shallow_k(k0, h);
  let kh = k * max(h, 0.01);
  let n = 0.5 * (1.0 + 2.0 * kh / sinh(min(2.0 * kh, 20.0)));
  let c = sqrt(G / k * tanh(kh));
  let cg0 = 0.5 * sqrt(G / k0);
  return sqrt(cg0 / max(n * c, 1e-3));
}
// The surf's phase across the beach: the integral of k(h) along the way in.
fn surf_phase_here(p: vec2f) -> f32 {
  let k0 = u.beach.z;
  let h = depth_here(p);
  let reach = min(h, 3.0 / k0);
  var sum = 0.0;
  for (var i = 0; i < 8; i++) {
    let hh = (f32(i) + 0.5) / 8.0 * reach;
    sum += shallow_k(k0, hh);
  }
  return (sum / 8.0 * reach + (h - reach) * k0) / u.beach.y;
}
// The coast's bed depth and surf phase at p: from its streamed 8 m cell when
// one is resident (bilinear over the cell's 2 m texels), else computed here.
fn shore_at(p: vec2f) -> vec2f {
  let c = vec2i(floor(p / CELL)) - vec2i(u.cells.xy);
  let side = i32(u.cells.z);
  if (all(c >= vec2i(0)) && all(c < vec2i(side))) {
    let k = u32(c.y * side + c.x);
    let slot = (word(W_TABLE + (k >> 2u)) >> (8u * (k & 3u))) & 0xffu;
    if (slot > 0u) {
      let s = slot - 1u;
      let q = (p - floor(p / CELL) * CELL) / u.cells.w;
      let i = min(vec2u(q), vec2u(CELL_TEXELS - 2u));
      let f = q - vec2f(i);
      let at = W_CELLS + s * CELL_TEXELS * CELL_TEXELS + i.y * CELL_TEXELS + i.x;
      let a = unpack2x16float(word(at));
      let b = unpack2x16float(word(at + 1u));
      let d = unpack2x16float(word(at + CELL_TEXELS));
      let e = unpack2x16float(word(at + CELL_TEXELS + 1u));
      let v = mix(mix(a, b, f.x), mix(d, e, f.x), f.y);
      return vec2f(v.x, v.y + u.bases[s >> 2u][s & 3u]);
    }
  }
  return vec2f(depth_here(p), surf_phase_here(p));
}
fn depth_at(p: vec2f) -> f32 { return shore_at(p).x; }
fn surf_phase(p: vec2f) -> f32 { return shore_at(p).y; }
struct Surf { lift: f32, white: f32, gain: f32, depth: f32 }
fn surf(p: vec2f, t: f32) -> Surf {
  var o: Surf;
  let sh = shore_at(p);
  let h = sh.x;
  o.depth = h;
  let k0 = u.beach.z;
  o.gain = select(min(shoal_gain(k0, h), 2.2), 1.0, h > 3.0 / k0);
  let height = u.wind.w * 0.5 * o.gain;
  let breaking = clamp(height / (0.78 * max(h, 0.02)) - 0.6, 0.0, 1.0);
  // the waves come in shore-normal, their crests bending with the beach's wobble
  let phase = sh.y + u.beach.w * t + 0.6 * noise(vec2f(p.x * 0.03, 0.0)) * 6.28;
  let crest = pow(0.5 + 0.5 * cos(phase), 6.0);
  let line = pow(0.5 + 0.5 * cos(phase - 0.25), 40.0);
  o.lift = crest * height * 0.6 * smoothstep(0.0, 0.4, h) * (1.0 - 0.6 * breaking);
  let broken = noise(vec2f(p.x * 0.7, phase * 2.0)) * 0.6 + 0.4;
  // white paint on the breaking crest, and a thinner trail behind it
  let trail = pow(0.5 + 0.5 * cos(phase + 0.9), 10.0) * 0.6;
  o.white = breaking * smoothstep(0.45, 0.8, max(line, trail) * broken + 0.15 * breaking * breaking) * smoothstep(0.03, 0.2, h);
  return o;
}

// The sea's surface at rest point p: its height over the level.
fn sea_height(p: vec2f, t: f32) -> f32 {
  let s = surf(p, t);
  let h = s.depth;
  let taper = smoothstep(0.0, 0.6, h);
  // a trough never bares the bed: shallow water's waves ride over it
  let wave = max(coarse(p).x * min(s.gain, 1.6) * taper, -0.5 * h);
  return u.sea.x + wave + s.lift;
}

// The painted sky: zenith to a pale haze at the horizon, warm toward the sun.
fn sky(d: vec3f) -> vec3f {
  let up = clamp(d.y, 0.0, 1.0);
  let zenith = vec3f(0.36, 0.60, 0.80);
  let horizon = vec3f(0.80, 0.88, 0.86);
  var c = mix(horizon, zenith, pow(up, 0.55));
  let s = max(dot(d, u.sun.xyz), 0.0);
  c += vec3f(1.0, 0.86, 0.62) * (pow(s, 8.0) * 0.30 + pow(s, 900.0) * 1.2);
  // soft clouds, broad and pale
  if (d.y > 0.0) {
    let q = d.xz / (d.y + 0.15) * 1.6;
    let cl = smoothstep(0.55, 0.85, noise(q) * 0.65 + noise(q * 2.3 + 4.0) * 0.35);
    c = mix(c, vec3f(0.95, 0.96, 0.93), cl * 0.55 * smoothstep(0.0, 0.25, d.y));
  }
  return c;
}
// The air between the eye and what it sees: BotW's teal-green haze.
fn haze(c: vec3f, d: vec3f, dist: f32) -> vec3f {
  let a = 1.0 - exp(-dist / 420.0);
  let tint = mix(vec3f(0.74, 0.86, 0.84), vec3f(1.0, 0.90, 0.74), pow(max(dot(d, u.sun.xyz), 0.0), 6.0) * 0.5);
  return mix(c, tint, a * 0.85);
}

fn sand(p: vec2f, n: vec3f, wet: f32) -> vec3f {
  let dry = mix(vec3f(0.84, 0.77, 0.60), vec3f(0.78, 0.71, 0.55), noise(p * 1.7));
  let base = mix(dry, vec3f(0.64, 0.58, 0.45), wet);
  let light = 0.55 + 0.6 * max(dot(n, u.sun.xyz), 0.0);
  return base * light * vec3f(1.0, 0.97, 0.92);
}

// The water in the painted look.
fn water(p: vec2f, world: vec3f, d: vec3f, dist: f32, t: f32) -> vec3f {
  let c = coarse(p);
  let s = surf(p, t);
  let h = s.depth;
  // slopes: cascade 0 shoaled, the fine cascade faded with the pixel's footprint
  let footprint = dist * 2.0 * u.eye.w / u.size.y / max(abs(d.y), 0.08);
  let keep = clamp(u.sea.z / 64.0 / max(footprint, 1e-4) - 0.5, 0.0, 1.0);
  let farfade = 1.0 - smoothstep(150.0, 900.0, dist);
  let slope = (c.zw * min(s.gain, 1.6) + fine(p * 1.0 + vec2f(u.wind.x, u.wind.y) * t * 0.3) * keep) * farfade;
  let n = normalize(vec3f(-slope.x, 1.0, -slope.y));
  let calm = normalize(vec3f(-slope.x / 3.0, 1.0, -slope.y / 3.0));
  // depth bands, each edge soft over 0.15 m, and a deeper band for the open sea
  let shallow = vec3f(0.30, 0.72, 0.74);
  let mid = vec3f(0.10, 0.45, 0.55);
  let deep = vec3f(0.05, 0.22, 0.36);
  let open = vec3f(0.035, 0.17, 0.31);
  var col = mix(shallow, mid, smoothstep(0.25, 0.55, h));
  col = mix(col, deep, smoothstep(1.35, 1.65, h));
  col = mix(col, open, smoothstep(6.0, 14.0, h));
  // the land's light on the bands, the waves' faces a little darker
  col *= 0.82 + 0.3 * max(dot(n, u.sun.xyz), 0.0);
  // the painted sky in Schlick's Fresnel, at most half of it
  let v = -d;
  let r = reflect(d, calm);
  let fres = 0.02 + 0.98 * pow(1.0 - max(dot(calm, v), 0.0), 5.0);
  col = mix(col, sky(vec3f(r.x, abs(r.y), r.z)), min(fres, 0.5));
  // sparkles on a grid within a few degrees of the sun's reflection, twinkling
  let rs = reflect(-u.sun.xyz, n);
  let cell = select(0.15, 0.25, u.mode.y > 0.5) * max(1.0, footprint / 0.05);
  let g = floor(world.xz / cell);
  let tw = hash2(g + floor(t * 3.0));
  let near = smoothstep(cos(0.07 + footprint * 0.02), 1.0, dot(rs, v));
  col += vec3f(1.0, 0.95, 0.85) * step(0.82, tw) * near * 1.4 * farfade;
  // whitecaps from the foam field, broken into paint by noise; the surf; the waterline
  let froth = noise(p * 1.4 + vec2f(t * 0.2, 0.0)) * 0.6 + noise(p * 4.1) * 0.4;
  let caps = smoothstep(0.45, 0.75, c.y * (0.6 + froth));
  // the waterline: where the water over the sand runs thin
  let thin = world.y - ground(p);
  let line = (1.0 - smoothstep(0.006, 0.02, thin + 0.004 * sin(p.x * 3.1 + t * 1.5))) * smoothstep(0.3, 0.6, froth + 0.25);
  let white = max(max(caps * 0.95, s.white * smoothstep(0.3, 0.6, froth + 0.3)), line);
  col = mix(col, vec3f(0.97, 0.98, 0.96) * (0.85 + 0.15 * max(dot(n, u.sun.xyz), 0.0)), clamp(white, 0.0, 1.0));
  return col;
}

// The traced pass.
struct Hit { t: f32, water: bool }
fn trace(o: vec3f, d: vec3f, t0: f32) -> Hit {
  var hit = Hit(1e9, false);
  let amp = u.wind.w * 1.3 + 0.4;
  // the beach: march the ground until it rises over the ray
  var t = 0.0;
  var prev = o.y - ground(o.xz);
  for (var i = 0; i < 64; i++) {
    let step = 0.4 + t * 0.06;
    let nt = t + step;
    let q = o + d * nt;
    let gap = q.y - ground(q.xz);
    if (gap < 0.0) { hit.t = t + step * prev / max(prev - gap, 1e-4); break; }
    prev = gap; t = nt;
    if (t > 900.0) { break; }
  }
  // the water: between the planes the waves can reach, then secant refinement
  // (only when the clipmap isn't drawing the sea)
  if (u.mode.z < 0.5 && (d.y < 0.0 || o.y < u.sea.x + amp)) {
    let top = (o.y - (u.sea.x + amp)) / max(-d.y, 1e-4);
    let bottom = (o.y - (u.sea.x - amp)) / max(-d.y, 1e-4);
    var a = max(top, 0.0);
    var b = select(min(bottom, 2000.0), 2000.0, d.y >= 0.0);
    let steps = 14;
    var fa = (o + d * a).y - sea_height((o + d * a).xz, t0);
    var found = false;
    for (var i = 1; i <= steps; i++) {
      let tb = a + (b - a) * f32(i) / f32(steps);
      let q = o + d * tb;
      let fb = q.y - sea_height(q.xz, t0);
      if (fb < 0.0) { b = tb; found = true; break; }
      a = tb; fa = fb;
    }
    if (found) {
      var fb = (o + d * b).y - sea_height((o + d * b).xz, t0);
      for (var i = 0; i < 4; i++) {
        let tm = a + (b - a) * fa / max(fa - fb, 1e-5);
        let fm = (o + d * tm).y - sea_height((o + d * tm).xz, t0);
        if (fm > 0.0) { a = tm; fa = fm; } else { b = tm; fb = fm; }
      }
      let tw = a + (b - a) * fa / max(fa - fb, 1e-5);
      if (tw < hit.t) { hit = Hit(tw, true); }
    } else if (d.y < 0.0 && bottom > 2000.0 && 2000.0 < hit.t) {
      hit = Hit((o.y - u.sea.x) / -d.y, true);
    }
  }
  return hit;
}


// ---- under the water (phase W7; verse/water-under.coax holds the CPU half and its checks).
// Each pixel's near-plane point is tested against the surface over it, so the view splits
// exactly where the near plane straddles the waves (the split waterline). Under it: per-channel
// Beer-Lambert fog toward a painted teal, the surface from below through Snell's window (the sky
// bent through it inside 48.6 degrees of straight up, the water's own colour mirrored past it),
// caustics on the sand, drifting motes; on a desktop also the view's wavering, 16 samples of sun
// shafts and the meniscus where the surface crosses the lens.
const UNDER_ABSORB = vec3f(0.30, 0.075, 0.06);
const UNDER_SCATTER = vec3f(0.05, 0.33, 0.42);
fn caustic_pattern(p: vec2f, t: f32) -> f32 {
  let a = noise(p * 1.1 + vec2f(t * 0.25, t * 0.1));
  let b = noise(p * 1.9 + vec2f(3.1, 7.7) - vec2f(t * 0.18, t * 0.3));
  let ra = 1.0 - abs(2.0 * a - 1.0);
  let rb = 1.0 - abs(2.0 * b - 1.0);
  return pow(ra, 7.0) * 0.8 + pow(rb, 7.0) * 0.55;
}
fn surface_normal(p: vec2f, t: f32) -> vec3f {
  let c = coarse(p);
  let sl = c.zw + fine(p + vec2f(u.wind.x, u.wind.y) * t * 0.3);
  return normalize(vec3f(-sl.x, 1.0, -sl.y));
}
fn hash3(c: vec3f) -> f32 { return fract(sin(dot(c, vec3f(127.1, 311.7, 74.7))) * 43758.5453); }
fn motes(o: vec3f, d: vec3f, reach: f32, t: f32) -> f32 {
  var a = 0.0;
  let n = select(8, 4, u.mode.y > 0.5);
  for (var i = 0; i < n; i++) {
    let c = floor(o + d * ((f32(i) + 0.5) * reach / f32(n)));
    if (hash3(c + 0.31) > 0.25) { continue; }
    let m = c + vec3f(hash3(c), hash3(c + 7.0), hash3(c + 13.0)) + vec3f(sin(t * 0.3 + hash3(c) * 6.0), cos(t * 0.21), 0.0) * 0.08;
    let along = max(dot(m - o, d), 0.05);
    if (along > reach) { continue; }
    let size = 0.0025 * along * u.size.y / 900.0 * 4.0 + 0.004;
    a += (1.0 - smoothstep(size * 0.4, size, length(m - (o + d * along)))) * 0.5 * exp(-along * 0.2);
  }
  return a;
}
fn shafts(o: vec3f, d: vec3f, path: f32, t: f32) -> f32 {
  if (u.mode.y > 0.5) { return 0.0; }
  var sum = 0.0;
  let reach = min(path, 14.0);
  for (var i = 0; i < 16; i++) {
    let q = o + d * ((f32(i) + 0.5) / 16.0 * reach);
    let below = max(u.sea.x - q.y, 0.0);
    sum += caustic_pattern((q.xz + u.sun.xz * below * 0.75) * 0.5, t) * exp(-below * 0.12 - length(q - o) * 0.08);
  }
  return sum / 16.0 * reach * 0.12;
}
fn under_view(o: vec3f, d: vec3f, t: f32) -> vec3f {
  // the sand
  var tg = 1e9;
  var s = 0.0;
  var prev = o.y - ground(o.xz);
  for (var i = 0; i < 48; i++) {
    let step = 0.3 + s * 0.08;
    let q = o + d * (s + step);
    let gap = q.y - ground(q.xz);
    if (gap < 0.0) { tg = s + step * prev / max(prev - gap, 1e-4); break; }
    prev = gap; s += step;
    if (s > 120.0) { break; }
  }
  // the surface from below
  var ts = 1e9;
  if (d.y > 0.0) {
    let amp = u.wind.w * 1.3 + 0.4;
    let top = (u.sea.x + amp - o.y) / d.y;
    var a = 0.0;
    var fa = sea_height(o.xz, t) - o.y;
    for (var i = 1; i <= 16; i++) {
      let b = top * f32(i) / 16.0;
      let q = o + d * b;
      let fb = sea_height(q.xz, t) - q.y;
      if (fb < 0.0) { ts = a + (b - a) * fa / max(fa - fb, 1e-5); break; }
      a = b; fa = fb;
    }
  }
  let depth_eye = max(u.sea.x - o.y, 0.0);
  let light = UNDER_SCATTER * (0.55 + 0.45 * exp(-depth_eye * 0.08)) * (0.6 + 0.4 * u.sun.w);
  var col = light;
  var path = 160.0;
  if (ts < tg && ts < 1e8) {
    path = ts;
    let w = o + d * ts;
    let n = surface_normal(w.xz, t);
    let up = dot(d, n);
    let r = refract(d, -n, 1.33);
    let mirrored = light * 0.8;
    if (dot(r, r) < 1e-6) {
      col = mirrored;           // past the critical angle: the surface mirrors the water
    } else {
      let f0 = 0.0200593;
      let tr = 1.0 - (f0 + (1.0 - f0) * pow(1.0 - clamp(up, 0.0, 1.0), 5.0));
      col = mix(mirrored, sky(r) * 1.05, tr * smoothstep(0.0, 0.08, up));
      col += vec3f(1.0, 0.95, 0.8) * pow(max(dot(r, u.sun.xyz), 0.0), 300.0) * 2.0 * tr;
    }
    // foam floating over the eye
    col = mix(col, vec3f(0.9, 0.95, 0.94), coarse(w.xz).y * 0.6);
  } else if (tg < 1e8) {
    path = tg;
    let w = o + d * tg;
    let e = 0.05;
    let n = normalize(vec3f(ground(w.xz - vec2f(e, 0.0)) - ground(w.xz + vec2f(e, 0.0)), 2.0 * e,
                            ground(w.xz - vec2f(0.0, e)) - ground(w.xz + vec2f(0.0, e))));
    let below = max(u.sea.x - w.y, 0.0);
    var c = sand(w.xz, n, 1.0) * exp(-UNDER_ABSORB * below);
    c += vec3f(1.0, 0.96, 0.82) * caustic_pattern(w.xz * 0.9, t) * exp(-below * 0.15) * smoothstep(0.0, 0.3, below) * 0.9 * max(n.y, 0.0);
    col = c;
  }
  let tr = exp(-UNDER_ABSORB * path);
  col = col * tr + light * (vec3f(1.0) - tr);
  col += vec3f(0.95, 0.97, 0.85) * (motes(o, d, min(path, 7.0), t) * 0.7 + shafts(o, d, path, t));
  return col;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
// The depth buffer's value for a point `along` the view ray d (1 - near / view z: the
// surface's projection), so the clipmap hides behind the traced beach.
const NEAR = 0.1;
fn depth_of(d: vec3f, along: f32) -> f32 { return clamp(1.0 - NEAR / max(along * dot(d, u.fwd.xyz), NEAR), 0.0, 1.0); }
struct Out { @location(0) c: vec4f, @builtin(frag_depth) depth: f32 }
@fragment fn bg_fs(@builtin(position) frag: vec4f) -> Out {
  let ndc = vec2f(frag.x / u.size.x * 2.0 - 1.0, 1.0 - frag.y / u.size.y * 2.0);
  let aspect = u.size.x / u.size.y;
  let d = normalize(u.fwd.xyz + u.right.xyz * ndc.x * u.eye.w * aspect + u.up.xyz * ndc.y * u.eye.w);
  let o = u.eye.xyz;
  let t = u.size.z;
  // the split waterline: is this pixel's point on the near plane under the waves?
  let near = o + d * 0.1;
  let line = near.y - sea_height(near.xz, t);
  if (line < 0.0) {
    // on a desktop the view wavers a little under the water
    let waver = select(vec3f(sin(frag.y * 0.02 + t * 1.7), cos(frag.x * 0.017 + t * 1.3), 0.0) * 0.004, vec3f(0.0), u.mode.y > 0.5);
    var c = under_view(o, normalize(d + waver.x * u.right.xyz + waver.y * u.up.xyz), t);
    // the meniscus: a bright edge where the surface crosses the lens
    if (u.mode.y < 0.5) { c = mix(c, vec3f(0.85, 0.95, 0.95), (1.0 - smoothstep(0.0, 0.006, -line)) * 0.7); }
    return Out(vec4f(c, 1.0), 0.0);  // nothing draws over the view under the water
  }
  let hit = trace(o, d, t);
  var c: vec3f;
  if (hit.t > 1e8) {
    c = haze(sky(d), d, 3000.0 * (1.0 - smoothstep(0.0, 0.2, d.y)));
  } else {
    let w = o + d * hit.t;
    if (hit.water) {
      c = water(w.xz, w, d, hit.t, t);
    } else {
      let e = 0.05;
      let n = normalize(vec3f(ground(w.xz - vec2f(e, 0.0)) - ground(w.xz + vec2f(e, 0.0)), 2.0 * e,
                              ground(w.xz - vec2f(0.0, e)) - ground(w.xz + vec2f(0.0, e))));
      // the swash leaves the sand wet a little above the still waterline
      let wet = 1.0 - smoothstep(0.08, 0.3, w.y - u.sea.x);
      c = sand(w.xz, n, wet);
    }
    c = haze(c, d, hit.t);
  }
  return Out(vec4f(c, 1.0), select(depth_of(d, hit.t), 1.0, hit.t > 1e8));
}

// ---- The clipmap sea (W10): a vertex from its index, as ocean_vertex does it.
struct S { @builtin(position) pos: vec4f, @location(0) rest: vec2f, @location(1) world: vec3f }
@vertex fn surface_vs(@builtin(vertex_index) vid: u32) -> S {
  let n = u32(u.clip.x);
  let count = u32(u.clip.y);
  let half = n / 2u;
  var q = vid / 6u;
  let corner = vid % 6u;
  let full = n * n;
  let ring = full - half * half;
  var l = 0u;
  if (q >= full) { q -= full; l = min(1u + q / ring, count - 1u); q = q - (l - 1u) * ring; }
  let lv = u.levels[l];
  var qx = q % n; var qz = q / n;
  if (l > 0u) {
    let hole = u32(lv.w);
    let hx = n / 4u + (hole & 1u); let hz = n / 4u + (hole >> 1u);
    let top = hz * n;
    if (q >= top) {
      let band = q - top;
      if (band < half * half) {
        let col = band % half;
        qx = select(col + half, col, col < hx); qz = hz + band / half;
      } else {
        let rest = band - half * half;
        qx = rest % n; qz = hz + half + rest / n;
      }
    }
  }
  let cx = array(0u, 1u, 0u, 0u, 1u, 1u);
  let cz = array(0u, 0u, 1u, 1u, 0u, 1u);
  let ix = i32(qx + cx[corner]); let iz = i32(qz + cz[corner]);
  let gx = i32(round(lv.x / lv.z)) - i32(half) + ix;
  let gz = i32(round(lv.y / lv.z)) - i32(half) + iz;
  let rx = abs(ix - i32(half)); let rz = abs(iz - i32(half));
  let edge = max(rx, rz);
  var a = 0.0;
  if (l + 1u < count) { a = clamp(f32(edge - (i32(half) - i32(u.clip.z))) / u.clip.z, 0.0, 1.0); }
  var p = vec2f((f32(gx) - a * f32(gx & 1)) * lv.z, (f32(gz) - a * f32(gz & 1)) * lv.z);
  if (l + 1u == count && edge == i32(half)) { p = lv.xy + (p - lv.xy) * (u.clip.w / (f32(half) * lv.z)); }
  // the waves: the sea's height, and cascade 0's choppy displacement, both faded out where
  // the quads grow too coarse to carry them (past a few patches)
  let t = u.size.z;
  let reach = length(p - u.eye.xz);
  let keep = 1.0 - smoothstep(u.sea.y * 2.5, u.sea.y * 8.0, reach);
  let taper = smoothstep(0.0, 0.6, depth_at(p));
  let moved = chop(p) * (keep * taper);
  let w = vec3f(p.x + moved.x, u.sea.x + (sea_height(p, t) - u.sea.x) * keep, p.y + moved.y);
  var o: S;
  let rel = w - u.eye.xyz;
  let vz = dot(rel, u.fwd.xyz);
  let aspect = u.size.x / u.size.y;
  o.pos = vec4f(dot(rel, u.right.xyz) / (u.eye.w * aspect), dot(rel, u.up.xyz) / u.eye.w, vz - NEAR, vz);
  o.rest = p;
  o.world = w;
  return o;
}
@fragment fn surface_fs(v: S) -> @location(0) vec4f {
  let rel = v.world - u.eye.xyz;
  let dist = length(rel);
  let d = rel / max(dist, 1e-4);
  let c = water(v.rest, v.world, d, dist, u.size.z);
  return vec4f(haze(c, d, dist), 1.0);
}

// The strokes (none yet): host.js draws them over the pass.
struct V { @builtin(position) pos: vec4f, @location(0) tint: vec4f }
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) e: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  o.tint = c;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f { return vec4f(v.tint.rgb, 1.0); }

// The painterly style's shading (verse/style.coax), written by build.sh.
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter) by coax wgsl; do not edit
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
