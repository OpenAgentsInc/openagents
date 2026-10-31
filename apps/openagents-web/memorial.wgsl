// A memorial's page (memorial.coax): two passes. The background is the
// garden's night in its flat style: a dusty blue sky paler toward the
// horizon, a flat ivory moon with a soft halo, a few stars, and low faceted
// hills along the foot that slide a little as the page scrolls; over it the
// glass of up to four panels (the page's text, the stone's face, a photo's
// dark surround). Then the strokes: the stroke font, the sans, the lines.
// Every number comes from memorial.coax. Orange is Bitcoin's: none here.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, pad0: f32, pad1: f32, pad2: f32,
  panels: array<vec4f, 8>,   // per panel: x0, y0, x1, y1 (device pixels), then corner, alpha, kind, arch
  moon: vec4f,               // the moon's x, y and radius (device pixels), how far the page scrolled
  pad3: vec4f,
}
@group(0) @binding(0) var<uniform> u: U;
// The system sans for small text: the host's glyph atlas (coax/web/host.js, "the sans atlas").
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}

// A ridge of straight runs between hashed peaks: its height (0 to 1 of the band) at x.
fn ridge(x: f32, step: f32, seed: f32) -> f32 {
  let i = floor(x / step);
  let f = fract(x / step);
  let a = hash(vec2f(i, seed));
  let b = hash(vec2f(i + 1.0, seed));
  return mix(a, b, f);
}

@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let h = u.size.y;
  let t = clamp(pos.y / h, 0.0, 1.0);
  // the sky: deep dusty blue up top, paler toward the foot (sRGB, as the garden's palette)
  var c = mix(vec3f(0.235, 0.29, 0.42), vec3f(0.47, 0.53, 0.66), smoothstep(0.0, 1.0, t));
  // a few stars, still in a shot
  let cell = floor(pos.xy / 3.0);
  let tw = select(0.5 + 0.5 * sin(u.time * 1.1 + hash(cell) * 40.0), 1.0, u.still > 0.5);
  c += vec3f(step(0.9985, hash(cell)) * 0.45 * tw * (1.0 - t));
  // the moon: a flat ivory disc, a soft halo round it
  let m = u.moon;
  if (m.z > 0.0) {
    let d = length(pos.xy - m.xy) / m.z;
    c = mix(c, vec3f(0.62, 0.66, 0.8), exp(-pow(max(d - 1.0, 0.0) * 1.2, 2.0)) * 0.35);
    c = mix(c, vec3f(0.96, 0.94, 0.87), 1.0 - smoothstep(0.985, 1.015, d));
  }
  // two rows of low faceted hills along the foot, sliding a little as the page scrolls
  let k = u.size.y / 800.0;
  let slide = u.moon.w * 0.08;
  let far = h - (110.0 + 70.0 * ridge(pos.x + slide * 0.5, 180.0 * k, 3.0)) * k;
  let near = h - (40.0 + 80.0 * ridge(pos.x + slide, 160.0 * k, 7.0)) * k;
  if (pos.y > far) { c = mix(c, vec3f(0.33, 0.4, 0.5), 0.9); }
  if (pos.y > near) {
    // a facet: the ridge's run lit or in shade by its slope
    let s = ridge(pos.x + slide + 1.0, 160.0 * k, 7.0) - ridge(pos.x + slide - 1.0, 160.0 * k, 7.0);
    c = select(vec3f(0.27, 0.36, 0.38), vec3f(0.33, 0.43, 0.42), s < 0.0);
  }
  c = c * u.fade;
  for (var i = 0; i < 4; i++) { c = panel(pos.xy, c, u.panels[i * 2], u.panels[i * 2 + 1]); }
  return vec4f(c, 1.0);
}

// A panel's glass: a rounded box, dark periwinkle and see-through, a thin pale edge.
fn panel(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  let alpha = s.y;
  if (alpha <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = s.x;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  var dd = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  // a tombstone (s.w > 0): its top an arch s.w tall, half an ellipse across its width
  let arch = s.w;
  if (arch > 0.0 && p.y < r.y + arch) {
    let e = vec2f((p.x - (r.x + half.x)) / half.x, (p.y - (r.y + arch)) / arch);
    dd = (length(e) - 1.0) * min(half.x, arch);
  }
  if (dd > 1.0) { return under; }
  var fill = vec3f(0.11, 0.13, 0.2);
  if (s.z > 0.5 && s.z < 1.5) { fill = mix(vec3f(0.42, 0.46, 0.58), vec3f(0.3, 0.34, 0.45), clamp((p.y - r.y) / max(r.w - r.y, 1.0), 0.0, 1.0)); }   // the stone's face, lit from the moon above
  if (s.z > 1.5) { fill = vec3f(0.04, 0.05, 0.08); }               // round a photo
  let inside = clamp(0.5 - dd, 0.0, 1.0);
  var c = mix(under, fill, alpha * inside);
  if (s.z < 1.5) {
    let edge = clamp(1.5 - abs(dd + 0.75), 0.0, 1.0);
    c = mix(c, vec3f(0.86, 0.88, 0.96), edge * 0.35);
  }
  return c;
}

// ---- the strokes: text and lines, one quad each (memorial.coax lit, font.coax draw_sans)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels (a glyph: its atlas coordinate)
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  return o;
}
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  if (v.tint.w > 3.5) {
    let lod = clamp(log2(64.0 / max(v.shape.x, 1.0)), 0.0, 4.0);
    let a = pow(textureSampleLevel(sans_atlas, sans_sampler, v.place, lod).a, 0.85);
    return vec4f(v.tint.rgb * a * min(v.shape.z * 1.15, 1.0) * u.fade, 1.0);
  }
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.22;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
