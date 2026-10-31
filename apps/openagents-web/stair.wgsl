// Hold the Stair (stair.coax): bg_fs draws the tower's floor seen from above
// with a slight tilt: the back wall and its torches, flagstones lit by them,
// the stair winding up in steps, the cellar door, the magic circle at the
// tower top, the party standing on their tiles (painted figures, one per
// tile, looked up by cell) and the panels; fx_vs/fx_fs draw the strokes
// stair_frame writes (beasts, bolts, numbers, text) in four shapes: a
// capsule, a ring, a glossy bubble and a soft light.
struct U {
  size: vec2f, time: f32, fade: f32,
  map: vec4f,      // origin x, y, tile (device px), squash
  info: vec4f,     // screen cols, rows, portrait, hover cell
  info2: vec4f,    // hurt, selected cell, plates, 0
  cells: array<vec4f, 32>,   // per cell: kind (+ the stair's progress as the fraction; 10 + class for a defender)
  unit: array<vec4f, 32>,    // per cell with a defender: facing (sign) × (1 + its attack flash)
  plate: array<vec4f, 48>,
}
@group(0) @binding(0) var<uniform> u: U;

const COLS = 14;
const ROWS = 9;
const INK = vec3f(0.05, 0.04, 0.07);
const WALL = 0.55;   // the back wall's height above the map, in rows
const RIM = 0.16;    // the stone rim on the other sides

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn cell_raw(i: i32) -> f32 {
  if (i < 0 || i >= 126) { return -1.0; }
  return u.cells[i / 4][i % 4];
}
fn cell_kind(i: i32) -> i32 { return i32(floor(cell_raw(i) + 0.001)); }
fn on_stair(k: i32) -> bool { return k == 1 || k == 3 || k == 4; }
// A logical cell's index (-1 off the map).
fn idx(c: i32, r: i32) -> i32 {
  if (c < 0 || c >= COLS || r < 0 || r >= ROWS) { return -1; }
  return c + r * COLS;
}
// A screen cell's logical index.
fn screen_cell(sc: vec2f) -> i32 {
  if (sc.x < 0.0 || sc.y < 0.0 || sc.x >= u.info.x || sc.y >= u.info.y) { return -1; }
  if (u.info.z > 0.5) { return idx(COLS - 1 - i32(sc.y), i32(sc.x)); }
  return idx(i32(sc.x), i32(sc.y));
}
// Logical tile units to the screen's (local) tile units.
fn to_local(x: f32, y: f32) -> vec2f {
  if (u.info.z > 0.5) { return vec2f(y, f32(COLS) - x); }
  return vec2f(x, y);
}

// ---- shapes (y up)
fn sd_circle(p: vec2f, c: vec2f, r: f32) -> f32 { return length(p - c) - r; }
fn sd_box(p: vec2f, c: vec2f, b: vec2f, r: f32) -> f32 {
  let q = abs(p - c) - b + vec2f(r);
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
fn sd_seg(p: vec2f, a: vec2f, b: vec2f, r: f32) -> f32 {
  let pa = p - a; let ba = b - a;
  let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h) - r;
}
// A trapezoid centred at c: half-width r1 at the bottom, r2 at the top, half-height he.
fn sd_trap(p0: vec2f, c: vec2f, r1: f32, r2: f32, he: f32) -> f32 {
  var p = p0 - c;
  let k1 = vec2f(r2, he);
  let k2 = vec2f(r2 - r1, 2.0 * he);
  p.x = abs(p.x);
  let ca = vec2f(p.x - min(p.x, select(r2, r1, p.y < 0.0)), abs(p.y) - he);
  let cb = p - k1 + k2 * clamp(dot(k1 - p, k2) / dot(k2, k2), 0.0, 1.0);
  let s = select(1.0, -1.0, cb.x < 0.0 && ca.y < 0.0);
  return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}

