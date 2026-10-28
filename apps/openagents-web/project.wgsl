// A project page's two passes (project.coax decides every number): a dark
// nebula, dimmer than the splash's so the HUD reads over it, with the glass
// of up to ten HUD panels, then the strokes.
//
// Temporary: hand-written WGSL until Coax emits WGSL (coax/web.md). The
// nebula and the stroke modes are splash.wgsl's.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, count: f32, pad0: f32, pad1: f32,
  panels: array<vec4f, 20>,   // per panel: x0, y0, x1, y1 (device pixels), then cut, alpha, tint, 0
}
@group(0) @binding(0) var<uniform> u: U;

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
  let t = u.time * 0.03;
  let w = vec2f(fbm(uv * 1.4 + vec2f(t, -t)), fbm(uv * 1.4 + vec2f(-t, t * 0.7) + 4.0));
  let n = fbm(uv * 2.0 + 2.4 * w + vec2f(0.0, t));
  let glow = exp(-1.6 * dot(uv, uv));
  let tint = mix(vec3f(0.035, 0.038, 0.05), vec3f(0.13, 0.14, 0.18), n * n);
  var c = tint * glow * (0.25 + 0.75 * n) * 0.8;
  // a faint warm horizon behind the Grid
  c += vec3f(0.06, 0.03, 0.008) * exp(-pow((uv.y - 0.08) * 6.0, 2.0)) * glow;
  let star = step(0.998, hash(floor(pos.xy * 0.5))) * (0.5 + 0.5 * sin(u.time * 1.3 + hash(floor(pos.xy)) * 40.0));
  c += vec3f(star * 0.35 * (1.0 - glow * 0.6));
  c = c * u.fade;
  for (var k = 0; k < 10; k++) {
    if (f32(k) >= u.count) { break; }
    c = panel(pos.xy, c, u.panels[k * 2], u.panels[k * 2 + 1]);
  }
  return vec4f(c, 1.0);
}

// A HUD panel's glass: dark, its plain corners (top right, bottom left) clipped at 45°, faint scanlines and
// an orange glow inside its edge.
fn panel(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  let alpha = s.y * u.fade;
  if (alpha <= 0.0) { return under; }
  let x = p.x - r.x; let y = p.y - r.y; let w = r.z - r.x; let h = r.w - r.y;
  let side = min(min(x, w - x), min(y, h - y));
  let corner = min((w - x) + y, x + (h - y)) - s.x;
  let inside = min(side, corner * 0.7071);
  if (inside <= 0.0) { return under; }
  let tint = vec3f(1.0, 0.55, 0.12);
  let shade = mix(vec3f(0.03, 0.022, 0.016), vec3f(0.01, 0.011, 0.016), clamp(y / max(h, 1.0), 0.0, 1.0));
  let line = select(0.0, 0.018, fract(p.y / 3.0) < 0.34);
  let edge = exp(-inside / 22.0) * 0.07;
  let glass = mix(under * 0.25, shade, 0.85) + vec3f(line) + tint * edge;
  return mix(under, glass, clamp(alpha, 0.0, 1.0) * smoothstep(0.0, 1.5, inside));
}

// ---- the strokes: text, the HUD, the Grid, the tree and the agents, one quad each
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode (project.coax MODE_*)
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f) -> V {
  var o: V;
  let ndc = vec2f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0);
  o.pos = vec4f(ndc, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let mode = v.tint.w;
  if (mode > 1.5 && mode < 2.5) {
    // A payment in flight, as the Payment Network demo draws one
    // (aethyr-cpp src/paynet/render.cpp): a capsule glow-wide with a
    // white-hot head, its tail fading back along the path.
    let r = d / max(v.shape.w, 1.0);
    if (r >= 1.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let tail = select(1.0, mix(0.08, 1.0, clamp(v.place.x / v.shape.x, 0.0, 1.0)), v.shape.x > 1.0);
    let g = exp(-r * r * 5.0) * (1.0 - r);
    let hot = exp(-r * r * 36.0) * v.shape.y;
    let c = (v.tint.rgb * g + vec3f(1.0, 0.95, 0.86) * hot) * tail * v.shape.z * u.fade;
    return vec4f(c, 1.0);
  }
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  if (mode > 0.5 && mode < 1.5) {
    // A link: soft across, and faded within 8 px of its nodes.
    let ends = smoothstep(0.0, 8.0, v.place.x) * smoothstep(0.0, 8.0, v.shape.x - v.place.x);
    return vec4f(v.tint.rgb * (core * 0.8 + halo) * ends * v.shape.z * u.fade, 1.0);
  }
  if (mode > 2.5) {
    // An amount: a near-white core in a coloured glow.
    let c = (mix(v.tint.rgb, vec3f(1.0, 0.97, 0.92), 0.75) * core + v.tint.rgb * halo * 1.8) * v.shape.z * u.fade;
    return vec4f(c, 1.0);
  }
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
