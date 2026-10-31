/* Fixed host adapter for EP1. The linked Coax policy admits requests and
 * metadata and encodes every frame. This file owns table access, stdio and
 * the one actual function-pointer call; it supplies no check verdict. */
#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>

#include "native-check-observer.h"
#include "native-check-execution-policy.h"

enum { FRAME_CAPACITY = 8232 };

static coax_string argument(const char *value) {
  coax_string result = {value, value ? (int64_t)strlen(value) : 0};
  return result;
}

static int write_frame(const uint8_t *frame, int64_t size) {
  /* The host independently protects the concrete I/O buffer extent. */
  if (size < 0 || (uint64_t)size > FRAME_CAPACITY) return 0;
  if (fwrite(frame, 1, (size_t)size, stdout) != (size_t)size) return 0;
  return fflush(stdout) == 0;
}

static int row_valid(int64_t count, int64_t ordinal, const Coax_VerificationCheck_v1 *row) {
  return EP_row_valid(count, ordinal, row->declaration_index, row->line, row->column,
                      row->name_count, row->source_count, row->run != NULL,
                      row->name != NULL, row->source != NULL);
}

static int write_row(uint32_t kind, int64_t ordinal, const Coax_VerificationCheck_v1 *row,
                     uint8_t *frame) {
  coax_string name = {row->name, row->name_count};
  coax_string source = {row->source, row->source_count};
  coax_v_u8 output = {frame, FRAME_CAPACITY};
  int64_t size = EP_encode_row(kind, ordinal, row->declaration_index, name, source,
                               row->line, row->column, output);
  return write_frame(frame, size);
}

int main(int argc, char **argv) {
  coax_string mode = argument(argc > 1 ? argv[1] : NULL);
  coax_string ordinal_text = argument(argc > 2 ? argv[2] : NULL);
  coax_t_EP_Request request = EP_request(argc, mode, ordinal_text);
  if (request.reason) return 64;
  if (!EP_native_profile(sizeof(void *), sizeof(long), CHAR_BIT)) return 69;

  int64_t count = -1;
  const Coax_VerificationCheck_v1 *checks = coax_verification_checks_v1(&count);
  if (!EP_table(count, checks != NULL)) return 65;
  for (int64_t ordinal = 0; ordinal < count; ++ordinal)
    if (!row_valid(count, ordinal, &checks[ordinal])) return 66;

  uint8_t frame[FRAME_CAPACITY];
  if (request.mode == 1) {
    coax_v_u8 output = {frame, FRAME_CAPACITY};
    if (!write_frame(frame, EP_encode_header(count, output))) return 68;
    for (int64_t ordinal = 0; ordinal < count; ++ordinal)
      if (!write_row(2, ordinal, &checks[ordinal], frame)) return 68;
    return 0;
  }

  if (request.mode != 2) return 64;
  if (!EP_select(count, request.ordinal)) return 67;
  const Coax_VerificationCheck_v1 *row = &checks[request.ordinal];
  /* No body runs until its complete Coax-encoded BEGIN reached stdio and
   * fflush completed. An assertion/signal leaves BEGIN without END. */
  if (!write_row(3, request.ordinal, row, frame)) return 68;
  row->run();
  if (!write_row(4, request.ordinal, row, frame)) return 68;
  return 0;
}