// ---- the party: painted figures, outlined, lit from the upper left
struct Pen { c: vec3f, a: f32, glow: vec3f, }
fn paint(pen: ptr<function, Pen>, d: f32, col: vec3f, aa: f32, line: f32) {
  let fill = clamp(0.5 - d / aa, 0.0, 1.0);
  if (fill <= 0.0) { return; }
  let edge = clamp(0.5 + (line + d) / aa, 0.0, 1.0);
  // a little light on the upper-left inside edge
  (*pen).c = mix((*pen).c, mix(col, INK, edge * 0.9), fill);
  (*pen).a = max((*pen).a, fill);
}
fn lit_col(col: vec3f, q: vec2f, c: vec2f, r: f32) -> vec3f {
  let n = (q - c) / r;
  return col * (0.72 + 0.5 * clamp(dot(n, vec2f(-0.55, 0.65)) * 0.5 + 0.5, 0.0, 1.0));
}
fn glow_at(pen: ptr<function, Pen>, d: f32, col: vec3f, spread: f32) {
  (*pen).glow += col * exp(-max(d, 0.0) / spread) * 0.6;
}
const SKIN = vec3f(0.93, 0.76, 0.63);
const STEEL = vec3f(0.66, 0.71, 0.8);

fn figure(q0: vec2f, cls: i32, face: f32, flash: f32, aa: f32, seed: f32) -> Pen {
  var pen = Pen(vec3f(0.0), 0.0, vec3f(0.0));
  // breathing
  var q = vec2f(q0.x * face, q0.y - 0.008 * sin(u.time * 2.2 + seed * 6.0));
  let line = aa * 1.4;
  // the shadow it stands in
  let sh = length((q0 - vec2f(0.0, 0.0)) * vec2f(1.0, 2.6)) - 0.2;
  pen.a = max(pen.a, 0.5 * clamp(0.5 - sh / 0.06, 0.0, 1.0));
  if (cls == 0) {
    // FIGHTER: steel and a blue tabard, a round shield, a longsword that swings when it attacks
    paint(&pen, sd_box(q, vec2f(-0.055, 0.07), vec2f(0.035, 0.075), 0.02), vec3f(0.3, 0.24, 0.26), aa, line);
    paint(&pen, sd_box(q, vec2f(0.055, 0.07), vec2f(0.035, 0.075), 0.02), vec3f(0.3, 0.24, 0.26), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.27), vec2f(0.12, 0.13), 0.05), lit_col(STEEL, q, vec2f(0.0, 0.27), 0.14), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.23), vec2f(0.055, 0.12), 0.02), lit_col(vec3f(0.2, 0.36, 0.85), q, vec2f(0.0, 0.24), 0.12), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.17), vec2f(0.12, 0.018), 0.008), vec3f(0.38, 0.26, 0.2), aa, line);
    // the sword arm: idle, the blade up; on its turn it comes down across
    let ang = mix(1.25, -0.35, flash);
    let dir = vec2f(cos(ang), sin(ang));
    let hand = vec2f(0.15, 0.27);
    let tip = hand + dir * 0.36;
    let blade = sd_seg(q, hand + dir * 0.05, tip, 0.022);
    paint(&pen, blade, mix(vec3f(0.85, 0.9, 1.0), vec3f(1.0, 1.0, 1.0), flash), aa, line);
    if (flash > 0.05) { glow_at(&pen, blade, vec3f(0.6, 0.75, 1.0) * flash, 0.05); }
    let nrm = vec2f(-dir.y, dir.x);
    paint(&pen, sd_seg(q, hand + dir * 0.05 - nrm * 0.055, hand + dir * 0.05 + nrm * 0.055, 0.014), vec3f(0.95, 0.8, 0.35), aa, line);
    paint(&pen, sd_circle(q, hand, 0.032), SKIN, aa, line);
    // the shield on the off arm
    paint(&pen, sd_circle(q, vec2f(-0.1, 0.25), 0.105), lit_col(vec3f(0.78, 0.8, 0.86), q, vec2f(-0.1, 0.25), 0.105), aa, line);
    paint(&pen, sd_circle(q, vec2f(-0.1, 0.25), 0.075), lit_col(vec3f(0.18, 0.32, 0.8), q, vec2f(-0.1, 0.25), 0.075), aa, line * 0.6);
    paint(&pen, sd_circle(q, vec2f(-0.1, 0.25), 0.022), vec3f(0.95, 0.82, 0.4), aa, line * 0.6);
    // head, helm, plume
    paint(&pen, sd_circle(q, vec2f(0.01, 0.5), 0.092), lit_col(SKIN, q, vec2f(0.01, 0.5), 0.092), aa, line);
    paint(&pen, max(sd_circle(q, vec2f(0.0, 0.515), 0.104), 0.515 - q.y), lit_col(STEEL, q, vec2f(0.0, 0.55), 0.1), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.513), vec2f(0.108, 0.014), 0.006), vec3f(0.55, 0.58, 0.66), aa, line * 0.7);
    paint(&pen, sd_circle(q, vec2f(0.055, 0.48), 0.014), INK, aa, 0.0);
    paint(&pen, sd_seg(q, vec2f(0.0, 0.62), vec2f(-0.1, 0.68), 0.028), vec3f(0.86, 0.16, 0.22), aa, line);
  } else if (cls == 1) {
    // WIZARD: a violet robe and a pointed hat, a white beard, a staff with a glowing orb
    let robe = vec3f(0.44, 0.27, 0.8);
    paint(&pen, sd_seg(q, vec2f(0.17, 0.0), vec2f(0.17, 0.7), 0.018), vec3f(0.46, 0.32, 0.2), aa, line);
    paint(&pen, sd_trap(q, vec2f(0.0, 0.2), 0.165, 0.075, 0.2), lit_col(robe, q, vec2f(0.0, 0.22), 0.2), aa, line);
    paint(&pen, max(sd_trap(q, vec2f(0.0, 0.2), 0.165, 0.075, 0.2), q.y - 0.045), vec3f(0.95, 0.82, 0.38), aa, line * 0.5);
    paint(&pen, sd_seg(q, vec2f(0.03, 0.31), vec2f(0.15, 0.27), 0.042), lit_col(robe, q, vec2f(0.09, 0.3), 0.08), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.16, 0.27), 0.032), SKIN, aa, line);
    paint(&pen, sd_circle(q, vec2f(0.0, 0.46), 0.085), lit_col(SKIN, q, vec2f(0.0, 0.46), 0.085), aa, line);
    paint(&pen, sd_trap(q, vec2f(0.015, 0.38), 0.012, 0.07, 0.06), vec3f(0.93, 0.93, 0.96), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.045, 0.47), 0.013), INK, aa, 0.0);
    // the hat leans back
    let hq = vec2f(q.x + 0.32 * max(q.y - 0.53, 0.0), q.y);
    let hat = vec3f(0.27, 0.18, 0.62);
    paint(&pen, sd_trap(hq, vec2f(-0.005, 0.68), 0.1, 0.006, 0.15), lit_col(hat, hq, vec2f(0.0, 0.62), 0.12), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.535), vec2f(0.155, 0.018), 0.018), hat * 0.85, aa, line);
    paint(&pen, sd_circle(hq, vec2f(0.0, 0.63), 0.02), vec3f(1.0, 0.92, 0.45), aa, 0.0);
    // the orb
    let orb = sd_circle(q, vec2f(0.17, 0.75), 0.048);
    let oc = vec3f(0.62, 0.78, 1.0);
    paint(&pen, orb, mix(oc, vec3f(1.0), 0.35 + 0.4 * flash), aa, line * 0.6);
    glow_at(&pen, orb, oc * (0.55 + 1.2 * flash + 0.15 * sin(u.time * 3.0 + seed)), 0.05 + 0.04 * flash);
  } else {
    // CLERIC: a pale robe with a gold sun, a hood, a mace, a halo that flares on its turn
    let robe = vec3f(0.9, 0.88, 0.8);
    paint(&pen, sd_trap(q, vec2f(0.0, 0.2), 0.17, 0.09, 0.2), lit_col(robe, q, vec2f(0.0, 0.22), 0.2), aa, line);
    paint(&pen, sd_box(q, vec2f(0.0, 0.2), vec2f(0.032, 0.19), 0.01), vec3f(0.93, 0.78, 0.3), aa, line * 0.5);
    let sun = sd_circle(q, vec2f(0.0, 0.29), 0.042);
    paint(&pen, sun, vec3f(1.0, 0.86, 0.36), aa, line * 0.6);
    paint(&pen, sd_seg(q, vec2f(0.14, 0.18), vec2f(0.19, 0.46), 0.015), vec3f(0.45, 0.3, 0.2), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.195, 0.49), 0.042), lit_col(STEEL, q, vec2f(0.195, 0.49), 0.042), aa, line);
    paint(&pen, sd_seg(q, vec2f(0.03, 0.31), vec2f(0.14, 0.26), 0.042), lit_col(robe, q, vec2f(0.09, 0.3), 0.08), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.145, 0.25), 0.032), SKIN, aa, line);
    paint(&pen, sd_circle(q, vec2f(-0.015, 0.47), 0.108), lit_col(vec3f(0.8, 0.77, 0.7), q, vec2f(-0.015, 0.48), 0.11), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.022, 0.455), 0.074), lit_col(SKIN, q, vec2f(0.022, 0.455), 0.074), aa, line);
    paint(&pen, sd_circle(q, vec2f(0.058, 0.465), 0.013), INK, aa, 0.0);
    let halo = abs(length((q - vec2f(-0.01, 0.62)) * vec2f(1.0, 3.2)) - 0.1) - 0.012;
    paint(&pen, halo, vec3f(1.0, 0.9, 0.5), aa, 0.0);
    glow_at(&pen, halo, vec3f(1.0, 0.86, 0.45) * (0.35 + 1.4 * flash), 0.03 + 0.05 * flash);
    if (flash > 0.05) { glow_at(&pen, sun, vec3f(1.0, 0.9, 0.5) * flash, 0.06); }
  }
  return pen;
}

