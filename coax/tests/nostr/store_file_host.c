/* Host for store_file_main.coax (compiled with -Dmain=coax_store_file_main):
     store_file_host FILE PHASE
   runs one phase over FILE on a 64 MiB stack and prints one observation per
   line. Effects only. */
#include "store-file-native.h"
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int coax_store_file_main(void);
static int64_t phase;

int64_t sfm_phase(void) { return phase; }
int64_t sfm_out(int64_t v) { printf("%lld\n", (long long)v); return 0; }

static void *run(void *arg) { (void)arg; coax_store_file_main(); return NULL; }

int main(int argc, char **argv) {
  if (argc != 3 || cxstore_set_path(argv[1]) != 0) { fprintf(stderr, "usage: store_file_host FILE PHASE\n"); return 2; }
  phase = strtoll(argv[2], NULL, 10);
  setvbuf(stdout, NULL, _IOLBF, 0);
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)64 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) return 1;
  pthread_join(t, NULL);
  return 0;
}
