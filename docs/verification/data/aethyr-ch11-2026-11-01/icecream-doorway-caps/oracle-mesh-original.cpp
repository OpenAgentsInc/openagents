// Ice Cream Man's models: toon-mesh's shapes and scene.rs's builders, ported
// one for one (see mesh.h).
#include "mesh.h"

#include <algorithm>
#include <array>
#include <cmath>

namespace icecream::toon {
namespace {

constexpr float kPi = 3.14159265358979f, kTau = 2 * kPi, kHalfPi = kPi / 2;

Vec3 vmin(Vec3 a, Vec3 b) { return {std::min(a.x, b.x), std::min(a.y, b.y), std::min(a.z, b.z)}; }
Vec3 vmax(Vec3 a, Vec3 b) { return {std::max(a.x, b.x), std::max(a.y, b.y), std::max(a.z, b.z)}; }
Vec3 splat3(float s) { return {s, s, s}; }
float len2(Vec3 a) { return dot(a, a); }
Vec3 normOrZero(Vec3 a) {
  float l = std::sqrt(len2(a));
  return l > 1e-20f ? a * (1 / l) : Vec3{0, 0, 0};
}
Vec3 neg(Vec3 a) { return {-a.x, -a.y, -a.z}; }
float sgn(float x) { return std::copysign(1.0f, x); }
constexpr Vec3 kY{0, 1, 0};

using Corner = std::pair<Vec3, Vec3>;  // position, hull direction
using Tri = std::array<Corner, 3>;

void quad(std::vector<Tri>& out, Corner a, Corner b, Corner c, Corner d) {
  out.push_back({a, b, c});
  out.push_back({a, c, d});
}

std::vector<Tri> triangles(Shape s) {
  std::vector<Tri> out;
  switch (s.kind) {
    case Shape::Cube: {
      auto corner = [](float x, float y, float z) {
        Vec3 p = Vec3{x, y, z} * 0.5f;
        return Corner{p, normalize(p)};
      };
      for (int axis = 0; axis < 3; ++axis)
        for (float sign : {-1.0f, 1.0f}) {
          auto v = [&](float u, float w) {
            float c[3] = {0, 0, 0};
            c[axis] = sign;
            c[(axis + 1) % 3] = u;
            c[(axis + 2) % 3] = w;
            return corner(c[0], c[1], c[2]);
          };
          quad(out, v(-1, -1), v(1, -1), v(1, 1), v(-1, 1));
        }
      break;
    }
    case Shape::Frustum: {
      uint32_t sides = s.a;
      float bottom = s.p, top = s.q;
      auto ring = [&](float r, float y, uint32_t i) {
        float a = (float)i / (float)sides * kTau;
        Vec3 p{r * std::cos(a), y, r * std::sin(a)};
        return Corner{p, normalize(Vec3{std::cos(a), y * 1.2f, std::sin(a)})};
      };
      Corner up{{0, 0.5f, 0}, kY}, down{{0, -0.5f, 0}, {0, -1, 0}};
      for (uint32_t i = 0; i < sides; ++i) {
        Corner b0 = ring(bottom, -0.5f, i), b1 = ring(bottom, -0.5f, i + 1);
        if (top > 0) {
          Corner t0 = ring(top, 0.5f, i), t1 = ring(top, 0.5f, i + 1);
          quad(out, b0, b1, t1, t0);
          out.push_back({up, t0, t1});
        } else {
          out.push_back({b0, b1, up});
        }
        out.push_back({down, b1, b0});
      }
      break;
    }
    case Shape::Sphere: {
      uint32_t rings = s.a, segments = s.b;
      auto point = [&](uint32_t ring, uint32_t seg) {
        float lat = (float)ring / (float)rings * kPi, lon = (float)seg / (float)segments * kTau;
        Vec3 n{std::sin(lat) * std::cos(lon), std::cos(lat), std::sin(lat) * std::sin(lon)};
        return Corner{n * 0.5f, n};
      };
      for (uint32_t r = 0; r < rings; ++r)
        for (uint32_t sg = 0; sg < segments; ++sg) {
          Corner a = point(r, sg), b = point(r, sg + 1), c = point(r + 1, sg + 1), d = point(r + 1, sg);
          if (r == 0) out.push_back({a, c, d});
          else if (r == rings - 1) out.push_back({a, b, c});
          else quad(out, a, b, c, d);
        }
      break;
    }
    case Shape::Torus: {
      uint32_t segments = s.a, sides = s.b;
      float radius = s.p, tube = s.q;
      auto point = [&](uint32_t i, uint32_t j) {
        float u = (float)i / (float)segments * kTau, v = (float)j / (float)sides * kTau;
        Vec3 centre = Vec3{std::cos(u), std::sin(u), 0} * radius;
        Vec3 n{std::cos(u) * std::cos(v), std::sin(u) * std::cos(v), std::sin(v)};
        return Corner{centre + n * tube, n};
      };
      for (uint32_t i = 0; i < segments; ++i)
        for (uint32_t j = 0; j < sides; ++j) quad(out, point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
      break;
    }
    case Shape::Square: {
      auto corner = [](float x, float z) {
        Vec3 p{x * 0.5f, 0, z * 0.5f};
        return Corner{p, normalize(normalize(p) + kY)};
      };
      quad(out, corner(-1, -1), corner(-1, 1), corner(1, 1), corner(1, -1));
      break;
    }
    case Shape::Disc: {
      uint32_t sides = s.a;
      Corner centre{{0, 0, 0}, kY};
      auto at = [&](uint32_t i) {
        float a = (float)i / (float)sides * kTau;
        Vec3 p{0.5f * std::cos(a), 0, 0.5f * std::sin(a)};
        return Corner{p, normalize(normalize(p) + kY)};
      };
      for (uint32_t i = 0; i < sides; ++i) out.push_back({centre, at(i), at(i + 1)});
      break;
    }
  }
  return out;
}

// The inverse transpose of a 3x3 (row-major), for normals.
void inverseTranspose(const float m[9], float out[9]) {
  float a = m[0], b = m[1], c = m[2], d = m[3], e = m[4], f = m[5], g = m[6], h = m[7], i = m[8];
  float A = e * i - f * h, B = -(d * i - f * g), C = d * h - e * g;
  float D = -(b * i - c * h), E = a * i - c * g, F = -(a * h - b * g);
  float G = b * f - c * e, H = -(a * f - c * d), I = a * e - b * d;
  float det = a * A + b * B + c * C;
  float k = std::fabs(det) > 1e-30f ? 1 / det : 0;
  // inverse = adjugate / det, adjugate = cofactor transposed; its transpose
  // is the cofactor matrix itself.
  float co[9] = {A, B, C, D, E, F, G, H, I};
  for (int n = 0; n < 9; ++n) out[n] = co[n] * k;
}

Vec3 apply3(const float m[9], Vec3 p) {
  return {m[0] * p.x + m[1] * p.y + m[2] * p.z, m[3] * p.x + m[4] * p.y + m[5] * p.z,
          m[6] * p.x + m[7] * p.y + m[8] * p.z};
}

uint32_t hashU(uint32_t a, uint32_t b) {
  uint32_t x = (a * 0x9E3779B9u) ^ (b * 0x85EBCA6Bu);
  x ^= x >> 15;
  x *= 0x2C1B3C6Du;
  x ^= x >> 12;
  return x;
}
float noise(uint32_t a, uint32_t b) { return (float)(hashU(a, b) % 10000) / 10000.0f; }

void boxed(Mesh& m, Vec3 lo, Vec3 hi, uint32_t colour, float weight) { m.block(lo, hi, colour, weight); }
Vec3 v3(V2 p, float y) { return {p.x, y, p.z}; }

// The face of a box that looks toward `toward`, as a unit axis.
Vec3 front(const Block& b, V2 toward) {
  V2 c = (b.min + b.max) * 0.5f;
  V2 d = toward - c;
  if (std::fabs(d.x) > std::fabs(d.z)) return {sgn(d.x), 0, 0};
  return {0, 0, sgn(d.z)};
}

const Flavor kFlavors[4] = {Flavor::Strawberry, Flavor::Mint, Flavor::Vanilla, Flavor::Chocolate};
const Side kSidesAll[4] = {Side::North, Side::East, Side::South, Side::West};

void prop(Mesh& mesh, PropKind kind, const Block& b, V2 room, uint32_t wallColour, uint32_t carpetColour, uint32_t trim) {
  Vec3 lo = v3(b.min, 0), hi = v3(b.max, b.height);
  Vec3 mid = (lo + hi) * 0.5f, size = hi - lo;
  Vec3 f = front(b, room);
  auto face = [&](float y) { return Vec3{mid.x + f.x * size.x / 2, y, mid.z + f.z * size.z / 2}; };
  Vec3 along{std::fabs(f.z), 0, std::fabs(f.x)};
  const float w = 1;
  // A back slab on the side away from the room (sofa, toilet tank).
  auto backLo = [&](float y) {
    return Vec3{f.x > 0 ? lo.x : f.x < 0 ? hi.x - 0.25f : lo.x, y, f.z > 0 ? lo.z : f.z < 0 ? hi.z - 0.25f : lo.z};
  };
  auto backHi = [&]() { return Vec3{f.x > 0 ? lo.x + 0.25f : hi.x, hi.y, f.z > 0 ? lo.z + 0.25f : hi.z}; };
  switch (kind) {
    case PropKind::Bed: {
      boxed(mesh, lo, {hi.x, 0.35f, hi.z}, 0x8A5A40, w);
      boxed(mesh, lo + Vec3{0.08f, 0.35f, 0.08f}, {hi.x - 0.08f, 0.55f, hi.z - 0.08f}, kWhite, w);
      bool longX = size.x > size.z;
      Vec3 bl, bh, pl, ph;
      if (longX) {
        float toward = room.x > mid.x ? 1.0f : -1.0f;
        float split = mid.x - toward * size.x * 0.1f;
        float a = toward > 0 ? split : lo.x, e = toward > 0 ? hi.x : split;
        float wallEnd = toward > 0 ? lo.x : hi.x;
        bl = {a, 0.55f, lo.z + 0.05f}, bh = {e, 0.62f, hi.z - 0.05f};
        pl = {std::min(wallEnd, wallEnd + toward * 0.7f), 0.55f, mid.z - size.z * 0.3f};
        ph = {std::max(wallEnd, wallEnd + toward * 0.7f), 0.72f, mid.z + size.z * 0.3f};
      } else {
        float toward = room.z > mid.z ? 1.0f : -1.0f;
        float split = mid.z - toward * size.z * 0.1f;
        float a = toward > 0 ? split : lo.z, e = toward > 0 ? hi.z : split;
        float wallEnd = toward > 0 ? lo.z : hi.z;
        bl = {lo.x + 0.05f, 0.55f, a}, bh = {hi.x - 0.05f, 0.62f, e};
        pl = {mid.x - size.x * 0.3f, 0.55f, std::min(wallEnd, wallEnd + toward * 0.7f)};
        ph = {mid.x + size.x * 0.3f, 0.72f, std::max(wallEnd, wallEnd + toward * 0.7f)};
      }
      boxed(mesh, bl, bh, wall(Flavor::Strawberry), w);
      boxed(mesh, pl, ph, 0xFFDCE8, w);
      break;
    }
    case PropKind::Closet: {
      boxed(mesh, lo, hi, 0xE8C99B, w);
      Vec3 seam = face(hi.y / 2);
      boxed(mesh, seam - Vec3{0.03f, hi.y / 2 - 0.1f, 0.03f}, seam + Vec3{0.03f, hi.y / 2 - 0.1f, 0.03f} + f * 0.02f,
            0x9C7650, 0);
      for (float s : {-1.0f, 1.0f}) {
        Vec3 knob = face(1.2f) + along * (0.18f * s) + f * 0.05f;
        boxed(mesh, knob - splat3(0.05f), knob + splat3(0.05f), kGold, 0);
      }
      break;
    }
    case PropKind::Dresser: {
      boxed(mesh, lo, hi, 0xC98B5E, w);
      for (int i = 0; i < 3; ++i) {
        Vec3 line = face(0.2f + (float)i * 0.3f);
        float r = std::max(size.x, size.z) * 0.4f;
        boxed(mesh, line - along * r - kY * 0.02f, line + along * r + kY * 0.02f + f * 0.02f, 0x8A5A40, 0);
      }
      break;
    }
    case PropKind::Counter:
      boxed(mesh, lo, {hi.x, hi.y - 0.1f, hi.z}, 0x9ED9C0, w);
      boxed(mesh, {lo.x - 0.03f, hi.y - 0.1f, lo.z - 0.03f}, hi + Vec3{0.03f, 0, 0.03f}, kWhite, w);
      break;
    case PropKind::Fridge: {
      boxed(mesh, lo, hi, 0xF2F5F8, w);
      Vec3 handle = face(hi.y * 0.55f) + along * (std::min(size.x, size.z) * 0.3f) + f * 0.04f;
      boxed(mesh, handle - Vec3{0.04f, 0.35f, 0.04f}, handle + Vec3{0.04f, 0.35f, 0.04f}, 0x9AA4AE, 0);
      Vec3 magnet = face(hi.y * 0.75f) - along * 0.2f + f * 0.03f;
      boxed(mesh, magnet - splat3(0.07f), magnet + splat3(0.07f), 0xFF6FA8, 0);
      Vec3 seam = face(hi.y * 0.7f) + f * 0.01f;
      float r = std::min(size.x, size.z) * 0.48f;
      boxed(mesh, seam - along * r - kY * 0.015f, seam + along * r + kY * 0.015f, 0xB8C2CC, 0);
      break;
    }
    case PropKind::Table: {
      float top = hi.y;
      boxed(mesh, {lo.x, top - 0.08f, lo.z}, hi, 0xC98B5E, w);
      const float legs[4][2] = {{lo.x + 0.1f, lo.z + 0.1f}, {hi.x - 0.1f, lo.z + 0.1f}, {lo.x + 0.1f, hi.z - 0.1f},
                                {hi.x - 0.1f, hi.z - 0.1f}};
      for (auto& l : legs) boxed(mesh, {l[0] - 0.05f, 0, l[1] - 0.05f}, {l[0] + 0.05f, top - 0.08f, l[1] + 0.05f}, 0x8A5A40, w);
      Vec3 bowl{mid.x, top + 0.08f, mid.z};
      mesh.add(Shape::frustum(8, 0.3f, 0.5f), srt({0.4f, 0.16f, 0.4f}, Xf{}, bowl), kWhite, w);
      mesh.add(Shape::sphere(4, 8), srt(0.26f, Xf{}, bowl + kY * 0.12f), wall(Flavor::Strawberry), w);
      break;
    }
    case PropKind::Sofa:
      boxed(mesh, lo, {hi.x, 0.45f, hi.z}, 0xE77FA0, w);
      boxed(mesh, backLo(0.45f), backHi(), wall(Flavor::Strawberry), w);
      boxed(mesh, lo + Vec3{0.1f, 0.45f, 0.1f}, {hi.x - 0.1f, 0.55f, hi.z - 0.1f}, 0xFFDCE8, w);
      break;
    case PropKind::Tv: {
      boxed(mesh, lo, {hi.x, 0.6f, hi.z}, 0x8A5A40, w);
      Vec3 sl{mid.x - along.x * size.x * 0.45f - std::fabs(f.x) * 0.05f, 0.6f,
              mid.z - along.z * size.z * 0.45f - std::fabs(f.z) * 0.05f};
      Vec3 sh{mid.x + along.x * size.x * 0.45f + std::fabs(f.x) * 0.05f, hi.y,
              mid.z + along.z * size.z * 0.45f + std::fabs(f.z) * 0.05f};
      boxed(mesh, sl, sh, 0x22242A, w);
      Vec3 glow = Vec3{mid.x, (0.6f + hi.y) / 2, mid.z} + f * 0.06f;
      float r = std::max(size.x, size.z) * 0.4f;
      boxed(mesh, glow - along * r - kY * 0.3f, glow + along * r + kY * 0.3f, 0x7FC8E8, 0);
      break;
    }
    case PropKind::Tub:
      boxed(mesh, lo, hi, kWhite, w);
      boxed(mesh, lo + Vec3{0.15f, 0, 0.15f}, {hi.x - 0.15f, hi.y + 0.01f, hi.z - 0.15f}, 0x9FD8F0, 0);
      break;
    case PropKind::Toilet:
      boxed(mesh, lo + Vec3{0.1f, 0, 0.1f}, {hi.x - 0.1f, 0.45f, hi.z - 0.1f}, kWhite, w);
      boxed(mesh, backLo(0.45f), backHi(), kWhite, w);
      break;
    case PropKind::Sink:
      boxed(mesh, lo, hi, kWhite, w);
      boxed(mesh, {mid.x - 0.25f, hi.y, mid.z - 0.25f}, {mid.x + 0.25f, hi.y + 0.02f, mid.z + 0.25f}, 0x9FD8F0, 0);
      break;
    case PropKind::Washer: {
      boxed(mesh, lo, hi, kWhite, w);
      Vec3 door = face(hi.y * 0.5f) + f * 0.02f;
      Xf turn = std::fabs(f.x) > 0 ? rotZ(kHalfPi * sgn(f.x) * -1.0f) : rotX(kHalfPi * sgn(f.z));
      mesh.add(Shape::disc(12), srt(0.7f, turn, door), 0x3C4A5A, 0);
      break;
    }
    case PropKind::Shelf:
      boxed(mesh, lo, hi, 0x8A5A40, w);
      for (int i = 0; i < 5; ++i) {
        float x = lo.x + 0.3f + (float)i * std::max(size.x - 0.6f, 0.1f) / 4.0f;
        mesh.add(Shape::frustum(8, 0.4f, 0.5f), srt({0.35f, 0.25f, 0.35f}, Xf{}, {x, hi.y + 0.12f, mid.z}),
                 wall(kFlavors[i % 4]), w);
      }
      break;
    case PropKind::Stairs: {
      const int steps = 8;
      bool longX = size.x > size.z;
      for (int i = 0; i < steps; ++i) {
        float t0 = (float)i / steps, t1 = (float)(i + 1) / steps, y = hi.y * t1;
        Vec3 a, e;
        if (longX) a = {lo.x + size.x * t0, 0, lo.z}, e = {lo.x + size.x * t1, y, hi.z};
        else a = {lo.x, 0, lo.z + size.z * t0}, e = {hi.x, y, lo.z + size.z * t1};
        boxed(mesh, a, e, i % 2 == 0 ? carpetColour : wallColour, w);
      }
      break;
    }
    case PropKind::Freezer:
      boxed(mesh, lo, {hi.x, hi.y - 0.08f, hi.z}, 0xDDEFFF, w);
      boxed(mesh, {lo.x - 0.03f, hi.y - 0.08f, lo.z - 0.03f}, hi + Vec3{0.03f, 0, 0.03f}, 0xBFE3FF, w);
      break;
    case PropKind::Pillar:
    case PropKind::Partition:
      boxed(mesh, lo, hi, wallColour, 0);
      boxed(mesh, lo - Vec3{0.05f, 0, 0.05f}, {hi.x + 0.05f, 0.28f, hi.z + 0.05f}, trim, 0);
      boxed(mesh, {lo.x - 0.05f, hi.y - 0.22f, lo.z - 0.05f}, hi + Vec3{0.05f, 0, 0.05f}, trim, 0);
      break;
    case PropKind::Plant:
      mesh.add(Shape::frustum(8, 0.3f, 0.5f), srt({0.8f, 0.7f, 0.8f}, Xf{}, {mid.x, 0.35f, mid.z}), kWaffle, w);
      mesh.add(Shape::sphere(5, 8), srt(0.7f, Xf{}, {mid.x, 0.85f, mid.z}), wall(Flavor::Mint), w);
      break;
  }
}

void frontDoor(Mesh& mesh, const House& house) {
  const FrontDoor& f = house.front;
  V2 o = outward(f.side);
  Vec3 out{o.x, 0, o.z};
  Vec3 along{std::fabs(out.z), 0, std::fabs(out.x)};
  Vec3 face = Vec3{f.at.x, 0, f.at.z} - out * 0.17f;
  Vec3 lo = face - along * 0.75f - out * 0.06f;
  Vec3 hi = face + along * 0.75f + kY * kDoorHeight;
  boxed(mesh, vmin(lo, hi), vmax(lo, hi), 0x6B3A1E, 1);
  for (int i = 0; i < 3; ++i) {
    Vec3 hole = face + kY * (0.9f + (float)i * 0.22f) + along * 0.5f - out * 0.08f;
    boxed(mesh, hole - splat3(0.07f), hole + splat3(0.07f), kGold, 0);
  }
  Vec3 knob = face + kY * 1.1f + along * 0.55f - out * 0.12f;
  mesh.add(Shape::sphere(4, 8), srt(0.14f, Xf{}, knob), kGold, 1);
  Vec3 sign = face + kY * (kDoorHeight + 0.15f) - out * 0.06f;
  Vec3 half = along * 0.5f + kY * 0.1f + out * 0.04f;
  boxed(mesh, sign - half, sign + half, 0x3BB273, 1);
}

}  // namespace

// ---- transforms ----

Vec3 Xf::linear(Vec3 d) const { return apply3(m, d); }
Vec3 Xf::apply(Vec3 p) const { return apply3(m, p) + t; }
Xf operator*(const Xf& a, const Xf& b) {
  Xf r;
  for (int i = 0; i < 3; ++i)
    for (int j = 0; j < 3; ++j) r.m[i * 3 + j] = a.m[i * 3] * b.m[j] + a.m[i * 3 + 1] * b.m[3 + j] + a.m[i * 3 + 2] * b.m[6 + j];
  r.t = a.apply(b.t);
  return r;
}
Xf translate(Vec3 t) {
  Xf r;
  r.t = t;
  return r;
}
Xf scale(Vec3 s) {
  Xf r;
  r.m[0] = s.x, r.m[4] = s.y, r.m[8] = s.z;
  return r;
}
Xf rotX(float a) {
  Xf r;
  float c = std::cos(a), s = std::sin(a);
  r.m[4] = c, r.m[5] = -s, r.m[7] = s, r.m[8] = c;
  return r;
}
Xf rotY(float a) {
  Xf r;
  float c = std::cos(a), s = std::sin(a);
  r.m[0] = c, r.m[2] = s, r.m[6] = -s, r.m[8] = c;
  return r;
}
Xf rotZ(float a) {
  Xf r;
  float c = std::cos(a), s = std::sin(a);
  r.m[0] = c, r.m[1] = -s, r.m[3] = s, r.m[4] = c;
  return r;
}
Xf arc(Vec3 from, Vec3 to) {
  Vec3 v = cross(from, to);
  float c = dot(from, to);
  Xf r;
  if (c < -0.99999f) return rotX(kPi);
  float k = 1 / (1 + c);
  r.m[0] = c + v.x * v.x * k, r.m[1] = v.x * v.y * k - v.z, r.m[2] = v.x * v.z * k + v.y;
  r.m[3] = v.y * v.x * k + v.z, r.m[4] = c + v.y * v.y * k, r.m[5] = v.y * v.z * k - v.x;
  r.m[6] = v.z * v.x * k - v.y, r.m[7] = v.z * v.y * k + v.x, r.m[8] = c + v.z * v.z * k;
  return r;
}

// ---- meshes ----

void Mesh::add(Shape shape, const Xf& xf, uint32_t colour, float weight) {
  float normals[9];
  inverseTranspose(xf.m, normals);
  for (const Tri& tri : triangles(shape)) {
    Corner c[3];
    for (int k = 0; k < 3; ++k) c[k] = {xf.apply(tri[k].first), normOrZero(xf.linear(tri[k].second))};
    Vec3 n = cross(c[1].first - c[0].first, c[2].first - c[0].first);
    if (len2(n) < 1e-12f) continue;
    // Face outward: the same side as the corners' hull directions.
    Vec3 outward = apply3(normals, tri[0].second) + apply3(normals, tri[1].second) + apply3(normals, tri[2].second);
    if (dot(n, outward) < 0) {
      std::swap(c[1], c[2]);
      n = neg(n);
    }
    n = normalize(n);
    for (const Corner& k : c) v.push_back({k.first, n, k.second, colour, weight});
  }
}

void Mesh::block(Vec3 lo, Vec3 hi, uint32_t colour, float weight) {
  add(Shape::cube(), srt(hi - lo, Xf{}, (lo + hi) * 0.5f), colour, weight);
}

// ---- palette ----

uint32_t mix(uint32_t a, uint32_t b, float t) {
  t = std::clamp(t, 0.0f, 1.0f);
  auto channel = [&](int shift) {
    float x = (float)((a >> shift) & 0xFF), y = (float)((b >> shift) & 0xFF);
    return (uint32_t)std::round(x + (y - x) * t) << shift;
  };
  return channel(16) | channel(8) | channel(0);
}
uint32_t wall(Flavor f) {
  switch (f) {
    case Flavor::Strawberry: return 0xF6B3C8;
    case Flavor::Mint: return 0xBDEBD4;
    case Flavor::Vanilla: return 0xFFF1CF;
    case Flavor::Chocolate: return 0xB98466;
  }
  return 0xFFFFFF;
}
uint32_t carpet(Flavor f) {
  switch (f) {
    case Flavor::Strawberry: return 0xE88AA6;
    case Flavor::Mint: return 0x8FD3B4;
    case Flavor::Vanilla: return 0xE9D3A4;
    case Flavor::Chocolate: return 0x8A5A40;
  }
  return 0xFFFFFF;
}
bool cold(Module m) {
  return m == Module::Hallway || m == Module::Stairwell || m == Module::FreezerRoom || m == Module::Laundry;
}

// ---- rooms ----

Mesh room(const House& house, size_t index, float yellow) {
  Mesh mesh;
  const Room& r = house.rooms[index];
  const uint32_t ix = (uint32_t)index;
  V2 c = centre(r.cell);
  float x0 = c.x - kCell / 2, z0 = c.z - kCell / 2;
  bool backrooms = yellow >= 0.5f;
  uint32_t colour = mix(wall(r.flavor), kBackWall, yellow), floor = mix(carpet(r.flavor), kBackCarpet, yellow);
  uint32_t trim = mix(kWaffle, kBackTrim, yellow), frame = mix(kWaffleDark, kBackTrim, yellow);
  const float half = 0.15f;
  boxed(mesh, {x0, -0.1f, z0}, {x0 + kCell, 0, z0 + kCell}, floor, 0);
  boxed(mesh, {x0, kWallHeight, z0}, {x0 + kCell, kWallHeight + 0.1f, z0 + kCell}, mix(kCeiling, kBackCeiling, yellow), 0);
  if (backrooms) {
    for (uint32_t i = 0; i < 7; ++i) {
      float x = x0 + 1 + noise(ix + 31, i * 2) * (kCell - 2), z = z0 + 1 + noise(ix + 31, i * 2 + 1) * (kCell - 2);
      float size = 0.6f + noise(ix + 77, i) * 1.4f;
      mesh.add(Shape::disc(10),
               srt({size, 1, size * (0.6f + noise(ix, 90 + i) * 0.6f)}, rotY(noise(ix + 5, i) * kPi), {x, 0.008f, z}),
               kBackDamp, 0);
    }
    for (int i = 1; i < 5; ++i) {
      float a = (float)i * kCell / 5;
      boxed(mesh, {x0 + a - 0.02f, kWallHeight - 0.02f, z0}, {x0 + a + 0.02f, kWallHeight, z0 + kCell}, kBackTrim, 0);
      boxed(mesh, {x0, kWallHeight - 0.02f, z0 + a - 0.02f}, {x0 + kCell, kWallHeight, z0 + a + 0.02f}, kBackTrim, 0);
    }
  }
  for (uint32_t i = 0; i < (backrooms ? 0u : 36u); ++i) {
    float x = x0 + 0.4f + noise(ix, i * 2) * (kCell - 0.8f), z = z0 + 0.4f + noise(ix, i * 2 + 1) * (kCell - 0.8f);
    mesh.add(Shape::cube(), srt({0.05f, 0.02f, 0.2f}, rotY(noise(ix + 99, i) * kPi), {x, 0.01f, z}), kSprinkles[i % 5], 0);
  }
  for (int si = 0; si < 4; ++si) {
    Side side = kSidesAll[si];
    bool open = r.doors[si];
    V2 out = outward(side);
    bool horizontal = side == Side::North || side == Side::South;
    float line = horizontal ? c.z + out.z * kCell / 2 : c.x + out.x * kCell / 2;
    float inner = horizontal ? line - out.z * half : line - out.x * half, outer = line;
    float a0 = horizontal ? x0 : z0, a1 = a0 + kCell, mid = (a0 + a1) / 2, gap = kDoor / 2;
    auto piece = [&](float s0, float s1, float y0, float y1, uint32_t col, float grow) {
      float lo = std::min(inner, outer), hi = std::max(inner, outer);
      if (inner < outer) lo -= grow;
      else hi += grow;
      if (horizontal) boxed(mesh, {s0, y0, lo}, {s1, y1, hi}, col, 0);
      else boxed(mesh, {lo, y0, s0}, {hi, y1, s1}, col, 0);
    };
    std::vector<std::pair<float, float>> spans;
    if (open) spans = {{a0, mid - gap}, {mid + gap, a1}};
    else spans = {{a0, a1}};
    for (auto [s0, s1] : spans) {
      piece(s0, s1, 0, kWallHeight, colour, 0);
      piece(s0, s1, 0, 0.28f, trim, 0.05f);
      piece(s0, s1, kWallHeight - 0.22f, kWallHeight, trim, 0.05f);
      if (backrooms)
        for (float at = s0 + 0.25f; at < s1 - 0.1f; at += 0.5f)
          piece(at, std::min(at + 0.07f, s1), 0.28f, kWallHeight - 0.22f, kBackStripe, 0.01f);
    }
    if (open) {
      piece(mid - gap, mid + gap, kDoorHeight, kWallHeight, colour, 0);
      piece(mid - gap - 0.12f, mid - gap, 0, kDoorHeight, frame, 0.06f);
      piece(mid + gap, mid + gap + 0.12f, 0, kDoorHeight, frame, 0.06f);
      piece(mid - gap - 0.12f, mid + gap + 0.12f, kDoorHeight - 0.12f, kDoorHeight, frame, 0.06f);
    }
    for (uint32_t i = 0; i < (backrooms ? 0u : 6u); ++i) {
      float a = a0 + 0.6f + noise(ix * 7 + (uint32_t)si, i) * (kCell - 1.2f);
      if (open && std::fabs(a - mid) < gap + 0.3f) continue;
      float length = 0.15f + noise(ix, 50 + i) * 0.45f;
      Vec3 at = horizontal ? Vec3{a, kWallHeight - 0.22f - length / 2, inner - out.z * 0.04f}
                           : Vec3{inner - out.x * 0.04f, kWallHeight - 0.22f - length / 2, a};
      mesh.add(Shape::frustum(6, 0.5f, 0.15f), srt({0.16f, length, 0.16f}, rotX(kPi), at), colour, 0);
      mesh.add(Shape::sphere(4, 6), srt(0.11f, Xf{}, at - kY * (length / 2)), colour, 0);
    }
  }
  for (uint32_t i = 0; i < (backrooms ? 0u : 5u); ++i) {
    float x = c.x - 3.5f + noise(ix + 7, i) * 7, z = c.z - 3.5f + noise(ix + 9, i) * 7;
    float length = 0.2f + noise(ix + 3, i) * 0.35f;
    mesh.add(Shape::frustum(6, 0.5f, 0.1f), srt({0.18f, length, 0.18f}, rotX(kPi), {x, kWallHeight - length / 2, z}),
             wall(kFlavors[hashU(ix, i) % 4]), 0);
  }
  for (const Prop& p : house.props(index)) {
    Block b{c + p.min, c + p.max, p.height};
    prop(mesh, p.kind, b, c, colour, floor, trim);
  }
  if (house.front.room == index) frontDoor(mesh, house);
  return mesh;
}

Mesh lights(const House& house, size_t index, float yellow) {
  Mesh mesh;
  V2 c = centre(house.rooms[index].cell);
  if (yellow >= 0.5f) {
    const float at[5][2] = {{-3, -2}, {3, -2}, {-3, 2}, {3, 2}, {0, 0}};
    for (auto& o : at) {
      Vec3 p{c.x + o[0], kWallHeight - 0.03f, c.z + o[1]};
      boxed(mesh, p - Vec3{0.3f, 0.03f, 0.95f}, p + Vec3{0.3f, 0.03f, 0.95f}, kWhite, 0);
    }
    return mesh;
  }
  for (float offset : {-2.5f, 2.5f}) {
    Vec3 p{c.x + offset, kWallHeight - 0.03f, c.z};
    boxed(mesh, p - Vec3{0.35f, 0.03f, 0.9f}, p + Vec3{0.35f, 0.03f, 0.9f}, kWhite, 0);
  }
  return mesh;
}

// ---- the Ice Cream Man ----

float manTop(float d) {
  float hip = kManModelHip * kManScale;
  return hip + (kManHeight - hip) * std::cos(kManDuck * std::clamp(d, 0.0f, 1.0f));
}

float duck(const House& house, V2 at) {
  float best = 0;
  for (auto [mid, alongX] : house.doorways()) {
    float across = alongX ? std::fabs(at.z - mid.z) : std::fabs(at.x - mid.x);
    float along = alongX ? std::fabs(at.x - mid.x) : std::fabs(at.z - mid.z);
    float d = 0;
    if (along <= kDoor / 2 + 0.6f) {
      float t = std::clamp((2.3f - across) / 1.0f, 0.0f, 1.0f);
      d = t * t * (3 - 2 * t);
    }
    best = std::max(best, d);
  }
  return best;
}

Avatar avatar() {
  Avatar a;
  a.torso.block({-0.4f, 0.8f, -0.2f}, {0.4f, 1.55f, 0.2f}, kShirt, 1);
  // A cone on the shirt.
  a.torso.block({-0.08f, 1.05f, 0.2f}, {0.08f, 1.3f, 0.22f}, kWaffle, 0);
  a.torso.block({-0.12f, 1.3f, 0.2f}, {0.12f, 1.42f, 0.22f}, wall(Flavor::Strawberry), 0);
  a.head.block({-0.28f, 1.57f, -0.26f}, {0.28f, 2.07f, 0.26f}, kSkin, 1);
  for (float x : {-0.11f, 0.11f}) a.head.block({x - 0.04f, 1.84f, 0.26f}, {x + 0.04f, 1.94f, 0.28f}, kInk, 0);
  // A smile.
  a.head.block({-0.12f, 1.7f, 0.26f}, {0.12f, 1.73f, 0.28f}, kInk, 0);
  a.head.block({-0.15f, 1.73f, 0.26f}, {-0.11f, 1.77f, 0.28f}, kInk, 0);
  a.head.block({0.11f, 1.73f, 0.26f}, {0.15f, 1.77f, 0.28f}, kInk, 0);
  // Hair.
  a.head.block({-0.3f, 1.98f, -0.28f}, {0.3f, 2.12f, 0.28f}, 0x6B3A1E, 1);
  a.arm.block({-0.13f, -0.38f, -0.13f}, {0.13f, 0.0f, 0.13f}, kShirt, 1);
  a.arm.block({-0.13f, -0.72f, -0.13f}, {0.13f, -0.38f, 0.13f}, kSkin, 1);
  a.leg.block({-0.18f, -0.8f, -0.18f}, {0.18f, 0.0f, 0.18f}, kPants, 1);
  a.leg.block({-0.19f, -0.8f, -0.2f}, {0.19f, -0.68f, 0.24f}, 0x3A3A3A, 1);
  return a;
}

ManModel man() {
  ManModel m;
  m.body.add(Shape::frustum(12, 0.2f, 0.6f), srt({1, 1.25f, 1}, Xf{}, {0, 1.38f, 0}), kWaffle, 1);
  for (int i = 0; i < 4; ++i) {
    float t = ((float)i + 0.6f) / 4.6f, y = 0.76f + t * 1.25f, r = 0.2f + t * 0.4f + 0.01f;
    m.body.add(Shape::torus(14, 3, r, 0.025f), translate({0, y, 0}) * rotX(kHalfPi), kWaffleDark, 0);
  }
  for (int i = 0; i < 6; ++i) {
    float a = (float)i / 6 * kTau;
    Vec3 dir{std::cos(a), 0, std::sin(a)};
    Vec3 lo = Vec3{0, 0.8f, 0} + dir * 0.24f, hi = Vec3{0, 1.98f, 0} + dir * 0.6f;
    Vec3 mid = (lo + hi) * 0.5f, up = normalize(hi - lo);
    m.body.add(Shape::cube(), srt({0.035f, std::sqrt(len2(hi - lo)), 0.035f}, arc(kY, up), mid + dir * 0.012f), kWaffleDark, 0);
  }
  m.body.add(Shape::torus(14, 4, 0.6f, 0.07f), translate({0, 2, 0}) * rotX(kHalfPi), 0xE8B878, 1);
  m.leg.add(Shape::frustum(8, 0.5f, 0.5f), srt({0.2f, 0.8f, 0.2f}, Xf{}, {0, -0.4f, 0}), kWaffle, 1);
  m.leg.block({-0.13f, -0.85f, -0.12f}, {0.13f, -0.72f, 0.22f}, 0x6B3A1E, 1);
  m.arm.add(Shape::frustum(8, 0.4f, 0.6f), srt({0.16f, 0.7f, 0.16f}, Xf{}, {0, -0.35f, 0}), kWaffle, 1);
  m.arm.add(Shape::sphere(5, 8), srt(0.24f, Xf{}, {0, -0.75f, 0}), kWhite, 1);
  m.head.add(Shape::sphere(8, 14), srt({1.3f, 0.9f, 1.3f}, Xf{}, {0, 2.3f, 0}), kVanilla, 1);
  for (int i = 0; i < 10; ++i) {
    float a = (float)i / 10 * kTau, r = 0.6f, drop = 0.12f + (float)((i * 7) % 4) * 0.05f;
    m.head.add(Shape::sphere(4, 6), srt({0.2f, 0.2f + drop, 0.2f}, Xf{}, {std::cos(a) * r, 2.0f - drop / 2, std::sin(a) * r}),
               kVanilla, 0);
  }
  auto surface = [](float x, float y) {
    const float rx = 0.65f, ry = 0.45f;
    float u = x / rx, w = (y - 2.3f) / ry;
    return rx * std::sqrt(std::max(1 - u * u - w * w, 0.0f));
  };
  for (float x : {-0.12f, 0.12f})
    m.head.add(Shape::sphere(4, 6), srt(0.1f, Xf{}, {x, 2.5f, surface(x, 2.5f) + 0.01f}), kInk, 0);
  for (int i = 0; i < 11; ++i) {
    float t = (float)i / 10, a = kPi * (1.15f + 0.7f * t);
    Vec3 p{std::cos(a) * 0.34f, 2.44f + std::sin(a) * 0.24f, 0};
    float z = surface(p.x, p.y);
    m.head.block({p.x - 0.045f, p.y - 0.04f, z - 0.03f}, {p.x + 0.045f, p.y + 0.04f, z + 0.03f}, 0x8A1E2E, 0);
    if (i >= 2 && i < 9)
      m.head.block({p.x - 0.03f, p.y + 0.03f, z - 0.02f}, {p.x + 0.03f, p.y + 0.07f, z + 0.035f}, kWhite, 0);
  }
  for (float side : {-1.0f, 1.0f}) {
    Vec3 cheek{side * 0.38f, 2.32f, surface(side * 0.38f, 2.32f) - 0.01f};
    m.head.add(Shape::sphere(4, 8), srt({0.2f, 0.14f, 0.08f}, Xf{}, cheek), 0xFF9DB8, 0);
    for (int i = 0; i < 4; ++i) {
      Vec3 o{side * (0.05f * (float)(i % 2) - 0.03f), 0.06f * ((float)i - 1.5f), 0.05f};
      m.head.add(Shape::cube(), srt({0.025f, 0.08f, 0.025f}, rotZ(0.6f * (float)i + side), cheek + o), kSprinkles[i % 5], 0);
    }
  }
  m.forehead.add(Shape::sphere(10, 16), srt({1.2f, 1.25f, 1.15f}, Xf{}, {0, 2.72f, -0.04f}), kForehead, 1);
  m.forehead.add(Shape::sphere(4, 8), srt({0.2f, 0.12f, 0.06f}, rotX(-0.6f), {0.18f, 3.06f, 0.4f}), 0xFFFFFF, 0);
  return m;
}

Mesh tool(Tool t) {
  Mesh m;
  switch (t) {
    case Tool::Fudge:
      m.block({-0.1f, -0.12f, -0.15f}, {0.1f, 0.1f, 0.45f}, 0x6B3A1E, 1);
      m.add(Shape::frustum(8, 0.5f, 0.3f), srt({0.16f, 0.3f, 0.16f}, rotX(kHalfPi), {0, 0, 0.6f}), 0xC9A27E, 1);
      m.add(Shape::sphere(4, 8), srt(0.26f, Xf{}, {0, 0.2f, 0.05f}), kFudge, 1);
      break;
    case Tool::Fart:
      m.add(Shape::frustum(8, 0.3f, 0.3f), srt({0.24f, 0.6f, 0.24f}, rotX(kHalfPi), {0, 0, 0.2f}), 0x8DC63F, 1);
      m.add(Shape::frustum(8, 0.25f, 0.5f), srt({0.4f, 0.2f, 0.4f}, rotX(kHalfPi), {0, 0, 0.58f}), 0xB6E06A, 1);
      break;
    case Tool::Sword: {
      for (int i = 0; i < 8; ++i) {
        float z0 = -0.1f + (float)i * 0.13f;
        m.block({-0.045f, -0.045f, z0}, {0.045f, 0.045f, z0 + 0.13f}, i % 2 == 0 ? kCandyRed : kWhite, 1);
      }
      const float hook[4][2] = {{0.06f, 1.0f}, {0.16f, 1.04f}, {0.24f, 0.98f}, {0.24f, 0.88f}};
      for (int i = 0; i < 4; ++i) {
        float y = hook[i][0], z = hook[i][1];
        m.block({-0.045f, y - 0.05f, z - 0.05f}, {0.045f, y + 0.05f, z + 0.05f}, i % 2 == 0 ? kWhite : kCandyRed, 1);
      }
      break;
    }
    case Tool::Spear:
      m.block({-0.05f, -0.05f, -0.35f}, {0.05f, 0.05f, 0.25f}, kWaffleDark, 1);
      m.add(Shape::frustum(10, 0.5f, 0.02f), srt({0.3f, 2.4f, 0.3f}, rotX(kHalfPi), {0, 0, 1.45f}), kWaffle, 1);
      for (int i = 0; i < 5; ++i) {
        float z = 0.45f + (float)i * 0.42f, r = 0.15f * (1 - (z - 0.25f) / 2.4f) + 0.012f;
        m.add(Shape::torus(10, 3, r, 0.012f), translate({0, 0, z}), kWaffleDark, 0);
      }
      m.add(Shape::sphere(4, 8), srt(0.3f, Xf{}, {0, 0, 0.28f}), wall(Flavor::Strawberry), 1);
      break;
    case Tool::Pistol:
      m.block({-0.06f, -0.3f, -0.12f}, {0.06f, 0, 0.05f}, 0x8A5A40, 1);
      m.block({-0.08f, -0.02f, -0.15f}, {0.08f, 0.12f, 0.25f}, 0xFF9DB8, 1);
      m.add(Shape::frustum(8, 0.15f, 0.5f), srt({0.24f, 0.35f, 0.24f}, rotX(kHalfPi), {0, 0.05f, 0.42f}), kWaffle, 1);
      break;
    case Tool::Sugar:
      m.add(Shape::frustum(10, 0.5f, 0.5f), srt({0.26f, 0.9f, 0.26f}, rotX(kHalfPi), {0, 0, 0.25f}), 0x7FC8E8, 1);
      m.block({-0.12f, 0.1f, -0.05f}, {0.12f, 0.35f, 0.25f}, kWhite, 1);
      m.add(Shape::frustum(10, 0.5f, 0.7f), srt({0.3f, 0.15f, 0.3f}, rotX(kHalfPi), {0, 0, 0.75f}), 0xFF6FA8, 1);
      break;
    case Tool::Staff:
      m.block({-0.05f, -0.05f, -0.5f}, {0.05f, 0.05f, 1.7f}, kSprinkles[0], 1);
      for (float z : {-0.5f, 1.7f}) m.add(Shape::sphere(4, 8), srt(0.1f, Xf{}, {0, 0, z}), kSprinkles[0], 1);
      m.add(Shape::sphere(5, 8), srt(0.22f, Xf{}, {0, 0, 1.75f}), kGold, 1);
      break;
    case Tool::Keys:
      m.add(Shape::torus(12, 4, 0.14f, 0.025f), translate({0, 0, 0.1f}), kGold, 1);
      for (int i = 0; i < 3; ++i) {
        float a = (float)i * 0.5f - 0.5f;
        m.block({a * 0.2f - 0.025f, -0.3f, 0.08f}, {a * 0.2f + 0.025f, -0.1f, 0.12f}, kGold, 1);
      }
      break;
  }
  return m;
}

Mesh key() {
  Mesh m;
  m.add(Shape::torus(12, 4, 0.16f, 0.05f), translate({0, 0.25f, 0}), kGold, 1);
  m.block({-0.04f, -0.3f, -0.04f}, {0.04f, 0.1f, 0.04f}, kGold, 1);
  m.block({0.04f, -0.3f, -0.03f}, {0.14f, -0.22f, 0.03f}, kGold, 1);
  m.block({0.04f, -0.16f, -0.03f}, {0.12f, -0.09f, 0.03f}, kGold, 1);
  return m;
}

Mesh ball(uint32_t colour) {
  Mesh m;
  m.add(Shape::sphere(5, 8), Xf{}, colour, 1);
  return m;
}

Mesh sprinkle(uint32_t colour) {
  Mesh m;
  m.block({-0.04f, -0.04f, -0.13f}, {0.04f, 0.04f, 0.13f}, colour, 0.6f);
  return m;
}

Mesh splat(uint32_t colour) {
  Mesh m;
  m.add(Shape::disc(10), Xf{}, colour, 0);
  return m;
}

Mesh treat() {
  Mesh m;
  m.add(Shape::frustum(10, 0.03f, 0.5f), srt({0.32f, 0.45f, 0.32f}, Xf{}, {0, 0.22f, 0}), kWaffle, 1);
  m.add(Shape::sphere(5, 10), srt(0.36f, Xf{}, {0, 0.5f, 0}), wall(Flavor::Strawberry), 1);
  for (int i = 0; i < 5; ++i) {
    float a = (float)i * 1.25f;
    m.block({std::cos(a) * 0.12f - 0.015f, 0.62f, std::sin(a) * 0.12f - 0.015f},
            {std::cos(a) * 0.12f + 0.015f, 0.66f, std::sin(a) * 0.12f + 0.05f}, kSprinkles[i % 5], 0);
  }
  return m;
}

Mesh coneShot() {
  Mesh m;
  m.add(Shape::frustum(8, 0.02f, 0.5f), srt({0.24f, 0.35f, 0.24f}, rotX(-kHalfPi), {0, 0, 0}), kWaffle, 1);
  return m;
}

Mesh button(uint32_t colour) {
  Mesh m;
  m.block({-0.22f, 0, -0.22f}, {0.22f, 0.9f, 0.22f}, kWaffleDark, 1);
  m.add(Shape::sphere(4, 10), srt({0.38f, 0.18f, 0.38f}, Xf{}, {0, 0.92f, 0}), colour, 1);
  return m;
}

Mesh cabinet() {
  Mesh m;
  m.block({-0.5f, 0, -0.5f}, {0.5f, 0.15f, 0.5f}, kWaffleDark, 1);
  m.block({-0.5f, 2.05f, -0.5f}, {0.5f, 2.2f, 0.5f}, kWaffleDark, 1);
  const float posts[4][2] = {{-0.5f, -0.5f}, {0.4f, -0.5f}, {-0.5f, 0.4f}, {0.4f, 0.4f}};
  for (auto& p : posts) m.block({p[0], 0.15f, p[1]}, {p[0] + 0.1f, 2.05f, p[1] + 0.1f}, kWaffle, 1);
  return m;
}

Mesh bigCone() {
  Mesh m;
  m.add(Shape::frustum(12, 0.05f, 0.6f), srt({1, 1.6f, 1}, Xf{}, {0, 0.8f, 0}), kWaffle, 1);
  return m;
}

}  // namespace icecream::toon
