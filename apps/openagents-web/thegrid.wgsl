// The Grid's three passes: the field behind (near black, the HUD's plates
// over it), the scene and the HUD's strokes. The scene is the Grid's art in
// its flat colours, unlit (pack_grid.py wrote them as the display values
// the demo drew), fading into the field with distance: the faces, the lines
// (a quad each, built here from its two ends and kept a pixel and a half
// wide on the screen) and the robots, skinned here from their clips' frames.
// The people (you and everyone else walking the Grid) are the human
// character from Cast.bin (verse/cast.coax), drawn in vertex order from its
// triangles and skinned with the joint palettes the face writes each frame,
// textured from its sheets and lit softly from above.
// Every number comes from thegrid.coax.

struct U {
  size: vec2f, time: f32, fade: f32, still: f32, plates: f32, k: f32, robot_base: f32,
  r0: vec4f, r1: vec4f, r2: vec4f, r3: vec4f,   // the view: clip = rows · world
  eye: vec4f,                                   // the eye, and a line's width in device pixels
  field: vec4f,                                 // fading from x to y metres; bones a frame
  plate: array<vec4f, 24>,
  people: vec4f,                                // Cast.bin's body vertices and indices (words), joints, the first person's instance
}
@group(0) @binding(0) var<uniform> u: U;

const FIELD = vec3f(0.0392, 0.0392, 0.0392);   // the art's background, 0.003 linear

fn plate(p: vec2f, under: vec3f, r: vec4f, s: vec4f) -> vec3f {
  if (s.x <= 0.0) { return under; }
  let half = (r.zw - r.xy) * 0.5;
  let corner = 6.0 * s.z;
  let q = abs(p - (r.xy + half)) - half + vec2f(corner);
  let d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - corner;
  if (d > 1.0) { return under; }
  let over = s.y >= 2.0;
  let fill = select(vec3f(0.035, 0.035, 0.04), vec3f(0.08, 0.08, 0.09), over);
  let line = select(vec3f(0.55, 0.55, 0.58), vec3f(0.92, 0.92, 0.95), over || s.y >= 1.0);
  let w = 1.25 * s.z;
  let inside = clamp(0.5 - d, 0.0, 1.0);
  let edge = clamp(w * 0.5 + 0.5 - abs(d + w * 0.5), 0.0, 1.0);
  let a = clamp(s.x, 0.0, 1.0);
  return mix(mix(under, fill, a * inside), line, edge * min(1.0, a + 0.1) * 0.8);
}
fn plates(p: vec2f, under: vec3f) -> vec3f {
  var c = under;
  for (var k = 0; k < 12; k++) {
    if (f32(k) >= u.plates) { break; }
    c = plate(p, c, u.plate[k * 2], u.plate[k * 2 + 1]);
  }
  return c;
}

@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(plates(pos.xy, FIELD), 1.0);
}

// ---- the scene
@group(1) @binding(0) var<storage, read> vertices: array<u32>;   // 6 words: x, y, z, colour, joints, weights
@group(1) @binding(1) var<storage, read> lines: array<u32>;      // 7 words: two ends, colour
@group(1) @binding(2) var<storage, read> bones: array<vec4f>;    // a frame's bones, three rows each
@group(1) @binding(3) var<storage, read> body: array<u32>;        // Cast.bin
struct Robot { place: vec4f, frames: vec4f }                      // x, y, z, yaw; scale, frame a, frame b, blend
@group(1) @binding(4) var<storage, read> robots: array<Robot>;
@group(1) @binding(5) var<storage, read> palette: array<vec4f>;  // each person's joints, three rows each, placed in the world
@group(1) @binding(6) var sheets: texture_2d_array<f32>;          // the character's texture layers
@group(1) @binding(7) var sheet: sampler;
fn cf(i: u32) -> f32 { return bitcast<f32>(body[i]); }
fn f(i: u32) -> f32 { return bitcast<f32>(vertices[i]); }
fn lf(i: u32) -> f32 { return bitcast<f32>(lines[i]); }

