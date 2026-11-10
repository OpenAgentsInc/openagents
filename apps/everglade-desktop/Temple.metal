// Everglade for desktop: the Golemancers' Workshop as a place. A round marble temple open to the
// night, above a sea of cloud: an outer colonnade lit warm from its feet, six inner columns (the
// pylons) banded with pale blue light and crowned with floating crystals, the unfinished golem in
// the middle with the sphere in its chest, the bronze door in its pier at the back, and holographic
// displays hung between the pylons. One full-screen pass marches a distance field and writes depth,
// so your model, drawn after it, stands behind and in front of things as it should.
//
// The layout is workshop.coax's (ws_walls, WS_PYLON_RING, WS_WALL): what stops you there is what is
// drawn here. Keep the two in step.
#include <metal_stdlib>
using namespace metal;

// The view (workshop.coax ws_camera), the picture's size in pixels, the clock, and where you stand.
struct TU { float4 eye; float4 right; float4 up; float4 forward; float2 size; float time; float pad; float4 you; };

constant float PYLON_RING = 4.6;     // WS_PYLON_RING
constant float PYLON_H = 3.4;        // WS_PYLON_HEIGHT
constant float WALL = 7.2;           // WS_WALL: the balustrade and the door's face
constant float COLONNADE = 7.75;     // the outer columns, behind the balustrade
constant float COLUMN_H = 6.0;
constant float TAU = 6.2831853;
constant float3 MOON = float3(-0.36, 0.42, -0.83);   // toward the moon, over the door (not unit; normalized where used)
constant float3 CYAN = float3(0.42, 0.82, 1.0);
constant float3 AMBER = float3(1.0, 0.58, 0.24);
constant float3 CORE = float3(0.0, 1.5, 0.15);      // the sphere in the golem's chest

static float hash1(float2 p) { return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }
static float hash3(float3 p) { return fract(sin(dot(p, float3(127.1, 311.7, 74.7))) * 43758.5453); }
static float noise2(float2 p) {
  float2 i = floor(p), f = fract(p); float2 s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash1(i), hash1(i + float2(1, 0)), s.x), mix(hash1(i + float2(0, 1)), hash1(i + float2(1, 1)), s.x), s.y);
}
static float fbm2(float2 p) { float v = 0.0, a = 0.5; for (int k = 0; k < 5; k++) { v += a * noise2(p); p = p * 2.03 + float2(1.7, 9.2); a *= 0.5; } return v; }
static float noise3(float3 p) {
  float3 i = floor(p), f = fract(p); float3 s = f * f * (3.0 - 2.0 * f);
  float a = mix(mix(hash3(i), hash3(i + float3(1, 0, 0)), s.x), mix(hash3(i + float3(0, 1, 0)), hash3(i + float3(1, 1, 0)), s.x), s.y);
  float b = mix(mix(hash3(i + float3(0, 0, 1)), hash3(i + float3(1, 0, 1)), s.x), mix(hash3(i + float3(0, 1, 1)), hash3(i + float3(1, 1, 1)), s.x), s.y);
  return mix(a, b, s.z);
}

