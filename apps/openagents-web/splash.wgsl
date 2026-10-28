// The splash's two passes. Every number they draw from comes from
// splash.coax: the canvas size, the nebula's clock, the fade, and each
// stroke's place, width, light and glow.
//
// Temporary: hand-written WGSL until Coax emits WGSL (coax/web.md). The
// nebula is the previous openagents.com splash's, unchanged.

struct U { size: vec2f, time: f32, fade: f32, still: f32, pad0: f32, pad1: f32, pad2: f32 }
@group(0) @binding(0) var<uniform> u: U;

// ---- the nebula: one full-screen triangle
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x), s.y);
}
fn fbm(q: vec2f) -> f32 {
  var p = q; var v = 0.0; var a = 0.5;
  for (var k = 0; k < 5; k++) { v += a * noise(p); p = p * 2.03 + vec2f(1.7, 9.2); a *= 0.5; }
  return v;
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let uv = (pos.xy - 0.5 * u.size) / u.size.y;
  let t = u.time * 0.035;
  let w = vec2f(fbm(uv * 1.6 + vec2f(t, -t)), fbm(uv * 1.6 + vec2f(-t, t * 0.7) + 4.0));
  let n = fbm(uv * 2.2 + 2.4 * w + vec2f(0.0, t));
  let glow = exp(-2.2 * dot(uv, uv));
  let tint = mix(vec3f(0.05, 0.055, 0.075), vec3f(0.20, 0.21, 0.27), n * n);
  var c = tint * glow * glow * (0.2 + 0.8 * n);
  let star = step(0.9975, hash(floor(pos.xy * 0.5))) * (0.5 + 0.5 * sin(u.time * 1.3 + hash(floor(pos.xy)) * 40.0));
  c += vec3f(star * 0.5 * (1.0 - glow * 0.6));
  return vec4f(c * u.fade, 1.0);
}

// ---- the words: one quad per stroke, a sharp core and a soft glow
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f) -> V {
  var o: V;
  let ndc = vec2f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0);
  o.pos = vec4f(ndc, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  let light = (core + halo) * v.shape.z * u.fade;
  return vec4f(vec3f(0.93, 0.93, 0.95) * light, 1.0);
}