// The figure standing on a screen cell, at a pixel (local tile units), over what is there.
fn stand(c: vec3f, local: vec2f, sc: vec2f, t: f32) -> vec3f {
  let i = screen_cell(sc);
  let k = cell_kind(i);
  if (k < 10) { return c; }
  let v = u.unit[i / 4][i % 4];
  let ghost = abs(v) > 2.5;
  let face = select(-1.0, 1.0, v >= 0.0);
  let flash = select(clamp(abs(v) - 1.0, 0.0, 1.0), 0.0, ghost);
  let feet = vec2f(sc.x + 0.5, sc.y + 0.74);
  let q = vec2f(local.x - feet.x, (feet.y - local.y) * u.map.w) * 0.74;
  let pen = figure(q, k - 10, face, flash, 1.3 / t, f32(i) * 0.37);
  // the ghost of one about to go down: see-through, breathing
  let a = select(pen.a, pen.a * (0.45 + 0.15 * sin(u.time * 5.0)), ghost);
  return mix(c, pen.c, a) + pen.glow * select(1.0, 0.5, ghost);
}

// ---- the map
fn torch_x(i: i32, n: i32) -> f32 { return (f32(i) + 0.5) * u.info.x / f32(n); }
fn torch_count() -> i32 { return select(4, 3, u.info.x < 11.0); }
fn flicker(i: f32) -> f32 { return 0.82 + 0.18 * noise(vec2f(u.time * 7.0 + i * 13.0, i)); }