static float sdBox(float3 p, float3 b) { float3 q = abs(p) - b; return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0); }
// A capped cylinder standing on y = 0, h tall.
static float sdPost(float3 p, float r, float h) { float2 d = float2(length(p.xz) - r, abs(p.y - h * 0.5) - h * 0.5); return min(max(d.x, d.y), 0.0) + length(max(d, 0.0)); }
static float sdCapsule(float3 p, float3 a, float3 b, float r) { float3 pa = p - a, ba = b - a; return length(pa - ba * saturate(dot(pa, ba) / dot(ba, ba))) - r; }
// The smaller of two distances, run together over k where they meet.
static float smin(float a, float b, float k) { float h = saturate(0.5 + 0.5 * (b - a) / k); return mix(b, a, h) - k * h * (1.0 - h); }
static float sdOcta(float3 p, float s) { p = abs(p); return (p.x + p.y + p.z - s) * 0.57735; }
// A ring of rectangular section round the y axis.
static float sdRing(float3 p, float radius, float half_w, float y, float half_h) {
  float2 d = float2(abs(length(p.xz) - radius) - half_w, abs(p.y - y) - half_h);
  return min(max(d.x, d.y), 0.0) + length(max(d, 0.0));
}
// The point turned into the one of n equal sectors round the y axis that holds it: x is then out
// from the middle along the sector's own line.
static float3 sector(float3 p, float n, float offset) {
  float s = TAU / n, a = atan2(p.z, p.x) - offset;
  a -= s * floor(a / s + 0.5);
  float r = length(p.xz);
  return float3(cos(a) * r, p.y, sin(a) * r);
}
// A fluted column standing at the origin: shaft, a round base, and a Doric capital with its slab.
static float column(float3 p, float r, float h, float flutes) {
  float a = atan2(p.z, p.x);
  float taper = r * (1.0 - 0.10 * saturate(p.y / h));
  float shaft = sdPost(p, taper - 0.022 * r * 3.0 * (0.5 + 0.5 * cos(a * flutes)), h);
  float base = min(sdPost(p, r + 0.14, 0.14), sdPost(p - float3(0, 0.14, 0), r + 0.07, 0.10)) - 0.015;
  float3 q = p - float3(0, h - 0.34, 0);
  float flare = sdPost(q, taper + 0.02 + 0.22 * saturate(q.y / 0.2), 0.2);
  float slab = sdBox(p - float3(0, h - 0.07, 0), float3(r + 0.24, 0.07, r + 0.24)) - 0.01;
  return min(min(shaft, base), min(flare, slab));
}
static float3 crystal_at(float time) { return float3(PYLON_RING, PYLON_H + 0.62 + 0.07 * sin(time * 1.1), 0.0); }

