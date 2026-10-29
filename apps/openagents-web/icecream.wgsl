struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, outline: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
  fog: vec4f,
  range: vec4f,
  hud: vec4f,
  spare: vec4f,
  rooms: array<vec4f, 100>,
  panel: array<vec4f, 80>,
}
@group(0) @binding(0) var<uniform> u: U;
fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.y <= 0.0) { return under; }
  if (p.x < r.x - 2.0 || p.y < r.y - 2.0 || p.x > r.z + 2.0 || p.y > r.w + 2.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = min(select(4.0 * s.x, s.w, s.w > 0.0), min(half.x, half.y));
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let look = i32(s.z + 0.5);
  var fill = vec3f(0.165, 0.114, 0.078);
  var line = vec3f(0.69, 0.541, 0.29);
  if (look == 1) { line = vec3f(0.86, 0.70, 0.42); }
  if (look == 2) { fill = vec3f(0.235, 0.168, 0.115); line = vec3f(0.95, 0.80, 0.52); }
  if (look == 3) { fill = vec3f(0.29, 0.13, 0.17); line = vec3f(1.0, 0.56, 0.69); }
  if (look == 4) { fill = vec3f(0.22, 0.16, 0.07); line = vec3f(0.97, 0.77, 0.17); }
  if (look == 5) { fill = vec3f(0.30, 0.20, 0.07); line = vec3f(1.0, 0.86, 0.40); }
  let w = 1.25 * s.x * select(1.0, 1.6, look >= 3);
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.y, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1));
}
fn panels(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 40; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}
fn overlay(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  let hurt = clamp(u.hud.x, 0.0, 1.0);
  if (hurt > 0.0) {
    let e = min(min(p.x, u.size.x - p.x), min(p.y, u.size.y - p.y)) / (120.0 * u.k);
    c = mix(c, vec3f(1.0, 0.24, 0.43), hurt * 0.75 * clamp(1.0 - e, 0.0, 1.0));
  }
  if (u.hud.z > 0.5) {
    let band = (p.y / u.k) % 46.0;
    c = mix(c, vec3f(0.157, 0.094, 0.055), select(0.35, 0.94, band < 34.0));
  }
  let day = clamp(u.hud.y, 0.0, 1.0);
  if (day > 0.0) {
    let q = p / u.size - vec2f(0.7, 0.25);
    let r = length(q * vec2f(u.size.x / max(u.size.y, 1.0), 1.0));
    let sky = select(select(vec3f(0.749, 0.91, 1.0), vec3f(1.0, 0.898, 0.541), r < 0.14), vec3f(1.0, 0.984, 0.878), r < 0.08);
    c = mix(c, sky, day);
  }
  return c;
}
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(panels(pos.xy, overlay(pos.xy, u.fog.xyz * u.fade)), 1.0);
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<vec4f>;
const CHUNK: u32 = 1020u;
const OBJ_BASE: u32 = 8388608u;
const OBJ_OUTLINE: u32 = 4194304u;
const LIT_FLAT: u32 = 128u;
const INK: vec3f = vec3f(0.17, 0.11, 0.08);
struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) colour: vec3f,
  @location(3) @interpolate(flat) look: vec4f,
  @location(4) @interpolate(flat) shine: f32,
}
fn clip_of(w: vec3f) -> vec4f {
  let p = vec4f(w, 1.0);
  return vec4f(dot(u.r0, p), dot(u.r1, p), dot(u.r2, p), dot(u.r3, p));
}
@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  var o: S;
  let object = instance >= OBJ_BASE;
  var outline = false;
  var at = instance * CHUNK + id;
  var light = vec3f(1.0);
  var tint = vec3f(1.0);
  var mode = 0.0;
  var shine = 0.0;
  var r0 = vec4f(1.0, 0.0, 0.0, 0.0);
  var r1 = vec4f(0.0, 1.0, 0.0, 0.0);
  var r2 = vec4f(0.0, 0.0, 1.0, 0.0);
  if (object) {
    let k = instance - OBJ_BASE;
    outline = (k & OBJ_OUTLINE) != 0u;
    let i = ((k & (OBJ_OUTLINE - 1u)) >> 10u) * 6u;
    r0 = objects[i]; r1 = objects[i + 1u]; r2 = objects[i + 2u];
    tint = objects[i + 3u].xyz; shine = objects[i + 3u].w;
    light = objects[i + 4u].xyz;
    if (objects[i + 4u].w > 0.5) { mode = 1.0; light = tint; }
    at = (u32(objects[i + 5u].x) + (k & 1023u)) * CHUNK + id;
  }
  let b = at * 6u;
  let p = vec3f(bitcast<f32>(vertices[b]), bitcast<f32>(vertices[b + 1u]), bitcast<f32>(vertices[b + 2u]));
  let n = unpack4x8snorm(vertices[b + 3u]);
  let c = unpack4x8unorm(vertices[b + 4u]);
  let slot = vertices[b + 4u] >> 24u;
  let hull = unpack4x8snorm(vertices[b + 5u]);
  let w = vec3f(dot(r0.xyz, p) + r0.w, dot(r1.xyz, p) + r1.w, dot(r2.xyz, p) + r2.w);
  let normal = vec3f(dot(r0.xyz, n.xyz), dot(r1.xyz, n.xyz), dot(r2.xyz, n.xyz));
  if (!object) {
    if (slot >= LIT_FLAT) { light = min(u.rooms[min(slot - LIT_FLAT, 99u)].xyz * 1.05, vec3f(1.0)); mode = 1.0; }
    else { light = u.rooms[min(slot, 99u)].xyz; }
    shine = max(n.w, 0.0);
  }
  var clip = clip_of(w);
  if (outline) {
    mode = 2.0;
    if (hull.w <= 0.0 || (n.x == 0.0 && n.y == 0.0 && n.z == 0.0)) { clip = vec4f(2.0, 2.0, 2.0, 1.0); }
    else {
      let hw = vec3f(dot(r0.xyz, hull.xyz), dot(r1.xyz, hull.xyz), dot(r2.xyz, hull.xyz));
      let push = vec4f(dot(u.r0.xyz, hw), dot(u.r1.xyz, hw), dot(u.r2.xyz, hw), dot(u.r3.xyz, hw));
      var dir = push.xy;
      let len = length(dir);
      if (len > 1e-6) { dir = dir / len; }
      let near = clamp(6.0 / clip.w, 0.35, 1.0);
      clip = vec4f(clip.xy + dir * u.outline * hull.w * near * 2.0 / u.size * clip.w, clip.zw);
    }
  }
  o.pos = clip;
  o.world = w;
  o.normal = normal;
  o.colour = c.bgr * tint;
  o.look = vec4f(light, mode);
  o.shine = shine;
  return o;
}
@fragment fn scene_fs(s: S, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  let mode = s.look.w;
  let shine = s.shine;
  let fog = smoothstep(u.range.x, u.range.y, length(u.eye.xyz - s.world)) * u.range.z;
  var c: vec3f;
  if (mode > 1.5) {
    if (front) { discard; }
    c = mix(INK, u.fog.xyz, fog * 0.6);
  } else {
    if (!front) { discard; }
    if (mode > 0.5) {
      c = mix(s.look.xyz, u.fog.xyz, fog * 0.6);
    } else {
      let n = normalize(s.normal);
      let l = normalize(vec3f(0.35, 1.0, 0.25));
      let d = dot(n, l);
      let band = select(select(0.74, 0.86, d > -0.35), 1.0, d > 0.25);
      c = s.colour * s.look.xyz * band;
      if (shine > 0.0) {
        let v = normalize(u.eye.xyz - s.world);
        let h = normalize(l + v);
        let sp = pow(max(dot(n, h), 0.0), 28.0);
        c += vec3f(1.0) * smoothstep(0.4, 0.5, sp) * 0.6 * shine + vec3f(0.06) * shine;
      }
      c = mix(c, u.fog.xyz, fog);
    }
  }
  return vec4f(panels(s.pos.xy, overlay(s.pos.xy, c * u.fade)), 1.0);
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
