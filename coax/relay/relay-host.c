/* The Coax relay's C host: flags, then the Coax program (relay.coax,
   compiled with -Dmain=coax_relay_main) on a thread with a 512 MiB stack,
   since its connection, frame and store buffers are Coax locals.
     relay --listen 127.0.0.1 --port 7447     (port 0 picks a free one)
   Effects only; every relay decision is Coax. */
#include "socket-server-native.h"
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int coax_relay_main(void);
static int64_t port = 7447;

int64_t rl_host_port(void) { return port; }

static void *run(void *arg) {
  (void)arg;
  coax_relay_main();
  return NULL;
}

int main(int argc, char **argv) {
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) cxsrv_set_address(argv[++i]);
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = strtoll(argv[++i], NULL, 10);
    else { fprintf(stderr, "usage: relay [--listen ADDRESS] [--port N]\n"); return 2; }
  }
  setvbuf(stdout, NULL, _IOLBF, 0);
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)512 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) { fprintf(stderr, "relay: no thread\n"); return 1; }
  pthread_join(t, NULL);
  return 0;
}