// Materials: 1 the floor, 2 marble, 3 the bronze door, 4 the golem, 5 a crystal, 6 the golem's sphere.
static float2 nearer(float2 a, float2 b) { return a.x < b.x ? a : b; }
// `ground` leaves the floor out: a ray mirrored off the floor climbs away from it and can't meet
// it again, and with the floor in the field it would creep along just above it and run out of steps.
static float2 map(float3 p, float time, bool ground) {
  float r = length(p.xz);
  // The floor: a slab of marble, and a wider step under it, then nothing but cloud.
  float2 d = float2(ground ? min(sdPost(p + float3(0, 1.0, 0), 8.7, 1.0), sdPost(p + float3(0, 1.5, 0), 9.4, 1.2)) : 1000.0, 1.0);
  // The golem, in the middle, unfinished: one arm ends at the elbow, and its chest is open.
  if (r < 1.3 && p.y < 2.9) {
    // A statue's shapes, run together: chest and waist, a head on a neck, shoulders, limbs that taper.
    float body = length((p - float3(0, 1.62, 0)) * float3(1.0, 0.86, 1.65)) - 0.36;
    body = smin(body, length((p - float3(0, 1.16, 0)) * float3(1.0, 0.9, 1.5)) - 0.27, 0.12);
    body = smin(body, length((p - float3(0, 0.98, 0)) * float3(1.0, 1.2, 1.45)) - 0.29, 0.08);
    body = max(body, -(length(p - float3(0, 1.5, 0.27)) - 0.2));
    body = smin(body, sdCapsule(p, float3(0, 1.95, 0), float3(0, 2.05, 0), 0.085), 0.05);
    body = smin(body, length((p - float3(0, 2.25, 0.01)) * float3(1.0, 0.86, 0.95)) - 0.17, 0.04);
    body = smin(body, length(p - float3(-0.42, 1.84, 0)) - 0.15, 0.08);
    body = smin(body, length(p - float3(0.42, 1.84, 0)) - 0.15, 0.08);
    body = smin(body, sdCapsule(p, float3(-0.19, 0.5, 0), float3(-0.2, 0.95, 0), 0.14), 0.06);
    body = smin(body, sdCapsule(p, float3(0.19, 0.5, 0), float3(0.2, 0.95, 0), 0.14), 0.06);
    body = smin(body, sdCapsule(p, float3(-0.19, 0.12, 0.01), float3(-0.19, 0.5, 0), 0.105), 0.04);
    body = smin(body, sdCapsule(p, float3(0.19, 0.12, 0.01), float3(0.19, 0.5, 0), 0.105), 0.04);
    body = smin(body, sdBox(p - float3(-0.19, 0.06, 0.06), float3(0.1, 0.06, 0.19)) - 0.02, 0.03);
    body = smin(body, sdBox(p - float3(0.19, 0.06, 0.06), float3(0.1, 0.06, 0.19)) - 0.02, 0.03);
    body = smin(body, sdCapsule(p, float3(-0.5, 1.8, 0), float3(-0.6, 1.3, 0.02), 0.1), 0.05);
    body = smin(body, sdCapsule(p, float3(-0.6, 1.3, 0.02), float3(-0.58, 0.82, 0.1), 0.08), 0.03);
    body = smin(body, length(p - float3(-0.58, 0.74, 0.12)) - 0.09, 0.03);
    body = smin(body, sdCapsule(p, float3(0.5, 1.8, 0), float3(0.6, 1.42, 0.02), 0.1), 0.05);
    d = nearer(d, float2(body, 4.0));
    d = nearer(d, float2(length(p - CORE) - 0.115, 6.0));
  } else { d.x = min(d.x, max(r - 1.1, p.y - 2.7)); }   // no nearer than this: it lets a ray pass by in few steps
  // The six pylons: a square foot (ws_walls's box), a fluted column, and a crystal afloat over it.
  if (r > 3.6 && r < 5.6 && p.y < 4.7) {
    float3 q = sector(p, 6.0, 0.0) - float3(PYLON_RING, 0, 0);
    float pylon = min(sdBox(q - float3(0, 0.15, 0), float3(0.52, 0.15, 0.52)) - 0.01, column(q - float3(0, 0.3, 0), 0.3, PYLON_H - 0.3, 16.0));
    d = nearer(d, float2(pylon, 2.0));
    float3 c = q + float3(PYLON_RING, 0, 0) - crystal_at(time);
    float ca = time * 0.6; c.xz = float2(c.x * cos(ca) - c.z * sin(ca), c.x * sin(ca) + c.z * cos(ca));
    d = nearer(d, float2(sdOcta(c * float3(1.0, 0.62, 1.0), 0.2), 5.0));
  } else { d.x = min(d.x, max(abs(r - PYLON_RING) - 0.9, p.y - 4.5)); }
  // The edge: a balustrade at the wall's line, the colonnade behind it and its entablature, and
  // the door in its pier.
  if (r > 6.3 || p.y > 5.2) {
    float rail = min(sdRing(p, WALL + 0.06, 0.09, 0.88, 0.06), sdRing(p, WALL + 0.06, 0.045, 0.42, 0.42));
    float3 b = sector(p, 64.0, 0.0) - float3(WALL + 0.06, 0, 0);
    rail = max(rail, -sdBox(b - float3(0, 0.44, 0), float3(0.2, 0.26, 0.2)));   // the openings between its posts
    float3 q = sector(p, 16.0, TAU / 32.0) - float3(COLONNADE, 0, 0);
    float outer = column(q, 0.4, COLUMN_H, 20.0);
    outer = min(outer, sdRing(p, COLONNADE, 0.62, COLUMN_H + 0.45, 0.45) - 0.02);
    outer = min(outer, sdRing(p, COLONNADE, 0.8, COLUMN_H + 1.0, 0.1) - 0.02);
    float pier = sdBox(p - float3(0, 3.0, -7.95), float3(2.3, 3.0, 0.6)) - 0.02;
    pier = min(pier, sdBox(p - float3(0, 2.25, -7.3), float3(1.95, 2.25, 0.12)) - 0.02);
    d = nearer(d, float2(min(min(rail, outer), pier), 2.0));
    d = nearer(d, float2(sdBox(p - float3(0, 2.1, -7.02), float3(1.7, 2.1, 0.18)) - 0.01, 3.0));
  } else { d.x = min(d.x, 6.82 - r); }
  return d;
}
static float3 normal_at(float3 p, float time) {
  const float2 e = float2(0.0015, -0.0015);
  return normalize(e.xyy * map(p + e.xyy, time, true).x + e.yyx * map(p + e.yyx, time, true).x + e.yxy * map(p + e.yxy, time, true).x + e.xxx * map(p + e.xxx, time, true).x);
}
struct Hit { float t; float mat; float glow; };
// The nearest surface along a ray, and how close the ray came to a crystal (its halo).
static Hit march(float3 ro, float3 rd, float time, int steps, float far, bool ground) {
  Hit h; h.t = 0.02; h.mat = 0.0; h.glow = 100.0;
  float last = 0.0;
  for (int i = 0; i < steps; i++) {
    float3 p = ro + rd * h.t;
    float2 d = map(p, time, ground);
    last = d.y;
    if (d.y == 5.0) { h.glow = min(h.glow, d.x); }
    if (d.x < 0.002 * h.t + 0.001) { h.mat = d.y; return h; }
    h.t += d.x * 0.96;
    if (h.t > far) { h.t = far; return h; }
  }
  // Out of steps short of the far end: the ray was closing on something, so it is that.
  h.mat = last; return h;
}
// How much of the moon reaches a point: a soft shadow. Each step keeps the narrowest gap the ray
// squeezed through, measured between this step and the last so the penumbra has no terraces, and
// `jitter` starts each pixel's ray at a slightly different place so what banding is left is grain.
static float shadow(float3 ro, float3 rd, float time, float far, float jitter) {
  float res = 1.0, t = 0.04 + 0.12 * jitter, before = 1e20;
  for (int i = 0; i < 16; i++) {
    float d = map(ro + rd * t, time, true).x;
    if (d < 0.001) { return 0.0; }
    float y = d * d / (2.0 * before);
    float gap = sqrt(max(d * d - y * y, 0.0));
    res = min(res, 6.0 * gap / max(0.0001, t - y));
    before = d;
    t += clamp(d, 0.05, 1.1);
    if (t > far) { break; }
  }
  res = saturate(res);
  return res * res * (3.0 - 2.0 * res);
}

