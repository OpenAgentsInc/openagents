// Star Catcher (starcatch.coax): bg_fs draws a night sky (a gradient, a few
// twinkling far stars, a moon) over a dark hill; a red flush when a star
// lands. fx_vs/fx_fs draw the strokes catch_frame writes (capsules).
struct U {
  size: vec2f, time: f32, fade: f32,
  info: vec4f,     // the ground's top (device px), the flash, 0, 0
}
@group(0) @binding(0) var<uniform> u: U;

fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}

@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let p = pos.xy;
  let t = p.y / u.size.y;
  var c = mix(vec3f(0.02, 0.03, 0.09), vec3f(0.10, 0.12, 0.28), t);
  // far stars on a grid of cells, each twinkling
  let cell = 34.0 * max(u.size.x / 1440.0, 0.6);
  let g = floor(p / cell);
  let h = hash(g);
  if (h > 0.82) {
    let at = (g + vec2f(hash(g + 3.1), hash(g + 7.7))) * cell;
    let d = length(p - at);
    let tw = 0.6 + 0.4 * sin(u.time * (1.0 + 3.0 * h) + h * 40.0);
    c += vec3f(0.8, 0.85, 1.0) * exp(-d * d / 3.0) * tw;
  }
  // the moon, upper left of centre
  let m = vec2f(u.size.x * 0.3, u.size.y * 0.2);
  let mr = min(u.size.x, u.size.y) * 0.06;
  let md = length(p - m);
  c = mix(c, vec3f(0.92, 0.92, 0.85), 1.0 - smoothstep(mr - 1.0, mr + 1.0, md));
  c += vec3f(0.25, 0.25, 0.3) * exp(-max(md - mr, 0.0) / (mr * 1.5)) * 0.4;
  // the hill
  let ground = u.info.x + sin(p.x / u.size.x * 6.0) * u.size.y * 0.012;
  if (p.y > ground) {
    let e = (p.y - ground) / max(u.size.y - ground, 1.0);
    c = mix(vec3f(0.06, 0.13, 0.08), vec3f(0.02, 0.05, 0.03), e);
  }
  // a star landed: the edges flush red
  if (u.info.y > 0.0) {
    let q = (p / u.size - 0.5) * 2.0;
    c = mix(c, vec3f(0.55, 0.05, 0.04), smoothstep(0.6, 1.4, length(q)) * u.info.y * 0.6);
  }
  return vec4f(c, 1.0);
}

struct V {
  @builtin(position) pos: vec4f,
  @location(0) place: vec2f,
  @location(1) shape: vec4f,
  @location(2) tint: vec4f,
  @location(3) bar: vec4f,
}
@vertex fn fx_vs(@location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f) -> V {
  var o: V;
  o.pos = vec4f(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  o.place = a.zw;
  o.shape = b;
  o.tint = c;
  o.bar = d;
  return o;
}
// A capsule: shape.x its length, y its core's half width, z its light, w its glow's reach.
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
