/* The Coax relay's C host: flags, then the Coax program (relay.coax,
   compiled with -Dmain=coax_relay_main) on a thread with a 512 MiB stack,
   since its connection, frame and store buffers are Coax locals.
     relay --listen 127.0.0.1 --port 7447 [--url ws://name:7447] [--store FILE]
   (port 0 picks a free one). --url is the relay URL NIP-42 AUTH events
   must name; without it the relay uses ws://ADDRESS:PORT. Challenges take
   32 bytes from getentropy. --store FILE keeps events in FILE (append-only,
   replayed on start); without it they are in memory only. Effects only; every relay decision is Coax. */
#define _DEFAULT_SOURCE /* getentropy */
#include "socket-server-native.h"
#include "store-file-native.h"
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

int coax_relay_main(void);
static int64_t port = 7447;
static const char *address = "127.0.0.1";
static const char *url = "";
static int store = 0;

int64_t ry_host_port(void) { return port; }
int64_t ry_host_url_count(void) { return (int64_t)strlen(url); }
int64_t ry_host_url_byte(int64_t i) { return (unsigned char)url[i]; }
int64_t ry_host_address_count(void) { return (int64_t)strlen(address); }
int64_t ry_host_address_byte(int64_t i) { return (unsigned char)address[i]; }
/* One byte of OS entropy, or -1. */
int64_t ry_host_store(void) { return store; }
int64_t ry_host_random(void) {
  unsigned char b;
  return getentropy(&b, 1) == 0 ? b : -1;
}

static void *run(void *arg) {
  (void)arg;
  coax_relay_main();
  return NULL;
}

int main(int argc, char **argv) {
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) { address = argv[++i]; cxsrv_set_address(address); }
    else if (!strcmp(argv[i], "--url") && i + 1 < argc) url = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = strtoll(argv[++i], NULL, 10);
    else if (!strcmp(argv[i], "--store") && i + 1 < argc) {
      if (cxstore_set_path(argv[++i]) != 0) { fprintf(stderr, "relay: bad --store path\n"); return 2; }
      store = 1;
    }
    else { fprintf(stderr, "usage: relay [--listen ADDRESS] [--port N] [--url ws://HOST[:PORT][/PATH]] [--store FILE]\n"); return 2; }
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