// The night: stars, the moon, and below the horizon a sea of cloud with a far city's lamps in its gaps.
static float3 sky(float3 ro, float3 rd, float time) {
  float3 moon = normalize(MOON);
  float up = saturate(rd.y);
  float3 c = mix(float3(0.035, 0.07, 0.14), float3(0.004, 0.008, 0.028), pow(up, 0.5));
  float3 g = rd * 190.0; float3 cell = floor(g);
  float star = hash3(cell);
  float twinkle = 0.6 + 0.4 * sin(time * (1.0 + star * 3.0) + star * 40.0);
  c += step(0.9972, star) * twinkle * (0.35 + 0.65 * hash3(cell + 7.0)) * smoothstep(0.02, 0.25, rd.y) * float3(0.9, 0.95, 1.0) * smoothstep(0.75, 0.2, length(fract(g) - 0.5));
  float m = dot(rd, moon);
  c += float3(1.0, 0.96, 0.86) * (smoothstep(0.99915, 0.9994, m) * 2.4 + pow(saturate(m), 900.0) * 0.5 + pow(saturate(m), 40.0) * 0.14);
  if (rd.y < 0.0) {
    float t = (ro.y + 46.0) / -rd.y;
    float2 at = ro.xz + rd.xz * t;
    float n = fbm2(at * 0.012 + float2(time * 0.004, 0.0));
    float billow = fbm2(at * 0.05 + 3.0);
    float cloud = smoothstep(0.38, 0.72, n + 0.18 * billow);
    float lit = 0.35 + 0.65 * saturate(dot(normalize(float3(moon.x, 0.0, moon.z)), normalize(float3(rd.x, 0.0, rd.z))));
    float3 sea = mix(float3(0.006, 0.012, 0.03), float3(0.13, 0.17, 0.26) * lit, cloud * (0.45 + 0.55 * billow));
    // The city: lamps in the gaps, gathered into districts.
    float2 lamp = floor(at * 0.33);
    float district = smoothstep(0.5, 0.75, fbm2(at * 0.006 + 20.0));
    float on = step(0.72, hash1(lamp)) * district * (1.0 - cloud) * smoothstep(0.5, 0.08, length(fract(at * 0.33) - 0.5));
    sea += AMBER * on * 5.0 + AMBER * district * (1.0 - cloud) * 0.05;
    float far = saturate(t / 900.0);
    c = mix(sea, float3(0.035, 0.07, 0.14), far * far);
  }
  return c;
}

