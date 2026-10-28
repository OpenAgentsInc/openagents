// A realm's three passes: the sky, the house and the HUD. Every number they
// draw from comes from Coax: the view, the eye, the lights and each
// object's place and colour from verse/haunt.coax and player.coax
// (through realm-face.c), the HUD's strokes and panels from realm.coax.
//
// The house is the iOS stage's (apps/openagents-ios/OpenAgents/Platform/
// Stages.metal haunt_vertex, haunt_fragment and haunt_sky_fragment), ported
// by hand; its light, fog and the tone curve are the Verse engine's own functions
// (haunt-light.coax, light.coax) converted by coax/web/wgsl.py at build time
// into the generated blocks below, as is the HUD bar (verse_bar). The
// phone adds a bloom pass over the scene; the browser doesn't yet, so the
// tone curve is applied as each pixel is drawn.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,   // the view: clip = (r0, r1, r2, r3) · world
  eye: vec4f,          // the eye, and how many lights
  flash: vec4f,        // lightning, 0, 0, 0
  panel: array<vec4f, 8>,   // per panel: x0, y0, x1, y1 (device pixels), then scale, alpha, tint, 0
}
@group(0) @binding(0) var<uniform> u: U;

fn tone(c: vec3f) -> vec3f { return vec3f(verse_tone(c.r), verse_tone(c.g), verse_tone(c.b)); }

// A HUD panel's dark glass behind its text, in verse_bar's shape (the
// plain corners, top right and bottom left, cut at 45°): faint scanlines and
// a glow inside its edge, orange.
fn glass(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let q = p - (r.xy + half);
  let inside = -coax_p_cut_distance(-q.x, q.y, half.x, half.y, max(s.x, 1.0));
  if (inside <= -1.0) { return under; }
  let tint = vec3f(1.0, 0.55, 0.12);
  let shade = mix(vec3f(0.04, 0.03, 0.02), vec3f(0.012, 0.012, 0.018), clamp((q.y + half.y) / max(2.0 * half.y, 1.0), 0.0, 1.0));
  let line = select(0.0, 0.02, fract(p.y / 3.0) < 0.34);
  let glow = exp(-max(inside, 0.0) / 18.0) * 0.06;
  let lit = mix(under * 0.3, shade, 0.8) + vec3f(line) + tint * glow;
  return mix(under, lit, clamp(s.y, 0.0, 1.0) * smoothstep(-0.5, 1.0, inside));
}
fn panels(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 4; k++) {
    if (f32(k) >= u.panels) { break; }
    c = glass(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}

// ---- the sky: one full-screen triangle (haunt_sky_fragment)
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let v = clamp(pos.y / max(u.size.y, 1.0), 0.0, 1.0);
  let sky = mix(vec3f(0.025, 0.03, 0.065), vec3f(0.035, 0.04, 0.07), v);
  return vec4f(panels(pos.xy, tone(sky)), 1.0);
}