// The light on the floor at a local point: torches on the back wall, the cellar's red, the tower top's gold.
fn light_at(l: vec2f) -> vec3f {
  var c = vec3f(0.09, 0.1, 0.16);
  let n = torch_count();
  for (var i = 0; i < 4; i++) {
    if (i >= n) { break; }
    let d = l - vec2f(torch_x(i, n), -0.35);
    c += vec3f(1.0, 0.84, 0.55) * flicker(f32(i)) * 1.35 / (1.0 + dot(d, d) * 0.38);
  }
  // sconces on the side walls
  for (var j = 0; j < 2; j++) {
    let y = u.info.y * (0.35 + 0.4 * f32(j));
    let a = l - vec2f(-0.2, y);
    let b = l - vec2f(u.info.x + 0.2, y);
    c += vec3f(1.0, 0.84, 0.55) * flicker(f32(j) + 7.0) * 0.75 * (1.0 / (1.0 + dot(a, a) * 0.6) + 1.0 / (1.0 + dot(b, b) * 0.6));
  }
  let door = l - to_local(0.5, 7.5);
  c += vec3f(1.0, 0.12, 0.14) * (0.8 + 0.25 * sin(u.time * 2.7)) / (1.0 + dot(door, door) * 0.7);
  let top = l - to_local(13.5, 1.5);
  c += vec3f(1.0, 0.86, 0.45) * 0.9 / (1.0 + dot(top, top) * 0.6);
  return c;
}

