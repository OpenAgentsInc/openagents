/* Independently declared verification-only ABI; generated C is a separate TU. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct Coax_VerificationCheck_v1 {
  int64_t declaration_index;
  const char *name;
  int64_t name_count;
  const char *source;
  int64_t source_count;
  int64_t line, column;
  void (*run)(void);
} Coax_VerificationCheck_v1;

extern const Coax_VerificationCheck_v1 *coax_verification_checks_v1(int64_t *count);

static void hex_bytes(const char *data, int64_t count) {
  for (int64_t i = 0; i < count; i++) printf("%02x", (unsigned char)data[i]);
}

int main(int argc, char **argv) {
  int64_t count = -1;
  const Coax_VerificationCheck_v1 *checks = coax_verification_checks_v1(&count);
  if (checks == NULL || count <= 0 || count > 100) return 90;
  if (argc == 2 && strcmp(argv[1], "metadata") == 0) {
    for (int64_t i = 0; i < count; i++) {
      const Coax_VerificationCheck_v1 *check = &checks[i];
      if (!check->name || !check->source || !check->run || check->name_count < 0 ||
          check->source_count <= 0 || check->line <= 0 || check->column <= 0) return 91;
      printf("%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t", check->declaration_index,
             check->line, check->column);
      hex_bytes(check->name, check->name_count);
      putchar('\t');
      hex_bytes(check->source, check->source_count);
      putchar('\n');
    }
    return 0;
  }
  if (argc == 3 && strcmp(argv[1], "one") == 0) {
    char *end = NULL;
    int64_t row = strtoll(argv[2], &end, 10);
    if (end == argv[2] || *end || row < 0 || row >= count) return 93;
    checks[row].run();
    puts("ran 1 native checks");
    return 0;
  }
  if (argc != 1) return 92;
  for (int64_t i = 0; i < count; i++) checks[i].run();
  printf("ran %" PRId64 " native checks\n", count);
  return 0;
}