struct Lit { float3 colour; float3 normal; float polish; };
// A surface point's colour under the temple's lights. `cheap` leaves the shadow march out (reflections).
static Lit shade(float3 p, float3 rd, float mat, float time, bool cheap, float jitter) {
  Lit o; o.polish = 0.0;
  float3 n = normal_at(p, time); o.normal = n;
  float r = length(p.xz);
  float3 albedo = float3(0.8); float3 glow = float3(0.0); float gloss = 0.25;
  if (mat == 1.0) {
    // The floor: rings of pale and slate marble, cut into radial stones, with inlaid lines of light.
    float a = atan2(p.z, p.x);
    float ring = floor(r / 1.45);
    float stones = 12.0 + ring * 6.0;
    // Pale stone, with slate for the dais in the middle and for a border round each ring.
    float edge = abs(fract(r / 1.45) - 0.5) * 2.0;
    float tile = max(smoothstep(1.5, 1.42, r), smoothstep(0.78, 0.86, edge) * step(1.45, r));
    float vein = fbm2(p.xz * 1.7 + ring * 3.0);
    float3 pale = float3(0.56, 0.52, 0.45) * (0.78 + 0.3 * vein);
    float3 slate = float3(0.10, 0.13, 0.19) * (0.8 + 0.5 * vein);
    albedo = mix(pale, slate, tile);
    // The joints between stones: how far, in metres, to the nearest ring line or radial line.
    float fr = fract(r / 1.45), fa = fract(a / TAU * stones + ring * 0.5);
    float joint = min(min(fr, 1.0 - fr) * 1.45, min(fa, 1.0 - fa) * (r * TAU / stones));
    albedo *= mix(0.45, 1.0, smoothstep(0.004, 0.02, joint));
    if (p.y < -0.01) { albedo = float3(0.74, 0.72, 0.68) * (0.85 + 0.2 * vein); }
    // The dais: two rings of light round the golem, and one under the pylons.
    float lines = exp(-pow((r - 1.45) * 60.0, 2.0)) + 0.6 * exp(-pow((r - 1.9) * 80.0, 2.0)) + 0.5 * exp(-pow((r - PYLON_RING) * 60.0, 2.0)) + 0.3 * exp(-pow((r - 2.9) * 80.0, 2.0));
    float dash = 0.6 + 0.4 * step(0.25, fract(a / TAU * 48.0 - time * 0.02));
    // A line of light at the balustrade's foot.
    lines += 1.4 * exp(-pow((r - (WALL - 0.08)) * 40.0, 2.0));
    if (p.y > -0.01) { glow += CYAN * lines * dash * (0.75 + 0.1 * sin(time * 1.3)); }
    o.polish = p.y > -0.01 ? mix(0.34, 0.55, tile) : 0.0;
    gloss = 0.7;
  } else if (mat == 2.0) {
    float vein = noise3(p * float3(2.2, 0.7, 2.2)) * 0.6 + noise3(p * 7.0) * 0.4;
    albedo = float3(0.80, 0.74, 0.64) * (0.8 + 0.24 * vein);
    gloss = 0.35;
    // The pylons' bands of light, and the strip of light under the entablature.
    if (r > 3.6 && r < 5.6 && p.y > 0.3) {
      float band = exp(-pow((p.y - 0.78) * 70.0, 2.0)) + exp(-pow((p.y - (PYLON_H - 0.62)) * 70.0, 2.0));
      float runes = step(0.5, hash1(float2(floor(atan2(p.z, p.x) * 20.0), floor(p.y * 14.0)))) * exp(-pow((p.y - 1.9) * 9.0, 2.0)) * 0.22;
      glow += CYAN * (band * (1.1 + 0.3 * sin(time * 1.7)) + runes);
    }
    if (p.y > COLUMN_H - 0.02 && p.y < COLUMN_H + 0.06 && r < COLONNADE - 0.3) { glow += CYAN * 1.1; }
    if (p.y > COLUMN_H + 0.4 && p.y < COLUMN_H + 0.5 && r < COLONNADE - 0.5) { glow += CYAN * 0.5; }
  } else if (mat == 3.0) {
    // The door: old bronze in two leaves, six panels each, and the marks across it still hot.
    float2 q = float2(p.x, p.y);
    float leaf = smoothstep(0.0, 0.03, abs(q.x));
    float2 panel = float2(fract(abs(q.x) / 0.82 + 0.02), fract(q.y / 1.38 + 0.02));
    float inset = smoothstep(0.06, 0.1, panel.x) * smoothstep(0.06, 0.1, 1.0 - panel.x) * smoothstep(0.05, 0.09, panel.y) * smoothstep(0.05, 0.09, 1.0 - panel.y);
    albedo = mix(float3(0.16, 0.105, 0.06), float3(0.3, 0.2, 0.1), inset) * (0.7 + 0.5 * noise3(p * 9.0)) * (0.4 + 0.6 * leaf);
    albedo = mix(albedo, float3(0.12, 0.2, 0.17), 0.35 * noise3(p * 3.0));
    gloss = 0.6;
    for (int k = 0; k < 3; k++) {
      float x0 = -0.9 + 0.4 * float(k), slope = 0.34;
      float dline = abs((q.x - x0) - (3.35 - q.y) * slope) / sqrt(1.0 + slope * slope);
      float along = smoothstep(1.2, 1.5, q.y) * smoothstep(3.45, 3.2, q.y);
      glow += AMBER * (exp(-dline * 55.0) * 2.4 + exp(-dline * 9.0) * 0.25) * along * (0.8 + 0.2 * sin(time * 2.3 + float(k)));
    }
  } else if (mat == 4.0) {
    // The golem: dark stone shot through with a night sky.
    float grain = noise3(p * 14.0);
    albedo = float3(0.07, 0.075, 0.095) * (0.7 + 0.6 * grain);
    float3 g = p * 46.0;
    glow += float3(0.7, 0.85, 1.0) * step(0.985, hash3(floor(g))) * smoothstep(0.5, 0.1, length(fract(g) - 0.5)) * 1.4;
    // Lines of pale light where its pieces were joined, and the glow of the sphere on its open chest.
    float seam = exp(-abs(fract(p.y * 2.2 + 0.2) - 0.5) * 90.0) * step(0.5, p.y);
    glow += CYAN * seam * 0.35;
    glow += AMBER * 0.6 * exp(-length(p - CORE) * 7.0);
    gloss = 0.5;
  } else if (mat == 5.0) {
    albedo = float3(0.2, 0.4, 0.5); glow += CYAN * (0.9 + 0.5 * pow(1.0 - saturate(dot(n, -rd)), 2.0) + 0.2 * sin(time * 2.1)); gloss = 0.9;
  } else {
    albedo = float3(0.4, 0.2, 0.1); glow += AMBER * (1.5 + 1.2 * pow(saturate(dot(n, -rd)), 2.0));
  }
  float3 moon = normalize(MOON);
  float3 view = -rd;
  // The moon, with shadows.
  float direct = saturate(dot(n, moon));
  if (!cheap && direct > 0.0) { direct *= shadow(p + n * 0.02, moon, time, 18.0, jitter); }
  float3 light = float3(0.42, 0.55, 0.95) * direct * 0.75;
  // The sky's own light, less in corners.
  float occ = cheap ? 1.0 : saturate(0.35 + map(p + n * 0.35, time, true).x * 2.0);
  light += float3(0.03, 0.05, 0.10) * (0.55 + 0.45 * n.y) * occ;
  // The sphere in the golem's chest: a warm pool in the middle of the floor.
  float3 to = CORE + float3(0, 0, 0.15) - p; float dc = length(to);
  light += AMBER * saturate(dot(n, to / dc)) * 2.6 / (1.0 + 0.6 * dc * dc) * occ;
  // The nearest pylon's crystal, and the strip of light under the entablature.
  float3 q = sector(p, 6.0, 0.0);
  float3 tc = crystal_at(time) - q; float dq = length(tc);
  float3 nq = sector(p + n, 6.0, 0.0) - q;
  light += CYAN * saturate(dot(nq, tc / dq) * 0.8 + 0.2) * 1.1 / (1.0 + 0.45 * dq * dq);
  float3 ts = float3(q.x / max(r, 0.001) * (COLONNADE - 0.7), COLUMN_H, 0.0) - float3(r, p.y, 0.0);
  light += CYAN * saturate(dot(float3(nq.x, n.y, 0.0), normalize(ts))) * 0.5 / (1.0 + 0.1 * dot(ts, ts));
  // The colonnade's lamps, one at each outer column's foot, throwing amber up the stone.
  float3 oq = sector(p, 16.0, TAU / 32.0);
  float3 lamp = float3(COLONNADE - 0.75, 0.35, 0.0) - oq; float dl = length(lamp);
  float3 on = sector(p + n, 16.0, TAU / 32.0) - oq;
  light += AMBER * saturate(dot(on, lamp / dl) * 0.8 + 0.2) * 2.6 / (1.0 + 0.85 * dl * dl) * occ;
  float3 c = albedo * light;
  float3 h = normalize(moon + view);
  c += float3(0.6, 0.7, 1.0) * pow(saturate(dot(n, h)), 40.0) * gloss * direct * 0.6;
  c += glow;
  o.colour = c;
  return o;
}

