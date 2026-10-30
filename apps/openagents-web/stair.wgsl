// Hold the Stair (stair.coax): bg_fs draws the tower's floor seen from above
// with a slight tilt, the stair winding up it, the cellar door, the tower
// top, the daises under the party and the panels; fx_vs/fx_fs draw the
// strokes stair_frame writes (beasts, the party, bolts, numbers, text).
struct U {
  size: vec2f, time: f32, fade: f32,
  map: vec4f,      // origin x, y, tile (device px), squash
  info: vec4f,     // screen cols, rows, portrait, hover cell
  info2: vec4f,    // hurt, selected cell, plates, 0
  cells: array<vec4f, 32>,
  plate: array<vec4f, 48>,
}
@group(0) @binding(0) var<uniform> u: U;

const COLS = 14;

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn cell_kind(i: i32) -> i32 {
  if (i < 0 || i >= 126) { return -1; }
  return i32(u.cells[i / 4][i % 4] + 0.5);
}

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 5.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  var fill = vec3f(0.018, 0.02, 0.03);
  var line = vec3f(0.0);
  if (s.y > 0.5) { fill = vec3f(0.05, 0.055, 0.075); line = vec3f(0.42, 0.44, 0.52); }
  if (s.y > 1.5) { fill = vec3f(0.1, 0.1, 0.14); line = vec3f(0.95, 0.85, 0.55); }
  if (s.y > 2.5) { fill = vec3f(0.03, 0.03, 0.04); line = vec3f(0.18, 0.18, 0.22); }
  let w = 1.0 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * a * 0.9);
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}

fn tile_colour(kind: i32, f: vec2f, id: vec2f, c: i32, r: i32) -> vec3f {
  let n = noise(id * 3.1 + f * 4.0) * 0.5 + hash(id) * 0.5;
  // mortar between flagstones
  let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
  let mortar = smoothstep(0.0, 0.06, edge);
  // the slight tilt: each slab's far edge catches light, its near edge falls in shadow
  let bevel = 1.0 + 0.22 * (1.0 - smoothstep(0.0, 0.12, f.y)) - 0.35 * smoothstep(0.84, 1.0, f.y);
  var base = vec3f(0.07, 0.075, 0.095) * (0.8 + 0.4 * n);
  if (kind == 1 || kind == 3 || kind == 4) {
    // the stair: worn warm stone, step lines across it
    base = vec3f(0.24, 0.17, 0.11) * (0.8 + 0.35 * n);
    let steps = smoothstep(0.0, 0.05, abs(fract(f.y * 3.0) - 0.5) - 0.42);
    base *= 0.85 + 0.15 * (1.0 - steps);
  }
  if (kind == 2) {
    let d = length((f - vec2f(0.5)) * vec2f(1.0, 1.0));
    base = mix(base, vec3f(0.14, 0.14, 0.18), smoothstep(0.42, 0.38, d));
  }
  if (kind == 3) {
    // the cellar door: a dark arch with fire behind it
    let d = length((f - vec2f(0.5, 0.62)) * vec2f(1.3, 1.0));
    base = mix(base, vec3f(0.5, 0.18, 0.05) * (0.7 + 0.3 * sin(u.time * 3.0)), smoothstep(0.42, 0.3, d));
  }
  if (kind == 4) {
    // the tower top: a gold landing
    let d = length(f - vec2f(0.5));
    base = mix(base, vec3f(0.55, 0.42, 0.16), smoothstep(0.46, 0.2, d));
  }
  return base * mortar * bevel;
}

@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let q = (p / u.size - 0.5) * 2.0;
  var c = vec3f(0.012, 0.012, 0.02) * (1.0 - 0.3 * length(q));
  let t = u.map.z;
  let local = (p - u.map.xy) / vec2f(t, t * u.map.w);
  if (t > 0.0 && local.x >= 0.0 && local.y >= 0.0 && local.x < u.info.x && local.y < u.info.y) {
    let sc = floor(local);
    let f = fract(local);
    var col = i32(sc.x); var row = i32(sc.y);
    var lf = f;
    if (u.info.z > 0.5) { col = COLS - 1 - i32(sc.y); row = i32(sc.x); lf = vec2f(1.0 - f.y, f.x); }
    let i = col + row * COLS;
    let kind = cell_kind(i);
    c = tile_colour(kind, f, sc, col, row);
    // torchlight pools along the walls
    c *= 0.85 + 0.3 * noise(p / max(t, 1.0) * 0.35 + vec2f(u.time * 0.05, 0.0));
    if (i == i32(u.info.w + 0.5) && u.info.w >= 0.0) { c += vec3f(0.06, 0.055, 0.04); }
    if (i == i32(u.info2.y + 0.5) && u.info2.y >= 0.0) {
      let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
      c = mix(c + vec3f(0.08, 0.07, 0.03), vec3f(1.0, 0.82, 0.4), 1.0 - smoothstep(0.03, 0.08, edge));
    }
  }
  // the hurt: the edges flush red when a beast reaches the top
  if (u.info2.x > 0.0) {
    let edge = smoothstep(0.55, 1.35, length(q));
    c = mix(c, vec3f(0.55, 0.04, 0.03), edge * u.info2.x * 0.6);
  }
  let count = i32(u.info2.z);
  for (var k = 0; k < 24; k++) {
    if (k >= count) { break; }
    c = plate(p, c, u.plate[k * 2], u.plate[k * 2 + 1]);
  }
  return vec4f(c, 1.0);
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
