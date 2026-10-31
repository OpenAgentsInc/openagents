// A first realm's three passes (droprealm.coax): the night and the floor
// behind (stars, then the floor where each pixel's ray meets it, lit by the
// lamp you carry, with the plates laid out over both), the pack's meshes
// (scene_vs, scene_fs: lit by a key light and your lamp, the plates kept over
// them) and the strokes over all of it (you, your flame, the words). Every
// place and size comes from droprealm.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,  // the view: clip = rows · world
  lamp: vec4f,                                 // the lamp: where (metres), strength
  eye: vec4f,                                  // the eye (metres), then the floor's radius
  fwd: vec4f, rx: vec4f, uy: vec4f,            // a pixel's ray: fwd + ndc.x · rx + ndc.y · uy
  panel: array<vec4f, 32>,  // per plate (droprealm.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
}
@group(0) @binding(0) var<uniform> u: U;
// A plate (verse.wgsl's): a dark wood fill with one thin bronze line round
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
// The lamp's light on what it reaches, a share of its strength.
fn lamp_near(p: vec3f, n: vec3f) -> f32 {
  let to = u.lamp.xyz - p;
  let d = length(to);
  return max(0.0, dot(n, to / max(d, 0.001))) * u.lamp.w / (1.0 + 0.18 * d * d);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let ndc = vec2f(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0);
  let ray = u.fwd.xyz + ndc.x * u.rx.xyz + ndc.y * u.uy.xyz;
  var sky = vec3f(0.012, 0.013, 0.02) + vec3f(0.03, 0.032, 0.045) * clamp(1.0 - ray.y * 3.0, 0.0, 1.0);
  sky += vec3f(stars(p)) * u.fade * smoothstep(0.0, 0.25, ray.y);
  // the floor where the ray meets it (derivatives first: they need every pixel)
  let down = ray.y < -0.0001;
  let t = select(1000.0, -u.eye.y / min(ray.y, -0.0001), down);
  let hit = u.eye.xyz + ray * min(t, 1000.0);
  let r = length(hit.xz);
  let w2 = max(fwidth(hit.xz), vec2f(0.0001));
  let wr = max(fwidth(r / 2.0) * 2.0, 0.0001);
  let g = abs(fract(hit.xz + 0.5) - 0.5) / w2;
  let grid = 1.0 - min(min(g.x, g.y), 1.0);
  let rings = 1.0 - min(abs(fract(r / 2.0 + 0.5) - 0.5) * 2.0 / wr, 1.0);
  let edge = select(0.0, 1.0 - smoothstep(u.eye.w - 0.6, u.eye.w, r), down);
  // a stone floor: a faint metre grid and rings two metres apart, lit by a dim key and your lamp
  let fade = exp(-t * 0.02);
  var ground = vec3f(0.045, 0.043, 0.05) + vec3f(0.035, 0.03, 0.025) * grid * fade + vec3f(0.05, 0.062, 0.078) * rings * 0.6 * fade;
  ground += vec3f(0.55, 0.78, 1.0) * lamp_near(hit, vec3f(0.0, 1.0, 0.0)) * 0.55;
  let off = (r - u.eye.w) * 2.5;
  ground += vec3f(0.45, 0.6, 0.78) * exp(-off * off) * 0.25;
  let c = mix(sky, ground, edge * u.fade);
  return vec4f(panels(p, c), 1.0);
}
// ---- the pack's meshes: its vertices (pack_realm.py and drops.c: the Haunted House's 48
// bytes, position, normal, uv, colour with its layer in the top byte), a
// transform and tint a draw, its texture layers
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
struct Object { r0: vec4f, r1: vec4f, r2: vec4f, material: vec4f }
@group(1) @binding(1) var<storage, read> objects: array<Object>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
fn word(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }
struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) tint: vec3f,
  @location(4) @interpolate(flat) slice: u32,
}
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  let b = id * 12u;
  let p = vec4f(word(b), word(b + 1u), word(b + 2u), 1.0);
  let n = vec3f(word(b + 3u), word(b + 4u), word(b + 5u));
  let m = objects[instance];
  let w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let wp = vec4f(w, 1.0);
  var o: S;
  o.pos = vec4f(dot(u.r0, wp), dot(u.r1, wp), dot(u.r2, wp), dot(u.r3, wp));
  o.world = w;
  o.normal = vec3f(dot(m.r0.xyz, n), dot(m.r1.xyz, n), dot(m.r2.xyz, n));
  o.uv = vec2f(word(b + 6u), word(b + 7u));
  o.tint = unpack4x8unorm(vertices[b + 8u]).rgb * m.material.rgb;
  o.slice = vertices[b + 8u] >> 24u;
  return o;
}
@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  if (tex.a < 0.35) { discard; }
  let n = normalize(s.normal);
  let key = max(0.0, dot(n, normalize(vec3f(-0.4, 0.8, 0.45))));
  var lit = vec3f(0.16, 0.17, 0.22) * (0.6 + 0.4 * n.y) + vec3f(0.55, 0.56, 0.65) * key;
  lit += vec3f(0.55, 0.78, 1.0) * lamp_near(s.world, n) * 1.6;
  let c = tex.rgb * s.tint * lit * u.fade;
  return vec4f(panels(s.pos.xy, c), 1.0);
}
// ---- the strokes: one quad per stroke or light (droprealm.coax lit, tinted_dot)
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
