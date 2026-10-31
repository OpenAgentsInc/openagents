// A realm's three passes: the sky, the house and the HUD. Every number they
// draw from comes from Coax: the view, the eye, the lights and each
// object's place and colour from verse/haunt.coax and player.coax
// (through realm-face.c), the HUD's strokes and panels from realm.coax.
//
// The house is the iOS stage's (apps/openagents-ios/OpenAgents/Platform/
// Stages.metal haunt_vertex, haunt_fragment and haunt_sky_fragment), ported
// by hand; its light, fog and the tone curve are the Verse engine's own functions
// (haunt-light.coax, light.coax) written as WGSL by coax wgsl at build time
// into the generated blocks below, as is the HUD bar (verse_bar). The
// phone adds a bloom pass over the scene; the browser doesn't yet, so the
// tone curve is applied as each pixel is drawn.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,   // the view: clip = (r0, r1, r2, r3) · world
  eye: vec4f,          // the eye, and how many lights
  flash: vec4f,        // lightning, then the cutaway's target (your chest; y under -1000: none)
  panel: array<vec4f, 56>,  // per plate (realm.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
}
@group(0) @binding(0) var<uniform> u: U;

fn tone(c: vec3f) -> vec3f { return vec3f(verse_haunt_night(c.r), verse_haunt_night(c.g), verse_haunt_night(c.b)); }

// A plate (realm.coax plate): a dark wood fill with one thin bronze line
// round it and small rounded corners. Look 1 lights the line; 2 (under
// the mouse or pressed) adds a step lighter fill and the bright line.
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
  for (var k = 0; k < 28; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}

// ---- the sky: one full-screen triangle (haunt_sky_fragment)
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
struct Lit { @location(0) light: vec4f, @location(1) aux: vec4f }
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> Lit {
  let v = clamp(pos.y / max(u.size.y, 1.0), 0.0, 1.0);
  let sky = mix(vec3f(0.025, 0.03, 0.065), vec3f(0.035, 0.04, 0.07), v);
  return Lit(vec4f(sky, 0.0), vec4f(0.0));
}

// ---- the house: each draw a mesh's indices over the shared vertices, one
// instance per object (Stages.metal haunt_vertex, haunt_fragment). A vertex
// is 48 bytes: position, normal, uv, a colour whose alpha is its sheet, its
// outline cut (verse/look.coax verse_outline_strength; 0 a full outline) and
// two reserved words.
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
  @location(6) cut: f32,
  @location(7) @interpolate(flat) material: u32,
}
fn word(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }

// ---- the moon's shadow map (haunt-light.coax verse_haunt_moon_dir, _shadow_reach): a square
// round the eye, snapped to whole texels so the shadows hold still as you walk, drawn first by
// shadow_vs/shadow_fs and read by scene_fs, twelve taps of the comparison sampler for a soft edge.
fn moon_dir() -> vec3f { return vec3f(verse_haunt_moon_dir(0.0), verse_haunt_moon_dir(1.0), verse_haunt_moon_dir(2.0)); }
fn shadow_place(w: vec3f) -> vec3f {
  let l = moon_dir();
  let r = normalize(cross(vec3f(0.0, 1.0, 0.0), l));
  let up = cross(l, r);
  let reach = verse_haunt_shadow_reach(0.0);
  let deep = verse_haunt_shadow_reach(1.0);
  let snap = reach * 2.0 / 1024.0;
  let c = vec3f(floor(dot(u.eye.xyz, r) / snap) * snap, floor(dot(u.eye.xyz, up) / snap) * snap, dot(u.eye.xyz, l));
  return vec3f((dot(w, r) - c.x) / reach, (dot(w, up) - c.y) / reach, 0.5 - (dot(w, l) - c.z) / (2.0 * deep));
}
struct Shade { @builtin(position) pos: vec4f, @location(0) uv: vec2f, @location(1) @interpolate(flat) slice: u32 }
@vertex fn shadow_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> Shade {
  let b = id * 12u;
  let p = vec4f(word(b), word(b + 1u), word(b + 2u), 1.0);
  let m = objects[instance];
  let q = shadow_place(vec3f(dot(m.r0, p), dot(m.r1, p), dot(m.r2, p)));
  var o: Shade;
  o.pos = vec4f(q.xy, q.z, 1.0);
  if (m.material.a > 0.5) { o.pos = vec4f(0.0, 0.0, -1.0, 1.0); }
  o.uv = vec2f(word(b + 6u), word(b + 7u));
  o.slice = vertices[b + 8u] >> 24u;
  return o;
}
@fragment fn shadow_fs(s: Shade) {
  if (textureSampleLevel(sheets, sheet, s.uv, s.slice, 0.0).a < 0.35) { discard; }
}
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
fn moonlit(w: vec3f, n: vec3f) -> f32 {
  let q = shadow_place(w + n * 0.06);
  let uv = vec2f(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
  let texel = 1.0 / f32(textureDimensions(shadow_map).x);
  let taps = array(vec2f(-0.94, -0.40), vec2f(0.95, -0.77), vec2f(-0.09, -0.93), vec2f(0.34, 0.29),
                   vec2f(-0.92, 0.46), vec2f(-0.82, -0.98)  * 0.5, vec2f(-0.38, 0.28), vec2f(0.97, 0.76),
                   vec2f(0.44, -0.37), vec2f(0.54, -0.87) * 0.5, vec2f(-0.26, 0.97), vec2f(0.79, 0.19));
  var lit = 0.0;
  for (var k = 0; k < 12; k++) {
    lit += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + taps[k] * texel * 2.2, q.z);
  }
  return verse_haunt_shadow_edge(uv.x, uv.y, lit / 12.0);
}
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
  o.cut = word(b + 9u);
  let tint = vec3u(clamp(m.material.rgb, vec3f(0.0), vec3f(1.0)) * 255.0);
  o.material = (o.slice * 37u + tint.r * 3u + tint.g * 5u + tint.b * 11u) & 127u;
  return o;
}
// The cutaway: a wall, beam or roof between the eye and you isn't drawn,
// so the one camera never loses you (a round window along the line of
// sight, wider near the eye; the floor under you stays).
fn cut_away(w: vec3f, n: vec3f) -> bool {
  let chest = u.flash.yzw;
  if (chest.y < -1000.0) { return false; }
  if (n.y > 0.7 && w.y < chest.y) { return false; }   // the floor you stand on stays
  if (length(w.xz - chest.xz) < 0.4 && w.y < chest.y + 1.0) { return false; }   // and you
  let line = chest - u.eye.xyz;
  let span = length(line);
  if (span < 0.3) { return false; }
  let along = dot(w - u.eye.xyz, line) / span;
  if (along <= 0.0 || along > span - 0.2) { return false; }
  if (w.y < chest.y - 1.1) { return false; }
  let off = length(w - (u.eye.xyz + line * (along / span)));
  return off < 1.05 + 0.35 * (1.0 - along / span);
}
@fragment fn scene_fs(s: S) -> Lit {
  let tex = textureSample(sheets, sheet, s.uv, s.slice);
  if (tex.a < 0.35) { discard; }
  if (cut_away(s.world, normalize(s.normal))) { discard; }
  let n = normalize(s.normal);
  let facing = max(0.0, dot(n, moon_dir()));
  let moon = select(0.0, facing * moonlit(s.world, n), facing > 0.0);
  var lit = vec3f(verse_haunt_ambient(n.y, 0.0), verse_haunt_ambient(n.y, 1.0), verse_haunt_ambient(n.y, 2.0))
          + moon * vec3f(verse_haunt_moonlight(0.0), verse_haunt_moonlight(1.0), verse_haunt_moonlight(2.0));
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
  let share = verse_haunt_bloom_share(max(c.r, max(c.g, c.b)), s.glow);
  let ink = u32(round(verse_outline_strength(s.cut, s.glow) * 15.0));
  return Lit(vec4f(c, share), vec4f(n, f32(s.material * 16u + ink)));
}