fn floor_stone(f: vec2f, id: vec2f) -> vec3f {
  let h = hash(id);
  // some tiles are two stones
  var g = f;
  var sub = id;
  if (h > 0.55) { if (f.x > 0.5) { g.x = (f.x - 0.5) * 2.0; sub = id + vec2f(0.5, 0.0); } else { g.x = f.x * 2.0; } }
  let n = noise(id * 3.1 + f * 5.0) * 0.5 + noise(id * 7.0 + f * 17.0) * 0.25 + hash(sub) * 0.4;
  let edge = min(min(g.x, 1.0 - g.x) * select(1.0, 0.5, h > 0.55), min(g.y, 1.0 - g.y));
  let mortar = smoothstep(0.0, 0.045, edge);
  let bevel = 1.0 + 0.25 * (1.0 - smoothstep(0.0, 0.1, g.y)) - 0.3 * smoothstep(0.86, 1.0, g.y) - 0.15 * smoothstep(0.88, 1.0, g.x);
  var base = vec3f(0.2, 0.21, 0.25) * (0.75 + 0.45 * n);
  // a crack now and then, moss in the corners of the old ones
  let crack = abs(noise(id * 11.0 + f * vec2f(3.0, 9.0)) - 0.5);
  if (hash(id + 3.3) > 0.7) { base *= 0.7 + 0.3 * smoothstep(0.0, 0.035, crack); }
  if (hash(id + 9.1) > 0.78) { base = mix(base, vec3f(0.14, 0.24, 0.13), (1.0 - smoothstep(0.0, 0.3, edge)) * 0.6 * noise(f * 9.0 + id)); }
  return mix(vec3f(0.03, 0.03, 0.04), base * bevel, mortar);
}

fn stair_stone(lf: vec2f, i: i32, kind: i32) -> vec3f {
  let col = i % COLS; let row = i / COLS;
  let d0 = fract(cell_raw(i)) * 100.0;
  let l = idx(col - 1, row); let r = idx(col + 1, row); let up = idx(col, row - 1); let dn = idx(col, row + 1);
  let kl = on_stair(cell_kind(l)); let kr = on_stair(cell_kind(r)); let ku = on_stair(cell_kind(up)); let kd = on_stair(cell_kind(dn));
  // how far along the climb this pixel is: across the run, from the lower end up
  var along = 0.0;
  if (kl || kr) {
    var dir = 1.0;
    if (kr && fract(cell_raw(r)) * 100.0 < d0) { dir = -1.0; }
    if (!kr && kl && fract(cell_raw(l)) * 100.0 > d0) { dir = -1.0; }
    along = (lf.x - 0.5) * dir;
    if ((ku || kd) && !(kl && kr)) { along = 0.0; }
  }
  if (((ku || kd) && !(kl || kr)) || ((ku || kd) && along == 0.0)) {
    var dir = 1.0;
    if (kd && fract(cell_raw(dn)) * 100.0 < d0) { dir = -1.0; }
    if (!kd && ku && fract(cell_raw(up)) * 100.0 > d0) { dir = -1.0; }
    along = (lf.y - 0.5) * dir;
  }
  let d = d0 + along;
  let s = fract(d * 2.0);
  let n = noise(vec2f(f32(i)) * 1.7 + lf * 6.0) * 0.5 + hash(vec2f(floor(d * 2.0), f32(i))) * 0.3;
  // higher steps catch more of the light above
  var base = vec3f(0.44, 0.4, 0.37) * (0.7 + 0.4 * n) * (0.85 + 0.25 * clamp(d / 28.0, 0.0, 1.0));
  base *= 1.0 + 0.18 * smoothstep(0.7, 0.97, s) - 0.5 * (1.0 - smoothstep(0.0, 0.1, s));
  // the stair's edges: a curb where it meets the floor
  var e = 1.0;
  if (!kl) { e = min(e, lf.x); }
  if (!kr) { e = min(e, 1.0 - lf.x); }
  if (!ku) { e = min(e, lf.y); }
  if (!kd) { e = min(e, 1.0 - lf.y); }
  base = mix(vec3f(0.5, 0.47, 0.44) * (0.8 + 0.3 * n), base, smoothstep(0.06, 0.1, e));
  base *= 0.55 + 0.45 * smoothstep(0.0, 0.05, e);
  if (kind == 4) {
    // the tower top: a magic circle turning on a gold landing
    let p = lf - vec2f(0.5);
    let rr = length(p);
    let a = atan2(p.y, p.x) + u.time * 0.4;
    base = mix(base, vec3f(0.35, 0.3, 0.22), smoothstep(0.47, 0.44, rr));
    let ringc = vec3f(1.0, 0.86, 0.45);
    let r1 = 1.0 - smoothstep(0.012, 0.025, abs(rr - 0.38));
    let r2 = 1.0 - smoothstep(0.01, 0.02, abs(rr - 0.27));
    let runes = (1.0 - smoothstep(0.02, 0.05, abs(rr - 0.325))) * step(0.55, fract(a * 12.0 / 6.2831853 * 2.0)) * step(0.3, hash(vec2f(floor(a * 24.0 / 6.2831853), 1.0)));
    let star = 1.0 - smoothstep(0.01, 0.02, abs(rr - 0.27 + 0.1 * abs(sin(a * 2.5))) ) ;
    base += ringc * (r1 + r2 * 0.8 + runes * 0.7 + star * 0.35 * step(rr, 0.27)) * (0.75 + 0.25 * sin(u.time * 2.0));
    base += ringc * 0.25 * (1.0 - smoothstep(0.0, 0.3, rr));
  }
  return base;
}

