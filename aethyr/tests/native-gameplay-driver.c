/* Native integration programs: C supplies dispatch, stack and process effects only. */
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { const char *name; void (*run)(void); unsigned body_calls; } NativeScenario;
#include "native-gameplay-registry.h"
extern void aethyr_native_assert_probe(void);

static void *execute(void *opaque) {
  const NativeScenario *scenario = opaque;
  scenario->run();
  return NULL;
}

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  if (!strcmp(argv[1], "--assert-probe")) { aethyr_native_assert_probe(); return 3; }
  const NativeScenario *chosen = NULL;
  for (unsigned i = 0; i < sizeof scenarios / sizeof scenarios[0]; ++i) {
    if (!strcmp(scenarios[i].name, argv[1])) {
      if (chosen) return 4;
      chosen = scenarios + i;
    }
  }
  if (!chosen) return 5;
  pthread_attr_t attributes;
  if (pthread_attr_init(&attributes) || pthread_attr_setstacksize(&attributes, 128u * 1024u * 1024u)) return 6;
  pthread_t worker;
  int created = pthread_create(&worker, &attributes, execute, (void *)chosen);
  pthread_attr_destroy(&attributes);
  if (created || pthread_join(worker, NULL)) return 7;
  printf("ok %s (%u original body calls)\n", chosen->name, chosen->body_calls);
  return 0;
}
