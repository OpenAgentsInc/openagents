/*
 * Coldcard generator vector and throughput harness (openagents #9297, OFR-015).
 *
 * Produces the expected values for the frozen generator vectors from the
 * PINNED TARGET SOURCE rather than from this repository's TypeScript
 * reproduction, and measures a candidate-search rate through that same source.
 *
 * Two passes run per vector and must agree:
 *
 *   target pass  calls the pinned `my_random_bytes` and `_rand_below`
 *                directly. This is the target's own control flow: its health
 *                check, its XOR order, its memcpy, its mask and retry loop.
 *                It yields the output bytes and the shuffle selections, but
 *                its intermediate generator state is not observable from
 *                outside the functions.
 *
 *   mirror pass  drives the pinned `my_yasmarang` and `_bit_length` one step
 *                at a time, so every intermediate state is observable. The
 *                stepping order is the reproduction's model of the target.
 *
 * The two passes are compared on output bytes, shuffle selections, and final
 * generator state. A disagreement means the reproduction's model of the
 * target's control flow is wrong, and the harness exits non-zero rather than
 * emitting a vector. That comparison is the falsifier the frozen vectors
 * cannot supply on their own.
 *
 * Every 32-bit value printed here came out of the pinned source's arithmetic.
 * This file supplies stepping order, the candidate sweep, and JSON framing.
 */

#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "oa_harness.h"

#define OA_MAX_CALLS 8192
#define OA_MAX_PROVIDER_WORDS 512
#define OA_MAX_SYMBOLS 64
#define OA_MAX_BYTES 4096

typedef enum { OA_PROVIDER_YASMARANG, OA_PROVIDER_FIXTURE } oa_provider_kind;

static oa_provider_kind provider_kind = OA_PROVIDER_YASMARANG;
static uint32_t provider_words[OA_MAX_PROVIDER_WORDS];
static int provider_word_count = 0;
static int provider_index = 0;
static uint32_t provider_seed_pad = 0;
static uint32_t provider_seed_n = 0;
static uint32_t provider_seed_d = 0;
static uint8_t provider_seed_dat = 0;

typedef struct {
  const char *kind;
  uint32_t output;
  int swap_index;
  int swap_selected;
  uint32_t lib_pad;
  uint32_t lib_n;
  uint32_t lib_d;
  uint8_t lib_dat;
  int prov_index;
  uint32_t prov_pad;
  uint32_t prov_n;
  uint32_t prov_d;
  uint8_t prov_dat;
} oa_call;

static oa_call calls[OA_MAX_CALLS];
static int call_count = 0;
static int shuffle_call_start = 0;

static void provider_reset(void) {
  provider_index = 0;
  if (provider_kind == OA_PROVIDER_YASMARANG) {
    oa_prov_set_state(provider_seed_pad, provider_seed_n, provider_seed_d, provider_seed_dat);
  }
}

static uint32_t provider_next(void) {
  if (provider_kind == OA_PROVIDER_FIXTURE) {
    if (provider_index >= provider_word_count) {
      oa_trap("approved provider fixture exhausted before the run completed");
    }
    return provider_words[provider_index++];
  }
  provider_index += 1;
  return oa_prov_yasmarang();
}

uint32_t oa_provider32(void) {
  return provider_next();
}

static void record(const char *kind, uint32_t output, int swap_index, int swap_selected) {
  if (call_count >= OA_MAX_CALLS) oa_trap("call trace overflow");
  oa_call *call = &calls[call_count++];
  call->kind = kind;
  call->output = output;
  call->swap_index = swap_index;
  call->swap_selected = swap_selected;
  oa_lib_get_state(&call->lib_pad, &call->lib_n, &call->lib_d, &call->lib_dat);
  call->prov_index = provider_index;
  if (provider_kind == OA_PROVIDER_YASMARANG) {
    oa_prov_get_state(&call->prov_pad, &call->prov_n, &call->prov_d, &call->prov_dat);
  } else {
    call->prov_pad = 0;
    call->prov_n = 0;
    call->prov_d = 0;
    call->prov_dat = 0;
  }
}

static void print_libngu_state(uint32_t pad, uint32_t n, uint32_t d, uint8_t dat) {
  printf("{\"dat\":%u,\"d\":%u,\"n\":%u,\"pad\":%u}", (unsigned)dat, d, n, pad);
}

