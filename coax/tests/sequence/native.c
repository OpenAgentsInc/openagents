// Actual emitted sequence instances, caller-owned buffers and independent
// position/byte oracles. Non-finite cases belong here: the evaluator refuses
// constructing them through f64_from_bits with E0412.
#include <assert.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t allocations;
static void *seq_malloc(size_t n) { allocations++; return malloc(n); }
static void *seq_calloc(size_t n, size_t m) { allocations++; return calloc(n, m); }
static void *seq_realloc(void *p, size_t n) { allocations++; return realloc(p, n); }
#define malloc seq_malloc
#define calloc seq_calloc
#define realloc seq_realloc
#include SEQUENCE_INPUT_C
#undef malloc
#undef calloc
#undef realloc

static double now_seconds(void) {
  struct timespec t;
  assert(clock_gettime(CLOCK_MONOTONIC, &t) == 0);
  return (double)t.tv_sec + (double)t.tv_nsec / 1e9;
}

int main(int argc, char **argv) {
  uint8_t bytes[20], original[20], expected[20];
  if (argc > 1) {
    memset(bytes, 17, sizeof bytes);
    coax_v_u8 view = {bytes + 2, 8};
    seq_fixture_swap_bytes(view, strcmp(argv[1], "negative") == 0 ? -1 : 8, 0);
    return 90; // An invalid index must trap before this line.
  }
  uint64_t cases = 0, calls = 0;
  double start = now_seconds();
  assert(seq_fixture_records()); cases++; calls++;
  for (int length = 0; length <= 16; length++) {
    for (int offset = 2; offset + length <= 18; offset++) {
      for (int i = 0; i < 20; i++) original[i] = (uint8_t)(37 * i + 11);
      memcpy(bytes, original, sizeof bytes);
      memcpy(expected, original, sizeof bytes);
      for (int i = 0; i < length; i++) expected[offset + i] = original[offset + length - 1 - i];
      coax_v_u8 view = {bytes + offset, length};
      assert(seq_fixture_reverse_bytes(view) == length); calls++;
      assert(memcmp(bytes, expected, sizeof bytes) == 0); cases++;
      for (int a = 0; a < length; a++) for (int b = 0; b < length; b++) {
        memcpy(bytes, original, sizeof bytes); memcpy(expected, original, sizeof bytes);
        for (int i = 0; i < length; i++) {
          int from = i == a ? b : i == b ? a : i;
          expected[offset + i] = original[offset + from];
        }
        seq_fixture_swap_bytes(view, a, b); calls++;
        assert(memcmp(bytes, expected, sizeof bytes) == 0); cases++;
      }
    }
  }
  // Observe copying on this actual native target, without an evaluator stand-in.
  const uint64_t bits[] = {0, UINT64_C(0x8000000000000000), UINT64_C(1),
    UINT64_C(0x7fefffffffffffff), UINT64_C(0x7ff0000000000000),
    UINT64_C(0xfff0000000000000), UINT64_C(0x7ff8000000001234), UINT64_C(0x7ff0000000001234)};
  double floats[8]; uint64_t actual[8];
  memcpy(floats, bits, sizeof bits);
  seq_fixture_reverse_floats((coax_v_f64){floats, 8}); calls++;
  memcpy(actual, floats, sizeof actual);
  for (int i = 0; i < 8; i++) assert(actual[i] == bits[7 - i]);
  cases++;
  assert(allocations == 0);
  printf("{\"kind\":\"sequence\",\"cases\":%" PRIu64 ",\"calls\":%" PRIu64
    ",\"allocations\":%" PRIu64 ",\"seconds\":%.9f}\n", cases, calls, allocations, now_seconds() - start);
  return 0;
}
