// The oracle for verse/icecream/mesh.coax: the demo's own C++ models
// (~/work/aethyr-demo/src/icecream/mesh.cpp, read only, built natively next
// to this file by test-mesh.sh) written out vertex by vertex, for
// mesh-check.c to compare with the Coax port. One record per case:
//   'H' seed:u64 difficulty:i32 yellow:f32 rooms:u32 firsts:u32[2*rooms+1] count:u32 vertices
//   'P' kind:i32 index:i32 count:u32 vertices      (models: 0 parts, 1 tools, 2 buttons, 3 sprinkles)
//   'D' seed:u64 difficulty:i32 x:f32 z:f32 duck:f32  and  'T' d:f32 top:f32
// A vertex is 11 floats: p, n, hull, the colour's bits, the weight.
//   mesh-oracle > oracle.bin
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "icecream/mesh.h"
#include "icecream/rules.h"

using namespace icecream;
namespace tn = icecream::toon;

static void put(const void* p, size_t n) { fwrite(p, 1, n, stdout); }
template <class T> static void put(T v) { put(&v, sizeof v); }

static void vertices(const std::vector<tn::Vtx>& v) {
  put((uint32_t)v.size());
  for (const tn::Vtx& x : v) {
    float f[11] = {x.p.x, x.p.y, x.p.z, x.n.x, x.n.y, x.n.z, x.hull.x, x.hull.y, x.hull.z, 0, x.weight};
    memcpy(&f[9], &x.colour, 4);
    put(f, sizeof f);
  }
}

static Tuning tuning(int d) {
  if (d == 3) return Tuning::original();
  return tuningFor(d == 0 ? Difficulty::Easy : d == 2 ? Difficulty::Hard : Difficulty::Normal);
}

int main() {
  const uint64_t seeds[] = {0, 1, 2, 3, 7, 12, 42, 1234567};
  const float yellows[] = {0, 1, 0.3f};
  for (uint64_t seed : seeds)
    for (int d = 0; d < 4; ++d)
      for (float y : yellows) {
        if (y != 0 && y != 1 && seed != 3) continue;
        House h = House::of(seed, tuning(d));
        uint32_t rooms = (uint32_t)h.rooms.size();
        std::vector<tn::Vtx> all;
        std::vector<uint32_t> first;
        for (size_t i = 0; i < rooms; ++i) {
          first.push_back((uint32_t)all.size());
          tn::Mesh m = tn::room(h, i, y);
          all.insert(all.end(), m.v.begin(), m.v.end());
        }
        for (size_t i = 0; i < rooms; ++i) {
          first.push_back((uint32_t)all.size());
          tn::Mesh m = tn::lights(h, i, y);
          all.insert(all.end(), m.v.begin(), m.v.end());
        }
        first.push_back((uint32_t)all.size());
        put('H'), put(seed), put((int32_t)d), put(y), put(rooms);
        put(first.data(), first.size() * 4);
        vertices(all);
        for (int k = 0; k < 9; ++k) {
          float x = h.spawn.x + (float)k * 1.7f - 6, z = h.spawn.z + (float)(k % 3) * 2.1f - 2;
          put('D'), put(seed), put((int32_t)d), put(x), put(z), put(tn::duck(h, V2{x, z}));
        }
      }
  tn::Avatar av = tn::avatar();
  tn::ManModel man = tn::man();
  std::vector<tn::Mesh> parts = {av.torso, av.head, av.arm, av.leg, man.body, man.leg, man.arm, man.head, man.forehead,
                                 tn::key(), tn::ball(tn::kFudge), tn::ball(tn::kCloud), tn::ball(0xFFFFFF),
                                 tn::sprinkle(tn::kGold), tn::splat(tn::kFudge), tn::splat(tn::kVanilla), tn::bigCone(),
                                 tn::treat(), tn::coneShot(), tn::ball(0xFFFFFF), tn::cabinet(), tn::splat(tn::kVanilla)};
  for (size_t i = 0; i < parts.size(); ++i) put('P'), put((int32_t)0), put((int32_t)i), vertices(parts[i].v);
  for (int t = 0; t < kTools; ++t) put('P'), put((int32_t)1), put((int32_t)t), vertices(tn::tool((Tool)t).v);
  for (int i = 0; i < 3; ++i) put('P'), put((int32_t)2), put((int32_t)i), vertices(tn::button(tn::kButtonColours[i]).v);
  for (int i = 0; i < 5; ++i) put('P'), put((int32_t)3), put((int32_t)i), vertices(tn::sprinkle(tn::kSprinkles[i]).v);
  for (int k = 0; k <= 8; ++k) put('T'), put((float)k / 8), put(tn::manTop((float)k / 8));
  return 0;
}
