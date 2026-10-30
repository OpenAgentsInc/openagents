#define _POSIX_C_SOURCE 200809L
#include "project_history_adapter.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc != 2) return 1;
  coax_t_Store *s = calloc(1, sizeof *s);
  coax_t_Fold *f = calloc(1, sizeof *f);
  coax_t_Tables *t = calloc(1, sizeof *t);
  if (!s || !f || !t) return 2;
  char *line = NULL; size_t cap = 0; ssize_t n;
  while ((n = getline(&line, &cap, stdin)) >= 0) {
    if (n && line[n-1] == '\n') --n;
    if (history_feed((coax_v_t_Store){s,1}, (coax_string){line,n}, (coax_string){argv[1],(int64_t)strlen(argv[1])}) < 0) return 3;
  }
  int64_t size = history_write((coax_v_t_Store){s,1},(coax_v_t_Fold){f,1},(coax_v_t_Tables){t,1});
  if (size < 0 || fwrite(t->log.e,1,(size_t)size,stdout) != (size_t)size) return 4;
  free(line); free(t); free(f); free(s); return 0;
}
