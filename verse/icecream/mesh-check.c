// Compares verse/icecream/mesh.coax with the demo's C++ models, vertex by
// vertex: reads what mesh-oracle.cpp wrote and builds every case again from
// the Coax (IceCream.h). The counts, the colours and the outline weights must
// be equal; positions and normals within 1e-4 (each side's libm may round a
// sine differently), and it says how many floats were bit for bit the same.
// test-mesh.sh builds and runs it.
//   mesh-check oracle.bin [--table fixtures/mesh-oracle.coax]
#include "IceCream.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CAP (4u << 20)
static float oracle[CAP * 11];
static coax_t_Vtx ours[CAP];
static int64_t firsts[256];
static uint32_t ofirst[256];
static FILE *in;
static long failures, floats, exact;
static double worst;

static void get(void *p, size_t n) {
  if (fread(p, 1, n, in) != n) { fprintf(stderr, "mesh-check: the oracle ended early\n"); exit(2); }
}
static uint32_t get32(void) { uint32_t v; get(&v, 4); return v; }

static uint32_t read_vertices(void) {
  uint32_t n = get32();
  if (n > CAP) { fprintf(stderr, "mesh-check: %u vertices is over the cap\n", n); exit(2); }
  get(oracle, (size_t)n * 11 * 4);
  return n;
}
static uint32_t bits(float f) { uint32_t b; memcpy(&b, &f, 4); return b; }

// One float pair; false past the tolerance.
static int same(float a, float b) {
  floats++;
  if (bits(a) == bits(b)) { exact++; return 1; }
  double d = fabs((double)a - (double)b);
  if (d > worst) worst = d;
  return d <= 1e-4;
}
static void compare(const char *what, uint32_t n, int64_t got) {
  if ((int64_t)n != got) { printf("FAIL %s: %lld vertices, the C++ has %u\n", what, (long long)got, n); failures++; return; }
  for (uint32_t i = 0; i < n; i++) {
    const float *o = &oracle[i * 11];
    const coax_t_Vtx *v = &ours[i];
    float mine[9] = {v->p.x, v->p.y, v->p.z, v->n.x, v->n.y, v->n.z, v->hull.x, v->hull.y, v->hull.z};
    int ok = 1;
    for (int k = 0; k < 9; k++) ok &= same(o[k], mine[k]);
    if (bits(o[9]) != v->colour || bits(o[10]) != bits(v->weight)) ok = 0;
    if (!ok) {
      printf("FAIL %s: vertex %u is (%g %g %g) #%06x %g, the C++ (%g %g %g) #%06x %g\n", what, i, v->p.x, v->p.y, v->p.z,
             v->colour, v->weight, o[0], o[1], o[2], bits(o[9]), o[10]);
      failures++;
      return;
    }
  }
}

// FNV-1a over every vertex: positions, normals and hulls on a 1/1024 grid
// (the evaluator's sine can be an ulp from libm's), then the colour and the
// weight's bits; mesh.coax's vertex_hash.
static uint64_t hash(uint32_t n) {
  uint64_t h = 0xcbf29ce484222325ull;
  for (uint32_t i = 0; i < n * 11; i++) {
    uint64_t w;
    if (i % 11 < 9) w = (uint64_t)(int64_t)floorf(oracle[i] * 1024.0f + 0.5f);
    else { uint32_t b; memcpy(&b, &oracle[i], 4); w = b; }
    h = (h ^ w) * 0x100000001b3ull;
  }
  return h;
}
typedef struct { float lo[3], hi[3]; } Bounds;
static Bounds bounds(uint32_t n) {
  Bounds b = {{INFINITY, INFINITY, INFINITY}, {-INFINITY, -INFINITY, -INFINITY}};
  for (uint32_t i = 0; i < n; i++)
    for (int k = 0; k < 3; k++) {
      float x = oracle[i * 11 + k];
      if (x < b.lo[k]) b.lo[k] = x;
      if (x > b.hi[k]) b.hi[k] = x;
    }
  return b;
}

