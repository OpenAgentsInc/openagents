// A realm someone built: its three passes (myrealm.coax). The sky and the ground behind (the sky by
// the realm's time of day, the ground by its kind, where each pixel's ray meets it, the water where
// it has some, lit by the sun and, after dark, the lamp you carry; the plates over both), the pack's
// meshes (scene_vs, scene_fs: lit by the same sun and lamp, fading into the haze) and the strokes
// over all of it. Low poly keeps this file's own sky and light. A painterly or flat realm reads its
// style block (verse/style-preset.coax, at its time of day; docs/verse/art-styles.md): painterly
// in soft toon bands with cool shadows, a rim, haze, a painted sky with clouds and a tone curve;
// flat in three hard tones of a few flat colours, a flat sky and one soft fade (the style
// functions are verse/style.coax's, generated below by build.sh). Forked from droprealm.wgsl;
// every place, size and colour comes from myrealm.coax.
struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, pad0: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,  // the view: clip = rows · world
  lamp: vec4f,                                 // the lamp: where (metres), strength
  eye: vec4f,                                  // the eye (metres), then the floor's radius
  fwd: vec4f, rx: vec4f, uy: vec4f,            // a pixel's ray: fwd + ndc.x · rx + ndc.y · uy; fwd.w 1: orthographic
                                               // (2D: every ray is fwd, from eye + ndc.x · rx + ndc.y · uy); rx.w: metres before haze
  sky: vec4f,                                  // the sky's top, then 1 for painterly
  horizon: vec4f,                              // the horizon, then the stars' share
  ground: vec4f,                               // the ground's colour, then the lamp's share
  sun: vec4f,                                  // toward the sun, then its strength
  water: vec4f,                                // the water: x, z, radius, on
  panel: array<vec4f, 48>,  // per plate (myrealm.coax PLATES): x0, y0, x1, y1 (device pixels), then scale, alpha, look, 0
  pool: array<vec4f, 16>,   // the lanterns' lights (myrealm.coax POOL_AT): x, y, z (metres), on
  pool_tint: vec4f,         // their colour, then their strength (the time of day's share)
  st: array<vec4f, 32>,     // the style block (myrealm.coax STYLE_AT; verse/style-preset.coax SU_*): 0 for low poly
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
// The lanterns' light on what they reach: a warm pool round each post after dark.
fn lanterns_near(p: vec3f, n: vec3f) -> vec3f {
  if (u.pool_tint.w <= 0.0) { return vec3f(0.0); }
  var sum = 0.0;
  for (var i = 0; i < 16; i++) {
    let l = u.pool[i];
    if (l.w < 0.5) { break; }
    let to = l.xyz - p;
    let d = length(to);
    sum += (0.25 + 0.75 * max(0.0, dot(n, to / max(d, 0.001)))) / (1.0 + 0.55 * d * d);
  }
  return u.pool_tint.rgb * sum * u.pool_tint.w;
}
fn noise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let w = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2f(1.0, 0.0)), w.x), mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), w.x), w.y);
}
// ---- the style block: SU_* rows (verse/style-preset.coax); st(31).y the style: 1 painterly, 3 flat
fn st(i: u32) -> vec4f { return u.st[i]; }
fn styled() -> bool { return u.st[31].y > 0.5; }
fn flat_style() -> bool { return u.st[31].y > 2.5; }
fn key_l() -> vec3f { return normalize(st(1u).xyz + vec3f(0.0, 0.00001, 0.0)); }
// The painted or flat sky along d (fixture.wgsl's painterly sky; fairy.wgsl's flat one), stars after dark.
fn sky_styled(d: vec3f, p: vec2f) -> vec3f {
  let up = d.y; let l = key_l(); let ct = dot(d, l);
  let moon = st(1u).w > 0.5;
  if (flat_style()) {
    var c = mix(st(12u).xyz, st(13u).xyz, verse_style_flat_sky(up));
    if (ct > cos(st(14u).x)) { c = st(17u).xyz * select(1.1, 0.85, moon); }
    return c + st(17u).xyz * stars(p) * st(14u).w * smoothstep(0.0, 0.2, up);
  }
  var c = mix(st(13u).xyz, st(12u).xyz, verse_style_sky_t(up));
  c = mix(c, st(10u).xyz, verse_style_sky_band(up, st(12u).w));
  let disc = smoothstep(cos(st(14u).x * 1.15), cos(st(14u).x * 0.85), ct);
  let halo = verse_style_halo(ct, st(14u).y) * 0.5 + verse_style_halo(ct, st(14u).z) * 0.25;
  if (moon) { c += st(0u).xyz * halo * 0.5; }
  else { c = mix(c, st(11u).xyz * 1.5, clamp(halo * 1.1, 0.0, 0.9)); }
  c = mix(c, select(vec3f(1.0, 0.97, 0.88) * 4.0, vec3f(0.85, 0.88, 1.0) * 1.6, moon), disc);
  let cl = st(15u); let c2 = st(16u);
  if (up > 0.01 && cl.x > 0.0) {
    let t = select(u.time, 0.0, u.still > 0.5);
    let q = d.xz / up * cl.y + st(25u).xy * cl.z * t;
    let n = verse_style_cloud_field(q.x, q.y, c2.x, c2.y, cl.w);
    let cover = verse_style_cloud_cover(n, cl.x, c2.z, c2.w);
    let toward = normalize(l.xz + vec2f(0.0001)) * 260.0;
    let lit = verse_style_cloud_lit(n, verse_style_cloud_field(q.x + toward.x, q.y + toward.y, c2.x, c2.y, cl.w));
    let night = st(14u).w;
    let cloud = mix(st(4u).xyz * st(17u).w * 0.9, st(17u).xyz, lit) * max(st(0u).w, 0.3) * (1.0 - 0.75 * night);
    c = mix(c, cloud, cover * verse_style_cloud_fade(up) * (1.0 - 0.5 * night));
  }
  return c + vec3f(stars(p)) * st(14u).w * smoothstep(0.0, 0.25, up);
}
// A painterly surface (fixture.wgsl's shade, without its shadow map): the albedo under the tinted,
// cooled ambient in the shadow band and under the key in the lit ones, and the rim of the key's light.
fn painted(a: vec3f, n: vec3f, d: vec3f) -> vec3f {
  let l = key_l();
  let ndl = dot(n, l);
  let band = verse_style_bands(verse_style_wrap(ndl), st(7u).x, st(7u).y, st(7u).z, st(7u).w);
  let amb = vec3f(verse_style_ambient(n.y, st(5u).x, st(6u).x, st(5u).w), verse_style_ambient(n.y, st(5u).y, st(6u).y, st(5u).w), verse_style_ambient(n.y, st(5u).z, st(6u).z, st(5u).w));
  let s = a * st(4u).xyz * amb;
  let t = st(4u).xyz;
  let tl = verse_style_luma(t.x, t.y, t.z);
  let k = st(21u).w;
  let m = vec3f(verse_style_cool(s.x, t.x, tl, k), verse_style_cool(s.y, t.y, tl, k), verse_style_cool(s.z, t.z, tl, k));
  let y = verse_style_luma(s.x, s.y, s.z); let ym = verse_style_luma(m.x, m.y, m.z);
  let cool = vec3f(verse_style_keep_value(m.x, y, ym), verse_style_keep_value(m.y, y, ym), verse_style_keep_value(m.z, y, ym));
  let lit = a * (st(0u).xyz * st(0u).w + amb);
  var c = vec3f(verse_style_banded(cool.x, lit.x, band), verse_style_banded(cool.y, lit.y, band), verse_style_banded(cool.z, lit.z, band));
  c += st(2u).xyz * verse_style_rim(dot(n, -d), ndl, st(3u).y, st(3u).z, st(2u).w);
  return c;
}
// A flat surface (fairy.wgsl's): three hard tones between the palette's shade and lit colours.
fn flat_lit(a: vec3f, n: vec3f) -> vec3f {
  let band = verse_style_flat_band(verse_style_flat_light(dot(n, key_l()), 1.0));
  return a * mix(st(4u).xyz, st(0u).xyz * st(0u).w, band);
}
// The lamp and the lanterns on a styled surface: soft for painterly, in two flat steps for flat.
fn styled_lights(a: vec3f, p: vec3f, n: vec3f) -> vec3f {
  let lamp = vec3f(1.0, 0.62, 0.28) * lamp_near(p, n) * 1.6 * u.ground.w;
  let pools = lanterns_near(p, n) * 1.4;
  if (flat_style()) {
    let w = lamp + pools;
    return (a * 2.2 + vec3f(0.04)) * floor(clamp(w * 2.0, vec3f(0.0), vec3f(2.0))) * 0.5;
  }
  return (a * 2.2 + vec3f(0.04)) * (lamp + pools);   // linear light: the pools read as warm light, not just a tint
}
// Distance: painterly haze (its height falloff, its hue, the key's glow through it); flat's one soft fade.
fn styled_far(c: vec3f, p: vec3f, dist: f32, d: vec3f) -> vec3f {
  let hz = st(9u);
  if (flat_style()) { return mix(c, st(13u).xyz, verse_style_flat_fade(max(dist - u.rx.w, 0.0), hz.x, 1.0 / max(hz.y, 0.0001), hz.z)); }
  let f = verse_style_haze(max(dist - u.rx.w, 0.0), p.y, 0.0, hz.x, hz.y, hz.z, hz.w);
  let hz3 = st(10u).xyz;
  let hue = vec3f(verse_style_haze_hue(hz3.x, 0.0, f, hz.z), verse_style_haze_hue(hz3.y, 1.0, f, hz.z), verse_style_haze_hue(hz3.z, 2.0, f, hz.z));
  let hc = mix(hue, st(11u).xyz, verse_style_haze_glow(dot(d, key_l())));
  return vec3f(verse_style_fog(c.x, hc.x, f), verse_style_fog(c.y, hc.y, f), verse_style_fog(c.z, hc.z, f));
}
// Painterly's tone curve, saturation and black lift (none for flat: the palette shows as it is).
fn tone(c: vec3f) -> vec3f {
  if (st(18u).w < 0.5) { return c; }
  let e = st(18u).x;
  var t = vec3f(verse_style_tone(c.x, e), verse_style_tone(c.y, e), verse_style_tone(c.z, e));
  let y = verse_style_luma(t.x, t.y, t.z);
  t = vec3f(verse_style_saturation(t.x, y, st(18u).y), verse_style_saturation(t.y, y, st(18u).y), verse_style_saturation(t.z, y, st(18u).y));
  return vec3f(verse_style_lift(t.x, st(18u).z), verse_style_lift(t.y, st(18u).z), verse_style_lift(t.z, st(18u).z));
}
// A styled realm's colour is linear light (the preset's numbers, the sheets' samples): toned, then
// encoded for the screen (fixture.wgsl's post).
fn encoded(c: vec3f) -> vec3f { return pow(clamp(tone(c), vec3f(0.0), vec3f(1.0)), vec3f(1.0 / 2.2)); }
// A styled realm's ground at hit (seen along ray from t away): brushed for painterly, broad flat
// patches for flat; the water in the preset's bands by depth (painterly mirrors the sky at a glance).
fn styled_ground(hit: vec3f, ray: vec3f, t: f32, p: vec2f) -> vec3f {
  let up = vec3f(0.0, 1.0, 0.0);
  let d = normalize(ray);
  var a = pow(u.ground.rgb, vec3f(2.2));   // myrealm.coax's ground colours are the screen's
  if (flat_style()) { a *= select(1.0, 0.88, noise(hit.xz * 0.07) > 0.58); }
  else {
    let big = noise(hit.xz * 0.18); let brush = noise(vec2f(hit.x * 0.9 + hit.z * 0.4, hit.z * 3.2));
    a *= 0.8 + 0.18 * big + 0.12 * brush;
  }
  var c = select(painted(a, up, d), flat_lit(a, up), flat_style());
  let wd = length(hit.xz - u.water.xy);
  if (u.water.w > 0.5 && wd < u.water.z) {
    let depth = (u.water.z - wd) * 0.5;
    let sh = st(27u); let md = st(28u); let dp = st(29u);
    let soft = max(dp.w, 0.01);
    var w = mix(mix(sh.xyz, md.xyz, smoothstep(sh.w - soft, sh.w + soft, depth)), dp.xyz, smoothstep(md.w - soft, md.w + soft, depth));
    if (flat_style()) {
      w = flat_lit(select(md.xyz, sh.xyz, depth < sh.w), up);
      w = mix(w, st(17u).xyz, 1.0 - step(st(30u).w, u.water.z - wd));   // the flat shore line
    } else {
      let rip = noise(hit.xz * 2.4 + vec2f(u.time * 0.3, u.time * 0.2)) - 0.5;
      w = painted(w, up, d);
      let f = 0.02 + 0.98 * pow(1.0 - max(-d.y, 0.0), 5.0);
      w = mix(w, sky_styled(vec3f(d.x + rip * 0.04, -d.y, d.z + rip * 0.04), p), clamp(f + 0.15, 0.0, 1.0));
    }
    c = w;
  }
  c += styled_lights(a, hit, up);
  return styled_far(c, hit, t, d);
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
  let off = ndc.x * u.rx.xyz + ndc.y * u.uy.xyz;
  let ortho = u.fwd.w > 0.5;
  let ray = select(u.fwd.xyz + off, u.fwd.xyz, ortho);
  let org = select(u.eye.xyz, u.eye.xyz + off, ortho);
  let down = ray.y < -0.0001;
  let t = select(1000.0, -org.y / min(ray.y, -0.0001), down);
  let hit = org + ray * min(t, 1000.0);
  let r = length(hit.xz);
  if (styled()) {
    let sky_s = sky_styled(normalize(ray), p);
    let edge_s = select(0.0, 1.0 - smoothstep(u.eye.w - 3.0, u.eye.w + 6.0, r), down);
    let far = st(13u).xyz;
    var cs = sky_s;
    if (down) { cs = mix(far, styled_ground(hit, ray, t * length(ray), p), edge_s); }
    return vec4f(panels(p, encoded(cs) * u.fade), 1.0);
  }
  let sky = sky_of(ray, p);
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
  ground += lanterns_near(hit, vec3f(0.0, 1.0, 0.0)) * 0.9;
  // the realm's edge fades into the horizon's haze
  let haze = clamp(1.0 - exp(-max(t - u.rx.w, 0.0) * 0.018), 0.0, 1.0);
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
  // painterly leaves are blobs: their cutout from a nearer mip, so pinholes of sky close up
  let cut = select(tex.a, textureSampleLevel(sheets, sheet, s.uv, s.slice, 2.0).a, u.st[31].y > 0.5 && u.st[31].y < 1.5);
  if (cut < 0.35) { discard; }
  let n = normalize(s.normal);
  if (styled()) {
    let d = normalize(s.world - u.eye.xyz);
    let dist = length(s.world - u.eye.xyz);
    // painted: the texture's grain softened toward a nearer mip; flat: large fills of colour, the
    // texture's own mean (a far mip) more than its grain
    let soft = textureSampleLevel(sheets, sheet, s.uv, s.slice, select(2.5, 5.0, flat_style())).rgb;
    let a = mix(tex.rgb, soft, select(0.55, 0.8, flat_style())) * s.tint;
    var c = select(painted(a, n, d), flat_lit(a, n), flat_style()) + styled_lights(a, s.world, n);
    c = styled_far(c, s.world, dist, d);
    return vec4f(panels(s.pos.xy, encoded(c) * u.fade), 1.0);
  }
  var lit = daylight(n);
  lit += vec3f(1.0, 0.62, 0.28) * lamp_near(s.world, n) * 1.6 * u.ground.w;
  lit += lanterns_near(s.world, n) * 1.4;
  let far = max(length(s.world - u.eye.xyz) - u.rx.w, 0.0);
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
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter, verse_style_flat_light, verse_style_flat_band, verse_style_flat_band2, verse_style_flat_sky, verse_style_flat_shadow, verse_style_flat_fade, verse_style_bayer, verse_style_flat_dither, verse_style_flat_hem, verse_style_flat_srgb, verse_style_flat_colour, verse_style_flat_time) by coax/web/wgsl.py; do not edit
fn coax_p_st_min(l_a: f32, l_b: f32) -> f32 {
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_max(l_a: f32, l_b: f32) -> f32 {
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_sat(l_x: f32) -> f32 {
  return coax_p_st_min(1.0, coax_p_st_max(0.0, l_x));
}
fn verse_style_wrap(l_ndl: f32) -> f32 {
  return coax_p_st_sat(((l_ndl + 0.25) / 1.25));
}
fn verse_style_soft_step(l_e: f32, l_s: f32, l_x: f32) -> f32 {
  var coax_t0: f32;
  var coax_t1: f32;
  coax_t0 = (l_x - (l_e - l_s));
  coax_t1 = coax_p_st_max((2.0 * l_s), 9.99999997e-07);
  var l_t: f32 = coax_p_st_sat((coax_t0 / coax_t1));
  return ((l_t * l_t) * (3.0 - (2.0 * l_t)));
}
fn verse_style_bands(l_w: f32, l_e0: f32, l_e1: f32, l_s: f32, l_mid: f32) -> f32 {
  var coax_t2: f32;
  var coax_t3: f32;
  var coax_t4: f32;
  var coax_t5: f32;
  coax_t2 = (l_mid * verse_style_soft_step(l_e0, l_s, l_w));
  coax_t4 = (1.0 - l_mid);
  coax_t5 = verse_style_soft_step(l_e1, l_s, l_w);
  coax_t3 = (coax_t4 * coax_t5);
  return (coax_t2 + coax_t3);
}
fn verse_style_bands2(l_w: f32, l_e: f32, l_s: f32) -> f32 {
  return verse_style_soft_step(l_e, l_s, l_w);
}
fn verse_style_shade(l_albedo: f32, l_shadow_light: f32, l_lit_light: f32, l_band: f32) -> f32 {
  var coax_t6: f32;
  var coax_t7: f32;
  coax_t6 = (l_lit_light - l_shadow_light);
  coax_t7 = coax_p_st_sat(l_band);
  return (l_albedo * (l_shadow_light + (coax_t6 * coax_t7)));
}
fn verse_style_cool(l_c: f32, l_tint: f32, l_tint_lum: f32, l_amount: f32) -> f32 {
  var coax_t8: f32;
  var coax_t9: f32;
  var coax_t10: f32;
  var coax_t11: f32;
  coax_t9 = coax_p_st_max(l_tint, 9.99999997e-07);
  coax_t10 = coax_p_st_max(l_tint_lum, 9.99999997e-07);
  coax_t8 = (coax_t9 / coax_t10);
  coax_t11 = coax_p_st_max(l_amount, 0.0);
  return (l_c * pow(coax_t8, coax_t11));
}
fn verse_style_keep_value(l_c: f32, l_before: f32, l_after: f32) -> f32 {
  var coax_t12: f32;
  var coax_t13: f32;
  coax_t12 = (l_c * l_before);
  coax_t13 = coax_p_st_max(l_after, 9.99999997e-07);
  return (coax_t12 / coax_t13);
}
fn verse_style_banded(l_shadow_c: f32, l_lit_c: f32, l_band: f32) -> f32 {
  var coax_t14: f32;
  var coax_t15: f32;
  coax_t14 = (l_lit_c - l_shadow_c);
  coax_t15 = coax_p_st_sat(l_band);
  return (l_shadow_c + (coax_t14 * coax_t15));
}
fn verse_style_ambient(l_ny: f32, l_sky: f32, l_ground: f32, l_scale: f32) -> f32 {
  var coax_t16: f32;
  var coax_t17: f32;
  coax_t16 = (l_sky - l_ground);
  coax_t17 = coax_p_st_sat(((l_ny * 0.5) + 0.5));
  return ((l_ground + (coax_t16 * coax_t17)) * l_scale);
}
fn verse_style_dab(l_rdl: f32, l_cut: f32, l_strength: f32) -> f32 {
  return select(0.0, l_strength, (l_rdl >= l_cut));
}
fn verse_style_rim(l_ndv: f32, l_ndl: f32, l_start: f32, l_full: f32, l_strength: f32) -> f32 {
  var l_t: f32 = coax_p_st_sat(((l_ndv - l_start) / (l_full - l_start)));
  var l_edge: f32 = ((l_t * l_t) * (3.0 - (2.0 * l_t)));
  return ((l_edge * (0.349999994 + (0.649999976 * coax_p_st_sat((l_ndl + 0.300000012))))) * l_strength);
}
fn verse_style_haze(l_d: f32, l_y: f32, l_base_y: f32, l_start: f32, l_density: f32, l_cap: f32, l_falloff: f32) -> f32 {
  var coax_t20: f32;
  var coax_t21: f32;
  var coax_t22: f32;
  var coax_t23: f32;
  coax_t20 = (-l_density);
  coax_t21 = coax_p_st_max((l_d - l_start), 0.0);
  var l_dist: f32 = (1.0 - exp((coax_t20 * coax_t21)));
  coax_t22 = (-coax_p_st_max((l_y - l_base_y), 0.0));
  coax_t23 = coax_p_st_max(l_falloff, 0.00100000005);
  var l_height: f32 = exp((coax_t22 / coax_t23));
  return coax_p_st_min((l_dist * (1.0 + ((l_height - 1.0) * 0.600000024))), l_cap);
}
fn verse_style_haze_glow(l_vdl: f32) -> f32 {
  var l_a: f32 = coax_p_st_sat(l_vdl);
  var l_a2: f32 = (l_a * l_a);
  var l_a4: f32 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0.449999988);
}
fn verse_style_haze_teal(l_ch: f32) -> f32 {
  if ((l_ch < 0.5))
  {
    return 0.699999988;
  }
  if ((l_ch < 1.5))
  {
    return 1.0;
  }
  return 0.839999974;
}
fn verse_style_haze_hue(l_h: f32, l_ch: f32, l_f: f32, l_cap: f32) -> f32 {
  var l_t: f32 = verse_style_haze_teal(l_ch);
  var l_far: f32 = verse_style_soft_step(((0.150000006 + l_cap) * 0.5), ((l_cap - 0.150000006) * 0.5), l_f);
  return (l_h * (l_t + ((1.0 - l_t) * l_far)));
}
fn verse_style_fog(l_c: f32, l_haze_c: f32, l_f: f32) -> f32 {
  var coax_t24: f32;
  var coax_t25: f32;
  coax_t24 = (l_haze_c - l_c);
  coax_t25 = coax_p_st_sat(l_f);
  return (l_c + (coax_t24 * coax_t25));
}
fn verse_style_sky_t(l_up: f32) -> f32 {
  return pow(coax_p_st_max(l_up, 9.99999997e-07), 0.550000012);
}
fn verse_style_sky_band(l_up: f32, l_lift: f32) -> f32 {
  return (l_lift * (1.0 - coax_p_st_sat((abs(l_up) / 0.104500003))));
}
fn verse_style_halo(l_cos_t: f32, l_k: f32) -> f32 {
  var coax_t26: f32;
  var coax_t27: f32;
  coax_t26 = (-l_k);
  coax_t27 = coax_p_st_max((1.0 - l_cos_t), 0.0);
  return exp((coax_t26 * coax_t27));
}
fn verse_style_hash(l_x: f32, l_y: f32) -> f32 {
  var l_v: f32 = (sin(((l_x * 127.099998) + (l_y * 311.700012))) * 43758.5469);
  return (l_v - floor(l_v));
}
fn verse_style_noise(l_x: f32, l_y: f32) -> f32 {
  var l_ix: f32 = floor(l_x);
  var l_iy: f32 = floor(l_y);
  var l_fx: f32 = (l_x - l_ix);
  var l_fy: f32 = (l_y - l_iy);
  var l_sx: f32 = ((l_fx * l_fx) * (3.0 - (2.0 * l_fx)));
  var l_sy: f32 = ((l_fy * l_fy) * (3.0 - (2.0 * l_fy)));
  var l_a: f32 = verse_style_hash(l_ix, l_iy);
  var l_b: f32 = verse_style_hash((l_ix + 1.0), l_iy);
  var l_c: f32 = verse_style_hash(l_ix, (l_iy + 1.0));
  var l_d: f32 = verse_style_hash((l_ix + 1.0), (l_iy + 1.0));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn verse_style_cloud_field(l_x: f32, l_z: f32, l_s1: f32, l_s2: f32, l_octaves: f32) -> f32 {
  var coax_t28: f32;
  var coax_t29: f32;
  var l_n: f32 = verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 1.5))
  {
    return l_n;
  }
  coax_t28 = (l_n * 0.649999976);
  coax_t29 = (verse_style_noise(((l_x * l_s2) + 3.0999999), ((l_z * l_s2) + 7.69999981)) * 0.349999994);
  return (coax_t28 + coax_t29);
}
fn verse_style_cloud_cover(l_n: f32, l_cover: f32, l_cut_lo: f32, l_cut_hi: f32) -> f32 {
  var coax_t30: f32;
  var coax_t31: f32;
  coax_t30 = verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0.5), ((l_cut_hi - l_cut_lo) * 0.5), (l_n + ((l_cover - 0.349999994) * 0.400000006)));
  coax_t31 = verse_style_soft_step(0.0250000004, 0.0250000004, l_cover);
  return (coax_t30 * coax_t31);
}
fn verse_style_cloud_lit(l_n: f32, l_toward: f32) -> f32 {
  return coax_p_st_sat((((l_n - l_toward) * 5.0) + 0.550000012));
}
fn verse_style_cloud_fade(l_up: f32) -> f32 {
  return (verse_style_soft_step(0.0850000009, 0.075000003, l_up) * 0.920000017);
}
fn verse_style_tone(l_c: f32, l_exposure: f32) -> f32 {
  return (1.0 - exp(((-coax_p_st_max(l_c, 0.0)) * l_exposure)));
}
fn verse_style_saturation(l_c: f32, l_lum: f32, l_amount: f32) -> f32 {
  return coax_p_st_max(0.0, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn verse_style_lift(l_c: f32, l_lift: f32) -> f32 {
  return (l_lift + ((1.0 - l_lift) * l_c));
}
fn verse_style_luma(l_r: f32, l_g: f32, l_b: f32) -> f32 {
  return (((0.212599993 * l_r) + (0.715200007 * l_g)) + (0.0722000003 * l_b));
}
fn verse_style_bloom_share(l_lum: f32, l_threshold: f32, l_knee: f32) -> f32 {
  var coax_t40: f32;
  var coax_t41: f32;
  if ((l_lum <= 0.0))
  {
    return 0.0;
  }
  var l_k: f32 = coax_p_st_max(l_knee, 9.99999975e-06);
  coax_t40 = coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0);
  coax_t41 = (2.0 * l_k);
  var l_q: f32 = coax_p_st_min(coax_t40, coax_t41);
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (4.0 * l_k)), (l_lum - l_threshold)) / l_lum), 1.0);
}
fn verse_style_bloom_mix(l_c: f32, l_bloom: f32, l_amount: f32) -> f32 {
  return (l_c + (l_bloom * l_amount));
}
fn verse_style_shadow_reach(l_on_phone: f32) -> f32 {
  if ((l_on_phone > 0.5))
  {
    return 40.0;
  }
  return 60.0;
}
fn verse_style_shadow_snap(l_x: f32, l_reach: f32, l_size: f32) -> f32 {
  var coax_t18: f32;
  var coax_t19: f32;
  coax_t18 = (2.0 * l_reach);
  coax_t19 = coax_p_st_max(l_size, 1.0);
  var l_texel: f32 = (coax_t18 / coax_t19);
  return (floor(((l_x / l_texel) + 0.5)) * l_texel);
}
fn verse_style_shadow_across(l_x: f32, l_reach: f32) -> f32 {
  return (l_x / coax_p_st_max(l_reach, 0.00100000005));
}
fn verse_style_shadow_depth(l_along: f32, l_reach: f32) -> f32 {
  return (0.5 - (l_along / (4.0 * coax_p_st_max(l_reach, 0.00100000005))));
}
fn verse_style_shadow_taps(l_on_phone: f32) -> f32 {
  if ((l_on_phone > 0.5))
  {
    return 4.0;
  }
  return 9.0;
}
fn verse_style_shadow_radius(l_on_phone: f32) -> f32 {
  return (2.5 + (0.0 * l_on_phone));
}
fn verse_style_shadow_tap(l_k: f32, l_on_phone: f32, l_which: f32) -> f32 {
  var l_x: f32 = 0.0;
  var l_y: f32 = 0.0;
  if ((l_on_phone > 0.5))
  {
    if ((l_k < 0.5))
    {
      l_x = (-0.600000024);
      l_y = (-0.25);
    }
    else
    if ((l_k < 1.5))
    {
      l_x = 0.25;
      l_y = (-0.600000024);
    }
    else
    if ((l_k < 2.5))
    {
      l_x = 0.600000024;
      l_y = 0.25;
    }
    else
    {
      l_x = (-0.25);
      l_y = 0.600000024;
    }
  }
  else
  {
    if ((l_k < 0.5))
    {
      l_x = 0.0;
      l_y = 0.0;
    }
    else
    if ((l_k < 1.5))
    {
      l_x = (-0.326000005);
      l_y = (-0.405999988);
    }
    else
    if ((l_k < 2.5))
    {
      l_x = (-0.839999974);
      l_y = (-0.074000001);
    }
    else
    if ((l_k < 3.5))
    {
      l_x = (-0.69599998);
      l_y = 0.456999987;
    }
    else
    if ((l_k < 4.5))
    {
      l_x = (-0.202999994);
      l_y = 0.620999992;
    }
    else
    if ((l_k < 5.5))
    {
      l_x = 0.962000012;
      l_y = (-0.194999993);
    }
    else
    if ((l_k < 6.5))
    {
      l_x = 0.47299999;
      l_y = (-0.479999989);
    }
    else
    if ((l_k < 7.5))
    {
      l_x = 0.518999994;
      l_y = 0.76700002;
    }
    else
    {
      l_x = 0.185000002;
      l_y = (-0.893000007);
    }
  }
  if ((l_which > 0.5))
  {
    return l_y;
  }
  return l_x;
}
fn verse_style_contact(l_cast_lit: f32, l_blob: f32, l_strength: f32) -> f32 {
  return coax_p_st_min(l_cast_lit, (1.0 - (l_strength * coax_p_st_sat(l_blob))));
}
fn verse_style_shaft_source(l_cos_t: f32) -> f32 {
  var coax_t42: f32;
  var coax_t43: f32;
  coax_t42 = (-24.0);
  coax_t43 = coax_p_st_max((1.0 - l_cos_t), 0.0);
  return exp((coax_t42 * coax_t43));
}
fn verse_style_shaft_fade(l_x: f32, l_y: f32, l_margin: f32) -> f32 {
  var coax_t44: f32;
  var coax_t45: f32;
  coax_t44 = coax_p_st_max((-l_x), (l_x - 1.0));
  coax_t45 = coax_p_st_max((-l_y), (l_y - 1.0));
  var l_out: f32 = coax_p_st_max(coax_p_st_max(coax_t44, coax_t45), 0.0);
  return (1.0 - verse_style_soft_step((l_margin * 0.5), (l_margin * 0.5), l_out));
}
fn verse_style_shaft_tap(l_i: f32, l_decay: f32) -> f32 {
  return pow(coax_p_st_max(l_decay, 9.99999997e-07), l_i);
}
fn verse_style_shaft_total(l_n: f32, l_decay: f32) -> f32 {
  var coax_t46: f32;
  var coax_t47: f32;
  if ((abs((1.0 - l_decay)) < 9.99999997e-07))
  {
    return l_n;
  }
  coax_t46 = (1.0 - pow(coax_p_st_max(l_decay, 9.99999997e-07), l_n));
  coax_t47 = (1.0 - l_decay);
  return (coax_t46 / coax_t47);
}
fn verse_style_shaft(l_sum: f32, l_total: f32, l_weight: f32, l_fade: f32) -> f32 {
  var coax_t48: f32;
  var coax_t49: f32;
  coax_t48 = ((l_sum / coax_p_st_max(l_total, 9.99999997e-07)) * l_weight);
  coax_t49 = coax_p_st_sat(l_fade);
  return (coax_t48 * coax_t49);
}
fn verse_style_gust(l_x: f32, l_z: f32, l_t: f32, l_wx: f32, l_wz: f32, l_speed: f32, l_scale: f32) -> f32 {
  return ((2.0 * verse_style_noise(((l_x - ((l_wx * l_speed) * l_t)) * l_scale), ((l_z - ((l_wz * l_speed) * l_t)) * l_scale))) - 1.0);
}
fn verse_style_wind(l_along: f32, l_gust: f32, l_t: f32, l_amp: f32, l_wavelength: f32, l_speed: f32) -> f32 {
  var coax_t32: f32;
  var coax_t33: f32;
  var l_k: f32 = (6.28318548 / coax_p_st_max(l_wavelength, 0.00100000005));
  coax_t32 = (0.600000024 * sin(((l_k * l_along) - ((l_k * l_speed) * l_t))));
  coax_t33 = (0.400000006 * l_gust);
  return (l_amp * (coax_t32 + coax_t33));
}
fn verse_style_bend(l_theta: f32, l_height: f32, l_share: f32) -> f32 {
  return (((sin(l_theta) * l_height) * l_share) * l_share);
}
fn verse_style_grass_tilt(l_theta: f32, l_amp: f32, l_tilt_most: f32) -> f32 {
  var coax_t34: f32;
  var coax_t35: f32;
  coax_t34 = (-1.0);
  coax_t35 = coax_p_st_min(1.0, (l_theta / coax_p_st_max(l_amp, 9.99999975e-05)));
  return (l_tilt_most * coax_p_st_max(coax_t34, coax_t35));
}
fn verse_style_blade_height(l_h: f32, l_low: f32, l_high: f32) -> f32 {
  var coax_t36: f32;
  var coax_t37: f32;
  coax_t36 = (l_high - l_low);
  coax_t37 = coax_p_st_sat(l_h);
  return (l_low + (coax_t36 * coax_t37));
}
fn verse_style_grass_fade(l_d: f32, l_radius: f32, l_fade: f32) -> f32 {
  return (1.0 - verse_style_soft_step((l_radius - (l_fade * 0.5)), (l_fade * 0.5), l_d));
}
fn verse_style_grass_blades(l_radius: f32, l_density: f32) -> f32 {
  return (((2.0 * l_radius) * l_radius) * l_density);
}
fn verse_style_grass_clear(l_past: f32) -> f32 {
  return verse_style_soft_step(0.125, 0.125, l_past);
}
fn verse_style_segment(l_x: f32, l_z: f32, l_ax: f32, l_az: f32, l_bx: f32, l_bz: f32) -> f32 {
  var coax_t38: f32;
  var coax_t39: f32;
  var l_ux: f32 = (l_bx - l_ax);
  var l_uz: f32 = (l_bz - l_az);
  var l_px: f32 = (l_x - l_ax);
  var l_pz: f32 = (l_z - l_az);
  coax_t38 = ((l_px * l_ux) + (l_pz * l_uz));
  coax_t39 = coax_p_st_max(((l_ux * l_ux) + (l_uz * l_uz)), 9.99999997e-07);
  var l_h: f32 = coax_p_st_sat((coax_t38 / coax_t39));
  var l_dx: f32 = (l_px - (l_ux * l_h));
  var l_dz: f32 = (l_pz - (l_uz * l_h));
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn verse_style_part(l_d: f32, l_radius: f32) -> f32 {
  return coax_p_st_sat((1.0 - (l_d / coax_p_st_max(l_radius, 0.00100000005))));
}
fn verse_style_tip(l_c: f32, l_ch: f32, l_lift: f32) -> f32 {
  if ((l_ch < 0.5))
  {
    return (l_c * (1.0 + (l_lift * 1.29999995)));
  }
  if ((l_ch < 1.5))
  {
    return (l_c * (1.0 + l_lift));
  }
  return l_c;
}
fn verse_style_jitter(l_c: f32, l_ch: f32, l_h: f32, l_jitter: f32) -> f32 {
  var l_j: f32 = (((l_h * 2.0) - 1.0) * l_jitter);
  if ((l_ch < 0.5))
  {
    return (l_c * (1.0 + l_j));
  }
  if ((l_ch < 1.5))
  {
    return l_c;
  }
  return (l_c * (1.0 - l_j));
}
fn verse_style_flat_light(l_fl_ndl: f32, l_fl_lit: f32) -> f32 {
  var coax_t54: f32;
  var coax_t55: f32;
  coax_t54 = coax_p_st_sat(((l_fl_ndl * 0.5) + 0.5));
  coax_t55 = coax_p_st_sat(l_fl_lit);
  return (coax_t54 * coax_t55);
}
fn verse_style_flat_band(l_fl_w: f32) -> f32 {
  return verse_style_bands(l_fl_w, 0.379999995, 0.620000005, 0.00999999978, 0.5);
}
fn verse_style_flat_band2(l_fl_w: f32) -> f32 {
  return verse_style_soft_step(0.449999988, 0.00999999978, l_fl_w);
}
fn verse_style_flat_sky(l_fl_up: f32) -> f32 {
  return (1.0 - verse_style_soft_step(0.119999997, 0.119999997, l_fl_up));
}
fn verse_style_flat_shadow(l_fl_d: f32) -> f32 {
  return (1.0 - verse_style_soft_step(0.850000024, 0.150000006, l_fl_d));
}
fn verse_style_flat_fade(l_fl_d: f32, l_fl_start: f32, l_fl_reach: f32, l_fl_cap: f32) -> f32 {
  var coax_t56: f32;
  var coax_t57: f32;
  coax_t56 = (l_fl_start + (l_fl_reach * 0.5));
  coax_t57 = coax_p_st_max((l_fl_reach * 0.5), 0.00100000005);
  return (l_fl_cap * verse_style_soft_step(coax_t56, coax_t57, l_fl_d));
}
fn coax_p_st_bayer2(l_fl_a: f32, l_fl_b: f32) -> f32 {
  return (((2.0 * l_fl_a) + (3.0 * l_fl_b)) - ((4.0 * l_fl_a) * l_fl_b));
}
fn verse_style_bayer(l_fl_px: f32, l_fl_py: f32) -> f32 {
  var coax_t58: f32;
  var coax_t59: f32;
  var l_fl_x: f32 = (l_fl_px - (4.0 * floor((l_fl_px * 0.25))));
  var l_fl_y: f32 = (l_fl_py - (4.0 * floor((l_fl_py * 0.25))));
  var l_fl_x0: f32 = (l_fl_x - (2.0 * floor((l_fl_x * 0.5))));
  var l_fl_y0: f32 = (l_fl_y - (2.0 * floor((l_fl_y * 0.5))));
  var l_fl_x1: f32 = floor((l_fl_x * 0.5));
  var l_fl_y1: f32 = floor((l_fl_y * 0.5));
  coax_t58 = (4.0 * coax_p_st_bayer2(l_fl_x0, l_fl_y0));
  coax_t59 = coax_p_st_bayer2(l_fl_x1, l_fl_y1);
  return (((coax_t58 + coax_t59) + 0.5) / 16.0);
}
fn verse_style_flat_dither(l_fl_t: f32, l_fl_px: f32, l_fl_py: f32, l_fl_steps: f32) -> f32 {
  var coax_t60: f32;
  var coax_t61: f32;
  var l_fl_n: f32 = coax_p_st_max(l_fl_steps, 1.0);
  coax_t60 = (coax_p_st_sat(l_fl_t) * l_fl_n);
  coax_t61 = verse_style_bayer(l_fl_px, l_fl_py);
  return coax_p_st_sat((floor((coax_t60 + coax_t61)) / l_fl_n));
}
fn verse_style_flat_hem(l_fl_y: f32, l_fl_width: f32, l_fl_aa: f32) -> f32 {
  return (1.0 - verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 9.99999997e-07), l_fl_y));
}
fn coax_p_st_pick(l_fl_ch: f32, l_fl_r: f32, l_fl_g: f32, l_fl_b: f32) -> f32 {
  if ((l_fl_ch < 0.5))
  {
    return l_fl_r;
  }
  if ((l_fl_ch < 1.5))
  {
    return l_fl_g;
  }
  return l_fl_b;
}
fn verse_style_flat_srgb(l_fl_realm: f32, l_fl_slot: f32, l_fl_ch: f32) -> f32 {
  if ((l_fl_realm < 0.5))
  {
    if ((l_fl_slot < 0.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.400000006, 0.463, 0.689999998);
    }
    if ((l_fl_slot < 1.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.745000005, 0.70599997, 0.847000003);
    }
    if ((l_fl_slot < 2.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.442999989, 0.583999991, 0.537);
    }
    if ((l_fl_slot < 3.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.493999988, 0.630999982, 0.564999998);
    }
    if ((l_fl_slot < 4.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.875, 0.819999993, 0.709999979);
    }
    if ((l_fl_slot < 5.5))
    {
      return coax_p_st_pick(l_fl_ch, 1.0, 0.99000001, 0.970000029);
    }
    if ((l_fl_slot < 6.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.660000026, 0.660000026, 0.860000014);
    }
    if ((l_fl_slot < 7.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.600000024, 0.600000024, 0.819999993);
    }
    if ((l_fl_slot < 8.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.980000019, 0.959999979, 0.899999976);
    }
    if ((l_fl_slot < 9.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.203999996, 0.211999997, 0.340999991);
    }
    if ((l_fl_slot < 10.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.964999974, 0.936999977, 0.870999992);
    }
    if ((l_fl_slot < 11.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.270999998, 0.446999997, 0.419999987);
    }
    if ((l_fl_slot < 12.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.455000013, 0.620000005, 0.49000001);
    }
    if ((l_fl_slot < 13.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.579999983, 0.819999993, 0.800000012);
    }
    if ((l_fl_slot < 14.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.689999998, 0.545000017, 0.419999987);
    }
    if ((l_fl_slot < 15.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.980000019, 0.875, 0.579999983);
    }
    if ((l_fl_slot < 16.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.779999971, 0.899999976, 0.850000024);
    }
    if ((l_fl_slot < 17.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.776000023, 0.764999986, 0.866999984);
    }
    if ((l_fl_slot < 18.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.532999992, 0.532999992, 0.722000003);
    }
    return coax_p_st_pick(l_fl_ch, 0.600000024, 0.699999988, 0.620000005);
  }
  if ((l_fl_realm > 1.5))
  {
    if ((l_fl_slot < 0.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.626999974, 0.694000006, 0.827000022);
    }
    if ((l_fl_slot < 1.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.834999979, 0.851000011, 0.921999991);
    }
    if ((l_fl_slot < 2.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.626999974, 0.757000029, 0.493999988);
    }
    if ((l_fl_slot < 3.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.545000017, 0.689999998, 0.458999991);
    }
    if ((l_fl_slot < 4.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.889999986, 0.824000001, 0.736999989);
    }
    if ((l_fl_slot < 5.5))
    {
      return coax_p_st_pick(l_fl_ch, 1.0, 0.984000027, 0.953000009);
    }
    if ((l_fl_slot < 6.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.579999983, 0.588, 0.800000012);
    }
    if ((l_fl_slot < 7.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.600000024, 0.600000024, 0.819999993);
    }
    if ((l_fl_slot < 8.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.991999984, 0.984000027, 0.964999974);
    }
    if ((l_fl_slot < 9.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.165000007, 0.165000007, 0.246999994);
    }
    if ((l_fl_slot < 10.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.961000025, 0.949000001, 0.910000026);
    }
    if ((l_fl_slot < 11.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.372999996, 0.643000007, 0.505999982);
    }
    if ((l_fl_slot < 12.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.596000016, 0.811999977, 0.486000001);
    }
    if ((l_fl_slot < 13.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.501999974, 0.783999979, 0.824000001);
    }
    if ((l_fl_slot < 14.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.681999981, 0.524999976, 0.455000013);
    }
    if ((l_fl_slot < 15.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.957000017, 0.870999992, 0.596000016);
    }
    if ((l_fl_slot < 16.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.783999979, 0.913999975, 0.859000027);
    }
    if ((l_fl_slot < 17.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.698000014, 0.666999996, 0.834999979);
    }
    if ((l_fl_slot < 18.5))
    {
      return coax_p_st_pick(l_fl_ch, 0.824000001, 0.681999981, 0.893999994);
    }
    return coax_p_st_pick(l_fl_ch, 0.917999983, 0.638999999, 0.728999972);
  }
  if ((l_fl_slot < 0.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.662999988, 0.827000022, 0.815999985);
  }
  if ((l_fl_slot < 1.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.910000026, 0.936999977, 0.815999985);
  }
  if ((l_fl_slot < 2.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.624000013, 0.709999979, 0.289999992);
  }
  if ((l_fl_slot < 3.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.501999974, 0.600000024, 0.243000001);
  }
  if ((l_fl_slot < 4.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.851000011, 0.753000021, 0.541000009);
  }
  if ((l_fl_slot < 5.5))
  {
    return coax_p_st_pick(l_fl_ch, 1.0, 0.980000019, 0.920000017);
  }
  if ((l_fl_slot < 6.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.699999988, 0.699999988, 0.839999974);
  }
  if ((l_fl_slot < 7.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.75999999, 0.75, 0.879999995);
  }
  if ((l_fl_slot < 8.5))
  {
    return coax_p_st_pick(l_fl_ch, 1.0, 0.964999974, 0.878000021);
  }
  if ((l_fl_slot < 9.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.114, 0.122000001, 0.165000007);
  }
  if ((l_fl_slot < 10.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.949000001, 0.917999983, 0.824000001);
  }
  if ((l_fl_slot < 11.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.435000002, 0.541000009, 0.226999998);
  }
  if ((l_fl_slot < 12.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.709999979, 0.811999977, 0.352999985);
  }
  if ((l_fl_slot < 13.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.56099999, 0.811999977, 0.753000021);
  }
  if ((l_fl_slot < 14.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.658999979, 0.528999984, 0.352999985);
  }
  if ((l_fl_slot < 15.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.878000021, 0.518000007, 0.226999998);
  }
  if ((l_fl_slot < 16.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.768999994, 0.925000012, 0.838999987);
  }
  if ((l_fl_slot < 17.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.662999988, 0.638999999, 0.768999994);
  }
  if ((l_fl_slot < 18.5))
  {
    return coax_p_st_pick(l_fl_ch, 0.787999988, 0.702000022, 0.878000021);
  }
  return coax_p_st_pick(l_fl_ch, 0.654999971, 0.709999979, 0.603999972);
}
fn verse_style_flat_colour(l_fl_realm: f32, l_fl_slot: f32, l_fl_ch: f32) -> f32 {
  return pow(verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 2.20000005);
}
fn verse_style_flat_time(l_fl_realm: f32, l_fl_ch: f32, l_fl_dusk: f32, l_fl_night: f32) -> f32 {
  var coax_t62: f32;
  var coax_t63: f32;
  var coax_t64: f32;
  var coax_t65: f32;
  if ((l_fl_realm < 0.5))
  {
    return 1.0;
  }
  coax_t62 = (coax_p_st_pick(l_fl_ch, 1.0, 0.800000012, 0.680000007) - 1.0);
  coax_t63 = coax_p_st_sat(l_fl_dusk);
  var l_fl_warm: f32 = (1.0 + (coax_t62 * coax_t63));
  coax_t64 = (coax_p_st_pick(l_fl_ch, 0.200000003, 0.25, 0.439999998) - l_fl_warm);
  coax_t65 = coax_p_st_sat(l_fl_night);
  return (l_fl_warm + (coax_t64 * coax_t65));
}
// ---- end generated