// The cellar door, in screen space: a stone arch, the conjurer's red light swirling inside.
fn door(c: vec3f, f: vec2f) -> vec3f {
  let p = vec2f(f.x - 0.5, 0.92 - f.y);   // y up from the sill
  let open = min(sd_box(p, vec2f(0.0, 0.25), vec2f(0.3, 0.25), 0.0), sd_circle(p, vec2f(0.0, 0.5), 0.3));
  let arch = min(sd_box(p, vec2f(0.0, 0.25), vec2f(0.4, 0.25), 0.0), sd_circle(p, vec2f(0.0, 0.5), 0.4));
  var o = c;
  // the arch's stones
  let ang = atan2(p.y - 0.5, p.x);
  let joints = step(0.88, fract(select(p.y * 5.0, ang * 7.0 / 3.14159, p.y > 0.5)));
  o = mix(o, vec3f(0.36, 0.34, 0.36) * (0.8 + 0.3 * noise(f * 10.0)) * (1.0 - 0.5 * joints), clamp(0.5 - arch * 60.0, 0.0, 1.0));
  // inside: swirling red and magenta, brightest low down
  let sw = noise(vec2f(atan2(p.y - 0.3, p.x) * 2.0 + u.time * 1.5, length(p - vec2f(0.0, 0.3)) * 6.0 - u.time * 2.0));
  let inner = mix(vec3f(0.35, 0.02, 0.06), vec3f(1.0, 0.16, 0.2), sw) * (0.7 + 0.3 * sin(u.time * 2.7));
  let deep = mix(inner, vec3f(0.95, 0.4, 0.75), 0.25 * smoothstep(0.6, 0.9, sw));
  o = mix(o, deep, clamp(0.5 - open * 60.0, 0.0, 1.0));
  o = mix(o, INK, clamp(1.0 - abs(open) * 45.0, 0.0, 1.0) * 0.7);
  return o;
}