static void print_provider_state(int index, uint32_t pad, uint32_t n, uint32_t d, uint8_t dat) {
  if (provider_kind == OA_PROVIDER_FIXTURE) {
    printf("{\"index\":%d,\"kind\":\"approved_fixture\"}", index);
    return;
  }
  printf("{\"kind\":\"yasmarang\",\"state\":");
  print_libngu_state(pad, n, d, dat);
  printf("}");
}

static void print_call(const oa_call *call, int sequence) {
  printf("{\"kind\":\"%s\",\"sequence\":%d,", call->kind, sequence);
  if (strcmp(call->kind, "shuffle_swap") == 0) {
    printf("\"swap\":{\"index\":%d,\"selected\":%d},", call->swap_index, call->swap_selected);
  } else {
    printf("\"output\":%" PRIu32 ",", call->output);
  }
  printf("\"libngu\":");
  print_libngu_state(call->lib_pad, call->lib_n, call->lib_d, call->lib_dat);
  printf(",\"provider\":");
  print_provider_state(call->prov_index, call->prov_pad, call->prov_n, call->prov_d,
                       call->prov_dat);
  printf("}");
}

static void print_state_pair(void) {
  uint32_t pad, n, d;
  uint8_t dat;
  oa_lib_get_state(&pad, &n, &d, &dat);
  printf("{\"libngu\":");
  print_libngu_state(pad, n, d, dat);
  printf(",\"provider\":");
  if (provider_kind == OA_PROVIDER_FIXTURE) {
    print_provider_state(provider_index, 0, 0, 0, 0);
  } else {
    uint32_t ppad, pn, pd;
    uint8_t pdat;
    oa_prov_get_state(&ppad, &pn, &pd, &pdat);
    print_provider_state(provider_index, ppad, pn, pd, pdat);
  }
  printf("}");
}

static uint32_t parse_u32(const char *text) {
  char *end = NULL;
  unsigned long long value = strtoull(text, &end, 0);
  if (end == text || *end != '\0' || value > 0xffffffffULL) oa_trap("bad 32-bit argument");
  return (uint32_t)value;
}

static const char *option(int argc, char **argv, const char *name) {
  for (int index = 1; index + 1 < argc; index += 1) {
    if (strcmp(argv[index], name) == 0) return argv[index + 1];
  }
  return NULL;
}

static int split_u32(const char *text, uint32_t *out, int limit) {
  int count = 0;
  char buffer[64];
  while (*text != '\0') {
    size_t length = 0;
    while (text[length] != '\0' && text[length] != ',') length += 1;
    if (length == 0 || length >= sizeof(buffer)) oa_trap("bad comma separated argument");
    memcpy(buffer, text, length);
    buffer[length] = '\0';
    if (count >= limit) oa_trap("comma separated argument too long");
    out[count++] = parse_u32(buffer);
    text += length;
    if (*text == ',') text += 1;
  }
  return count;
}

static void require_little_endian(void) {
  const uint32_t probe = 0x01020304u;
  uint8_t bytes[4];
  memcpy(bytes, &probe, 4);
  if (bytes[0] != 0x04 || bytes[3] != 0x01) {
    oa_trap("host is not little endian; the target memcpy word layout would differ");
  }
}

