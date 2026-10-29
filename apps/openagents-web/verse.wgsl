// The Verse landing and galaxy's two passes: the night behind (the
// playtest's field: a nebula and stars, and the plates verse.coax lays out) and the strokes over it (words, orbs, orbits and worlds). Every
// place and size comes from verse.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  panel: array<vec4f, 32>,  // per plate (verse.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
}
@group(0) @binding(0) var<uniform> u: U;
// The system sans for the lede and the brand: the host's glyph atlas (coax/web/host.js, "the sans atlas").
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;
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
// The playtest's PLAYTEST NOW (hud-button.js): dark glass with two corners
// cut, a thin bright edge, a sweep of light every 6 s, scan lines, and two
// corner brackets standing off it. Look 4; 5 under the mouse, 6 pressed.
fn band(x: f32, lo: f32, hi: f32) -> f32 { return step(lo, x) * step(x, hi); }
fn cover(d: f32) -> f32 { return 1.0 - smoothstep(-0.5, 0.5, d); }
fn rim(d: f32, w: f32) -> f32 { return 1.0 - smoothstep(w * 0.5 - 0.5, w * 0.5 + 0.5, abs(d + w * 0.5)); }
fn sd_box(p: vec2f, b: vec2f) -> f32 { let q = abs(p) - b; return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0); }
fn sd_cut(p: vec2f, b: vec2f, cut: f32) -> f32 { return max(sd_box(p, b), (abs(p.x + p.y) - (b.x + b.y - cut)) * 0.70710678); }
fn hud(pos: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  let k = s.x;
  let ext = (r.zw - r.xy) * 0.5;
  let q = pos - (r.xy + ext);   // y down
  if (any(abs(q) > ext + vec2f(40.0 * k))) { return under; }
  let press = select(0.0, 1.0, s.z >= 5.5);
  let hover = select(0.0, 1.0, s.z >= 4.5);
  let p = q / (1.0 - 0.03 * press);
  let n = p / ext;
  let glass = vec3f(0.035, 0.04, 0.05) + 0.05 * (0.5 - 0.5 * n.y);
  let edge = vec3f(0.92, 0.95, 1.0);
  let d_box = sd_box(p, ext);
  let d = sd_cut(p, ext, 14.0 * k);
  let inside = cover(d);
  let rgb = glass + 0.012 * step(2.0 * k, pos.y - 3.0 * k * floor(pos.y / (3.0 * k))) + 0.10 * press + 0.04 * hover;
  var col = rgb * 0.8 * inside;
  var alpha = 0.8 * inside;
  let t = select(u.time, 0.0, u.still > 0.5);
  let phase = fract(t / 6.0);
  let w = (p.x + p.y * 0.6) / ext.x;
  let sweep = exp(-pow((w - mix(-1.6, 1.6, phase / 0.22)) / 0.10, 2.0)) * band(phase, 0.0, 0.22);
  col += vec3f(0.10) * sweep * inside;
  let on_cut = step(d_box, d - 0.25);
  let e = rim(d, 1.2 * k) * (0.42 + 0.2 * hover) + rim(d, 2.6 * k) * on_cut * 0.95;
  col += edge * e;
  alpha += e;
  let off = 6.0 * k; let arm = 16.0 * k; let th = 1.4 * k;
  var br = 0.0;
  for (var c = 0; c < 2; c++) {
    let m = select(vec2f(-p.x, p.y), vec2f(p.x, -p.y), c == 0);
    let b = m - (ext + vec2f(off));
    br = max(br, max(band(b.x, -arm, th * 0.5) * step(abs(b.y), th * 0.5), band(b.y, -arm, th * 0.5) * step(abs(b.x), th * 0.5)));
  }
  col += vec3f(0.85) * br * 0.55;
  alpha += br * 0.55;
  alpha = min(alpha, 1.0);
  col = min(col, vec3f(alpha));
  let glow = exp(-max(d, 0.0) / (9.0 * k)) * (1.0 - inside) * 0.10;
  let opacity = 1.0 - 0.45 * press;
  return (col + under * (1.0 - alpha)) * opacity + under * (1.0 - opacity) + vec3f(glow);
}
fn panels(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 16; k++) {
    if (f32(k) >= u.panels) { break; }
    let s = u.panel[k * 2 + 1];
    if (s.z >= 3.5) { c = hud(p, c, u.panel[k * 2], s); } else { c = plate(p, c, u.panel[k * 2], s); }
  }
  return c;
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
// The playtest's field behind the landing and the map (app.js FIELD_SHADER,
// its cool tint): a slow nebula glowing from the middle, and stars.
fn field(pos: vec2f) -> vec3f {
  let uv = (pos - 0.5 * u.size) / u.size.y;
  let t = (20.0 + select(u.time, 0.0, u.still > 0.5)) * 0.035;
  let w = vec2f(fbm(uv * 1.6 + vec2f(t, -t)), fbm(uv * 1.6 + vec2f(-t, t * 0.7) + 4.0));
  let n = fbm(uv * 2.2 + 2.4 * w + vec2f(0.0, t));
  let glow = exp(-2.2 * dot(uv, uv));
  let tint = mix(vec3f(0.05, 0.055, 0.075), vec3f(0.20, 0.21, 0.27), n * n);
  var c = tint * glow * glow * (0.2 + 0.8 * n);
  let px = pos * min(2.0, max(u.k, 1.0)) / max(u.k, 1.0);
  let star = step(0.9975, hash(floor(px * 0.5))) * (0.5 + 0.5 * sin((20.0 + u.time) * 1.3 + hash(floor(px)) * 40.0));
  c += vec3f(star * 0.5 * (1.0 - glow * 0.6));
  return c;
}
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  return vec4f(panels(p, field(p) * u.fade), 1.0);
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
  if (v.tint.a > 4.5) {
    // an orb (verse.coax MODE_ORB): lit from the top left, #9a97a3 to #3c3a46, a faint halo
    let r = v.shape.y; let d = length(v.place);
    let inside = 1.0 - smoothstep(r - 0.75, r + 0.75, d);
    let t = clamp((length(v.place + vec2f(0.35 * r)) - 0.1 * r) / (0.9 * r), 0.0, 1.0);
    let lit = mix(vec3f(0.604, 0.592, 0.639), vec3f(0.235, 0.227, 0.275), t);
    let halo = 0.03 * (1.0 - smoothstep(0.8 * r, 2.0 * r, d)) * (1.0 - inside);
    // tint: white for the faint grey worlds, warm for the Agent orb (verse.coax draw_guide)
    return vec4f((lit * v.tint.rgb * inside + vec3f(0.93) * halo) * v.shape.z * u.fade, 1.0);
  }
  if (v.tint.a > 3.5) {
    // a sans glyph (font.coax draw_sans): place is the atlas coordinate, shape.x the em in pixels
    let lod = clamp(log2(64.0 / max(v.shape.x, 1.0)), 0.0, 4.0);
    let a = pow(textureSampleLevel(sans_atlas, sans_sampler, v.place, lod).a, 0.85);
    return vec4f(v.tint.rgb * a * min(v.shape.z * 1.15, 1.0) * u.fade, 1.0);
  }
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