// ---- the post pass (host.js postLayouts): the light's bright share, blurred at a quarter and an
// eighth of the size (bloom_fs), added back and toned onto the screen under the plates (post_fs).
struct Step { v: vec4f }
@group(1) @binding(0) var bloom_src: texture_2d<f32>;
@group(1) @binding(1) var bloom_lin: sampler;
@group(1) @binding(2) var<uniform> bloom_step: Step;
@fragment fn bloom_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let texel = 1.0 / vec2f(textureDimensions(bloom_src));
  let uv = pos.xy * bloom_step.v.w * texel;
  if (bloom_step.v.z < 0.5) {
    var c = vec3f(0.0);
    for (var k = 0; k < 4; k++) {
      let t = textureSampleLevel(bloom_src, bloom_lin, uv + (vec2f(f32(k & 1), f32(k >> 1u)) * 2.0 - 1.0) * texel, 0.0);
      c += t.rgb * min(t.a, 1.0);
    }
    return vec4f(c * 0.25, 1.0);
  }
  let d = bloom_step.v.xy * texel;
  var c = textureSampleLevel(bloom_src, bloom_lin, uv, 0.0).rgb * 0.227;
  c += (textureSampleLevel(bloom_src, bloom_lin, uv + d * 1.385, 0.0).rgb + textureSampleLevel(bloom_src, bloom_lin, uv - d * 1.385, 0.0).rgb) * 0.316;
  c += (textureSampleLevel(bloom_src, bloom_lin, uv + d * 3.231, 0.0).rgb + textureSampleLevel(bloom_src, bloom_lin, uv - d * 3.231, 0.0).rgb) * 0.070;
  return vec4f(c, 1.0);
}
@group(1) @binding(0) var post_light: texture_2d<f32>;
@group(1) @binding(1) var post_quarter: texture_2d<f32>;
@group(1) @binding(2) var post_eighth: texture_2d<f32>;
@group(1) @binding(3) var post_aux: texture_2d<f32>;
@group(1) @binding(4) var post_depth: texture_depth_2d;
@group(1) @binding(5) var post_lin: sampler;
// Outlines (verse/look.coax): the depth's second difference, creases between neighbours' normals
// and changes of material, inked as strongly as the nearer surface's material asks.
fn outlined(p: vec2i, light: vec3f) -> vec3f {
  let a = textureLoad(post_aux, p, 0);
  let tag = u32(round(a.a));
  let strength = f32(tag & 15u) / 15.0;
  if (strength <= 0.0) { return light; }
  let o = i32(max(1.0, floor(u.size.y / 800.0)));
  let top = vec2i(textureDimensions(post_depth)) - 1;
  let at = array(vec2i(-o, 0), vec2i(o, 0), vec2i(0, -o), vec2i(0, o));
  let dc = textureLoad(post_depth, p, 0);
  var d: array<f32, 4>;
  var edge = 0.0;
  for (var k = 0; k < 4; k++) {
    let q = clamp(p + at[k], vec2i(0), top);
    d[k] = textureLoad(post_depth, q, 0);
    let b = textureLoad(post_aux, q, 0);
    if (d[k] < 1.0) {
      edge = max(edge, verse_outline_crease(dot(a.rgb, b.rgb)));
      let other = u32(round(b.a)) >> 4u;
      if (other != tag >> 4u && dc <= d[k] + 0.00001 && (tag >> 4u) > other) { edge = max(edge, 0.7); }
    }
  }
  let lap = max(abs(d[0] + d[1] - 2.0 * dc), abs(d[2] + d[3] - 2.0 * dc));
  edge = max(edge, verse_outline_depth(lap, dc));
  return vec3f(verse_outline_ink(light.r, edge * strength), verse_outline_ink(light.g, edge * strength), verse_outline_ink(light.b, edge * strength));
}
@fragment fn post_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = vec2i(pos.xy);
  let uv = pos.xy / u.size;
  let light = outlined(p, textureLoad(post_light, p, 0).rgb);
  let near = textureSampleLevel(post_quarter, post_lin, uv, 0.0).rgb;
  let far = textureSampleLevel(post_eighth, post_lin, uv, 0.0).rgb;
  let c = vec3f(verse_haunt_glow(light.r, near.r, far.r), verse_haunt_glow(light.g, near.g, far.g), verse_haunt_glow(light.b, near.b, far.b));
  return vec4f(panels(pos.xy, tone(c)), 1.0);
}
// ---- generated: verse/light.coax (verse_bar, verse_tone) by coax wgsl; do not edit
fn verse_bar(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32, p_t: f32, p_press: f32, p_flash: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_t: f32 = p_t;
  var l_press: f32 = p_press;
  var l_flash: f32 = p_flash;
  var l_d: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_phase: f32;
  var l_sweep: f32;
  var l_glass: f32;
  var l_glow: f32;
  l_d = coax_p_cut_distance(l_x, l_y, l_w, l_h, l_k);
  l_inside = (0x1.000000p0f - coax_p_smooth((-0x1.000000p-1f), 0x1.000000p-1f, l_d));
  l_edge = (0x1.000000p0f - coax_p_smooth((0x1.000000p-1f * l_k), (0x1.99999ap0f * l_k), abs(l_d)));
  l_phase = ((l_t / 0x1.800000p2f) - floor((l_t / 0x1.800000p2f)));
  l_sweep = exp((-(((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f) * ((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f))));
  if ((l_phase > 0x1.c28f5cp-3f))
  {
    l_sweep = 0.0f;
  }
  l_glass = ((((0x1.1eb852p-5f + (0x1.99999ap-6f * (0x1.000000p0f - (l_y / l_h)))) + (0x1.99999ap-4f * l_press)) + (0x1.99999ap-4f * l_sweep)) * l_inside);
  l_glow = ((exp(((-coax_p_hi(l_d, 0.0f)) / (0x1.200000p3f * l_k))) * (0x1.000000p0f - l_inside)) * (0x1.99999ap-4f + (0x1.333334p-2f * l_flash)));
  return ((l_glass + (l_edge * (0x1.4cccccp-1f + (0x1.000000p-1f * l_flash)))) + l_glow);
}
fn verse_tone(p_light: f32) -> f32 {
  var l_light: f32 = p_light;
  return (0x1.000000p0f - exp(((-l_light) * 0x1.59999ap0f)));
}
fn coax_p_cut_distance(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_dx: f32;
  var l_dy: f32;
  var l_box: f32;
  l_dx = (abs(l_x) - l_w);
  l_dy = (abs(l_y) - l_h);
  l_box = (sqrt(((coax_p_hi(l_dx, 0.0f) * coax_p_hi(l_dx, 0.0f)) + (coax_p_hi(l_dy, 0.0f) * coax_p_hi(l_dy, 0.0f)))) + coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0f));
  return coax_p_hi(l_box, ((abs((l_x + l_y)) - ((l_w + l_h) - (0x1.c00000p3f * l_k))) * 0x1.6a09e6p-1f));
}
fn coax_p_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_hi(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lo(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sat(p_a: f32) -> f32 {
  var l_a: f32 = p_a;
  return coax_p_lo(0x1.000000p0f, coax_p_hi(0.0f, l_a));
}
// ---- end generated

// ---- generated: verse/haunt-light.coax (verse_haunt_ambient, verse_haunt_attenuate, verse_haunt_shade, verse_haunt_moon_dir, verse_haunt_moonlight, verse_haunt_shadow_reach, verse_haunt_shadow_edge, verse_haunt_night, verse_haunt_bloom_share, verse_haunt_glow) by coax wgsl; do not edit
fn verse_haunt_ambient(p_ny: f32, p_channel: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_channel: f32 = p_channel;
  var l_sky: f32;
  var l_ground: f32;
  l_sky = select(select(0x1.333334p-3f, 0x1.70a3d8p-4f, (l_channel < 0x1.800000p0f)), 0x1.47ae14p-4f, (l_channel < 0x1.000000p-1f));
  l_ground = select(select(0x1.47ae14p-6f, 0x1.99999ap-6f, (l_channel < 0x1.800000p0f)), 0x1.eb851ep-6f, (l_channel < 0x1.000000p-1f));
  return (l_ground + ((l_sky - l_ground) * ((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)));
}
fn verse_haunt_attenuate(p_d: f32, p_r: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_r: f32 = p_r;
  var l_q: f32;
  l_q = coax_p_hl_max(0.0f, (0x1.000000p0f - (l_d / coax_p_hl_max(l_r, 0x1.47ae14p-7f))));
  return (l_q * l_q);
}
fn verse_haunt_shade(p_albedo: f32, p_light: f32, p_glow: f32, p_distance: f32, p_channel: f32, p_flash: f32) -> f32 {
  var l_albedo: f32 = p_albedo;
  var l_light: f32 = p_light;
  var l_glow: f32 = p_glow;
  var l_distance: f32 = p_distance;
  var l_channel: f32 = p_channel;
  var l_flash: f32 = p_flash;
  var l_fog: f32;
  var l_fogcolor: f32;
  var l_lit: f32;
  l_fog = (0x1.000000p0f - exp((((-l_distance) * l_distance) * 0x1.fb82c2p-14f)));
  l_fogcolor = select(select(0x1.1eb852p-4f, 0x1.47ae14p-5f, (l_channel < 0x1.800000p0f)), 0x1.1eb852p-5f, (l_channel < 0x1.000000p-1f));
  l_lit = ((l_albedo * ((l_light + (l_glow * 0x1.000000p1f)) + (l_flash * 0x1.000000p1f))) * 0x1.4cccccp0f);
  return (l_lit + ((l_fogcolor - l_lit) * l_fog));
}
fn verse_haunt_moon_dir(p_axis: f32) -> f32 {
  var l_axis: f32 = p_axis;
  return select(select((-0x1.3f227ep-1f), 0x1.1b1b7ap-1f, (l_axis < 0x1.800000p0f)), (-0x1.1b1b7ap-1f), (l_axis < 0x1.000000p-1f));
}
fn verse_haunt_moonlight(p_channel: f32) -> f32 {
  var l_channel: f32 = p_channel;
  return select(select(0x1.051eb8p-1f, 0x1.800000p-2f, (l_channel < 0x1.800000p0f)), 0x1.51eb86p-2f, (l_channel < 0x1.000000p-1f));
}
fn verse_haunt_shadow_reach(p_axis: f32) -> f32 {
  var l_axis: f32 = p_axis;
  return select(0x1.180000p6f, 0x1.a00000p4f, (l_axis < 0x1.000000p-1f));
}
fn verse_haunt_shadow_edge(p_u: f32, p_v: f32, p_lit: f32) -> f32 {
  var l_u: f32 = p_u;
  var l_v: f32 = p_v;
  var l_lit: f32 = p_lit;
  var l_e: f32;
  var l_fade: f32;
  l_e = coax_p_hl_max(coax_p_hl_max(((l_u * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - (l_u * 0x1.000000p1f))), coax_p_hl_max(((l_v * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - (l_v * 0x1.000000p1f))));
  l_fade = coax_p_hl_min(0x1.000000p0f, coax_p_hl_max(0.0f, ((l_e - 0x1.ccccccp-1f) * 0x1.400000p3f)));
  return (l_lit + ((0x1.000000p0f - l_lit) * l_fade));
}
fn verse_haunt_night(p_light: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_t: f32;
  l_t = (0x1.000000p0f - exp(((-l_light) * 0x1.59999ap0f)));
  return ((l_t * 0x1.99999ap-1f) + (sqrt(coax_p_hl_max(l_t, 0.0f)) * 0x1.99999ap-3f));
}
fn verse_haunt_bloom_share(p_peak: f32, p_glow: f32) -> f32 {
  var l_peak: f32 = p_peak;
  var l_glow: f32 = p_glow;
  var l_own: f32;
  var l_past: f32;
  l_own = coax_p_hl_min(0x1.ccccccp-1f, (coax_p_hl_max(0.0f, l_glow) * 0x1.99999ap-1f));
  l_past = coax_p_hl_min(0x1.000000p0f, (coax_p_hl_max(0.0f, (l_peak - 0x1.000000p0f)) / (l_peak + 0x1.000000p-1f)));
  return (l_own + (((0x1.000000p0f - l_own) * l_past) * 0x1.333334p-1f));
}
fn verse_haunt_glow(p_light: f32, p_near: f32, p_far: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_near: f32 = p_near;
  var l_far: f32 = p_far;
  return ((l_light + (l_near * 0x1.333334p0f)) + (l_far * 0x1.19999ap0f));
}
fn coax_p_hl_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hl_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
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

// ---- generated: verse/look.coax (verse_outline_depth, verse_outline_crease, verse_outline_strength, verse_outline_ink) by coax wgsl; do not edit
fn verse_outline_depth(p_lap: f32, p_depth: f32) -> f32 {
  var l_lap: f32 = p_lap;
  var l_depth: f32 = p_depth;
  var l_jump: f32;
  l_jump = (abs(l_lap) / coax_p_lk_max((0x1.000000p0f - l_depth), 0x1.0c6f7ap-20f));
  return coax_p_lk_ramp(0x1.47ae14p-5f, 0x1.eb851ep-4f, l_jump);
}
fn verse_outline_crease(p_facing: f32) -> f32 {
  var l_facing: f32 = p_facing;
  return coax_p_lk_ramp(0x1.800000p-1f, 0x1.19999ap-1f, l_facing);
}
fn verse_outline_strength(p_cut: f32, p_glow: f32) -> f32 {
  var l_cut: f32 = p_cut;
  var l_glow: f32 = p_glow;
  return (coax_p_lk_max(0.0f, (0x1.000000p0f - coax_p_lk_max(l_cut, 0.0f))) * (0x1.000000p0f - coax_p_lk_ramp(0x1.000000p-2f, 0x1.333334p-1f, l_glow)));
}
fn verse_outline_ink(p_light: f32, p_edge: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_edge: f32 = p_edge;
  return (l_light * (0x1.000000p0f - (0x1.a3d70ap-1f * coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, l_edge)))));
}
fn coax_p_lk_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_lk_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
// ---- end generated
