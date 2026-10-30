/* The Coax relay's C host: flags, then the Coax program (relay.coax,
   compiled with -Dmain=coax_relay_main) on a thread with a 512 MiB stack,
   since its connection, frame and store buffers are Coax locals.
     relay --listen 127.0.0.1 --port 7447 [--url ws://name:7447] [--store FILE]
           [--tls-cert CHAIN.pem --tls-key KEY.pem]
   (port 0 picks a free one). --url is the relay URL NIP-42 AUTH events
   must name; without it the relay uses ws://ADDRESS:PORT, or
   wss://ADDRESS:PORT with TLS. --tls-cert and --tls-key (both, PEM) make
   every connection TLS through tls-native.c's server mode; the key file
   must be 0600 or 0400 and owned by this user, and nothing about it is
   ever printed. Challenges take
   32 bytes from getentropy. --store FILE keeps events in FILE (append-only,
   replayed on start); without it they are in memory only. Effects only; every relay decision is Coax. */
#define _DEFAULT_SOURCE /* getentropy */
#include "socket-server-native.h"
#include "store-file-native.h"
#include "tls-native.h"
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
static int tls = 0;

int64_t ry_host_port(void) { return port; }
int64_t ry_host_url_count(void) { return (int64_t)strlen(url); }
int64_t ry_host_url_byte(int64_t i) { return (unsigned char)url[i]; }
int64_t ry_host_address_count(void) { return (int64_t)strlen(address); }
int64_t ry_host_address_byte(int64_t i) { return (unsigned char)address[i]; }
/* One byte of OS entropy, or -1. */
int64_t ry_host_store(void) { return store; }
int64_t ry_host_tls(void) { return tls; }
int64_t ry_host_random(void) {
  unsigned char b;
  return getentropy(&b, 1) == 0 ? b : -1;
}

static void *run(void *arg) {
  (void)arg;
  coax_relay_main();
  return NULL;
}

static const char *tls_why(int64_t e) {
  if (e == CXTLS_KEY_MODE) return "the --tls-key file must be a regular file owned by this user with mode 0600 or 0400";
  if (e == CXTLS_KEY) return "cannot read the --tls-key file, or it is not the certificate's key";
  if (e == CXTLS_CERT) return "cannot read the --tls-cert file";
  if (e == CXTLS_UNSUPPORTED) return "TLS server mode is not available on this platform";
  return "cannot set up TLS";
}

int main(int argc, char **argv) {
  const char *cert = NULL, *key = NULL;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) { address = argv[++i]; cxsrv_set_address(address); }
    else if (!strcmp(argv[i], "--url") && i + 1 < argc) url = argv[++i];
    else if (!strcmp(argv[i], "--tls-cert") && i + 1 < argc) cert = argv[++i];
    else if (!strcmp(argv[i], "--tls-key") && i + 1 < argc) key = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = strtoll(argv[++i], NULL, 10);
    else if (!strcmp(argv[i], "--store") && i + 1 < argc) {
      if (cxstore_set_path(argv[++i]) != 0) { fprintf(stderr, "relay: bad --store path\n"); return 2; }
      store = 1;
    }
    else { fprintf(stderr, "usage: relay [--listen ADDRESS] [--port N] [--url ws://HOST[:PORT][/PATH]] [--store FILE] [--tls-cert CHAIN.pem --tls-key KEY.pem]\n"); return 2; }
  }
  if ((cert == NULL) != (key == NULL)) { fprintf(stderr, "relay: --tls-cert and --tls-key go together\n"); return 2; }
  if (cert != NULL) {
    int64_t e = cxtls_server_load(cert, key);
    if (e != 0) { fprintf(stderr, "relay: %s\n", tls_why(e)); return 2; }
    cxsrv_set_tls(1);
    tls = 1;
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