// ---- the house: each draw a mesh's indices over the shared vertices, one
// instance per object (Stages.metal haunt_vertex, haunt_fragment). A vertex
// is 48 bytes: position, normal, uv, a colour whose alpha is its sheet, and
// three reserved words.
struct Instance { r0: vec4f, r1: vec4f, r2: vec4f, material: vec4f }
struct Light { position: vec4f, color: vec4f }
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<Instance>;
@group(1) @binding(2) var<storage, read> lights: array<Light>;
@group(1) @binding(3) var sheets: texture_2d_array<f32>;
@group(1) @binding(4) var sheet: sampler;

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) tint: vec3f,
  @location(4) glow: f32,
  @location(5) @interpolate(flat) slice: u32,
}
fn word(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  let b = id * 12u;
  let p = vec4f(word(b), word(b + 1u), word(b + 2u), 1.0);
  let n = vec3f(word(b + 3u), word(b + 4u), word(b + 5u));
  let colour = unpack4x8unorm(vertices[b + 8u]);
  let m = objects[instance];
  let w = vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p));
  let wp = vec4f(w, 1.0);
  var o: S;
  o.pos = vec4f(dot(u.r0, wp), dot(u.r1, wp), dot(u.r2, wp), dot(u.r3, wp));
  o.world = w;
  o.normal = normalize(vec3f(dot(m.r0.xyz, n), dot(m.r1.xyz, n), dot(m.r2.xyz, n)));
  o.uv = vec2f(word(b + 6u), word(b + 7u));
  o.tint = colour.rgb * m.material.rgb;
  o.glow = m.material.a;
  o.slice = vertices[b + 8u] >> 24u;
  return o;
}
@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  if (tex.a < 0.35) { discard; }
  let n = normalize(s.normal);
  let moon = max(0.0, dot(n, normalize(vec3f(-0.55, 0.55, -0.62))));
  var lit = vec3f(verse_haunt_ambient(n.y, 0.0), verse_haunt_ambient(n.y, 1.0), verse_haunt_ambient(n.y, 2.0))
          + moon * vec3f(0.5625, 0.6375, 0.8625) * 0.48;
  let count = u32(u.eye.w);
  for (var i = 0u; i < count; i++) {
    let l = lights[i];
    let delta = l.position.xyz - s.world;
    let d = length(delta);
    let lambert = max(0.08, dot(n, delta / max(d, 0.001)));
    lit += l.color.rgb * verse_haunt_attenuate(d, l.position.w) * lambert * 2.2;
  }
  let colour = tex.rgb * s.tint;
  let d = length(u.eye.xyz - s.world);
  let f = u.flash.x;
  let c = vec3f(verse_haunt_shade(colour.r, lit.r, s.glow, d, 0.0, f),
                verse_haunt_shade(colour.g, lit.g, s.glow, d, 1.0, f),
                verse_haunt_shade(colour.b, lit.b, s.glow, d, 2.0, f));
  return vec4f(panels(s.pos.xy, tone(c)), 1.0);
}

