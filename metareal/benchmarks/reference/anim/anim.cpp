#include "anim.h"

#include <algorithm>
#include <cmath>
#include <cstring>

namespace anim {

Quat mul(Quat a, Quat b) {
  return {a.w * b.x + b.w * a.x + (a.y * b.z - a.z * b.y), a.w * b.y + b.w * a.y + (a.z * b.x - a.x * b.z),
          a.w * b.z + b.w * a.z + (a.x * b.y - a.y * b.x), a.w * b.w - (a.x * b.x + a.y * b.y + a.z * b.z)};
}

Quat conj(Quat q) { return {-q.x, -q.y, -q.z, q.w}; }

Vec3 rotate(Quat q, Vec3 v) {
  Vec3 u{q.x, q.y, q.z};
  Vec3 t = cross(u, v) * 2.0f;
  return v + t * q.w + cross(u, t);
}

Quat nlerp(Quat a, Quat b, float t) {
  float s = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w < 0 ? -1.0f : 1.0f;
  Quat q{a.x + (s * b.x - a.x) * t, a.y + (s * b.y - a.y) * t, a.z + (s * b.z - a.z) * t, a.w + (s * b.w - a.w) * t};
  float l = std::sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
  if (l < 1e-8f) return a;
  return {q.x / l, q.y / l, q.z / l, q.w / l};
}

Vec3 apply(const Rigid& r, Vec3 v) { return r.hidden ? r.t : rotate(r.q, v) + r.t; }

void Pose::add(const Layer& l) {
  if (count < kMaxLayers) layers[count++] = l;
  else layers[kMaxLayers - 1] = l;
}

void Pose::hide(int joint) {
  if (joint >= 0 && joint < kMaxJoints) hidden[joint / 32] |= 1u << (joint % 32);
}

bool Pose::isHidden(int joint) const {
  return joint >= 0 && joint < kMaxJoints && (hidden[joint / 32] >> (joint % 32) & 1u);
}

Pose Pose::clip(int clip, float phase, int fromClip, float fromPhase, float fromWeight, bool loop, bool fromLoop) {
  Pose p;
  if (fromClip >= 0 && fromWeight > 0) {
    Layer from;
    from.clip = fromClip, from.phase = fromPhase, from.loop = fromLoop;
    p.add(from);
  }
  Layer to;
  to.clip = clip, to.phase = phase, to.loop = loop;
  to.weight = p.count ? std::clamp(1 - fromWeight, 0.0f, 1.0f) : 1.0f;
  p.add(to);
  return p;
}

float halfToFloat(uint16_t h) {
  uint32_t sign = (uint32_t)(h & 0x8000) << 16, exp = (h >> 10) & 31, man = h & 1023, bits;
  if (exp == 0) {
    if (man == 0) {
      bits = sign;
    } else {  // subnormal
      int e = -1;
      do ++e, man <<= 1; while (!(man & 1024));
      bits = sign | (uint32_t)(127 - 15 - e) << 23 | (man & 1023) << 13;
    }
  } else if (exp == 31) {
    bits = sign | 0x7f800000u | man << 13;
  } else {
    bits = sign | (exp + 127 - 15) << 23 | man << 13;
  }
  float f;
  std::memcpy(&f, &bits, 4);
  return f;
}

uint16_t floatToHalf(float f) {
  uint32_t x;
  std::memcpy(&x, &f, 4);
  uint32_t sign = (x >> 16) & 0x8000;
  int exp = (int)((x >> 23) & 255) - 127 + 15;
  uint32_t man = x & 0x7fffff;
  if (exp <= 0) {
    if (exp < -10) return (uint16_t)sign;
    man |= 0x800000;
    uint32_t shift = (uint32_t)(14 - exp);
    uint32_t h = man >> shift;
    if ((man >> (shift - 1)) & 1) ++h;  // round
    return (uint16_t)(sign | h);
  }
  if (exp >= 31) return (uint16_t)(sign | 0x7c00);
  uint32_t h = sign | (uint32_t)exp << 10 | man >> 13;
  if (man & 0x1000) ++h;  // round to nearest
  return (uint16_t)h;
}

int Data::joint(const char* name) const {
  for (int i = 0; i < joints; ++i)
    if (names[i] == name) return i;
  return -1;
}

int Data::clipId(const char* name) const {
  for (size_t i = 0; i < clips.size(); ++i)
    if (clips[i].name == name) return (int)i;
  return -1;
}

Rigid Data::skin(int body, uint32_t frame, int j) const {
  const uint16_t* h = texels.data() + (((size_t)body * framesPerBody + frame) * joints + j) * 8;
  Rigid r;
  r.q = {halfToFloat(h[0]), halfToFloat(h[1]), halfToFloat(h[2]), halfToFloat(h[3])};
  float l = std::sqrt(r.q.x * r.q.x + r.q.y * r.q.y + r.q.z * r.q.z + r.q.w * r.q.w);
  if (l > 0) r.q = {r.q.x / l, r.q.y / l, r.q.z / l, r.q.w / l};  // halves are not quite unit
  r.t = {halfToFloat(h[4]), halfToFloat(h[5]), halfToFloat(h[6])};
  return r;
}

namespace {

uint32_t u32(const std::vector<char>& f, size_t at) {
  uint32_t v;
  std::memcpy(&v, f.data() + at, 4);
  return v;
}

bool startsWith(const std::string& s, const char* p) { return s.rfind(p, 0) == 0; }

// The masks, by joint name (the Universal Base Characters skeleton): the
// upper body is the spine from spine_01 up with the neck, head and arms,
// coming in over the lower spine so a twist spreads over it; the lower body
// is the rest.
float upperWeight(const std::string& name) {
  if (name == "spine_01") return 0.35f;
  if (name == "spine_02") return 0.75f;
  if (startsWith(name, "spine") || startsWith(name, "neck") || name == "Head" || startsWith(name, "clavicle") ||
      startsWith(name, "upperarm") || startsWith(name, "lowerarm") || startsWith(name, "hand") ||
      startsWith(name, "index") || startsWith(name, "middle") || startsWith(name, "pinky") ||
      startsWith(name, "ring") || startsWith(name, "thumb"))
    return 1;
  return 0;
}

}  // namespace

bool load(const std::vector<char>& f, const std::vector<char>& s, Data& d, std::string* error) {
  auto fail = [&](const char* why) {
    if (error) *error = why;
    return false;
  };
  if (f.size() < 24 || std::memcmp(f.data(), "ANM1", 4) != 0) return fail("anims.bin is missing or invalid");
  uint32_t joints = u32(f, 4), bodies = u32(f, 8), frames = u32(f, 12), clipCount = u32(f, 16);
  size_t texels = (size_t)bodies * frames * joints * 2;
  if (joints == 0 || joints > (uint32_t)kMaxJoints || f.size() != 24 + clipCount * 40 + texels * 8)
    return fail("anims.bin has the wrong size");
  d.joints = (int)joints, d.bodies = (int)bodies, d.framesPerBody = frames, d.fps = (float)u32(f, 20);
  d.clips.clear();
  for (uint32_t i = 0; i < clipCount; ++i) {
    const char* p = f.data() + 24 + i * 40;
    ClipInfo c;
    c.name.assign(p, strnlen(p, 32));
    c.firstFrame = u32(f, 24 + i * 40 + 32), c.frameCount = u32(f, 24 + i * 40 + 36);
    if (c.frameCount == 0 || c.firstFrame + c.frameCount > frames) return fail("anims.bin has a bad clip");
    d.clips.push_back(c);
  }
  d.texels.resize(texels * 4);
  std::memcpy(d.texels.data(), f.data() + 24 + clipCount * 40, texels * 8);

  if (s.size() < 12 || std::memcmp(s.data(), "SKL1", 4) != 0) return fail("skeleton.bin is missing or invalid");
  if (u32(s, 4) != joints || u32(s, 8) != bodies || s.size() != 12 + joints * 36 + (size_t)bodies * joints * 12)
    return fail("skeleton.bin does not match anims.bin");
  d.names.assign(joints, {});
  d.parent.assign(joints, -1);
  for (uint32_t j = 0; j < joints; ++j) {
    const char* p = s.data() + 12 + j * 36;
    d.names[j].assign(p, strnlen(p, 32));
    int32_t parent;
    std::memcpy(&parent, p + 32, 4);
    if (parent >= (int32_t)j) return fail("skeleton.bin lists a joint before its parent");
    d.parent[j] = parent;
  }
  d.bind.resize((size_t)bodies * joints);
  std::memcpy(d.bind.data(), s.data() + 12 + joints * 36, d.bind.size() * 12);
  d.moves.assign(joints, 0);
  d.masks.assign((size_t)kMaskCount * joints, 1);
  d.maxDepth = 0;
  for (int j = 0; j < d.joints; ++j) {
    d.moves[j] = d.parent[j] < 0 || d.names[j] == "pelvis";
    float up = upperWeight(d.names[j]);
    d.masks[(size_t)kUpperBody * joints + j] = up;
    d.masks[(size_t)kLowerBody * joints + j] = 1 - up;
    int depth = 0;
    for (int k = j; k >= 0; k = d.parent[k]) ++depth;
    d.maxDepth = std::max(d.maxDepth, depth);
  }
  return true;
}

void framesAt(const Layer& l, uint32_t count, float t, float fps, uint32_t& a, uint32_t& b, float& f) {
  float pos = t * fps + l.phase;
  float n = (float)count;
  if (l.loop) {
    pos = std::fmod(pos, n);
    if (pos < 0) pos += n;
  } else {
    pos = std::clamp(pos, 0.0f, n - 1);
  }
  a = std::min((uint32_t)pos, count - 1);
  f = pos - (float)a;
  // The last frame is held rather than blended back into the first: most
  // clips do not end where they start (as the shaders do).
  b = std::min(a + 1, count - 1);
  if (b == a) f = 0;
}

namespace {

// A joint's motion relative to its parent in one baked frame: the rotation,
// and how far it carries the joint's bind position (zero for every joint
// but the root and pelvis, up to the bake's rounding).
struct Rel {
  Quat r;
  Vec3 d{0, 0, 0};
};

Rel relative(const Data& d, int body, uint32_t frame, int k) {
  Rigid sk = d.skin(body, frame, k);
  Rel out;
  Vec3 tc = sk.t;
  if (d.parent[k] >= 0) {
    Rigid sp = d.skin(body, frame, d.parent[k]);
    Quat ip = conj(sp.q);
    out.r = mul(ip, sk.q);
    tc = rotate(ip, sk.t - sp.t);
  } else {
    out.r = sk.q;
  }
  Vec3 x = d.bind[(size_t)body * d.joints + k];
  out.d = rotate(out.r, x) + tc - x;
  return out;
}

Rel sample(const Data& d, int body, const Layer& l, float t, int k) {
  const ClipInfo& c = d.clips[l.clip];
  uint32_t a, b;
  float f;
  framesAt(l, c.frameCount, t, d.fps, a, b, f);
  uint32_t base = c.firstFrame;
  Rel ra = relative(d, body, base + a, k);
  Rel out = ra;
  if (f > 0 && b != a) {
    Rel rb = relative(d, body, base + b, k);
    out.r = nlerp(ra.r, rb.r, f);
    out.d = ra.d + (rb.d - ra.d) * f;
  }
  if (l.inPlace && d.moves[k]) {
    Rel first = relative(d, body, base, k);
    out.d.x -= first.d.x, out.d.z -= first.d.z;
  }
  return out;
}

// C_k: the pose's motion of joint k relative to its parent, all layers blended.
Rigid blended(const Data& d, int body, const Pose& pose, float t, int k) {
  Quat R;
  Vec3 D{0, 0, 0};
  for (int i = 0; i < pose.count; ++i) {
    const Layer& l = pose.layers[i];
    if (l.clip < 0 || l.clip >= (int)d.clips.size()) continue;
    float w = l.weight * d.mask(l.mask, k);
    if (w <= 0) continue;
    Rel s = sample(d, body, l, t, k);
    if (l.additive) {
      Layer ref = l;
      ref.loop = false;
      ref.phase = l.refFrame - t * d.fps;
      Rel r = sample(d, body, ref, t, k);
      R = mul(R, nlerp(Quat{}, mul(conj(r.r), s.r), std::min(w, 1.0f)));
      D = D + (s.d - r.d) * w;
    } else {
      w = std::min(w, 1.0f);
      R = nlerp(R, s.r, w);
      D = D + (s.d - D) * w;
    }
  }
  Vec3 x = d.bind[(size_t)body * d.joints + k];
  Rigid c;
  c.q = R;
  c.t = x + D - rotate(R, x);
  return c;
}

}  // namespace

Rigid evaluate(const Data& d, int body, const Pose& pose, float t, int joint) {
  Rigid T;
  Vec3 P{0, 0, 0};
  bool hidden = false;
  for (int k = joint; k >= 0; k = d.parent[k]) {
    if (pose.isHidden(k)) hidden = true, P = d.bind[(size_t)body * d.joints + k];
    Rigid c = blended(d, body, pose, t, k);
    if (hidden) {
      P = rotate(c.q, P) + c.t;
    } else {
      T.t = rotate(c.q, T.t) + c.t;
      T.q = mul(c.q, T.q);
    }
  }
  if (hidden) T.hidden = true, T.t = P, T.q = Quat{};
  return T;
}

void evaluateAll(const Data& d, int body, const Pose& pose, float t, std::vector<Rigid>& out) {
  // Each joint's C once, then the chains from the root down (parents come
  // before their children).
  out.assign(d.joints, Rigid{});
  std::vector<Vec3> fold(d.joints);
  for (int k = 0; k < d.joints; ++k) {
    Rigid c = blended(d, body, pose, t, k);
    int p = d.parent[k];
    Vec3 x = d.bind[(size_t)body * d.joints + k];
    if (p >= 0 && out[p].hidden) {
      out[k] = out[p];
    } else if (pose.isHidden(k)) {
      Vec3 at = rotate(c.q, x) + c.t;
      if (p >= 0) at = rotate(out[p].q, at) + out[p].t;
      out[k].hidden = true, out[k].t = at;
    } else if (p < 0) {
      out[k] = c;
    } else {
      out[k].q = mul(out[p].q, c.q);
      out[k].t = rotate(out[p].q, c.t) + out[p].t;
    }
  }
}

Vec3 jointPosition(const Data& d, int body, const Pose& pose, float t, int joint) {
  return apply(evaluate(d, body, pose, t, joint), d.bind[(size_t)body * d.joints + joint]);
}

Vec3 travel(const Data& d, int body, int clip, float frame) {
  int pelvis = d.joint("pelvis");
  if (clip < 0 || clip >= (int)d.clips.size() || pelvis < 0) return {0, 0, 0};
  const ClipInfo& c = d.clips[clip];
  uint32_t a = std::min((uint32_t)std::max(0.0f, frame), c.frameCount - 1);
  Vec3 x = d.bind[(size_t)body * d.joints + pelvis];
  Vec3 p0 = apply(d.skin(body, c.firstFrame, pelvis), x), p1 = apply(d.skin(body, c.firstFrame + a, pelvis), x);
  return {p1.x - p0.x, 0, p1.z - p0.z};
}

}  // namespace anim
