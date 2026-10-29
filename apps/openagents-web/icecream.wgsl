struct U {
  size: vec2f, time: f32, fade: f32, still: f32, panels: f32, k: f32, outline: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,
  eye: vec4f,
  fog: vec4f,
  range: vec4f,
  kid: vec4f,
  tool: vec4f,
  rooms: array<vec4f, 100>,
  panel: array<vec4f, 8>,
}
@group(0) @binding(0) var<uniform> u: U;
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
  for (var k = 0; k < 4; k++) {
    if (f32(k) >= u.panels) { break; }
    c = plate(p, c, u.panel[k * 2], u.panel[k * 2 + 1]);
  }
  return c;
}
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(panels(pos.xy, u.fog.xyz * u.fade), 1.0);
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
const CHUNK: u32 = 1020u;
const OUTLINE_BASE: u32 = 1048576u;
const LIT_FLAT: u32 = 128u;
const LIT_KID: u32 = 254u;
const LIT_TOOL: u32 = 253u;
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
  let outline = instance >= OUTLINE_BASE;
  let chunk = select(instance, instance - OUTLINE_BASE, outline);
  let b = (chunk * CHUNK + id) * 6u;
  let w = vec3f(bitcast<f32>(vertices[b]), bitcast<f32>(vertices[b + 1u]), bitcast<f32>(vertices[b + 2u]));
  let n = unpack4x8snorm(vertices[b + 3u]);
  let c = unpack4x8unorm(vertices[b + 4u]);
  let slot = vertices[b + 4u] >> 24u;
  let hull = unpack4x8snorm(vertices[b + 5u]);
  var o: S;
  var clip = clip_of(w);
  var light = vec3f(1.0);
  var mode = 0.0;
  if (slot == LIT_KID) { light = u.kid.xyz; }
  else if (slot == LIT_TOOL) { light = u.tool.xyz; }
  else if (slot >= LIT_FLAT) { light = min(u.rooms[min(slot - LIT_FLAT, 99u)].xyz * 1.05, vec3f(1.0)); mode = 1.0; }
  else { light = u.rooms[min(slot, 99u)].xyz; }
  if (outline) {
    mode = 2.0;
    if (hull.w <= 0.0) { clip = vec4f(2.0, 2.0, 2.0, 1.0); }
    else {
      let push = vec4f(dot(u.r0.xyz, hull.xyz), dot(u.r1.xyz, hull.xyz), dot(u.r2.xyz, hull.xyz), dot(u.r3.xyz, hull.xyz));
      var dir = push.xy;
      let len = length(dir);
      if (len > 1e-6) { dir = dir / len; }
      let near = clamp(6.0 / clip.w, 0.35, 1.0);
      clip = vec4f(clip.xy + dir * u.outline * hull.w * near * 2.0 / u.size * clip.w, clip.zw);
    }
  }
  o.pos = clip;
  o.world = w;
  o.normal = n.xyz;
  o.colour = c.bgr;
  o.look = vec4f(light, mode);
  o.shine = max(n.w, 0.0);
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
  return vec4f(panels(s.pos.xy, c * u.fade), 1.0);
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