fn clip(w: vec3f) -> vec4f { let p = vec4f(w, 1.0); return vec4f(dot(u.r0, p), dot(u.r1, p), dot(u.r2, p), dot(u.r3, p)); }

// A robot vertex in the world: its bones' matrices, between two frames, then its place.
fn bone(frame: f32, j: u32, row: u32) -> vec4f {
  return bones[(u32(frame) * u32(u.field.z) + j) * 3u + row];
}
fn skinned(v: u32, r: Robot) -> vec3f {
  let b = v * 6u;
  let p = vec4f(f(b), f(b + 1u), f(b + 2u), 1.0);
  let joints = unpack4x8unorm(vertices[b + 4u]) * 255.0;
  let weights = unpack4x8unorm(vertices[b + 5u]);
  var m = vec3f(0.0);
  for (var k = 0u; k < 4u; k++) {
    let w = weights[k];
    if (w <= 0.0) { continue; }
    let j = u32(joints[k] + 0.5);
    var q = vec3f(0.0);
    for (var row = 0u; row < 3u; row++) {
      let a = dot(bone(r.frames.y, j, row), p);
      let c = dot(bone(r.frames.z, j, row), p);
      q[row] = mix(a, c, r.frames.w);
    }
    m += q * w;
  }
  let s = r.frames.x * m;
  let cy = cos(r.place.w); let sy = sin(r.place.w);
  return vec3f(r.place.x + s.x * cy + s.z * sy, r.place.y + s.y, r.place.z - s.x * sy + s.z * cy);
}

struct S {
  @builtin(position) pos: vec4f,
  @location(0) world: vec3f,
  @location(1) colour: vec3f,
  @location(2) uv: vec2f,
  @location(3) normal: vec3f,
  @location(4) @interpolate(flat) layer: f32,   // a person's texture layer; -1 the art's flat colour, 255 a person's flat colour
}

// A person's corner: triangle corner id of Cast.bin's body, skinned with person h's palette.
fn person(id: u32, h: u32) -> S {
  var o: S;
  let v = body[u32(u.people.y) + id];
  let b = u32(u.people.x) + v * 16u;
  let p = vec4f(cf(b), cf(b + 1u), cf(b + 2u), 1.0);
  let n = vec3f(cf(b + 3u), cf(b + 4u), cf(b + 5u));
  let w = vec4f(cf(b + 8u), cf(b + 9u), cf(b + 10u), cf(b + 11u));
  let j = unpack4x8unorm(body[b + 12u]) * 255.0;
  var at = vec3f(0.0); var nn = vec3f(0.0);
  for (var k = 0u; k < 4u; k++) {
    if (w[k] <= 0.0) { continue; }
    let r = (h * u32(u.people.z) + u32(j[k] + 0.5)) * 3u;
    let r0 = palette[r]; let r1 = palette[r + 1u]; let r2 = palette[r + 2u];
    at += w[k] * vec3f(dot(r0, p), dot(r1, p), dot(r2, p));
    nn += w[k] * vec3f(dot(r0.xyz, n), dot(r1.xyz, n), dot(r2.xyz, n));
  }
  let tint = unpack4x8unorm(body[b + 13u]);
  o.world = at;
  o.pos = clip(at);
  o.colour = tint.rgb;
  o.uv = vec2f(cf(b + 6u), cf(b + 7u));
  o.normal = nn;
  o.layer = tint.a * 255.0;
  return o;
}
fn decode(c: vec3f) -> vec3f { return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045)); }
fn encode(c: vec3f) -> vec3f { let x = clamp(c, vec3f(0.0), vec3f(1.0)); return select(1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3f(0.0031308)); }