// A display hung in the air: a frame, a title, rows that fill and empty, a dial, and its scan lines.
static float3 display(float2 uv, float id, float time) {
  if (abs(uv.x) > 1.0 || abs(uv.y) > 1.0) { return float3(0.0); }
  float2 e = 1.0 - abs(uv);
  float a = 0.05;
  a += smoothstep(0.03, 0.0, min(e.x * 1.45, e.y)) * 0.9;                       // the frame
  a += step(0.78, uv.y) * step(uv.x, -0.25) * step(e.x, 0.93) * step(0.08, e.y) * 0.5;   // the title's bar
  float row = floor((uv.y + 0.62) / 0.2);
  if (uv.y > -0.62 && uv.y < 0.62 && uv.x < 0.18) {
    float fill = 0.25 + 0.7 * hash1(float2(row, id)) * (0.6 + 0.4 * sin(time * (0.4 + hash1(float2(id, row))) + row));
    float y = fract((uv.y + 0.62) / 0.2);
    a += step(0.3, y) * step(y, 0.62) * step(uv.x, -0.88 + fill) * step(-0.88, uv.x) * 0.75;
    a += step(0.3, y) * step(y, 0.62) * step(-0.88, uv.x) * 0.08;
  }
  float2 c = (uv - float2(0.58, 0.02)) * float2(0.72, 1.0); float rr = length(c);
  a += smoothstep(0.012, 0.0, abs(rr - 0.3)) * 0.8 + smoothstep(0.012, 0.0, abs(rr - 0.21)) * 0.35;
  float sweep = fract(atan2(c.y, c.x) / TAU - time * 0.12 - id * 0.3);
  a += step(rr, 0.3) * step(0.06, rr) * pow(sweep, 5.0) * 0.6;
  for (int k = 0; k < 5; k++) { float t = uv.x * 2.4 + float(k); a += step(-0.9, uv.y) * step(uv.y, -0.74) * step(fract(t * 1.7 + hash1(float2(float(k), id))), 0.5) * step(-0.9, uv.x) * step(uv.x, 0.9) * 0.06; }
  a *= 0.82 + 0.18 * sin(uv.y * 150.0 + time * 6.0);
  a *= 0.9 + 0.1 * sin(time * 9.0 + id * 5.0);
  return CYAN * a;
}