static int run_vector(int argc, char **argv) {
  const char *libngu_arg = option(argc, argv, "--libngu");
  const char *provider_yasmarang_arg = option(argc, argv, "--provider-yasmarang");
  const char *provider_fixture_arg = option(argc, argv, "--provider-fixture");
  const char *count_arg = option(argc, argv, "--count");
  const char *symbol_arg = option(argc, argv, "--symbols");
  const char *reseed_arg = option(argc, argv, "--reseed");
  if (libngu_arg == NULL || count_arg == NULL || symbol_arg == NULL) {
    oa_trap("vector mode requires --libngu, --count and --symbols");
  }
  if ((provider_yasmarang_arg == NULL) == (provider_fixture_arg == NULL)) {
    oa_trap("vector mode requires exactly one of --provider-yasmarang and --provider-fixture");
  }

  uint32_t libngu_fields[4];
  if (split_u32(libngu_arg, libngu_fields, 4) != 4) oa_trap("--libngu needs pad,n,d,dat");
  uint32_t lib_pad = libngu_fields[0];
  const uint32_t lib_n = libngu_fields[1];
  const uint32_t lib_d = libngu_fields[2];
  const uint8_t lib_dat = (uint8_t)libngu_fields[3];
  if (reseed_arg != NULL) {
    char *end = NULL;
    const unsigned long long reseed = strtoull(reseed_arg, &end, 0);
    if (end == reseed_arg || *end != '\0') oa_trap("bad --reseed argument");
    lib_pad = (uint32_t)(reseed & 0xffffffffULL);
  }

  if (provider_yasmarang_arg != NULL) {
    uint32_t fields[4];
    if (split_u32(provider_yasmarang_arg, fields, 4) != 4) {
      oa_trap("--provider-yasmarang needs pad,n,d,dat");
    }
    provider_kind = OA_PROVIDER_YASMARANG;
    provider_seed_pad = fields[0];
    provider_seed_n = fields[1];
    provider_seed_d = fields[2];
    provider_seed_dat = (uint8_t)fields[3];
  } else {
    provider_kind = OA_PROVIDER_FIXTURE;
    provider_word_count =
        split_u32(provider_fixture_arg, provider_words, OA_MAX_PROVIDER_WORDS);
    if (provider_word_count < 1) oa_trap("--provider-fixture needs at least one word");
  }

  const uint32_t count = parse_u32(count_arg);
  if (count < 1 || count > OA_MAX_BYTES) oa_trap("--count out of range");
  uint32_t symbol_values[OA_MAX_SYMBOLS];
  const int symbol_count = split_u32(symbol_arg, symbol_values, OA_MAX_SYMBOLS);
  if (symbol_count < 2) oa_trap("--symbols needs at least two entries");

  /* ---- target pass: the pinned source's own control flow ---- */
  oa_lib_set_state(lib_pad, lib_n, lib_d, lib_dat);
  provider_reset();
  uint8_t target_bytes[OA_MAX_BYTES];
  oa_lib_random_bytes(target_bytes, count);
  int target_selection[OA_MAX_SYMBOLS];
  int target_selection_count = 0;
  for (int index = symbol_count - 1; index > 0; index -= 1) {
    target_selection[target_selection_count++] = oa_lib_rand_below(index + 1);
  }
  uint32_t target_final_lib_pad, target_final_lib_n, target_final_lib_d;
  uint8_t target_final_lib_dat;
  oa_lib_get_state(&target_final_lib_pad, &target_final_lib_n, &target_final_lib_d,
                   &target_final_lib_dat);
  uint32_t target_final_prov_pad = 0, target_final_prov_n = 0, target_final_prov_d = 0;
  uint8_t target_final_prov_dat = 0;
  if (provider_kind == OA_PROVIDER_YASMARANG) {
    oa_prov_get_state(&target_final_prov_pad, &target_final_prov_n, &target_final_prov_d,
                      &target_final_prov_dat);
  }
  const int target_provider_draws = provider_index;

  /* ---- mirror pass: one observable step at a time ---- */
  oa_lib_set_state(lib_pad, lib_n, lib_d, lib_dat);
  provider_reset();
  call_count = 0;
  uint8_t mirror_bytes[OA_MAX_BYTES];
  uint32_t last_provider_output = 0;
  for (uint32_t offset = 0; offset < count; offset += 4) {
    const uint32_t provider_output = provider_next();
    if (provider_output == last_provider_output) {
      oa_trap("adjacent provider output health check tripped");
    }
    last_provider_output = provider_output;
    record("provider", provider_output, 0, 0);
    const uint32_t libngu_output = oa_lib_yasmarang();
    record("libngu", libngu_output, 0, 0);
    const uint32_t word = provider_output ^ libngu_output;
    record("combined_word", word, 0, 0);
    const uint32_t here = count - offset < 4 ? count - offset : 4;
    memcpy(mirror_bytes + offset, &word, here);
  }
  shuffle_call_start = call_count;
  uint32_t post_bytes_lib_pad, post_bytes_lib_n, post_bytes_lib_d;
  uint8_t post_bytes_lib_dat;
  oa_lib_get_state(&post_bytes_lib_pad, &post_bytes_lib_n, &post_bytes_lib_d,
                   &post_bytes_lib_dat);
  const int post_bytes_provider_index = provider_index;
  uint32_t post_bytes_prov_pad = 0, post_bytes_prov_n = 0, post_bytes_prov_d = 0;
  uint8_t post_bytes_prov_dat = 0;
  if (provider_kind == OA_PROVIDER_YASMARANG) {
    oa_prov_get_state(&post_bytes_prov_pad, &post_bytes_prov_n, &post_bytes_prov_d,
                      &post_bytes_prov_dat);
  }

  int shuffled[OA_MAX_SYMBOLS];
  for (int index = 0; index < symbol_count; index += 1) shuffled[index] = (int)symbol_values[index];
  int mirror_selection[OA_MAX_SYMBOLS];
  int mirror_selection_count = 0;
  for (int index = symbol_count - 1; index > 0; index -= 1) {
    const int maximum = index + 1;
    const int bits = oa_lib_bit_length((uint32_t)maximum);
    const uint32_t mask = ((uint32_t)2 << bits) - 1u;
    const uint32_t provider_output = provider_next();
    record("provider", provider_output, 0, 0);
    const uint32_t libngu_output = oa_lib_yasmarang();
    record("libngu", libngu_output, 0, 0);
    uint32_t candidate_word = provider_output ^ libngu_output;
    record("combined_word", candidate_word, 0, 0);
    int selected = 0;
    for (;;) {
      const uint32_t candidate = candidate_word & mask;
      if (candidate < (uint32_t)maximum) {
        selected = (int)candidate;
        break;
      }
      const uint32_t retry = oa_lib_yasmarang();
      candidate_word ^= retry;
      record("uniform_retry", retry, 0, 0);
    }
    mirror_selection[mirror_selection_count++] = selected;
    const int swap = shuffled[selected];
    shuffled[selected] = shuffled[index];
    shuffled[index] = swap;
    record("shuffle_swap", 0, index, selected);
  }
  uint32_t mirror_final_lib_pad, mirror_final_lib_n, mirror_final_lib_d;
  uint8_t mirror_final_lib_dat;
  oa_lib_get_state(&mirror_final_lib_pad, &mirror_final_lib_n, &mirror_final_lib_d,
                   &mirror_final_lib_dat);

  /* ---- the falsifier ---- */
  const int bytes_match = memcmp(target_bytes, mirror_bytes, count) == 0;
  int selections_match = target_selection_count == mirror_selection_count;
  for (int index = 0; selections_match && index < target_selection_count; index += 1) {
    selections_match = target_selection[index] == mirror_selection[index];
  }
  int final_state_match = target_final_lib_pad == mirror_final_lib_pad &&
                          target_final_lib_n == mirror_final_lib_n &&
                          target_final_lib_d == mirror_final_lib_d &&
                          target_final_lib_dat == mirror_final_lib_dat &&
                          target_provider_draws == provider_index;
  if (provider_kind == OA_PROVIDER_YASMARANG) {
    uint32_t pad, n, d;
    uint8_t dat;
    oa_prov_get_state(&pad, &n, &d, &dat);
    final_state_match = final_state_match && target_final_prov_pad == pad &&
                        target_final_prov_n == n && target_final_prov_d == d &&
                        target_final_prov_dat == dat;
  }
  if (!bytes_match || !selections_match || !final_state_match) {
    fprintf(stderr,
            "oa-coldcard-generator refusal: stepping model disagrees with the pinned target "
            "control flow (bytes=%d selections=%d finalState=%d)\n",
            bytes_match, selections_match, final_state_match);
    return 13;
  }

  printf("{\"schema\":\"openagents.coldcard_generator_c_capture.v1\",\"mode\":\"vector\",");
  printf("\"initialState\":{\"libngu\":");
  print_libngu_state(lib_pad, lib_n, lib_d, lib_dat);
  printf(",\"provider\":");
  print_provider_state(0, provider_seed_pad, provider_seed_n, provider_seed_d, provider_seed_dat);
  printf("},\"outputHex\":\"");
  for (uint32_t index = 0; index < count; index += 1) printf("%02x", target_bytes[index]);
  printf("\",\"calls\":[");
  for (int index = 0; index < shuffle_call_start; index += 1) {
    if (index > 0) printf(",");
    print_call(&calls[index], index + 1);
  }
  printf("],\"shuffleCalls\":[");
  for (int index = shuffle_call_start; index < call_count; index += 1) {
    if (index > shuffle_call_start) printf(",");
    print_call(&calls[index], index - shuffle_call_start + 1);
  }
  printf("],\"shuffled\":[");
  for (int index = 0; index < symbol_count; index += 1) {
    printf("%s%d", index > 0 ? "," : "", shuffled[index]);
  }
  printf("],\"selections\":[");
  for (int index = 0; index < target_selection_count; index += 1) {
    printf("%s%d", index > 0 ? "," : "", target_selection[index]);
  }
  printf("],\"postBytesState\":{\"libngu\":");
  print_libngu_state(post_bytes_lib_pad, post_bytes_lib_n, post_bytes_lib_d, post_bytes_lib_dat);
  printf(",\"provider\":");
  print_provider_state(post_bytes_provider_index, post_bytes_prov_pad, post_bytes_prov_n,
                       post_bytes_prov_d, post_bytes_prov_dat);
  printf("},\"finalState\":");
  print_state_pair();
  printf(",\"targetPathAgreement\":{\"bytes\":true,\"finalState\":true,\"selections\":true}}\n");
  return 0;
}

