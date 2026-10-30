#include "project_details_adapter.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
int main(void) {
  coax_t_Tables *tables = calloc(1, sizeof *tables);
  uint8_t in[4096], out[8192];
  if (!tables) return 2;
  size_t n = fread(in, 1, sizeof in, stdin);
  if (!feof(stdin)) { free(tables); return 3; }
  int64_t written = details_body((coax_v_t_Tables){tables, 1}, (coax_string){(char *)in, (int64_t)n}, (coax_v_u8){out, sizeof out});
  if (written < 0 || fwrite(out, 1, (size_t)written, stdout) != (size_t)written) { free(tables); return 4; }
  free(tables);
  return 0;
}
