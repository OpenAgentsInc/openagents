/* The Coax relay's C host: flags, then the Coax relay (relay.coax's
   #export_c ry_main) on a thread with a 512 MiB stack, since its
   connection and frame buffers are Coax locals. The store's arrays are
   allocated here, zeroed, at the size relay.coax gives for
   --store-capacity (ry_store_events, ry_store_arena, ry_store_tags), and
   lent to ry_main for the life of the process.
     relay --listen 127.0.0.1 --port 7447 [--url ws://name:7447] [--store FILE]
           [--store-capacity EVENTS] [--tls-cert CHAIN.pem --tls-key KEY.pem]
           [--rate EVENTS,REQS,KEY_EVENTS] [--pow BITS] [--admin PUBKEY]... [--groups]
   (port 0 picks a free one). --url is the relay URL NIP-42 AUTH events
   must name; without it the relay uses ws://ADDRESS:PORT, or
   wss://ADDRESS:PORT with TLS. --tls-cert and --tls-key (both, PEM) make
   every connection TLS through tls-native.c's server mode; the key file
   must be 0600 or 0400 and owned by this user, and nothing about it is
   ever printed. Challenges take
   32 bytes from getentropy. --store FILE keeps events in FILE (append-only,
   replayed on start); without it they are in memory only. --rate sets the
   per-minute limits (EVENTs and REQs per connection, EVENTs per key;
   relay_rate.coax's defaults 120,120,60 without it) and --pow the NIP-13
   difficulty an EVENT needs (0, off, without it); relay.coax checks both
   values. --admin PUBKEY (64 hex digits, up to 4 times) turns on NIP-86
   management for those keys; relay.coax checks each. --groups turns on
   NIP-29 groups. Effects only; every relay decision is Coax. */
#define _DEFAULT_SOURCE /* getentropy */
#include "socket-server-native.h"
#include "store-file-native.h"
#include "tls-native.h"
#include "relay.h"
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

static long long capacity = -1;
static coax_v_u8 arena, count_seen;
static coax_t_RsdRec *recs; static int64_t events;
static coax_v_s64 by_id, by_ak, by_time, tag_key, tag_rec, count_out;
static int64_t port = 7447;
static const char *address = "127.0.0.1";
static const char *url = "";
static int store = 0;
static int tls = 0;
static long long rate[3] = {-1, -1, -1};
static long long pow_bits = -1;
static const char *admins[8];
static int admin_n = 0;
static int groups = 0;

int64_t ry_host_port(void) { return port; }
int64_t ry_host_url_count(void) { return (int64_t)strlen(url); }
int64_t ry_host_url_byte(int64_t i) { return (unsigned char)url[i]; }
int64_t ry_host_address_count(void) { return (int64_t)strlen(address); }
int64_t ry_host_address_byte(int64_t i) { return (unsigned char)address[i]; }
/* One byte of OS entropy, or -1. */
int64_t ry_host_store(void) { return store; }
int64_t ry_host_tls(void) { return tls; }
/* --rate's three values (EVENTS, REQS, KEY_EVENTS), or -1 without --rate. */
int64_t ry_host_rate(int64_t i) { return i >= 0 && i < 3 ? (int64_t)rate[i] : -1; }
/* --pow's difficulty, or -1 without --pow. */
int64_t ry_host_pow(void) { return (int64_t)pow_bits; }
/* --admin's values: how many, and each one's bytes (relay.coax checks them). */
int64_t ry_host_admin_count(void) { return admin_n; }
int64_t ry_host_groups(void) { return groups; }
int64_t ry_host_admin_len(int64_t k) { return k >= 0 && k < admin_n ? (int64_t)strlen(admins[k]) : -1; }
int64_t ry_host_admin_byte(int64_t k, int64_t i) { return k >= 0 && k < admin_n && i >= 0 && i < (int64_t)strlen(admins[k]) ? (unsigned char)admins[k][i] : -1; }
int64_t ry_host_random(void) {
  unsigned char b;
  return getentropy(&b, 1) == 0 ? b : -1;
}

static void *run(void *arg) {
  (void)arg;
  ry_main(arena, (coax_v_t_RsdRec){recs, events}, by_id, by_ak, by_time, tag_key, tag_rec, count_out, count_seen);
  return NULL;
}
/* n zeroed int64 slots, or a view with no data. */
static coax_v_s64 ints(int64_t n) { return (coax_v_s64){calloc((size_t)n, sizeof(int64_t)), n}; }

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
    else if (!strcmp(argv[i], "--rate") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld,%lld,%lld%c", &rate[0], &rate[1], &rate[2], &tail) != 3) { fprintf(stderr, "relay: --rate takes EVENTS,REQS,KEY_EVENTS\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--pow") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &pow_bits, &tail) != 1) { fprintf(stderr, "relay: --pow takes a number of bits\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--groups")) groups = 1;
    else if (!strcmp(argv[i], "--admin") && i + 1 < argc) {
      if (admin_n == 8) { fprintf(stderr, "relay: too many --admin keys\n"); return 2; }
      admins[admin_n++] = argv[++i];
    }
    else if (!strcmp(argv[i], "--store-capacity") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &capacity, &tail) != 1) { fprintf(stderr, "relay: --store-capacity takes a number of events\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--store") && i + 1 < argc) {
      if (cxstore_set_path(argv[++i]) != 0) { fprintf(stderr, "relay: bad --store path\n"); return 2; }
      store = 1;
    }
    else { fprintf(stderr, "usage: relay [--listen ADDRESS] [--port N] [--url ws://HOST[:PORT][/PATH]] [--store FILE] [--store-capacity EVENTS] [--tls-cert CHAIN.pem --tls-key KEY.pem] [--rate EVENTS,REQS,KEY_EVENTS] [--pow BITS] [--admin PUBKEY]... [--groups]\n"); return 2; }
  }
  if ((cert == NULL) != (key == NULL)) { fprintf(stderr, "relay: --tls-cert and --tls-key go together\n"); return 2; }
  if (cert != NULL) {
    int64_t e = cxtls_server_load(cert, key);
    if (e != 0) { fprintf(stderr, "relay: %s\n", tls_why(e)); return 2; }
    cxsrv_set_tls(1);
    tls = 1;
  }
  setvbuf(stdout, NULL, _IOLBF, 0);
  events = ry_store_events(capacity);
  if (events < 0) return 2;
  int64_t bytes = ry_store_arena(events), tags = ry_store_tags(events);
  arena = (coax_v_u8){calloc((size_t)bytes, 1), bytes};
  recs = calloc((size_t)events, sizeof(coax_t_RsdRec));
  by_id = ints(events); by_ak = ints(events); by_time = ints(events);
  tag_key = ints(tags); tag_rec = ints(tags); count_out = ints(events);
  count_seen = (coax_v_u8){calloc((size_t)events, 1), events};
  if (!arena.data || !recs || !by_id.data || !by_ak.data || !by_time.data || !tag_key.data || !tag_rec.data || !count_out.data || !count_seen.data) {
    fprintf(stderr, "relay: cannot allocate a store of %lld events\n", (long long)events); return 1;
  }
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)512 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) { fprintf(stderr, "relay: no thread\n"); return 1; }
  pthread_join(t, NULL);
  return 0;
}