// ---- generated: verse/light.coax (verse_bar, verse_tone) by coax/web/wgsl.py; do not edit
fn coax_p_hi(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lo(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_cut_distance(l_x: f32, l_y: f32, l_w: f32, l_h: f32, l_k: f32) -> f32 {
  var coax_t14: f32;
  var coax_t15: f32;
  var coax_t16: f32;
  var coax_t17: f32;
  var coax_t18: f32;
  var coax_t19: f32;
  var coax_t20: f32;
  var coax_t21: f32;
  var coax_t22: f32;
  var coax_t23: f32;
  var l_dx: f32 = (abs(l_x) - l_w);
  var l_dy: f32 = (abs(l_y) - l_h);
  coax_t16 = coax_p_hi(l_dx, 0.0);
  coax_t17 = coax_p_hi(l_dx, 0.0);
  coax_t15 = (coax_t16 * coax_t17);
  coax_t19 = coax_p_hi(l_dy, 0.0);
  coax_t20 = coax_p_hi(l_dy, 0.0);
  coax_t18 = (coax_t19 * coax_t20);
  coax_t14 = sqrt((coax_t15 + coax_t18));
  coax_t21 = coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0);
  var l_box: f32 = (coax_t14 + coax_t21);
  coax_t22 = abs((l_x + l_y));
  coax_t23 = ((l_w + l_h) - (14.0 * l_k));
  return coax_p_hi(l_box, ((coax_t22 - coax_t23) * 0.707106769));
}
fn coax_p_sat(l_a: f32) -> f32 {
  return coax_p_lo(1.0, coax_p_hi(0.0, l_a));
}
fn coax_p_smooth(l_a: f32, l_b: f32, l_x: f32) -> f32 {
  var l_t: f32 = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_bar(l_x: f32, l_y: f32, l_w: f32, l_h: f32, l_k: f32, l_t: f32, l_press: f32, l_flash: f32) -> f32 {
  var coax_t24: f32;
  var coax_t25: f32;
  var coax_t26: f32;
  var coax_t27: f32;
  var coax_t28: f32;
  var coax_t29: f32;
  var coax_t30: f32;
  var coax_t31: f32;
  var coax_t32: f32;
  var coax_t33: f32;
  var coax_t34: f32;
  var l_d: f32 = coax_p_cut_distance(l_x, l_y, l_w, l_h, l_k);
  var l_inside: f32 = (1.0 - coax_p_smooth((-0.5), 0.5, l_d));
  coax_t24 = (0.5 * l_k);
  coax_t25 = (1.60000002 * l_k);
  coax_t26 = abs(l_d);
  var l_edge: f32 = (1.0 - coax_p_smooth(coax_t24, coax_t25, coax_t26));
  coax_t27 = (l_t / 6.0);
  coax_t28 = floor((l_t / 6.0));
  var l_phase: f32 = (coax_t27 - coax_t28);
  let coax_sq1: f32 = ((((l_x + (l_y * 0.600000024)) / l_w) - ((-1.60000002) + ((3.20000005 * l_phase) / 0.219999999))) / 0.100000001);
  var l_sweep: f32 = exp((-(coax_sq1 * coax_sq1)));
  if ((l_phase > 0.219999999))
  {
    l_sweep = 0.0;
  }
  var l_glass: f32 = ((((0.0350000001 + (0.0250000004 * (1.0 - (l_y / l_h)))) + (0.100000001 * l_press)) + (0.100000001 * l_sweep)) * l_inside);
  coax_t31 = (-coax_p_hi(l_d, 0.0));
  coax_t32 = (9.0 * l_k);
  coax_t30 = exp((coax_t31 / coax_t32));
  coax_t33 = (1.0 - l_inside);
  coax_t29 = (coax_t30 * coax_t33);
  coax_t34 = (0.100000001 + (0.300000012 * l_flash));
  var l_glow: f32 = (coax_t29 * coax_t34);
  return ((l_glass + (l_edge * (0.649999976 + (0.5 * l_flash)))) + l_glow);
}
fn verse_tone(l_light: f32) -> f32 {
  return (1.0 - exp(((-l_light) * 1.35000002)));
}
// ---- end generated

// ---- generated: verse/haunt-light.coax (verse_haunt_ambient, verse_haunt_attenuate, verse_haunt_shade) by coax/web/wgsl.py; do not edit
fn verse_haunt_ambient(l_ny: f32, l_channel: f32) -> f32 {
  var l_sky: f32 = select(select(0.150000006, 0.0900000036, (l_channel < 1.5)), 0.0799999982, (l_channel < 0.5));
  var l_ground: f32 = select(select(0.0199999996, 0.0250000004, (l_channel < 1.5)), 0.0299999993, (l_channel < 0.5));
  return (l_ground + ((l_sky - l_ground) * ((l_ny * 0.5) + 0.5)));
}
fn coax_p_hl_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn verse_haunt_attenuate(l_d: f32, l_r: f32) -> f32 {
  var l_q: f32 = coax_p_hl_max(0.0, (1.0 - (l_d / coax_p_hl_max(l_r, 0.00999999978))));
  return (l_q * l_q);
}
fn verse_haunt_shade(l_albedo: f32, l_light: f32, l_glow: f32, l_distance: f32, l_channel: f32, l_flash: f32) -> f32 {
  var l_fog: f32 = (1.0 - exp((((-l_distance) * l_distance) * 0.000120999997)));
  var l_fogcolor: f32 = select(select(0.0700000003, 0.0399999991, (l_channel < 1.5)), 0.0350000001, (l_channel < 0.5));
  var l_lit: f32 = ((l_albedo * ((l_light + (l_glow * 2.0)) + (l_flash * 2.0))) * 1.29999995);
  return (l_lit + ((l_fogcolor - l_lit) * l_fog));
}
// ---- end generated

// ---- the HUD: one quad per stroke or bar (realm.coax)
struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,   // along, across, pixels
  @location(1) shape: vec4f,   // length, core, light, glow
  @location(2) tint: vec4f,    // red, green, blue, mode (realm.coax MODE_*)
  @location(3) bar: vec4f,     // a HUD bar's press and flash
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
  if (v.tint.w > 3.5) {
    // A HUD bar (ui.coax bar), as splash.wgsl draws one: verse_bar mirrored
    // in x, so its cuts are on the plain corners.
    let p = v.place / (1.0 - 0.03 * v.bar.x);
    let c = verse_bar(-p.x, p.y, v.shape.x, v.shape.y, v.shape.z, u.time, v.bar.x, v.bar.y);
    return vec4f(v.tint.rgb * c * v.shape.w * u.fade, 1.0);
  }
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
