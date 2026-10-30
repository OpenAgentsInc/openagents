/* Measurement harness only. Reads PUBLIC BIP-340 fixtures, never real keys.
   Coax performs all signing. Mark only key and aux undefined during the call.
   Stage statuses are observable; signature bytes are observable after DONE.
   After signing ends, define the workspace only for an actual zero-byte check.
   No intermediate public-key/R declassification is inferred or injected. */
#include "es.h"
#include <valgrind/memcheck.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint8_t key[32], aux[32], message[1024], output[64], expected[64];
static coax_t_SignWork work;
static volatile unsigned control_sink;

static int nibble(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}
static int unhex(const char *text, uint8_t *bytes, size_t capacity) {
  if (!strcmp(text, "-")) return 0;
  size_t size = strlen(text);
  if ((size & 1) || size / 2 > capacity) return -1;
  for (size_t i = 0; i < size / 2; i++) {
    int a = nibble(text[i * 2]), b = nibble(text[i * 2 + 1]);
    if (a < 0 || b < 0) return -1;
    bytes[i] = (uint8_t)((a << 4) | b);
  }
  return (int)(size / 2);
}
static bool bytes_are(const void *pointer, size_t size, uint8_t expected_byte) {
  const uint8_t *bytes = pointer;
  for (size_t i = 0; i < size; i++) if (bytes[i] != expected_byte) return false;
  return true;
}
int main(int argc, char **argv) {
  char key_hex[65], aux_hex[65], message_hex[2049], signature_hex[129];
  if (argc != 2 || scanf("%64s %64s %2048s %128s", key_hex, aux_hex, message_hex, signature_hex) != 4) return 2;
  int key_size = unhex(key_hex, key, sizeof key);
  int aux_size = unhex(aux_hex, aux, sizeof aux);
  int message_size = unhex(message_hex, message, sizeof message);
  if (key_size != 32 || aux_size != 32 || message_size < 0 || unhex(signature_hex, expected, sizeof expected) != 64) return 2;
  const char *mode = argv[1];
  bool valid = !strcmp(mode, "valid"), control = !strcmp(mode, "positive-control");
  bool zero = !strcmp(mode, "key-zero"), order = !strcmp(mode, "key-order"), maximum = !strcmp(mode, "key-max");
  bool short_key = !strcmp(mode, "short-key"), overlap = !strcmp(mode, "overlap");
  bool changed_message = !strcmp(mode, "message-change"), changed_candidate = !strcmp(mode, "candidate-change");
  if (!(valid || control || zero || order || maximum || short_key || overlap || changed_message || changed_candidate)) return 2;
  if ((changed_message || changed_candidate) && message_size != 32) return 2;
  if (zero) memset(key, 0, sizeof key);
  if (maximum) memset(key, 255, sizeof key);
  if (order && unhex("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", key, 32) != 32) return 2;
  if (short_key) key_size = 31;
  memset(output, 165, sizeof output);
  VALGRIND_MAKE_MEM_UNDEFINED(key, sizeof key);
  VALGRIND_MAKE_MEM_UNDEFINED(aux, sizeof aux);
  if (control) {
    if (key[0] & 1) control_sink++; else control_sink += 2;
    puts("{\"ok\":true,\"positive_control\":true}");
    return 0;
  }
  coax_v_u8 k = {key, key_size}, a = {aux, aux_size}, m = {message, message_size}, o = {output, 64};
  coax_v_t_SignWork w = {&work, 1};
  int64_t status = 0;
  int calls = 0;
  do {
    status = schnorr_sign_step(k, a, m, w, o, overlap);
    calls++;
    VALGRIND_MAKE_MEM_DEFINED(&status, sizeof status);
    if (changed_message && calls == 2) message[0] ^= 1;
    if (changed_candidate && calls == 3) work.sig.e[40] ^= 1;
  } while (status == 1 && calls < 16);
  /* These are observations after the measured signing calls, not taint
     exemptions inside them. Byte checks also verify that actual stores clear
     the workspace and leave refused output untouched. */
  VALGRIND_MAKE_MEM_DEFINED(&work, sizeof work);
  bool clear = bytes_are(&work, sizeof work, 0);
  bool signature_matches = false, output_untouched = false;
  if (status == 2) {
    VALGRIND_MAKE_MEM_DEFINED(output, sizeof output);
    signature_matches = memcmp(output, expected, sizeof output) == 0;
  } else {
    VALGRIND_MAKE_MEM_DEFINED(output, sizeof output);
    output_untouched = bytes_are(output, sizeof output, 165);
  }
  int expected_calls = valid || changed_candidate ? 6 : changed_message ? 3 : 1;
  bool ok = clear && calls == expected_calls && (valid ? status == 2 && signature_matches : status == 0 && output_untouched);
  printf("{\"ok\":%s,\"calls\":%d,\"status\":%lld,\"workspace_zero\":%s,\"signature_matches\":%s,\"output_untouched\":%s}\n",
         ok ? "true" : "false", calls, (long long)status, clear ? "true" : "false",
         signature_matches ? "true" : "false", output_untouched ? "true" : "false");
  return ok ? 0 : 3;
}