// The back wall: courses of stone, a torch in an iron bracket every few tiles.
fn wall(local: vec2f) -> vec3f {
  let y = -local.y / WALL;     // 0 at the floor, 1 at the wall's top
  let course = floor(y * 4.0);
  let bx = local.x * 1.6 + course * 0.5;
  let bf = vec2f(fract(bx), fract(y * 4.0));
  let n = noise(vec2f(floor(bx), course) * 3.7 + bf * 4.0) * 0.5 + hash(vec2f(floor(bx), course)) * 0.35;
  var c = vec3f(0.17, 0.17, 0.21) * (0.7 + 0.5 * n);
  let joint = min(min(bf.x, 1.0 - bf.x) * 0.6, min(bf.y, 1.0 - bf.y));
  c *= 0.35 + 0.65 * smoothstep(0.0, 0.08, joint);
  c *= 0.45 + 0.55 * smoothstep(0.0, 0.35, y);   // the foot of the wall in shadow
  c *= light_at(vec2f(local.x, -0.05)) * 0.9;
  let n_t = torch_count();
  for (var i = 0; i < 4; i++) {
    if (i >= n_t) { break; }
    let tx = torch_x(i, n_t);
    let p = vec2f(local.x - tx, -local.y - WALL * 0.3) / WALL * 0.55;   // y up, wall units, the torch large
    // bracket
    c = mix(c, vec3f(0.1, 0.1, 0.12), clamp(0.5 - sd_box(p, vec2f(0.0, -0.12), vec2f(0.06, 0.16), 0.03) * 80.0, 0.0, 1.0));
    c = mix(c, vec3f(0.28, 0.2, 0.15), clamp(0.5 - sd_box(p, vec2f(0.0, 0.08), vec2f(0.08, 0.05), 0.02) * 80.0, 0.0, 1.0));
    // flame: a yellow-white core, red at the tips, licking
    let fl = flicker(f32(i));
    let wob = (noise(vec2f(p.y * 6.0 - u.time * 9.0, f32(i))) - 0.5) * 0.08 * (p.y - 0.1);
    let fp = vec2f(p.x - wob, p.y - 0.13);
    let flame = sd_trap(fp, vec2f(0.0, 0.17), 0.08, 0.005, 0.17 * fl);
    let core = sd_trap(fp, vec2f(0.0, 0.11), 0.045, 0.005, 0.1 * fl);
    let fc = mix(vec3f(1.0, 0.2, 0.08), vec3f(1.0, 0.9, 0.4), clamp(1.0 - fp.y * 2.2, 0.0, 1.0));
    c = mix(c, fc, clamp(0.5 - flame * 70.0, 0.0, 1.0));
    c = mix(c, vec3f(1.0, 0.98, 0.85), clamp(0.5 - core * 70.0, 0.0, 1.0));
    c += vec3f(1.0, 0.8, 0.45) * 0.25 * fl * exp(-length(fp - vec2f(0.0, 0.15)) * 5.0);
  }
  // the wall's top edge
  c = mix(c, vec3f(0.32, 0.31, 0.36), smoothstep(0.9, 0.95, y));
  return c;
}

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 7.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let v = (p.y - r.y) / max(r.w - r.y, 1.0);
  var fill = vec3f(0.018, 0.02, 0.03);
  var line = vec3f(0.0);
  if (s.y > 0.5) { fill = mix(vec3f(0.11, 0.11, 0.15), vec3f(0.05, 0.05, 0.07), v); line = vec3f(0.42, 0.44, 0.52); }
  if (s.y > 1.5) { fill = mix(vec3f(0.2, 0.18, 0.12), vec3f(0.09, 0.08, 0.07), v); line = vec3f(1.0, 0.86, 0.5); }
  if (s.y > 2.5) { fill = vec3f(0.03, 0.03, 0.04); line = vec3f(0.18, 0.18, 0.22); }
  let w = 1.0 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0);
  var c = mix(mix(under, fill, a * inside), line, edge * a * 0.9);
  // a class's portrait on its button
  if (s.w > 0.5 && inside > 0.0) {
    let h = r.w - r.y;
    let feet = vec2f(r.x + h * 0.5, r.w - h * 0.1);
    let sc = h * 0.95;
    let qq = vec2f(p.x - feet.x, feet.y - p.y) / sc;
    if (abs(qq.x) < 0.3 && qq.y > -0.1 && qq.y < 0.9) {
      let pen = figure(qq, i32(s.w + 0.5) - 1, 1.0, 0.0, 1.3 / sc, s.w);
      let dim = select(1.0, 0.35, s.y > 2.5);
      c = mix(c, pen.c * dim, pen.a * inside) + pen.glow * dim * inside;
    }
  }
  return c;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}

