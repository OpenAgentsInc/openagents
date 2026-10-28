// The Verse landing and galaxy's two passes: the night behind (stars, a
// faint glow where the hero or the galaxy sits, and the plates verse.coax
// lays out) and the strokes over it (words, orbs, orbits and worlds). Every
// place and size comes from verse.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  panel: array<vec4f, 32>,  // per plate (verse.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
}
@group(0) @binding(0) var<uniform> u: U;
// A plate (realm.wgsl's): a dark wood fill with one thin bronze line round
// it and small rounded corners. Look 1 lights the line; 2 (under the mouse
// or pressed) adds a step lighter fill and the bright line.
fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 4.0 * s.x;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.z >= 2.0;
  let lit = over || (s.z >= 1.0 && s.z < 2.0);
  let fill = select(vec3f(0.165, 0.114, 0.078), vec3f(0.235, 0.168, 0.115), over);
  let line = select(vec3f(0.69, 0.541, 0.29), vec3f(0.86, 0.70, 0.42), lit);
  let w = 1.25 * s.x;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.y, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1));
}
fn panels(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
// Stars: one at most per cell of 3 CSS pixels' grid, a few of them brighter, twinkling slowly.
fn stars(p: vec2f) -> f32 {
  let cell = 3.0 * max(u.k, 1.0) * 6.0;
  let g = floor(p / cell);
  let h = hash(g);
  if (h < 0.93) { return 0.0; }
  let at = (g + vec2f(hash(g + 1.7), hash(g + 9.1)) * 0.8 + 0.1) * cell;
  let d = length(p - at) / max(u.k, 1.0);
  let size = select(0.55, 0.95, h > 0.992);
  let twinkle = select(0.75 + 0.25 * sin(u.time * (0.6 + h * 2.0) + h * 40.0), 1.0, u.still > 0.5);
  let peak = select(0.32, 0.75, h > 0.992) * twinkle;
  return peak * (1.0 - smoothstep(size * 0.4, size + 0.6, d));
}
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let c = vec2f(0.5, 0.5) * u.size;
  let r = length((p - c) / max(u.size.y, 1.0));
  var sky = vec3f(0.012, 0.013, 0.02) + vec3f(0.035, 0.036, 0.045) * exp(-r * r * 6.0);
  sky += vec3f(stars(p)) * u.fade;
  return vec4f(panels(p, sky), 1.0);
}
// ---- the strokes: one quad per stroke or light (verse.coax lit, tinted_dot)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode
  @location(3) bar: vec4f,
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f) -> V {
  var o: V;
  let ndc = vec2f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0);
  o.pos = vec4f(ndc, 0.0, 1.0);
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