// The place is drawn at half the picture's size into two images, its colour and its depth, and
// place_fs lays them over the full picture: a quarter of the work, and your model and the text
// over it stay sharp.
struct Out { float4 colour [[color(0)]]; float depth [[color(1)]]; };
struct Laid { float4 colour [[color(0)]]; float depth [[depth(any)]]; };
struct PlaceV { float4 pos [[position]]; float2 uv; };
vertex PlaceV place_vs(uint i [[vertex_id]]) {
  const float2 p[3] = {float2(-1.0, -3.0), float2(-1.0, 1.0), float2(3.0, 1.0)};
  PlaceV o; o.pos = float4(p[i], 1.0, 1.0); o.uv = float2(p[i].x * 0.5 + 0.5, 0.5 - p[i].y * 0.5);
  return o;
}
fragment Laid place_fs(PlaceV v [[stage_in]], texture2d<float> colour [[texture(0)]], texture2d<float> depth [[texture(1)]], sampler smooth_s [[sampler(0)]], sampler near_s [[sampler(1)]]) {
  Laid o;
  o.colour = float4(colour.sample(smooth_s, v.uv).rgb, 1.0);
  o.depth = depth.sample(near_s, v.uv).r;
  return o;
}

vertex float4 temple_vs(uint i [[vertex_id]]) {
  const float2 p[3] = {float2(-1.0, -3.0), float2(-1.0, 1.0), float2(3.0, 1.0)};
  return float4(p[i], 1.0, 1.0);
}
fragment Out temple_fs(float4 pos [[position]], constant TU &u [[buffer(0)]]) {
  Out o;
  float3 ro = u.eye.xyz;
  float3 rd = normalize(u.forward.xyz * u.forward.w + u.right.xyz * (pos.x - u.size.x * 0.5) + u.up.xyz * (u.size.y * 0.5 - pos.y));
  float time = u.time;
  const float FAR = 70.0;
  Hit h = march(ro, rd, time, 80, FAR, true);
  float3 c;
  if (h.mat == 0.0) { c = sky(ro, rd, time); o.depth = 1.0; }
  else {
    float3 p = ro + rd * h.t;
    Lit s = shade(p, rd, h.mat, time, false, hash1(pos.xy));
    c = s.colour;
    if (s.polish > 0.0) {
      // Polished stone: what stands on it, and the sky, mirrored once.
      float3 rr = reflect(rd, s.normal);
      Hit m = march(p + s.normal * 0.02, rr, time, 48, 30.0, false);
      float3 mirrored = m.mat == 0.0 ? sky(p, rr, time) * 0.8 : shade(p + rr * m.t, rr, m.mat, time, true, 0.0).colour;
      float fresnel = 0.07 + 0.6 * pow(1.0 - saturate(dot(-rd, s.normal)), 3.0);
      c = mix(c, mirrored, fresnel * s.polish);
      // You stand on it: a soft dark under your feet.
      float2 f = p.xz - u.you.xz; float lift = max(u.you.y, 0.0);
      c *= 1.0 - 0.55 * exp(-dot(f, f) / (0.09 + 0.3 * lift)) / (1.0 + 2.0 * lift);
    }
    c = mix(c, float3(0.03, 0.055, 0.11), 1.0 - exp(-h.t * 0.006));
    float z = h.t * dot(rd, u.forward.xyz);
    const float near = 0.1, far = 200.0;
    o.depth = saturate(far * (z - near) / ((far - near) * z));
  }
  // Light in the air: the crystals' halos, and the threads from each crystal to the golem's sphere.
  c += CYAN * 0.006 / (h.glow * h.glow + 0.012);
  for (int k = 0; k < 6; k++) {
    float a = TAU / 6.0 * float(k);
    float3 top = float3(cos(a) * PYLON_RING, crystal_at(time).y, sin(a) * PYLON_RING);
    float3 ab = CORE - top, ao = ro - top;
    float bb = dot(ab, ab), bd = dot(ab, rd), od = dot(ao, rd), ob = dot(ao, ab);
    float den = bb - bd * bd;
    float s = saturate((ob * -1.0 + od * bd) / max(den, 0.0001));
    float t = dot(top + ab * s - ro, rd);
    if (t > 0.0 && t < h.t) {
      float d = length(top + ab * s - (ro + rd * t));
      float pulse = 0.5 + 0.5 * sin(time * 2.0 - s * 9.0 + float(k));
      c += CYAN * (0.0009 / (d * d + 0.0007)) * (0.25 + 0.75 * pulse) * 0.5;
    }
  }
  // The displays, hung in the gaps between the pylons: two flanking the door, two at the sides
  // nearer you. Each faces the middle. They are light, so you walk through them.
  const float where[4] = {TAU * 7.0 / 12.0, TAU * 11.0 / 12.0, TAU * 5.0 / 12.0, TAU * 1.0 / 12.0};
  for (int k = 0; k < 4; k++) {
    float a = where[k];
    float3 nrm = float3(-cos(a), 0.0, -sin(a));
    float3 centre = float3(cos(a) * PYLON_RING, 2.0 + 0.04 * sin(time * 0.8 + float(k)), sin(a) * PYLON_RING);
    float den = dot(rd, nrm);
    float t = dot(centre - ro, nrm) / den;
    if (abs(den) > 0.0001 && t > 0.0 && t < h.t) {
      float3 q = ro + rd * t - centre;
      float2 uv = float2(dot(q, float3(-nrm.z, 0.0, nrm.x)) / 1.05, q.y / 0.62);
      c += display(uv, float(k), time) * (den < 0.0 ? 0.95 : 0.5);
    }
  }
  // To the screen: a filmic curve, a little grain.
  c *= 0.95;
  c = c * (2.51 * c + 0.03) / (c * (2.43 * c + 0.59) + 0.14);
  c = pow(saturate(c), float3(1.0 / 2.2));
  c += (hash1(pos.xy + fract(time) * 100.0) - 0.5) * 0.008;
  o.colour = float4(c, 1.0);
  return o;
}