@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let q = (p / u.size - 0.5) * 2.0;
  var c = vec3f(0.012, 0.012, 0.02) * (1.0 - 0.3 * length(q));
  let t = u.map.z;
  let local = (p - u.map.xy) / vec2f(t, t * u.map.w);
  let inside = local.x >= 0.0 && local.y >= 0.0 && local.x < u.info.x && local.y < u.info.y;
  if (t > 0.0 && inside) {
    let sc = floor(local);
    let f = fract(local);
    var col = i32(sc.x); var row = i32(sc.y);
    var lf = f;
    if (u.info.z > 0.5) { col = COLS - 1 - i32(sc.y); row = i32(sc.x); lf = vec2f(1.0 - f.y, f.x); }
    let i = col + row * COLS;
    let kind = cell_kind(i);
    if (on_stair(kind)) {
      c = stair_stone(lf, i, kind);
    } else {
      c = floor_stone(f, sc);
      // the stair stands a step above the floor: its shadow falls on the tiles beside it
      var ao = 1.0;
      if (on_stair(cell_kind(idx(col - 1, row)))) { ao = min(ao, smoothstep(0.0, 0.2, lf.x)); }
      if (on_stair(cell_kind(idx(col + 1, row)))) { ao = min(ao, smoothstep(0.0, 0.2, 1.0 - lf.x)); }
      if (on_stair(cell_kind(idx(col, row - 1)))) { ao = min(ao, smoothstep(0.0, 0.2, lf.y)); }
      if (on_stair(cell_kind(idx(col, row + 1)))) { ao = min(ao, smoothstep(0.0, 0.2, 1.0 - lf.y)); }
      c *= 0.45 + 0.55 * ao;
    }
    c *= light_at(local);
    if (kind == 3) { c = door(c, f); }
    if (i == i32(u.info.w + 0.5) && u.info.w >= 0.0) { c += vec3f(0.06, 0.055, 0.04); }
    if (i == i32(u.info2.y + 0.5) && u.info2.y >= 0.0) {
      let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
      c = mix(c + vec3f(0.08, 0.07, 0.03), vec3f(1.0, 0.86, 0.5), 1.0 - smoothstep(0.03, 0.08, edge));
    }
    // the party: the figure on this tile, then the one in front of it (its head reaches up here)
    c = stand(c, local, sc, t);
    c = stand(c, local, sc + vec2f(0.0, 1.0), t);
  } else if (t > 0.0 && local.x >= -RIM && local.x < u.info.x + RIM && local.y >= -WALL && local.y < u.info.y + RIM) {
    if (local.y < 0.0 && local.x >= 0.0 && local.x < u.info.x) {
      c = wall(local);
      // a figure in the first row can lift its head over the wall's foot
      c = stand(c, local, vec2f(floor(local.x), 0.0), t);
    } else {
      // the rim: a worn stone ledge
      let n = noise(local * 9.0);
      c = vec3f(0.24, 0.23, 0.27) * (0.75 + 0.35 * n) * light_at(clamp(local, vec2f(0.0), u.info.xy)) * 0.8;
      if (local.y < 0.0) { c = wall(vec2f(clamp(local.x, 0.0, u.info.x - 0.01), local.y)) * 0.6; }
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
// Modes (tint.w): 0 a capsule, 1 a ring (bar.x its width, bar.y the squash), 2 a glossy bubble
// (bar.x the inner layer's radius as a share, bar.yzw its colour), 3 a soft light.
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let mode = i32(v.tint.w + 0.5);
  if (mode == 1) {
    let sq = select(1.0, v.bar.y, v.bar.y > 0.0);
    let d = length(vec2f(v.place.x - v.shape.x * 0.5, v.place.y / sq));
    let e = abs(d - v.shape.y);
    let core = 1.0 - smoothstep(v.bar.x - 0.5, v.bar.x + 0.6, e);
    let g = e / max(v.shape.w, 1.0);
    return vec4f(v.tint.rgb * (core + exp(-5.0 * g * g) * 0.28) * v.shape.z * u.fade, 1.0);
  }
  if (mode == 2) {
    let r = max(v.shape.y, 1.0);
    let o = vec2f(v.place.x - v.shape.x * 0.5, v.place.y);
    let d = length(o);
    let x = clamp(d / r, 0.0, 1.0);
    let body = 1.0 - smoothstep(r - 0.8, r + 0.8, d);
    var col = v.tint.rgb * (0.3 + 0.85 * pow(x, 4.0)) * body;
    if (v.bar.x > 0.0) {
      let ir = v.bar.x * r;
      let inner = (1.0 - smoothstep(ir - 1.0, ir + 1.0, length(o - vec2f(0.0, r * 0.12)))) * (0.35 + 0.25 * pow(clamp(length(o - vec2f(0.0, r * 0.12)) / ir, 0.0, 1.0), 3.0));
      col = max(col, v.bar.yzw * inner);
    }
    let hl = o - vec2f(-0.36, -0.42) * r;
    let spec = exp(-dot(hl, hl) / (r * r * 0.025));
    let spec2 = exp(-dot(o - vec2f(0.4, 0.35) * r, o - vec2f(0.4, 0.35) * r) / (r * r * 0.01)) * 0.4;
    col += vec3f(1.0) * (spec * 0.9 + spec2) * body;
    let g = max(d - r, 0.0) / max(v.shape.w, 1.0);
    col += v.tint.rgb * exp(-5.0 * g * g) * 0.22 * (1.0 - body);
    return vec4f(col * v.shape.z * u.fade, 1.0);
  }
  let d = length(vec2f(along, v.place.y));
  if (mode == 3) {
    let g = d / max(v.shape.y + v.shape.w, 1.0);
    return vec4f(v.tint.rgb * exp(-3.5 * g * g) * v.shape.z * u.fade, 1.0);
  }
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