int main(int argc, char **argv) {
  if (argc < 2) { fprintf(stderr, "usage: mesh-check oracle.bin [--table FILE]\n"); return 2; }
  in = fopen(argv[1], "rb");
  if (!in) { perror(argv[1]); return 2; }
  FILE *table = argc > 3 && !strcmp(argv[2], "--table") ? fopen(argv[3], "w") : NULL;
  static coax_t_Game games[1];
  coax_v_t_Game g = {games, 1};
  coax_v_t_Vtx v = {ours, CAP};
  long houses = 0, models = 0, ducks = 0, rows = 0;
  if (table) {
    fprintf(table, "// What the demo's C++ models (~/work/aethyr-demo/src/icecream/mesh.cpp) give: vertex\n"
                   "// counts and bounds, written by verse/icecream/test-mesh.sh --table. Do not edit.\n\n"
                   "OracleHouse :: struct { seed: u64; difficulty: s32; yellow: f32; rooms: int; count: int; first_room: int; hash: u64; lo: V3; hi: V3; }\n"
                   "OraclePart :: struct { kind: s32; index: s32; count: int; hash: u64; lo: V3; hi: V3; }\n");
  }
  static char text[64 * 1024], parts[64 * 1024];
  size_t at = 0, pat = 0;
  int c;
  while ((c = fgetc(in)) != EOF) {
    char what[96];
    if (c == 'H') {
      uint64_t seed; int32_t d; float y;
      get(&seed, 8), get(&d, 4), get(&y, 4);
      uint32_t rooms = get32();
      if (2 * rooms + 1 > 256) { fprintf(stderr, "mesh-check: %u rooms\n", rooms); return 2; }
      get(ofirst, (2 * rooms + 1) * 4);
      uint32_t n = read_vertices();
      coax_v_s64 fv = {firsts, 256};
      int64_t got = icecream_mesh_house(g, seed, d, y, v, fv);
      snprintf(what, sizeof what, "house seed %llu difficulty %d yellow %g", (unsigned long long)seed, d, y);
      if ((uint32_t)games[0].house.room_n != rooms) { printf("FAIL %s: %lld rooms, the C++ has %u\n", what, (long long)games[0].house.room_n, rooms); failures++; }
      for (uint32_t i = 0; i <= 2 * rooms; i++)
        if ((uint32_t)firsts[i] != ofirst[i]) { printf("FAIL %s: part %u starts at %lld, the C++ %u\n", what, i, (long long)firsts[i], ofirst[i]); failures++; break; }
      compare(what, n, got);
      // coax test rebuilds these few in the evaluator; every case runs here
      int few = (seed <= 3 && d == 0 && y == 0) || (seed > 3 && d == 3 && y == 1) || (seed == 3 && d == 0 && y > 0 && y < 1) ||
                (seed == 0 && d == 1 && y == 1);
      if (table && few) {
        at += (size_t)snprintf(text + at, sizeof text - at, "%s  OracleHouse.{", rows ? ",\n" : "");
        at += (size_t)snprintf(text + at, sizeof text - at, "seed = %llu, difficulty = %d, yellow = %.6f, rooms = %u, count = %u, first_room = %u, hash = 0x%llx, ",
                               (unsigned long long)seed, d, y, rooms, n, ofirst[1], (unsigned long long)hash(n));
        Bounds b = bounds(n);
        at += (size_t)snprintf(text + at, sizeof text - at, "lo = V3.{x = %.6f, y = %.6f, z = %.6f}, hi = V3.{x = %.6f, y = %.6f, z = %.6f}}",
                               b.lo[0], b.lo[1], b.lo[2], b.hi[0], b.hi[1], b.hi[2]);
        rows++;
      }
      houses++;
    } else if (c == 'D') {
      uint64_t seed; int32_t d; float x, z, duck;
      get(&seed, 8), get(&d, 4), get(&x, 4), get(&z, 4), get(&duck, 4);
      float mine = icecream_mesh_duck(g, x, z);
      floats++;
      if (bits(mine) == bits(duck)) exact++;
      if (fabsf(mine - duck) > 1e-5f) { printf("FAIL duck at %g %g (seed %llu): %g, the C++ %g\n", x, z, (unsigned long long)seed, mine, duck); failures++; }
      ducks++;
    } else if (c == 'T') {
      float d, top;
      get(&d, 4), get(&top, 4);
      float mine = icecream_mesh_man_top(d);
      floats++;
      if (bits(mine) == bits(top)) exact++;
      if (fabsf(mine - top) > 1e-5f) { printf("FAIL man_top(%g): %g, the C++ %g\n", d, mine, top); failures++; }
    } else if (c == 'P') {
      int32_t kind, index;
      get(&kind, 4), get(&index, 4);
      uint32_t n = read_vertices();
      int64_t got = kind == 0 ? icecream_mesh_part(index, v) : kind == 1 ? icecream_mesh_tool(index, v)
                    : kind == 2 ? icecream_mesh_button(index, v) : icecream_mesh_sprinkle(index, v);
      snprintf(what, sizeof what, "model %d.%d", kind, index);
      compare(what, n, got);
      if (table) {
        Bounds b = bounds(n);
        pat += (size_t)snprintf(parts + pat, sizeof parts - pat, "%s  OraclePart.{kind = %d, index = %d, count = %u, hash = 0x%llx, "
                                "lo = V3.{x = %.6f, y = %.6f, z = %.6f}, hi = V3.{x = %.6f, y = %.6f, z = %.6f}}",
                                models ? ",\n" : "", kind, index, n, (unsigned long long)hash(n), b.lo[0], b.lo[1], b.lo[2], b.hi[0], b.hi[1], b.hi[2]);
      }
      models++;
    } else {
      fprintf(stderr, "mesh-check: a record '%c' it doesn't know\n", c);
      return 2;
    }
  }
  (void)ducks;
  if (table) {
    fprintf(table, "\nORACLE_HOUSES :: [%ld] OracleHouse.[\n%s];\n\nORACLE_PARTS :: [%ld] OraclePart.[\n%s];\n", rows, text, models, parts);
    fclose(table);
  }
  printf("mesh: %ld houses, %ld models, %ld failures; %.4f%% of %ld floats bit for bit, worst %.3g\n", houses, models, failures,
         floats ? 100.0 * (double)exact / (double)floats : 0.0, floats, worst);
  return failures ? 1 : 0;
}