static int run_throughput(int argc, char **argv) {
  const char *min_nanos_arg = option(argc, argv, "--min-nanos");
  const char *batch_arg = option(argc, argv, "--batch");
  if (min_nanos_arg == NULL || batch_arg == NULL) {
    oa_trap("throughput mode requires --min-nanos and --batch");
  }
  char *end = NULL;
  const unsigned long long min_nanos = strtoull(min_nanos_arg, &end, 0);
  if (end == min_nanos_arg || *end != '\0' || min_nanos == 0) oa_trap("bad --min-nanos");
  const uint32_t batch = parse_u32(batch_arg);
  if (batch < 1) oa_trap("bad --batch");

  /* One candidate is one guess at the fallback provider's seed triple, taken
   * through the pinned generator to a full 32-byte wallet entropy draw and
   * compared against a known prefix. The seed sweep mirrors the work-factor
   * dimensions: the low index varies the timer-derived pad, the high index
   * varies the RTC-derived word. */
  provider_kind = OA_PROVIDER_YASMARANG;
  const uint32_t timer_span = 80000u;
  uint8_t entropy[32];
  const uint8_t target_prefix[8] = {0xde, 0xec, 0x71, 0x77, 0x00, 0x11, 0x22, 0x33};
  uint64_t candidates = 0;
  uint64_t matches = 0;
  struct timespec started, now;
  if (clock_gettime(CLOCK_MONOTONIC, &started) != 0) oa_trap("clock_gettime failed");
  unsigned long long elapsed = 0;
  do {
    for (uint32_t index = 0; index < batch; index += 1) {
      provider_seed_pad = 0x1a2b3c4du + (uint32_t)(candidates % timer_span);
      provider_seed_n = 0x00204060u + (uint32_t)(candidates / timer_span);
      provider_seed_d = 0x00005a5au;
      provider_seed_dat = 0;
      oa_lib_set_state(0x0a8ce26fu, 69u, 233u, 0u);
      provider_reset();
      oa_lib_random_bytes(entropy, sizeof(entropy));
      if (memcmp(entropy, target_prefix, sizeof(target_prefix)) == 0) matches += 1;
      candidates += 1;
    }
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) oa_trap("clock_gettime failed");
    elapsed = (unsigned long long)(now.tv_sec - started.tv_sec) * 1000000000ULL +
              (unsigned long long)(now.tv_nsec - started.tv_nsec);
  } while (elapsed < min_nanos);

  printf("{\"schema\":\"openagents.coldcard_generator_c_capture.v1\",\"mode\":\"throughput\",");
  printf("\"candidatesEvaluated\":\"%" PRIu64 "\",", candidates);
  printf("\"elapsedNanoseconds\":\"%llu\",", elapsed);
  printf("\"prefixMatches\":%" PRIu64 ",", matches);
  printf("\"candidateWorkUnit\":\"seed the pinned fallback provider from one candidate triple, "
         "run the pinned generator to a 32 byte wallet entropy draw, compare an 8 byte prefix; "
         "downstream BIP39 and BIP32 derivation is not included\"}\n");
  return 0;
}

int main(int argc, char **argv) {
  require_little_endian();
  if (argc < 2) {
    fprintf(stderr, "usage: oa-coldcard-generator (vector|throughput) [options]\n");
    return 2;
  }
  if (strcmp(argv[1], "vector") == 0) return run_vector(argc, argv);
  if (strcmp(argv[1], "throughput") == 0) return run_throughput(argc, argv);
  fprintf(stderr, "unknown mode %s\n", argv[1]);
  return 2;
}
