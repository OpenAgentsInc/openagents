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
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, palette: f32,   // palette: myrealm.coax PALETTES (1 pastel)
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,  // the view: clip = rows · world
  lamp: vec4f,                                 // the lamp: where (metres), strength
  eye: vec4f,                                  // the eye (metres), then the floor's radius
  fwd: vec4f, rx: vec4f, uy: vec4f,            // a pixel's ray: fwd + ndc.x · rx + ndc.y · uy; fwd.w 1: orthographic
                                               // (2D: every ray is fwd, from eye + ndc.x · rx + ndc.y · uy); rx.w: metres before haze
  sky: vec4f,                                  // the sky's top, then 1 for painterly
  horizon: vec4f,                              // the horizon, then the stars' share
  ground: vec4f,                               // the ground's colour, then the lamp's share
  sun: vec4f,                                  // toward the sun, then its strength
  water: vec4f,                                // the water: x, z, radius (a river's half width), then 0 none, 1 a pond,
                                               // 2 a river across the land, 3 a frozen pond, 4 a pool of lava
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
// A pastel realm's textures, lifted pale before their (pastel) tint: wood and hide read soft, not dark.
fn paletted(t: vec3f) -> vec3f { return select(t, t * 0.35 + vec3f(0.62), pal() > 0.5 && pal() < 1.5); }
// u.palette carries the land too (myrealm.coax LAND_*): the palette plus 10 times 1 for a floating
// island (the ground ends at its edge, open sky beyond) or 2 for an island in the sea.
fn land() -> f32 { return floor(u.palette / 10.0 + 0.001); }
fn pal() -> f32 { return u.palette - 10.0 * land(); }
// How far inside the water's edge q is (negative: outside): a pond is a circle, a river a band
// across the land that bends (myrealm.coax water_gap, the same curve).
fn water_in(q: vec2f) -> f32 {
  if (u.water.w > 1.5 && u.water.w < 2.5) { return u.water.z - abs(q.y - u.water.y - 2.5 * sin(q.x * 0.15)); }
  return u.water.z - length(q - u.water.xy);
}
// Ice: pale, a little blue, with dark cracks. Lava: bright and moving, with a dark crust; it glows.
fn ice_at(q: vec2f) -> vec3f {
  let crack = smoothstep(0.47, 0.5, noise(q * 1.3)) * (1.0 - smoothstep(0.5, 0.53, noise(q * 1.3)));
  return mix(vec3f(0.8, 0.88, 0.94), vec3f(0.55, 0.66, 0.75), crack * 0.8 + 0.15 * noise(q * 0.4));
}
fn lava_at(q: vec2f) -> vec3f {
  let flow = noise(q * 0.9 + vec2f(u.time * 0.15, u.time * 0.07));
  let crust = smoothstep(0.55, 0.7, noise(q * 1.7 - vec2f(u.time * 0.05, 0.0)));
  return mix(mix(vec3f(1.0, 0.3, 0.04), vec3f(1.0, 0.78, 0.25), flow), vec3f(0.12, 0.05, 0.04), crust * 0.85);
}
fn sea_at(q: vec2f, mirror: vec3f) -> vec3f {
  return mirror * 0.45 + vec3f(0.03, 0.16, 0.26) * (0.85 + 0.3 * noise(q * 0.5 + vec2f(u.time * 0.1, 0.0)));
}
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
  let wi = water_in(hit.xz);
  if (u.water.w > 0.5 && wi > 0.0) {
    let depth = wi * 0.5;
    let sh = st(27u); let md = st(28u); let dp = st(29u);
    let soft = max(dp.w, 0.01);
    var w = mix(mix(sh.xyz, md.xyz, smoothstep(sh.w - soft, sh.w + soft, depth)), dp.xyz, smoothstep(md.w - soft, md.w + soft, depth));
    if (flat_style()) {
      w = flat_lit(select(md.xyz, sh.xyz, depth < sh.w), up);
      w = mix(w, st(17u).xyz, 1.0 - step(st(30u).w, wi));   // the flat shore line
    } else {
      let rip = noise(hit.xz * 2.4 + vec2f(u.time * 0.3, u.time * 0.2)) - 0.5;
      w = painted(w, up, d);
      let f = 0.02 + 0.98 * pow(1.0 - max(-d.y, 0.0), 5.0);
      w = mix(w, sky_styled(vec3f(d.x + rip * 0.04, -d.y, d.z + rip * 0.04), p), clamp(f + 0.15, 0.0, 1.0));
    }
    if (u.water.w > 2.5 && u.water.w < 3.5) { w = select(painted(pow(ice_at(hit.xz), vec3f(2.2)), up, d), flat_lit(pow(ice_at(hit.xz), vec3f(2.2)), up), flat_style()); }
    if (u.water.w > 3.5) { w = pow(lava_at(hit.xz), vec3f(2.2)) * 1.6; }
    c = w;
  }
  // an island: the sea from the beach out, in the water's own bands
  if (land() > 1.5 && length(hit.xz) > u.eye.w - 3.0) {
    let sea = select(painted(st(28u).xyz, up, d), flat_lit(st(28u).xyz, up), flat_style());
    c = mix(c, sea, smoothstep(u.eye.w - 3.0, u.eye.w - 2.0, length(hit.xz)));
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
  // soft clouds drifting over the day, fewer at dawn and dusk, none at night (the stars' share)
  if (r.y > 0.0) {
    let q = r.xz / (r.y + 0.12) * 1.4 + vec2f(u.time * 0.015, 0.0);
    let n = noise(q) * 0.6 + noise(q * 2.3 + vec2f(7.0)) * 0.3 + noise(q * 5.1 + vec2f(3.0)) * 0.1;
    let cover = smoothstep(0.5, 0.78, n) * smoothstep(0.02, 0.3, r.y) * (1.0 - u.horizon.w);
    c = mix(c, mix(u.horizon.rgb, sun_tint() * 1.05, 0.6), cover * 0.85);
  }
  return c;
}
// The sun's colour by the realm's palette: natural warm white, pastel rosy, vivid golden, muted grey.
fn sun_tint() -> vec3f {
  let k = pal();
  if (k > 0.5 && k < 1.5) { return vec3f(1.0, 0.88, 0.92); }
  if (k > 1.5 && k < 2.5) { return vec3f(1.1, 0.95, 0.72); }
  if (k > 2.5) { return vec3f(0.84, 0.86, 0.9); }
  return vec3f(1.0, 0.94, 0.82);
}
// Sun and sky light on a surface.
fn daylight(n: vec3f) -> vec3f {
  let key = max(0.0, dot(n, normalize(u.sun.xyz)));
  let soft = select(key, key * 0.6 + 0.25, u.sky.w > 0.5);
  let amb = mix(u.horizon.rgb, u.sky.rgb, 0.5 + 0.5 * n.y) * 0.55 + vec3f(0.04);
  return amb + sun_tint() * soft * u.sun.w;
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
    if (down) { cs = mix(far, styled_ground(hit, ray, t * length(ray), p), select(edge_s, 1.0, land() > 1.5)); }
    if (land() > 0.5 && land() < 1.5 && down && r > u.eye.w) { cs = sky_s; }
    return vec4f(panels(p, encoded(cs) * u.fade), 1.0);
  }
  let sky = sky_of(ray, p);
  // the ground: its colour, broken up (brushed when painterly), lit by the sun and your lamp
  let big = noise(hit.xz * 0.18); let fine = noise(hit.xz * 1.7);
  let brush = noise(vec2f(hit.x * 0.9 + hit.z * 0.4, hit.z * 3.2));
  let vary = select(0.82 + 0.22 * big + 0.08 * fine, 0.78 + 0.2 * big + 0.16 * brush, u.sky.w > 0.5);
  // and broad patches of warmer, drier grass and cooler, lusher ground
  let patches = noise(hit.xz * 0.06 + vec2f(3.1, 7.7));
  let hue = mix(vec3f(1.12, 1.06, 0.78), vec3f(0.84, 1.0, 0.98), smoothstep(0.25, 0.75, patches));
  var ground = u.ground.rgb * vary * hue * daylight(vec3f(0.0, 1.0, 0.0));
  // water: the sky in it, rippled, a pale rim where it meets the ground
  let wi = water_in(hit.xz);
  if (u.water.w > 0.5 && wi > 0.0) {
    let rip = noise(hit.xz * 2.4 + vec2f(u.time * 0.3, u.time * 0.2)) - 0.5;
    let mirror = sky_of(vec3f(ray.x + rip * 0.05, -ray.y, ray.z + rip * 0.05), p);
    let deep = smoothstep(0.0, u.water.z * 0.6, wi);
    let water = mix(mirror * 0.75 + vec3f(0.02, 0.05, 0.08), vec3f(0.03, 0.09, 0.14) * daylight(vec3f(0.0, 1.0, 0.0)) + mirror * 0.4, deep * 0.6);
    let rim = 1.0 - smoothstep(0.0, 0.25, wi);
    ground = mix(water, u.ground.rgb * 1.2 * daylight(vec3f(0.0, 1.0, 0.0)), rim * 0.5);
    if (u.water.w > 2.5 && u.water.w < 3.5) { ground = ice_at(hit.xz) * daylight(vec3f(0.0, 1.0, 0.0)) + mirror * 0.12; }
    if (u.water.w > 3.5) { ground = lava_at(hit.xz); }
  }
  ground += vec3f(1.0, 0.62, 0.28) * lamp_near(hit, vec3f(0.0, 1.0, 0.0)) * 0.55 * u.ground.w;
  ground += lanterns_near(hit, vec3f(0.0, 1.0, 0.0)) * 0.9;
  // an island: the sea from the beach out to the horizon
  if (land() > 1.5) {
    let rip = noise(hit.xz * 1.2 + vec2f(u.time * 0.2, u.time * 0.1)) - 0.5;
    let sea = sea_at(hit.xz, sky_of(vec3f(ray.x + rip * 0.05, -ray.y, ray.z + rip * 0.05), p));
    ground = mix(ground, sea, smoothstep(u.eye.w - 3.0, u.eye.w - 2.2, r));
  }
  // a floating island: a band of earth at its edge, and past it nothing but sky
  if (land() > 0.5 && land() < 1.5) {
    ground = mix(ground, vec3f(0.32, 0.22, 0.15) * daylight(vec3f(0.0, 1.0, 0.0)), smoothstep(u.eye.w - 0.9, u.eye.w - 0.4, r));
  }
  // the realm's edge fades into the horizon's haze
  let haze = clamp(1.0 - exp(-max(t - u.rx.w, 0.0) * 0.018), 0.0, 1.0);
  ground = mix(ground, u.horizon.rgb, haze * 0.85);
  var edge = select(0.0, 1.0 - smoothstep(u.eye.w - 3.0, u.eye.w + 6.0, r), down);
  if (land() > 1.5 && down) { edge = 1.0; }
  var c = mix(sky, mix(u.horizon.rgb, ground, edge), select(0.0, 1.0, down)) * u.fade;
  if (land() > 0.5 && land() < 1.5 && down && r > u.eye.w) { c = sky * u.fade; }
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
  @location(5) @interpolate(flat) glow: f32,   // its own light (myrealm-things.coax glowing), or below 0 its paint (paint)
}
// A round piece (myrealm.coax ROUND, drawn from vertex ROUND_BASE on, no index): a round cone from
// end a (r0: its place, its radius) to end b (r1), two half balls and the side between, twelve
// round and three rings to a half (ROUND_VERTICES, 504), as Dead Gears and Hecate's draw their
// figures' bones. Figures, owls, a windmill's tower: what the pack has no round mesh for.
const ROUND_BASE: u32 = 16777216u;
const AROUND: u32 = 12u;
const RINGS: u32 = 3u;
fn quad(v: u32) -> vec2f {
  var q = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  return q[v];
}
struct Vert { p: vec3f, n: vec3f }
fn round_vertex(m: Object, id: u32) -> Vert {
  let q = id / 6u;
  let c = quad(id % 6u);
  let row = q / AROUND + u32(c.y);
  let k = f32(q % AROUND + u32(c.x)) / f32(AROUND) * 6.2831853;
  let a = m.r0.xyz; let b = m.r1.xyz;
  let axis = normalize(b - a + vec3f(0.0, 0.00001, 0.0));
  let side = normalize(cross(axis, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(axis.y) < 0.9)));
  let other = cross(axis, side);
  let out = side * cos(k) + other * sin(k);
  let top = row > RINGS;
  let step = f32(select(row, row - RINGS - 1u, top)) / f32(RINGS) * 1.5707963;
  let phi = select(step - 1.5707963, step, top);
  let n = out * cos(phi) + axis * sin(phi);
  let r = select(m.r0.w, m.r1.w, top);
  return Vert(select(a, b, top) + n * r, n);
}
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  if (id >= ROUND_BASE) {
    let m = objects[instance];
    let v = round_vertex(m, id - ROUND_BASE);
    var o: S;
    o.pos = vec4f(dot(u.r0, vec4f(v.p, 1.0)), dot(u.r1, vec4f(v.p, 1.0)), dot(u.r2, vec4f(v.p, 1.0)), dot(u.r3, vec4f(v.p, 1.0)));
    o.world = v.p; o.normal = v.n; o.uv = vec2f(0.0); o.tint = m.material.rgb * 0.8;
    o.slice = 4096u; o.glow = m.material.w;
    return o;
  }
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
  o.glow = m.material.w;
  return o;
}
// A painted piece: its texture's grain kept as light and dark, its own colour all the tint's.
// Stone in courses: blocks half a metre high and about a metre long, each course offset by half a
// block, each block its own shade, dark mortar between them.
fn masonry(p: vec3f) -> f32 {
  let row = floor(p.y / 0.5);
  let along = (p.x + p.z * 0.83) / 1.05 + 0.5 * (row - 2.0 * floor(row * 0.5));
  let fu = fract(along); let fv = fract(p.y / 0.5);
  let joint = min(min(fu, 1.0 - fu) * 1.05, min(fv, 1.0 - fv) * 0.5);
  let block = 0.78 + 0.2 * hash(vec2f(floor(along), row)) + 0.06 * noise(p.xz * 3.0 + p.yy * 2.0);
  return mix(0.42, block, smoothstep(0.012, 0.035, joint));
}
fn surfaced(t: vec3f, w: f32) -> vec3f { return mix(t, vec3f(0.42 + 0.8 * dot(t, vec3f(0.3, 0.59, 0.11))), clamp(-w, 0.0, 1.0)); }
@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  // a round piece has no texture: its tint, a little lighter toward its top
  let rounded = s.slice >= 4096u;
  let layer = select(s.slice, 0u, rounded);
  let sampled = textureSample(sheets, sheet, s.uv, layer);
  // plain (glow below -2.5, myrealm-things.coax untextured): its tint alone, like a round piece
  let bare = rounded || s.glow < -2.5;
  var tex = select(vec4f(surfaced(sampled.rgb, s.glow), sampled.a), vec4f(1.0), bare);
  // stone (glow below -1.5, myrealm-things.coax stone): laid blocks in courses, whatever the mesh
  if (s.glow < -1.5 && !bare) { tex = vec4f(vec3f(masonry(s.world)), 1.0); }
  // painterly leaves are blobs: their cutout from a nearer mip, so pinholes of sky close up
  let cut = select(select(tex.a, textureSampleLevel(sheets, sheet, s.uv, layer, 2.0).a, u.st[31].y > 0.5 && u.st[31].y < 1.5), 1.0, bare);
  if (cut < 0.35) { discard; }
  // nothing shows below the ground (a round piece's lower half ball, a rock's buried foot)
  if (s.world.y < -0.04 && (length(s.world.xz) < u.eye.w || land() < 0.5 || land() > 1.5)) { discard; }
  let n = normalize(s.normal);
  if (styled()) {
    let d = normalize(s.world - u.eye.xyz);
    let dist = length(s.world - u.eye.xyz);
    // painted: the texture's grain softened toward a nearer mip; flat: large fills of colour, the
    // texture's own mean (a far mip) more than its grain
    let soft = select(surfaced(textureSampleLevel(sheets, sheet, s.uv, s.slice, select(2.5, 5.0, flat_style())).rgb, s.glow), tex.rgb, s.glow < -1.5 || rounded);
    let a = paletted(mix(tex.rgb, soft, select(0.55, 0.8, flat_style()))) * s.tint;
    var c = select(painted(a, n, d), flat_lit(a, n), flat_style()) + styled_lights(a, s.world, n) + a * max(s.glow, 0.0);
    c = styled_far(c, s.world, dist, d);
    return vec4f(panels(s.pos.xy, encoded(c) * u.fade), 1.0);
  }
  var lit = daylight(n);
  lit += vec3f(1.0, 0.62, 0.28) * lamp_near(s.world, n) * 1.6 * u.ground.w;
  lit += lanterns_near(s.world, n) * 1.4 + vec3f(max(s.glow, 0.0));
  let far = max(length(s.world - u.eye.xyz) - u.rx.w, 0.0);
  let haze = clamp(1.0 - exp(-far * 0.018), 0.0, 1.0);
  // a floating island's sky is clear: what floats in it keeps its colour further off
  let thick = select(0.85, 0.4, land() > 0.5 && land() < 1.5);
  let c = mix(paletted(tex.rgb) * s.tint * lit, u.horizon.rgb, haze * thick) * u.fade;
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
// ---- generated: verse/style.coax (verse_style_wrap, verse_style_bands, verse_style_bands2, verse_style_shade, verse_style_cool, verse_style_keep_value, verse_style_banded, verse_style_ambient, verse_style_dab, verse_style_rim, verse_style_haze, verse_style_haze_glow, verse_style_haze_teal, verse_style_haze_hue, verse_style_fog, verse_style_sky_t, verse_style_sky_band, verse_style_halo, verse_style_hash, verse_style_noise, verse_style_cloud_field, verse_style_cloud_cover, verse_style_cloud_lit, verse_style_cloud_fade, verse_style_tone, verse_style_saturation, verse_style_lift, verse_style_luma, verse_style_bloom_share, verse_style_bloom_mix, verse_style_shadow_reach, verse_style_shadow_snap, verse_style_shadow_across, verse_style_shadow_depth, verse_style_shadow_taps, verse_style_shadow_radius, verse_style_shadow_tap, verse_style_contact, verse_style_shaft_source, verse_style_shaft_fade, verse_style_shaft_tap, verse_style_shaft_total, verse_style_shaft, verse_style_gust, verse_style_wind, verse_style_bend, verse_style_grass_tilt, verse_style_blade_height, verse_style_grass_fade, verse_style_grass_blades, verse_style_grass_clear, verse_style_segment, verse_style_part, verse_style_tip, verse_style_jitter, verse_style_flat_light, verse_style_flat_band, verse_style_flat_band2, verse_style_flat_sky, verse_style_flat_shadow, verse_style_flat_fade, verse_style_bayer, verse_style_flat_dither, verse_style_flat_hem, verse_style_flat_srgb, verse_style_flat_colour, verse_style_flat_time) by coax wgsl; do not edit
fn verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn verse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e0: f32 = p_e0;
  var l_e1: f32 = p_e1;
  var l_s: f32 = p_s;
  var l_mid: f32 = p_mid;
  return ((l_mid * coax_p_verse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_verse_style_soft_step(l_e1, l_s, l_w)));
}
fn verse_style_bands2(p_w: f32, p_e: f32, p_s: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  return coax_p_verse_style_soft_step(l_e, l_s, l_w);
}
fn verse_style_shade(p_albedo: f32, p_shadow_light: f32, p_lit_light: f32, p_band: f32) -> f32 {
  var l_albedo: f32 = p_albedo;
  var l_shadow_light: f32 = p_shadow_light;
  var l_lit_light: f32 = p_lit_light;
  var l_band: f32 = p_band;
  return (l_albedo * (l_shadow_light + ((l_lit_light - l_shadow_light) * coax_p_st_sat(l_band))));
}
fn verse_style_cool(p_c: f32, p_tint: f32, p_tint_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_tint: f32 = p_tint;
  var l_tint_lum: f32 = p_tint_lum;
  var l_amount: f32 = p_amount;
  return (l_c * coax_pow_verse_style_wrap((coax_p_st_max(l_tint, 0x1.0c6f7ap-20f) / coax_p_st_max(l_tint_lum, 0x1.0c6f7ap-20f)), coax_p_st_max(l_amount, 0.0f)));
}
fn verse_style_keep_value(p_c: f32, p_before: f32, p_after: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_before: f32 = p_before;
  var l_after: f32 = p_after;
  return ((l_c * l_before) / coax_p_st_max(l_after, 0x1.0c6f7ap-20f));
}
fn verse_style_banded(p_shadow_c: f32, p_lit_c: f32, p_band: f32) -> f32 {
  var l_shadow_c: f32 = p_shadow_c;
  var l_lit_c: f32 = p_lit_c;
  var l_band: f32 = p_band;
  return (l_shadow_c + ((l_lit_c - l_shadow_c) * coax_p_st_sat(l_band)));
}
fn verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn verse_style_dab(p_rdl: f32, p_cut: f32, p_strength: f32) -> f32 {
  var l_rdl: f32 = p_rdl;
  var l_cut: f32 = p_cut;
  var l_strength: f32 = p_strength;
  return select(0.0f, l_strength, (l_rdl >= l_cut));
}
fn verse_style_rim(p_ndv: f32, p_ndl: f32, p_start: f32, p_full: f32, p_strength: f32) -> f32 {
  var l_ndv: f32 = p_ndv;
  var l_ndl: f32 = p_ndl;
  var l_start: f32 = p_start;
  var l_full: f32 = p_full;
  var l_strength: f32 = p_strength;
  var l_t: f32;
  var l_edge: f32;
  l_t = coax_p_st_sat(((l_ndv - l_start) / (l_full - l_start)));
  l_edge = ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
  return ((l_edge * (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_st_sat((l_ndl + 0x1.333334p-2f))))) * l_strength);
}
fn verse_style_haze(p_d: f32, p_y: f32, p_base_y: f32, p_start: f32, p_density: f32, p_cap: f32, p_falloff: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_y: f32 = p_y;
  var l_base_y: f32 = p_base_y;
  var l_start: f32 = p_start;
  var l_density: f32 = p_density;
  var l_cap: f32 = p_cap;
  var l_falloff: f32 = p_falloff;
  var l_dist: f32;
  var l_height: f32;
  l_dist = (0x1.000000p0f - exp(((-l_density) * coax_p_st_max((l_d - l_start), 0.0f))));
  l_height = exp(((-coax_p_st_max((l_y - l_base_y), 0.0f)) / coax_p_st_max(l_falloff, 0x1.0624dep-10f)));
  return coax_p_st_min((l_dist * (0x1.000000p0f + ((l_height - 0x1.000000p0f) * 0x1.333334p-1f))), l_cap);
}
fn verse_style_haze_glow(p_vdl: f32) -> f32 {
  var l_vdl: f32 = p_vdl;
  var l_a: f32;
  var l_a2: f32;
  var l_a4: f32;
  l_a = coax_p_st_sat(l_vdl);
  l_a2 = (l_a * l_a);
  l_a4 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0x1.ccccccp-2f);
}
fn verse_style_haze_teal(p_ch: f32) -> f32 {
  var l_ch: f32 = p_ch;
  if ((l_ch < 0x1.000000p-1f))
  {
    return 0x1.666666p-1f;
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return 0x1.000000p0f;
  }
  return 0x1.ae147ap-1f;
}
fn verse_style_haze_hue(p_h: f32, p_ch: f32, p_f: f32, p_cap: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_ch: f32 = p_ch;
  var l_f: f32 = p_f;
  var l_cap: f32 = p_cap;
  var l_t: f32;
  var l_far: f32;
  l_t = verse_style_haze_teal(l_ch);
  l_far = coax_p_verse_style_soft_step(((0x1.333334p-3f + l_cap) * 0x1.000000p-1f), ((l_cap - 0x1.333334p-3f) * 0x1.000000p-1f), l_f);
  return (l_h * (l_t + ((0x1.000000p0f - l_t) * l_far)));
}
fn verse_style_fog(p_c: f32, p_haze_c: f32, p_f: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_haze_c: f32 = p_haze_c;
  var l_f: f32 = p_f;
  return (l_c + ((l_haze_c - l_c) * coax_p_st_sat(l_f)));
}
fn verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_verse_style_wrap(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn verse_style_sky_band(p_up: f32, p_lift: f32) -> f32 {
  var l_up: f32 = p_up;
  var l_lift: f32 = p_lift;
  return (l_lift * (0x1.000000p0f - coax_p_st_sat((abs(l_up) / 0x1.ac0832p-4f))));
}
fn verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn verse_style_noise(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = verse_style_hash(l_ix, l_iy);
  l_b = verse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = verse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = verse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn verse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_s1: f32 = p_s1;
  var l_s2: f32 = p_s2;
  var l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (verse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn verse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_cover: f32 = p_cover;
  var l_cut_lo: f32 = p_cut_lo;
  var l_cut_hi: f32 = p_cut_hi;
  return (coax_p_verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_verse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn verse_style_cloud_lit(p_n: f32, p_toward: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_toward: f32 = p_toward;
  return coax_p_st_sat((((l_n - l_toward) * 0x1.400000p2f) + 0x1.19999ap-1f));
}
fn verse_style_cloud_fade(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return (coax_p_verse_style_soft_step(0x1.5c28f6p-4f, 0x1.333334p-4f, l_up) * 0x1.d70a3ep-1f);
}
fn verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn verse_style_lift(p_c: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lift: f32 = p_lift;
  return (l_lift + ((0x1.000000p0f - l_lift) * l_c));
}
fn verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn verse_style_bloom_share(p_lum: f32, p_threshold: f32, p_knee: f32) -> f32 {
  var l_lum: f32 = p_lum;
  var l_threshold: f32 = p_threshold;
  var l_knee: f32 = p_knee;
  var l_k: f32;
  var l_q: f32;
  if ((l_lum <= 0.0f))
  {
    return 0.0f;
  }
  l_k = coax_p_st_max(l_knee, 0x1.4f8b58p-17f);
  l_q = coax_p_st_min(coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0f), (0x1.000000p1f * l_k));
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (0x1.000000p2f * l_k)), (l_lum - l_threshold)) / l_lum), 0x1.000000p0f);
}
fn verse_style_bloom_mix(p_c: f32, p_bloom: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_bloom: f32 = p_bloom;
  var l_amount: f32 = p_amount;
  return (l_c + (l_bloom * l_amount));
}
fn verse_style_shadow_reach(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    return 0x1.400000p5f;
  }
  return 0x1.e00000p5f;
}
fn verse_style_shadow_snap(p_x: f32, p_reach: f32, p_size: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  var l_size: f32 = p_size;
  var l_texel: f32;
  l_texel = ((0x1.000000p1f * l_reach) / coax_p_st_max(l_size, 0x1.000000p0f));
  return (floor(((l_x / l_texel) + 0x1.000000p-1f)) * l_texel);
}
fn verse_style_shadow_across(p_x: f32, p_reach: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  return (l_x / coax_p_st_max(l_reach, 0x1.0624dep-10f));
}
fn verse_style_shadow_depth(p_along: f32, p_reach: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_reach: f32 = p_reach;
  return (0x1.000000p-1f - (l_along / (0x1.000000p2f * coax_p_st_max(l_reach, 0x1.0624dep-10f))));
}
fn verse_style_shadow_taps(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    return 0x1.000000p2f;
  }
  return 0x1.200000p3f;
}
fn verse_style_shadow_radius(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  return (0x1.400000p1f + (0.0f * l_on_phone));
}
fn verse_style_shadow_tap(p_k: f32, p_on_phone: f32, p_which: f32) -> f32 {
  var l_k: f32 = p_k;
  var l_on_phone: f32 = p_on_phone;
  var l_which: f32 = p_which;
  var l_x: f32;
  var l_y: f32;
  l_x = 0.0f;
  l_y = 0.0f;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = (-0x1.333334p-1f);
      l_y = (-0x1.000000p-2f);
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = 0x1.000000p-2f;
        l_y = (-0x1.333334p-1f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = 0x1.333334p-1f;
          l_y = 0x1.000000p-2f;
        }
        else
        {
          l_x = (-0x1.000000p-2f);
          l_y = 0x1.333334p-1f;
        }
      }
    }
  }
  else
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = 0.0f;
      l_y = 0.0f;
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = (-0x1.4dd2f2p-2f);
        l_y = (-0x1.9fbe76p-2f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = (-0x1.ae147ap-1f);
          l_y = (-0x1.2f1aa0p-4f);
        }
        else
        {
          if ((l_k < 0x1.c00000p1f))
          {
            l_x = (-0x1.645a1cp-1f);
            l_y = 0x1.d3f7cep-2f;
          }
          else
          {
            if ((l_k < 0x1.200000p2f))
            {
              l_x = (-0x1.9fbe76p-3f);
              l_y = 0x1.3df3b6p-1f;
            }
            else
            {
              if ((l_k < 0x1.600000p2f))
              {
                l_x = 0x1.ec8b44p-1f;
                l_y = (-0x1.8f5c28p-3f);
              }
              else
              {
                if ((l_k < 0x1.a00000p2f))
                {
                  l_x = 0x1.e45a1cp-2f;
                  l_y = (-0x1.eb851ep-2f);
                }
                else
                {
                  if ((l_k < 0x1.e00000p2f))
                  {
                    l_x = 0x1.09ba5ep-1f;
                    l_y = 0x1.88b43ap-1f;
                  }
                  else
                  {
                    l_x = 0x1.7ae148p-3f;
                    l_y = (-0x1.c9374cp-1f);
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if ((l_which > 0x1.000000p-1f))
  {
    return l_y;
  }
  return l_x;
}
fn verse_style_contact(p_cast_lit: f32, p_blob: f32, p_strength: f32) -> f32 {
  var l_cast_lit: f32 = p_cast_lit;
  var l_blob: f32 = p_blob;
  var l_strength: f32 = p_strength;
  return coax_p_st_min(l_cast_lit, (0x1.000000p0f - (l_strength * coax_p_st_sat(l_blob))));
}
fn verse_style_shaft_source(p_cos_t: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  return exp(((-0x1.800000p4f) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn verse_style_shaft_fade(p_x: f32, p_y: f32, p_margin: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_margin: f32 = p_margin;
  var l_out: f32;
  l_out = coax_p_st_max(coax_p_st_max(coax_p_st_max((-l_x), (l_x - 0x1.000000p0f)), coax_p_st_max((-l_y), (l_y - 0x1.000000p0f))), 0.0f);
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_margin * 0x1.000000p-1f), (l_margin * 0x1.000000p-1f), l_out));
}
fn verse_style_shaft_tap(p_i: f32, p_decay: f32) -> f32 {
  var l_i: f32 = p_i;
  var l_decay: f32 = p_decay;
  return coax_pow_verse_style_wrap(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_i);
}
fn verse_style_shaft_total(p_n: f32, p_decay: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_decay: f32 = p_decay;
  if ((abs((0x1.000000p0f - l_decay)) < 0x1.0c6f7ap-20f))
  {
    return l_n;
  }
  return ((0x1.000000p0f - coax_pow_verse_style_wrap(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_n)) / (0x1.000000p0f - l_decay));
}
fn verse_style_shaft(p_sum: f32, p_total: f32, p_weight: f32, p_fade: f32) -> f32 {
  var l_sum: f32 = p_sum;
  var l_total: f32 = p_total;
  var l_weight: f32 = p_weight;
  var l_fade: f32 = p_fade;
  return (((l_sum / coax_p_st_max(l_total, 0x1.0c6f7ap-20f)) * l_weight) * coax_p_st_sat(l_fade));
}
fn verse_style_gust(p_x: f32, p_z: f32, p_t: f32, p_wx: f32, p_wz: f32, p_speed: f32, p_scale: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: f32 = p_t;
  var l_wx: f32 = p_wx;
  var l_wz: f32 = p_wz;
  var l_speed: f32 = p_speed;
  var l_scale: f32 = p_scale;
  return ((0x1.000000p1f * verse_style_noise(((l_x - ((l_wx * l_speed) * l_t)) * l_scale), ((l_z - ((l_wz * l_speed) * l_t)) * l_scale))) - 0x1.000000p0f);
}
fn verse_style_wind(p_along: f32, p_gust: f32, p_t: f32, p_amp: f32, p_wavelength: f32, p_speed: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_gust: f32 = p_gust;
  var l_t: f32 = p_t;
  var l_amp: f32 = p_amp;
  var l_wavelength: f32 = p_wavelength;
  var l_speed: f32 = p_speed;
  var l_k: f32;
  l_k = (0x1.921fb6p2f / coax_p_st_max(l_wavelength, 0x1.0624dep-10f));
  return (l_amp * ((0x1.333334p-1f * sin(((l_k * l_along) - ((l_k * l_speed) * l_t)))) + (0x1.99999ap-2f * l_gust)));
}
fn verse_style_bend(p_theta: f32, p_height: f32, p_share: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_height: f32 = p_height;
  var l_share: f32 = p_share;
  return (((sin(l_theta) * l_height) * l_share) * l_share);
}
fn verse_style_grass_tilt(p_theta: f32, p_amp: f32, p_tilt_most: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_amp: f32 = p_amp;
  var l_tilt_most: f32 = p_tilt_most;
  return (l_tilt_most * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, (l_theta / coax_p_st_max(l_amp, 0x1.a36e2ep-14f)))));
}
fn verse_style_blade_height(p_h: f32, p_low: f32, p_high: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return (l_low + ((l_high - l_low) * coax_p_st_sat(l_h)));
}
fn verse_style_grass_fade(p_d: f32, p_radius: f32, p_fade: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  var l_fade: f32 = p_fade;
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_radius - (l_fade * 0x1.000000p-1f)), (l_fade * 0x1.000000p-1f), l_d));
}
fn verse_style_grass_blades(p_radius: f32, p_density: f32) -> f32 {
  var l_radius: f32 = p_radius;
  var l_density: f32 = p_density;
  return (((0x1.000000p1f * l_radius) * l_radius) * l_density);
}
fn verse_style_grass_clear(p_past: f32) -> f32 {
  var l_past: f32 = p_past;
  return coax_p_verse_style_soft_step(0x1.000000p-3f, 0x1.000000p-3f, l_past);
}
fn verse_style_segment(p_x: f32, p_z: f32, p_ax: f32, p_az: f32, p_bx: f32, p_bz: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ax: f32 = p_ax;
  var l_az: f32 = p_az;
  var l_bx: f32 = p_bx;
  var l_bz: f32 = p_bz;
  var l_ux: f32;
  var l_uz: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_h: f32;
  var l_dx: f32;
  var l_dz: f32;
  l_ux = (l_bx - l_ax);
  l_uz = (l_bz - l_az);
  l_px = (l_x - l_ax);
  l_pz = (l_z - l_az);
  l_h = coax_p_st_sat((((l_px * l_ux) + (l_pz * l_uz)) / coax_p_st_max(((l_ux * l_ux) + (l_uz * l_uz)), 0x1.0c6f7ap-20f)));
  l_dx = (l_px - (l_ux * l_h));
  l_dz = (l_pz - (l_uz * l_h));
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn verse_style_part(p_d: f32, p_radius: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  return coax_p_st_sat((0x1.000000p0f - (l_d / coax_p_st_max(l_radius, 0x1.0624dep-10f))));
}
fn verse_style_tip(p_c: f32, p_ch: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_lift: f32 = p_lift;
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + (l_lift * 0x1.4cccccp0f)));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return (l_c * (0x1.000000p0f + l_lift));
  }
  return l_c;
}
fn verse_style_jitter(p_c: f32, p_ch: f32, p_h: f32, p_jitter: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_h: f32 = p_h;
  var l_jitter: f32 = p_jitter;
  var l_j: f32;
  l_j = (((l_h * 0x1.000000p1f) - 0x1.000000p0f) * l_jitter);
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + l_j));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return l_c;
  }
  return (l_c * (0x1.000000p0f - l_j));
}
fn verse_style_flat_light(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  return (coax_p_st_sat(((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f)) * coax_p_st_sat(l_fl_lit));
}
fn verse_style_flat_band(p_fl_w: f32) -> f32 {
  var l_fl_w: f32 = p_fl_w;
  return verse_style_bands(l_fl_w, 0x1.851eb8p-2f, 0x1.3d70a4p-1f, 0x1.47ae14p-7f, 0x1.000000p-1f);
}
fn verse_style_flat_band2(p_fl_w: f32) -> f32 {
  var l_fl_w: f32 = p_fl_w;
  return coax_p_verse_style_soft_step(0x1.ccccccp-2f, 0x1.47ae14p-7f, l_fl_w);
}
fn verse_style_flat_sky(p_fl_up: f32) -> f32 {
  var l_fl_up: f32 = p_fl_up;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-4f, l_fl_up));
}
fn verse_style_flat_shadow(p_fl_d: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.b33334p-1f, 0x1.333334p-3f, l_fl_d));
}
fn verse_style_flat_fade(p_fl_d: f32, p_fl_start: f32, p_fl_reach: f32, p_fl_cap: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  var l_fl_start: f32 = p_fl_start;
  var l_fl_reach: f32 = p_fl_reach;
  var l_fl_cap: f32 = p_fl_cap;
  return (l_fl_cap * coax_p_verse_style_soft_step((l_fl_start + (l_fl_reach * 0x1.000000p-1f)), coax_p_st_max((l_fl_reach * 0x1.000000p-1f), 0x1.0624dep-10f), l_fl_d));
}
fn verse_style_bayer(p_fl_px: f32, p_fl_py: f32) -> f32 {
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_x: f32;
  var l_fl_y: f32;
  var l_fl_x0: f32;
  var l_fl_y0: f32;
  var l_fl_x1: f32;
  var l_fl_y1: f32;
  l_fl_x = (l_fl_px - (0x1.000000p2f * floor((l_fl_px * 0x1.000000p-2f))));
  l_fl_y = (l_fl_py - (0x1.000000p2f * floor((l_fl_py * 0x1.000000p-2f))));
  l_fl_x0 = (l_fl_x - (0x1.000000p1f * floor((l_fl_x * 0x1.000000p-1f))));
  l_fl_y0 = (l_fl_y - (0x1.000000p1f * floor((l_fl_y * 0x1.000000p-1f))));
  l_fl_x1 = floor((l_fl_x * 0x1.000000p-1f));
  l_fl_y1 = floor((l_fl_y * 0x1.000000p-1f));
  return ((((0x1.000000p2f * coax_p_st_bayer2(l_fl_x0, l_fl_y0)) + coax_p_st_bayer2(l_fl_x1, l_fl_y1)) + 0x1.000000p-1f) / 0x1.000000p4f);
}
fn verse_style_flat_dither(p_fl_t: f32, p_fl_px: f32, p_fl_py: f32, p_fl_steps: f32) -> f32 {
  var l_fl_t: f32 = p_fl_t;
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_steps: f32 = p_fl_steps;
  var l_fl_n: f32;
  l_fl_n = coax_p_st_max(l_fl_steps, 0x1.000000p0f);
  return coax_p_st_sat((floor(((coax_p_st_sat(l_fl_t) * l_fl_n) + verse_style_bayer(l_fl_px, l_fl_py))) / l_fl_n));
}
fn verse_style_flat_hem(p_fl_y: f32, p_fl_width: f32, p_fl_aa: f32) -> f32 {
  var l_fl_y: f32 = p_fl_y;
  var l_fl_width: f32 = p_fl_width;
  var l_fl_aa: f32 = p_fl_aa;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 0x1.0c6f7ap-20f), l_fl_y));
}
fn verse_style_flat_srgb(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.99999ap-2f, 0x1.da1cacp-2f, 0x1.6147aep-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7d70a4p-1f, 0x1.6978d4p-1f, 0x1.b1a9fcp-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c5a1cap-2f, 0x1.2b020cp-1f, 0x1.12f1aap-1f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f9db22p-2f, 0x1.43126ep-1f, 0x1.2147aep-1f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c00000p-1f, 0x1.a3d70ap-1f, 0x1.6b851ep-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.fae148p-1f, 0x1.f0a3d8p-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-1f, 0x1.51eb86p-1f, 0x1.b851ecp-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.eb851ep-1f, 0x1.ccccccp-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a1cac0p-3f, 0x1.b22d0ep-3f, 0x1.5d2f1ap-2f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ee147ap-1f, 0x1.dfbe76p-1f, 0x1.bdf3b6p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.158106p-2f, 0x1.c9ba5ep-2f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-2f, 0x1.3d70a4p-1f, 0x1.f5c290p-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.6147aep-1f, 0x1.170a3ep-1f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.c00000p-1f, 0x1.28f5c2p-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8f5c28p-1f, 0x1.ccccccp-1f, 0x1.b33334p-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8d4fe0p-1f, 0x1.87ae14p-1f, 0x1.bbe76cp-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.10e560p-1f, 0x1.10e560p-1f, 0x1.71a9fcp-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.666666p-1f, 0x1.3d70a4p-1f);
  }
  if ((l_fl_realm > 0x1.800000p0f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.6353f8p-1f, 0x1.a76c8cp-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ab851ep-1f, 0x1.b3b646p-1f, 0x1.d81062p-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.839582p-1f, 0x1.f9db22p-2f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.170a3ep-1f, 0x1.6147aep-1f, 0x1.d60418p-2f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c7ae14p-1f, 0x1.a5e354p-1f, 0x1.795810p-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f7cedap-1f, 0x1.e7ef9ep-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.2d0e56p-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.fbe76cp-1f, 0x1.f7cedap-1f, 0x1.ee147ap-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-3f, 0x1.51eb86p-3f, 0x1.f9db22p-3f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ec0832p-1f, 0x1.e5e354p-1f, 0x1.d1eb86p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7df3b6p-2f, 0x1.49374cp-1f, 0x1.03126ep-1f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.3126eap-1f, 0x1.9fbe76p-1f, 0x1.f1a9fcp-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.916872p-1f, 0x1.a5e354p-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.5d2f1ap-1f, 0x1.0cccccp-1f, 0x1.d1eb86p-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.e9fbe8p-1f, 0x1.bdf3b6p-1f, 0x1.3126eap-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.916872p-1f, 0x1.d3f7cep-1f, 0x1.b7cedap-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.656042p-1f, 0x1.558106p-1f, 0x1.ab851ep-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a5e354p-1f, 0x1.5d2f1ap-1f, 0x1.c9ba5ep-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.d60418p-1f, 0x1.472b02p-1f, 0x1.753f7cp-1f);
  }
  if ((l_fl_slot < 0x1.000000p-1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.a76c8cp-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.800000p0f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-1f, 0x1.dfbe76p-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.400000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.3f7ceep-1f, 0x1.6b851ep-1f, 0x1.28f5c2p-2f);
  }
  if ((l_fl_slot < 0x1.c00000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.333334p-1f, 0x1.f1a9fcp-3f);
  }
  if ((l_fl_slot < 0x1.200000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.b3b646p-1f, 0x1.818938p-1f, 0x1.14fdf4p-1f);
  }
  if ((l_fl_slot < 0x1.600000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f5c290p-1f, 0x1.d70a3ep-1f);
  }
  if ((l_fl_slot < 0x1.a00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.666666p-1f, 0x1.666666p-1f, 0x1.ae147ap-1f);
  }
  if ((l_fl_slot < 0x1.e00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.851eb8p-1f, 0x1.800000p-1f, 0x1.c28f5cp-1f);
  }
  if ((l_fl_slot < 0x1.100000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.ee147ap-1f, 0x1.c18938p-1f);
  }
  if ((l_fl_slot < 0x1.300000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d2f1aap-4f, 0x1.f3b646p-4f, 0x1.51eb86p-3f);
  }
  if ((l_fl_slot < 0x1.500000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.e5e354p-1f, 0x1.d60418p-1f, 0x1.a5e354p-1f);
  }
  if ((l_fl_slot < 0x1.700000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.bd70a4p-2f, 0x1.14fdf4p-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.900000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.6b851ep-1f, 0x1.9fbe76p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.b00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.1f3b64p-1f, 0x1.9fbe76p-1f, 0x1.818938p-1f);
  }
  if ((l_fl_slot < 0x1.d00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.516872p-1f, 0x1.0ed916p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.f00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.c18938p-1f, 0x1.09374cp-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.080000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.89ba5ep-1f, 0x1.d9999ap-1f, 0x1.ad9168p-1f);
  }
  if ((l_fl_slot < 0x1.180000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.472b02p-1f, 0x1.89ba5ep-1f);
  }
  if ((l_fl_slot < 0x1.280000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.9374bcp-1f, 0x1.676c8cp-1f, 0x1.c18938p-1f);
  }
  return coax_p_st_pick(l_fl_ch, 0x1.4f5c28p-1f, 0x1.6b851ep-1f, 0x1.353f7cp-1f);
}
fn verse_style_flat_colour(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  return coax_pow_verse_style_wrap(verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 0x1.19999ap1f);
}
fn verse_style_flat_time(p_fl_realm: f32, p_fl_ch: f32, p_fl_dusk: f32, p_fl_night: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_dusk: f32 = p_fl_dusk;
  var l_fl_night: f32 = p_fl_night;
  var l_fl_warm: f32;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_fl_warm = (0x1.000000p0f + ((coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.99999ap-1f, 0x1.5c28f6p-1f) - 0x1.000000p0f) * coax_p_st_sat(l_fl_dusk)));
  return (l_fl_warm + ((coax_p_st_pick(l_fl_ch, 0x1.99999ap-3f, 0x1.000000p-2f, 0x1.c28f5cp-2f) - l_fl_warm) * coax_p_st_sat(l_fl_night)));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_st_bayer2(p_fl_a: f32, p_fl_b: f32) -> f32 {
  var l_fl_a: f32 = p_fl_a;
  var l_fl_b: f32 = p_fl_b;
  return (((0x1.000000p1f * l_fl_a) + (0x1.800000p1f * l_fl_b)) - ((0x1.000000p2f * l_fl_a) * l_fl_b));
}
fn coax_p_st_pick(p_fl_ch: f32, p_fl_r: f32, p_fl_g: f32, p_fl_b: f32) -> f32 {
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_r: f32 = p_fl_r;
  var l_fl_g: f32 = p_fl_g;
  var l_fl_b: f32 = p_fl_b;
  if ((l_fl_ch < 0x1.000000p-1f))
  {
    return l_fl_r;
  }
  if ((l_fl_ch < 0x1.800000p0f))
  {
    return l_fl_g;
  }
  return l_fl_b;
}
fn coax_bits_verse_style_wrap(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_verse_style_wrap(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_verse_style_wrap(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_verse_style_wrap(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}
// ---- end generated
