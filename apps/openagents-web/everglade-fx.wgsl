// Everglade's spell fire and smoke (everglade-spells.coax sp_fx): soft camera-facing puffs drawn
// after the scene and the river (host.js particles), depth-tested and not written, in two passes:
// the smoke first, over what is behind it (premultiplied, particle_over_vs/particle_over_fs), then
// the fire, the glows and the sparks, added to the light (particle_vs/particle_fs), so what burns
// glows through its own smoke and blooms. Every puff is three rows (everglade-spells.coax SP_FX_*):
//   at.xyz, size (its radius, m)
//   colour.rgb (linear light), heat (fire: 0 to 1, how white its heart is; smoke: how opaque)
//   axis.xyz (a streak's half-length along its flight, m; zero for a round puff), kind + seed / 2
// kind: 0 a billow of fire, 1 smoke or dust, 2 a smooth glow or a spark's streak, 3 a ring of light
// lying on the ground (a shockwave). Group 0 is everglade.wgsl's uniforms, read up to the eye.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, horizon: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
}
@group(0) @binding(0) var<uniform> u: U;
struct Puff { at: vec4f, colour: vec4f, axis: vec4f }
@group(1) @binding(0) var<storage, read> puffs: array<Puff>;
// The scene's depth as it stood before the river (host.js W5's copy): a puff thins out where what
// stands behind it comes close, so the ground and walls never cut it with a hard line.
@group(3) @binding(1) var particle_depth: texture_depth_2d;

const FIRE = 0.0; const SMOKE = 1.0; const GLOW = 2.0; const RING = 3.0;

struct P {
  @builtin(position) pos: vec4f,
  @location(0) corner: vec2f,      // across the quad, -1 to 1 (a streak's: along it, then across)
  @location(1) colour: vec4f,
  @location(2) @interpolate(flat) look: vec4f,   // kind, seed, a streak's length over its width (fire: how opaque), distance
  @location(3) depth: vec2f,       // its own w (the view's distance along the eye's axis), and how soft its meeting is (m)
  @location(4) @interpolate(flat) near: f32,   // how much of it shows for how close the eye is: a puff rolling past the eye thins away, never a wall across the view
}
fn clip(w: vec3f) -> vec4f {
  let p = vec4f(w, 1.0);
  return vec4f(dot(u.r0, p), dot(u.r1, p), dot(u.r2, p), dot(u.r3, p));
}
fn gone() -> P { var o: P; o.pos = vec4f(0.0, 0.0, 2.0, 1.0); return o; }
fn place(v: u32, n: u32) -> P {
  var o: P;
  let e = puffs[n];
  let quad = array(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0));
  let c = quad[v];
  let kind = floor(e.axis.w);
  let seed = fract(e.axis.w) * 2.0;
  let size = e.at.w;
  let to_eye = u.eye.xyz - e.at.xyz;
  let d = max(length(to_eye), 0.001);
  let view = to_eye / d;
  var w = e.at.xyz;
  let streak = select(0.0, length(e.axis.xyz), kind == GLOW);   // fire's axis.x is how opaque it is
  var along = 0.0;
  if (kind == RING) {
    // flat on the ground, a square round the ring
    w += vec3f(c.x, 0.0, c.y) * size;
  } else if (streak > 0.001) {
    // a streak: a quad along its flight, turned to face the eye, a width round each end
    let a = e.axis.xyz / streak;
    let side = normalize(cross(a, view) + vec3f(0.00001, 0.0, 0.0));
    w += a * c.x * (streak + size) + side * c.y * size;
    along = streak / max(size, 0.001);
  } else {
    // a puff: facing the eye, upright, and drawn a little toward the eye so the ground and walls it
    // rolls over cut less of it
    let right = normalize(cross(vec3f(0.0, 1.0, 0.0), view) + vec3f(0.00001, 0.0, 0.0));
    let up = cross(view, right);
    w += (right * c.x + up * c.y) * size + view * min(size * 0.2, d * 0.5);
  }
  o.pos = clip(w);
  o.depth = vec2f(o.pos.w, max(size * 0.6, 0.15));
  o.corner = c;
  o.colour = e.colour;
  o.look = vec4f(kind, seed, select(along, e.axis.x, kind == FIRE), d);
  o.near = select(smoothstep(size * 0.9 + 0.3, size * 2.2 + 1.0, d), 1.0, kind == RING);
  return o;
}
@vertex fn particle_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) n: u32) -> P {
  if (floor(puffs[n].axis.w) == SMOKE) { return gone(); }
  return place(v, n);
}
@vertex fn particle_over_vs(@builtin(vertex_index) v: u32, @builtin(instance_index) n: u32) -> P {
  let kind = floor(puffs[n].axis.w);
  if (kind != SMOKE && kind != FIRE) { return gone(); }
  return place(v, n);
}

