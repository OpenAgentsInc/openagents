// A realm someone built: its three passes (myrealm.coax). The sky and the ground behind (the sky by
// the realm's time of day, the ground by its kind, where each pixel's ray meets it, the water where
// it has some, lit by the sun and, after dark, the lamp you carry; the plates over both), the pack's
// meshes (scene_vs, scene_fs: lit by the same sun and lamp, fading into the haze) and the strokes
// over all of it. A painterly realm's light is softer and its ground brushed. Forked from
// droprealm.wgsl; every place, size and colour comes from myrealm.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,  // the view: clip = rows · world
  lamp: vec4f,                                 // the lamp: where (metres), strength
  eye: vec4f,                                  // the eye (metres), then the floor's radius
  fwd: vec4f, rx: vec4f, uy: vec4f,            // a pixel's ray: fwd + ndc.x · rx + ndc.y · uy
  sky: vec4f,                                  // the sky's top, then 1 for painterly
  horizon: vec4f,                              // the horizon, then the stars' share
  ground: vec4f,                               // the ground's colour, then the lamp's share
  sun: vec4f,                                  // toward the sun, then its strength
  water: vec4f,                                // the water: x, z, radius, on
  panel: array<vec4f, 48>,  // per plate (myrealm.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
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
  for (var k = 0; k < 24; k++) {
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
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let w = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), w.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), w.x), w.y);
}
// The sky along a ray: the horizon below, the top above, the sun's glow, and stars after dark.
fn sky_of(ray: vec3f, p: vec2f) -> vec3f {
  let r = normalize(ray);
  var c = mix(u.horizon.rgb, u.sky.rgb, smoothstep(-0.02, 0.45, r.y));
  let toward = max(dot(r, normalize(u.sun.xyz)), 0.0);
  c += vec3f(1.0, 0.85, 0.6) * pow(toward, 40.0) * u.sun.w * 0.6 + u.horizon.rgb * pow(toward, 6.0) * 0.25;
  c += vec3f(stars(p)) * u.horizon.w * smoothstep(0.0, 0.25, r.y);
  return c;
}
// Sun and sky light on a surface.
fn daylight(n: vec3f) -> vec3f {
  let key = max(0.0, dot(n, normalize(u.sun.xyz)));
  let soft = select(key, key * 0.6 + 0.25, u.sky.w > 0.5);
  let amb = mix(u.horizon.rgb, u.sky.rgb, 0.5 + 0.5 * n.y) * 0.55 + vec3f(0.04);
  return amb + vec3f(1.0, 0.94, 0.82) * soft * u.sun.w;
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let ndc = vec2f(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0);
  let ray = u.fwd.xyz + ndc.x * u.rx.xyz + ndc.y * u.uy.xyz;
  let sky = sky_of(ray, p);
  let down = ray.y < -0.0001;
  let t = select(1000.0, -u.eye.y / min(ray.y, -0.0001), down);
  let hit = u.eye.xyz + ray * min(t, 1000.0);
  let r = length(hit.xz);
  // the ground: its colour, broken up (brushed when painterly), lit by the sun and your lamp
  let big = noise(hit.xz * 0.18); let fine = noise(hit.xz * 1.7);
  let brush = noise(vec2f(hit.x * 0.9 + hit.z * 0.4, hit.z * 3.2));
  let vary = select(0.82 + 0.22 * big + 0.08 * fine, 0.78 + 0.2 * big + 0.16 * brush, u.sky.w > 0.5);
  var ground = u.ground.rgb * vary * daylight(vec3f(0.0, 1.0, 0.0));
  // water: the sky in it, rippled, a pale rim where it meets the ground
  let wd = length(hit.xz - u.water.xy);
  if (u.water.w > 0.5 && wd < u.water.z) {
    let rip = noise(hit.xz * 2.4 + vec2f(u.time * 0.3, u.time * 0.2)) - 0.5;
    let mirror = sky_of(vec3f(ray.x + rip * 0.05, -ray.y, ray.z + rip * 0.05), p);
    let deep = smoothstep(0.0, u.water.z * 0.6, u.water.z - wd);
    let water = mix(mirror * 0.75 + vec3f(0.02, 0.05, 0.08), vec3f(0.03, 0.09, 0.14) * daylight(vec3f(0.0, 1.0, 0.0)) + mirror * 0.4, deep * 0.6);
    let rim = 1.0 - smoothstep(0.0, 0.25, u.water.z - wd);
    ground = mix(water, u.ground.rgb * 1.2 * daylight(vec3f(0.0, 1.0, 0.0)), rim * 0.5);
  }
  ground += vec3f(1.0, 0.62, 0.28) * lamp_near(hit, vec3f(0.0, 1.0, 0.0)) * 0.55 * u.ground.w;
  // the realm's edge fades into the horizon's haze
  let haze = clamp(1.0 - exp(-t * 0.018), 0.0, 1.0);
  ground = mix(ground, u.horizon.rgb, haze * 0.85);
  let edge = select(0.0, 1.0 - smoothstep(u.eye.w - 3.0, u.eye.w + 6.0, r), down);
  let c = mix(sky, mix(u.horizon.rgb, ground, edge), select(0.0, 1.0, down)) * u.fade;
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
  var lit = daylight(n);
  lit += vec3f(1.0, 0.62, 0.28) * lamp_near(s.world, n) * 1.6 * u.ground.w;
  let far = length(s.world - u.eye.xyz);
  let haze = clamp(1.0 - exp(-far * 0.018), 0.0, 1.0);
  let c = mix(tex.rgb * s.tint * lit, u.horizon.rgb, haze * 0.85) * u.fade;
  return vec4f(panels(s.pos.xy, c), 1.0);
}
// ---- the strokes: one quad per stroke or light (myrealm.coax lit, tinted_dot)
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