@vertex fn scene_vs(@builtin(vertex_index) id: u32, @builtin(instance_index) instance: u32) -> S {
  var o: S;
  if (f32(instance) >= u.people.w) { return person(id, instance - u32(u.people.w)); }
  o.layer = -1.0;
  let r = robots[instance];
  if ((id & 0x80000000u) == 0u) {
    // a face's corner: the art's, in the world, or a robot's
    var w = vec3f(f(id * 6u), f(id * 6u + 1u), f(id * 6u + 2u));
    if (f32(id) >= u.robot_base) { w = skinned(id, r); }
    o.world = w;
    o.pos = clip(w);
    o.colour = unpack4x8unorm(vertices[id * 6u + 3u]).rgb;
    return o;
  }
  // a line's corner: its quad across the screen, a pixel and a half wide
  let k = id & 0x7fffffffu;
  let b = (k / 6u) * 7u;
  let c = lines[b + 6u];
  var a = vec3f(lf(b), lf(b + 1u), lf(b + 2u));
  var e = vec3f(lf(b + 3u), lf(b + 4u), lf(b + 5u));
  if ((c >> 24u) == 0u) { a = skinned(lines[b], r); e = skinned(lines[b + 1u], r); }
  let corner = k % 6u;
  let along = array(0.0, 1.0, 1.0, 0.0, 1.0, 0.0)[corner];
  let side = array(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0)[corner];
  var pa = clip(a); var pe = clip(e);
  // keep both ends in front of the eye, so a line that runs behind it still draws
  let near = 0.05;
  if (pa.w < near && pe.w < near) { o.pos = vec4f(0.0, 0.0, 2.0, 1.0); return o; }
  if (pa.w < near) { let t = (near - pa.w) / (pe.w - pa.w); pa = mix(pa, pe, t); a = mix(a, e, t); }
  if (pe.w < near) { let t = (near - pe.w) / (pa.w - pe.w); pe = mix(pe, pa, t); e = mix(e, a, t); }
  let sa = pa.xy / pa.w * u.size * 0.5; let se = pe.xy / pe.w * u.size * 0.5;
  var dir = se - sa;
  if (length(dir) < 0.0001) { dir = vec2f(1.0, 0.0); }
  let n = normalize(vec2f(-dir.y, dir.x)) * u.eye.w * 0.5 * side;
  let p = mix(pa, pe, along);
  o.pos = vec4f(p.xy + n / (u.size * 0.5) * p.w, p.z, p.w);
  o.world = mix(a, e, along);
  o.colour = unpack4x8unorm(c).rgb;
  return o;
}
@fragment fn scene_fs(s: S) -> @location(0) vec4f {
  let d = length(u.eye.xz - s.world.xz);
  let t = clamp((d - u.field.x) / max(u.field.y - u.field.x, 1.0), 0.0, 1.0);
  // a person: the sheet's colour under the tint, lit from above and the front (sampled every time, so
  // the sample stays in uniform control flow)
  let albedo = textureSample(sheets, sheet, s.uv, i32(clamp(s.layer, 0.0, 5.0))).rgb;
  let up = normalize(vec3f(0.35, 0.85, 0.4));
  let light = 0.5 + 0.6 * max(dot(normalize(s.normal + vec3f(0.0, 0.0001, 0.0)), up), 0.0);
  var colour = s.colour;
  if (s.layer >= 0.0) {
    colour = select(encode(albedo * decode(s.colour) * light), encode(decode(s.colour) * light), s.layer > 250.0);
  }
  let c = mix(colour, FIELD, t * t * (3.0 - 2.0 * t));
  return vec4f(plates(s.pos.xy, c), 1.0);
}

// ---- the HUD's strokes
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
@fragment fn fx_fs(v: V) -> @location(0) vec4f {
  let along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  let d = length(vec2f(along, v.place.y));
  let core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  let g = d / max(v.shape.w, 1.0);
  let halo = exp(-5.0 * g * g) * 0.28;
  return vec4f(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