fn hash2(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise2(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p);
  let s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), s.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), s.x), s.y);
}
// A billow's churn: two octaves drifting apart, 0 to 1.
fn churn(c: vec2f, seed: f32) -> f32 {
  let t = u.time;
  return 0.62 * noise2(c * 1.7 + vec2f(seed * 19.0 + t * 0.35, seed * 7.0 - t * 0.55)) + 0.38 * noise2(c * 3.9 + vec2f(-t * 0.9, seed * 13.0 + t * 0.4));
}
// How much of a puff is at its corner c: whole inside a lumpy, churning edge, a painted cloud's;
// as its cover falls it is eaten away from that edge in, never turned to a see-through veil.
fn billow(c: vec2f, seed: f32, rag: f32, soft: f32, cover: f32) -> f32 {
  let n = churn(c, seed);
  let edge = 1.0 - length(c) + (n - 0.5) * rag - (1.0 - clamp(cover, 0.0, 1.0)) * (1.0 + 0.5 * rag);
  return smoothstep(0.0, soft, edge);
}
// How much of a puff shows at this pixel for what stands behind it: none where it meets it, all a
// little in front. The scene's w from its depth: clip z = b - a·w along any ray (a perspective's), b
// the eye's clip z, a from one point ahead of the eye.
fn soft(p: P) -> f32 {
  let b = dot(u.r2, vec4f(u.eye.xyz, 1.0));
  let ahead = u.eye.xyz + u.r3.xyz;
  let a = (b - dot(u.r2, vec4f(ahead, 1.0))) / max(dot(u.r3.xyz, u.r3.xyz), 1e-6);
  let at = clamp(vec2i(p.pos.xy), vec2i(0), vec2i(textureDimensions(particle_depth)) - 1);
  let z = textureLoad(particle_depth, at, 0);
  let under = z + a;   // never zero short of the far plane: the scene's w on the far side of its sign
  let scene = select(1e6, b / under, abs(under) > 1e-7);
  return clamp((scene - p.depth.x) / p.depth.y, 0.0, 1.0);
}
// Far puffs fade into the haze.
fn far(d: f32) -> f32 { return exp(-max(d - 25.0, 0.0) * max(u.eye.w, 0.002) * 1.5); }

// Fire's colour at a billow's corner: its own, a deeper red at its edge, white-hot at its heart as its
// heat says; m how much of it is there.
fn fire(p: P, m: f32) -> vec3f {
  // a ball of fire in the town's painted bands: bright where it bulges toward you, its rim rolled
  // under into a darker, sooty red, so each billow stands out from those behind it; tongues and
  // folds churning through it; its heart white-hot as its heat says
  let n = 0.6 * churn(p.corner * 1.6 + vec2f(0.0, -u.time * 0.7), p.look.y + 5.0) + 0.4 * noise2(p.corner * 5.5 + vec2f(p.look.y * 31.0, -u.time * 2.2));
  let r = length(p.corner);
  let bulge = sqrt(max(1.0 - r * r, 0.0));
  let v = bulge * (0.5 + 0.8 * n);
  let band = smoothstep(0.18, 0.26, v) * 0.45 + smoothstep(0.5, 0.58, v) * 0.55;
  let heart = p.colour.a * smoothstep(0.72, 0.8, v);
  let c = mix(p.colour.rgb * vec3f(0.42, 0.2, 0.13), p.colour.rgb * 1.15, band);
  return mix(c, vec3f(1.0, 0.88, 0.62) * max(max(p.colour.r, 0.7), 1.0) * 1.25, heart);
}

@fragment fn particle_fs(p: P) -> @location(0) vec4f {
  let kind = p.look.x;
  var a = 0.0;
  var c = p.colour.rgb;
  if (kind == RING) {
    // a ring at the quad's rim, thin and soft
    let r = length(p.corner);
    let band = 1.0 - smoothstep(0.0, 0.16, abs(r - 0.86));
    a = band * (0.7 + 0.6 * noise2(p.corner * 6.0 + vec2f(p.look.y * 9.0, u.time)));
  } else if (kind == GLOW) {
    // a smooth glow, or a spark's streak: a hot thin core in a soft halo
    var q = p.corner;
    if (p.look.z > 0.0) { q.x = max(abs(q.x) * (p.look.z + 1.0) - p.look.z, 0.0); }
    let r2 = dot(q, q);
    if (r2 >= 1.0) { discard; }
    a = exp(-r2 * 3.5) * (1.0 - r2);
    c = mix(c, vec3f(max(c.r, max(c.g, c.b))), p.colour.a * exp(-r2 * 18.0));
  } else {
    // fire: its body is drawn over (particle_over_fs); here only its heat's glow, which blooms
    let m = billow(p.corner, p.look.y, 0.6, 0.08, p.look.z);
    if (m <= 0.002) { discard; }
    c = fire(p, m) * (0.08 + 0.4 * p.colour.a);
    a = m;
  }
  let k = a * p.near * far(p.look.w) * select(soft(p), 1.0, kind == RING);
  // what it adds, and how much of that blooms
  return vec4f(c * k, clamp(k * 0.6, 0.0, 1.0));
}

@fragment fn particle_over_fs(p: P) -> @location(0) vec4f {
  if (p.look.x == FIRE) {
    // a billow of fire: its own light, covering what is behind as thickly as it is there
    let m = billow(p.corner, p.look.y, 0.6, 0.08, p.look.z);
    if (m <= 0.002) { discard; }
    let a = clamp(m, 0.0, 1.0) * p.near * far(p.look.w) * soft(p);
    return vec4f(fire(p, m) * a, a);
  }
  // smoke: lit from above, darker under, ragged and churning, as thick as it says
  let m = billow(p.corner, p.look.y, 0.7, 0.12, p.colour.a);
  if (m <= 0.002) { discard; }
  let a = clamp(m * min(p.colour.a * 1.6, 0.92), 0.0, 1.0) * p.near * far(p.look.w) * soft(p);
  // a ball of smoke lit from above: its top and its bulges catch the light, its underside in shadow
  let n = churn(p.corner * 1.1 + vec2f(0.0, 0.4), p.look.y + 3.0);
  let r = length(p.corner);
  let bulge = sqrt(max(1.0 - r * r, 0.0));
  let up = clamp(0.5 + 0.5 * (p.corner.y + 0.6 * bulge - 0.3), 0.0, 1.0);
  let lit = mix(0.5, 1.3, smoothstep(0.42, 0.5, up * (0.55 + 0.8 * n)));   // two painted bands
  return vec4f(p.colour.rgb * lit * a, a);
}
