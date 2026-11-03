/* The Coax relay's C host: flags, then the Coax relay (relay.coax's
   #export_c ry_main) on a thread with a 512 MiB stack, since its
   connection and frame buffers are Coax locals. The store's arrays are
   allocated here, zeroed, at the size relay.coax gives for
   --store-capacity (ry_store_events, ry_store_arena, ry_store_tags), and
   lent to ry_main for the life of the process.
     relay --listen 127.0.0.1 --port 7447 [--url ws://name:7447] [--store FILE]
           [--store-capacity EVENTS] [--tls-cert CHAIN.pem --tls-key KEY.pem]
           [--rate EVENTS,REQS,KEY_EVENTS] [--pow BITS] [--admin PUBKEY]... [--groups]
           [--checkpoint FILE [--adopt]] [--import EXPORT --now T | --check-store [--now T]]
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
   NIP-29 groups. With --store FILE the NIP-86 policy's changes go to
   FILE.nip86 (ry_journal_*: the same effects as store-file-native.c's,
   for a second file, created only at the first change); relay.coax frames
   and replays it. --checkpoint FILE keeps the freshness checkpoint
   (store_fresh.coax) in FILE: read on start, replaced after every commit
   (written to FILE.tmp, synced, renamed, the folder synced); --adopt lets
   a start without one take the files as they are. --import EXPORT (with
   --store, --url and --now) writes a new --store file from a JSON-lines
   export and exits; --check-store replays the store (and checks the
   checkpoint) and exits without listening; --now T is the clock for both.
   --export OUT [--from BYTE] replays the store, then writes its frames
   from BYTE on (the start of a frame) as JSON lines to OUT, which must
   not exist (OUT.tmp written, synced, renamed), and exits.
   The tombstones (NIP-09 and NIP-62) are allocated at what the input (the
   export, or the --store file) can ask for plus --tombstones N more for
   later requests (8,192 without it): relay.coax's ry_store_tombs counts
   them and says how much memory they take.
   Effects only; every relay decision is Coax. */
#define _DEFAULT_SOURCE /* getentropy */
#include "socket-server-native.h"
#include "store-file-native.h"
#include "tls-native.h"
#include "relay.h"
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

static long long capacity = -1;
static coax_v_u8 arena, count_seen;
static coax_t_RsdRec *recs; static int64_t events;
static coax_v_s64 by_id, by_ak, by_time, tag_key, tag_rec, count_out, ids, addrs, tomb_slots;
static coax_v_t_NsdTomb tombs;
static long long tombs_extra = -1; /* --tombstones */
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
static int mode = 0;               /* 0 serve, 1 --import, 2 --check-store, 3 --export */
static long long now_flag = -1;    /* --now */
static int adopt = 0;
static int exit_code = 0;
static const char *import_path = NULL;
static char fresh_path[4200];
static coax_v_t_NegItem items;

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
/* ---- FILE.nip86, the policy journal (relay.coax's ry_journal_*) ---- */
static char journal_path[4200];
static int journal_fd = -1, journal_broken = 0;
static unsigned char *journal_loaded;
static int64_t journal_loaded_n;
#define RY_JOURNAL_PENDING (4 + 8192)
static unsigned char journal_pending[RY_JOURNAL_PENDING];
static int64_t journal_pending_n;

/* fsyncs the directory that holds the journal: 0, or -1. */
static int journal_sync_parent(void) {
  char dir[sizeof journal_path];
  memcpy(dir, journal_path, sizeof dir);
  char *slash = strrchr(dir, '/');
  if (!slash) { dir[0] = '.'; dir[1] = 0; }
  else if (slash == dir) dir[1] = 0;
  else *slash = 0;
  int d = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (d < 0) return -1;
  int r = fsync(d);
  close(d);
  return r;
}
/* Opens and exclusively locks the journal, creating it (0600, directory
   synced) only when create is set: 0, 1 when it doesn't exist and create
   is not set, or -1. */
static int journal_open(int create) {
  if (journal_fd >= 0) return 0;
  if (!journal_path[0]) return -1;
  int made = 0;
  journal_fd = open(journal_path, O_RDWR | O_CLOEXEC);
  if (journal_fd < 0 && errno == ENOENT) {
    if (!create) return 1;
    journal_fd = open(journal_path, O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    made = 1;
  }
  if (journal_fd < 0) return -1;
  if (flock(journal_fd, LOCK_EX | LOCK_NB) != 0 || (made && journal_sync_parent() != 0)) { close(journal_fd); journal_fd = -1; return -1; }
  return 0;
}
/* Reads the journal whole: its size (0 when it doesn't exist), or -1. */
int64_t ry_journal_load(void) {
  int o = journal_open(0);
  free(journal_loaded); journal_loaded = NULL; journal_loaded_n = 0;
  if (o == 1) return 0;
  if (o != 0) return -1;
  struct stat st;
  if (fstat(journal_fd, &st) != 0 || st.st_size < 0) return -1;
  if (st.st_size > 0) {
    journal_loaded = malloc((size_t)st.st_size);
    if (!journal_loaded) return -1;
    int64_t got = 0;
    while (got < st.st_size) {
      ssize_t r = pread(journal_fd, journal_loaded + got, (size_t)(st.st_size - got), got);
      if (r < 0 && errno == EINTR) continue;
      if (r <= 0) { free(journal_loaded); journal_loaded = NULL; return -1; }
      got += r;
    }
  }
  journal_loaded_n = st.st_size;
  return journal_loaded_n;
}
/* Byte i of what ry_journal_load read, or -1. */
int64_t ry_journal_byte(int64_t i) { return i >= 0 && i < journal_loaded_n ? journal_loaded[i] : -1; }
/* Buffers one byte for the next commit: 0, or -1. */
int64_t ry_journal_put(int64_t b) {
  if (b < 0 || b > 255 || journal_pending_n >= RY_JOURNAL_PENDING) return -1;
  journal_pending[journal_pending_n++] = (unsigned char)b;
  return 0;
}
/* Appends the buffered bytes and fsyncs: 0, or -1 after cutting the
   append back (if that fails, every later commit fails). */
int64_t ry_journal_commit(void) {
  int64_t n = journal_pending_n;
  journal_pending_n = 0;
  if (journal_broken || journal_open(1) != 0) return -1;
  off_t end = lseek(journal_fd, 0, SEEK_END);
  if (end < 0) return -1;
  int64_t done = 0;
  while (done < n) {
    ssize_t w = pwrite(journal_fd, journal_pending + done, (size_t)(n - done), end + done);
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) break;
    done += w;
  }
  if (done == n && fsync(journal_fd) == 0) return 0;
  if (ftruncate(journal_fd, end) != 0 || fsync(journal_fd) != 0) journal_broken = 1;
  return -1;
}
/* Cuts the journal to n bytes and fsyncs: 0, or -1. */
int64_t ry_journal_truncate(int64_t n) {
  if (journal_fd < 0 || n < 0) return -1;
  if (ftruncate(journal_fd, (off_t)n) != 0 || fsync(journal_fd) != 0) return -1;
  if (n < journal_loaded_n) journal_loaded_n = n;
  return 0;
}

/* ---- modes, the clock and the process's measurements ---- */
int64_t ry_host_mode(void) { return mode; }
int64_t ry_host_tombs_extra(void) { return (int64_t)tombs_extra; }
int64_t ry_host_now_flag(void) { return (int64_t)now_flag; }
int64_t ry_host_adopt(void) { return adopt; }
void ry_host_exit(int64_t code) { exit_code = (int)code; }
/* Monotonic nanoseconds. */
int64_t ry_host_ns(void) {
  struct timespec t;
  if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) return 0;
  return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}
/* The process's peak resident memory so far, in KiB (getrusage). */
int64_t ry_host_peak_kib(void) {
  struct rusage u;
  if (getrusage(RUSAGE_SELF, &u) != 0) return -1;
#if defined(__APPLE__)
  return (int64_t)u.ru_maxrss / 1024;
#else
  return (int64_t)u.ru_maxrss;
#endif
}
/* ---- --import EXPORT: read whole ---- */
static unsigned char *import_data;
static int64_t import_n;
/* Reads the export: its size, or -1. */
int64_t ry_import_load(void) {
  if (!import_path) return -1;
  int f = open(import_path, O_RDONLY | O_CLOEXEC);
  if (f < 0) return -1;
  struct stat st;
  if (fstat(f, &st) != 0 || st.st_size < 0) { close(f); return -1; }
  free(import_data); import_n = 0;
  import_data = malloc(st.st_size > 0 ? (size_t)st.st_size : 1);
  if (!import_data) { close(f); return -1; }
  int64_t got = 0;
  while (got < st.st_size) {
    ssize_t r = pread(f, import_data + got, (size_t)(st.st_size - got), got);
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) { close(f); return -1; }
    got += r;
  }
  close(f);
  import_n = got;
  return got;
}
int64_t ry_import_byte(int64_t i) { return i >= 0 && i < import_n ? import_data[i] : -1; }
/* ---- --export OUT: written to OUT.tmp, synced, renamed over OUT ---- */
static const char *export_path = NULL;
static long long export_from = 0;   /* --from */
static char export_tmp[4200];
static FILE *export_file;
int64_t ry_host_export_from(void) { return (int64_t)export_from; }
/* Opens OUT.tmp for writing; OUT must not exist: 0, or -1. */
int64_t ry_export_open(void) {
  struct stat st;
  if (!export_path || stat(export_path, &st) == 0 || errno != ENOENT) return -1;
  snprintf(export_tmp, sizeof export_tmp, "%s.tmp", export_path);
  int f = open(export_tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
  if (f < 0) return -1;
  export_file = fdopen(f, "w");
  if (!export_file) { close(f); unlink(export_tmp); return -1; }
  setvbuf(export_file, NULL, _IOFBF, 1 << 20);
  return 0;
}
int64_t ry_export_put(int64_t b) {
  if (!export_file || b < 0 || b > 255) return -1;
  return fputc((int)b, export_file) == EOF ? -1 : 0;
}
/* Flushes, syncs and renames OUT.tmp to OUT, then syncs the folder: 0, or -1 (and OUT.tmp removed). */
int64_t ry_export_commit(void) {
  if (!export_file) return -1;
  int ok = fflush(export_file) == 0 && fsync(fileno(export_file)) == 0;
  ok = fclose(export_file) == 0 && ok;
  export_file = NULL;
  if (!ok || rename(export_tmp, export_path) != 0) { unlink(export_tmp); return -1; }
  char dir[sizeof export_tmp];
  snprintf(dir, sizeof dir, "%s", export_path);
  char *slash = strrchr(dir, '/');
  if (!slash) { dir[0] = '.'; dir[1] = 0; } else if (slash == dir) dir[1] = 0; else *slash = 0;
  int d = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (d < 0) return -1;
  int r = fsync(d);
  close(d);
  return r == 0 ? 0 : -1;
}
/* Drops OUT.tmp. */
void ry_export_abort(void) {
  if (export_file) { fclose(export_file); export_file = NULL; }
  unlink(export_tmp);
}
/* ---- --checkpoint FILE ---- */
int64_t ry_host_fresh(void) { return fresh_path[0] != 0; }
#define RY_FRESH_MAX 4096
static unsigned char fresh_in[RY_FRESH_MAX], fresh_out[RY_FRESH_MAX];
static int64_t fresh_in_n, fresh_out_n;
/* Reads the checkpoint: its size (at most RY_FRESH_MAX bytes are kept; a
   longer one reads as that size), -1 when it doesn't exist, -2 when it
   can't be read. */
int64_t ry_fresh_load(void) {
  fresh_in_n = 0;
  int f = open(fresh_path, O_RDONLY | O_CLOEXEC);
  if (f < 0) return errno == ENOENT ? -1 : -2;
  struct stat st;
  if (fstat(f, &st) != 0 || !S_ISREG(st.st_mode)) { close(f); return -2; }
  int64_t got = 0;
  while (got < RY_FRESH_MAX) {
    ssize_t r = read(f, fresh_in + got, (size_t)(RY_FRESH_MAX - got));
    if (r < 0 && errno == EINTR) continue;
    if (r < 0) { close(f); return -2; }
    if (r == 0) break;
    got += r;
  }
  close(f);
  fresh_in_n = got;
  return st.st_size > got ? (int64_t)st.st_size : got;
}
int64_t ry_fresh_byte(int64_t i) { return i >= 0 && i < fresh_in_n ? fresh_in[i] : -1; }
int64_t ry_fresh_put(int64_t b) {
  if (b < 0 || b > 255 || fresh_out_n >= RY_FRESH_MAX) return -1;
  fresh_out[fresh_out_n++] = (unsigned char)b;
  return 0;
}
/* Replaces the checkpoint with the buffered bytes: FILE.tmp written and
   synced, renamed over FILE, the folder synced. 0, or -1 (FILE unchanged). */
int64_t ry_fresh_commit(void) {
  int64_t n = fresh_out_n;
  fresh_out_n = 0;
  char tmp[sizeof fresh_path + 8];
  snprintf(tmp, sizeof tmp, "%s.tmp", fresh_path);
  int f = open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
  if (f < 0) return -1;
  int64_t done = 0;
  while (done < n) {
    ssize_t w = write(f, fresh_out + done, (size_t)(n - done));
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) { close(f); unlink(tmp); return -1; }
    done += w;
  }
  if (fsync(f) != 0) { close(f); unlink(tmp); return -1; }
  if (close(f) != 0 || rename(tmp, fresh_path) != 0) { unlink(tmp); return -1; }
  char dir[sizeof fresh_path];
  memcpy(dir, fresh_path, sizeof dir);
  char *slash = strrchr(dir, '/');
  if (!slash) { dir[0] = '.'; dir[1] = 0; } else if (slash == dir) dir[1] = 0; else *slash = 0;
  int d = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (d < 0) return -1;
  int r = fsync(d);
  close(d);
  return r == 0 ? 0 : -1;
}

int64_t ry_host_random(void) {
  unsigned char b;
  return getentropy(&b, 1) == 0 ? b : -1;
}

static void *run(void *arg) {
  (void)arg;
  ry_main(arena, (coax_v_t_RsdRec){recs, events}, by_id, by_ak, by_time, tag_key, tag_rec, count_out, count_seen, items, ids, addrs, tombs, tomb_slots);
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
    else if (!strcmp(argv[i], "--adopt")) adopt = 1;
    else if (!strcmp(argv[i], "--check-store")) mode = mode ? -1 : 2;
    else if (!strcmp(argv[i], "--import") && i + 1 < argc) { import_path = argv[++i]; mode = mode ? -1 : 1; }
    else if (!strcmp(argv[i], "--export") && i + 1 < argc) {
      export_path = argv[++i]; mode = mode ? -1 : 3;
      if (!export_path[0] || strlen(export_path) + 5 > sizeof export_tmp) { fprintf(stderr, "relay: bad --export path\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--from") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &export_from, &tail) != 1 || export_from < 0) { fprintf(stderr, "relay: --from takes a byte offset\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--checkpoint") && i + 1 < argc) {
      const char *cp = argv[++i];
      if (!cp[0] || strlen(cp) + 5 > sizeof fresh_path) { fprintf(stderr, "relay: bad --checkpoint path\n"); return 2; }
      snprintf(fresh_path, sizeof fresh_path, "%s", cp);
    }
    else if (!strcmp(argv[i], "--now") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &now_flag, &tail) != 1 || now_flag < 0) { fprintf(stderr, "relay: --now takes a time in seconds\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--admin") && i + 1 < argc) {
      if (admin_n == 8) { fprintf(stderr, "relay: too many --admin keys\n"); return 2; }
      admins[admin_n++] = argv[++i];
    }
    else if (!strcmp(argv[i], "--tombstones") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &tombs_extra, &tail) != 1 || tombs_extra < 0) { fprintf(stderr, "relay: --tombstones takes a number of tombstones\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--store-capacity") && i + 1 < argc) {
      char tail = 0;
      if (sscanf(argv[++i], "%lld%c", &capacity, &tail) != 1) { fprintf(stderr, "relay: --store-capacity takes a number of events\n"); return 2; }
    }
    else if (!strcmp(argv[i], "--store") && i + 1 < argc) {
      const char *sp = argv[++i];
      if (cxstore_set_path(sp) != 0 || strlen(sp) + 7 > sizeof journal_path) { fprintf(stderr, "relay: bad --store path\n"); return 2; }
      snprintf(journal_path, sizeof journal_path, "%s.nip86", sp);
      store = 1;
    }
    else { fprintf(stderr, "usage: relay [--listen ADDRESS] [--port N] [--url ws://HOST[:PORT][/PATH]] [--store FILE] [--store-capacity EVENTS] [--tls-cert CHAIN.pem --tls-key KEY.pem] [--rate EVENTS,REQS,KEY_EVENTS] [--pow BITS] [--admin PUBKEY]... [--groups] [--checkpoint FILE [--adopt]] [--tombstones N] [--import EXPORT --now T | --check-store [--now T] | --export OUT [--from BYTE] [--now T]]\n"); return 2; }
  }
  if (mode < 0) { fprintf(stderr, "relay: --import, --check-store and --export don't go together\n"); return 2; }
  if (mode != 0 && !store) { fprintf(stderr, "relay: --import, --check-store and --export need --store\n"); return 2; }
  if (export_from != 0 && mode != 3) { fprintf(stderr, "relay: --from goes with --export\n"); return 2; }
  if (mode == 1 && now_flag < 0) { fprintf(stderr, "relay: --import needs --now (the export's time)\n"); return 2; }
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
  items = (coax_v_t_NegItem){calloc((size_t)events, sizeof(coax_t_NegItem)), events};
  int64_t slots = ry_store_slots(events);
  ids = ints(slots); addrs = ints(slots);
  int64_t ntombs = ry_store_tombs();
  if (ntombs < 0) return 2;
  tombs = (coax_v_t_NsdTomb){calloc((size_t)(ntombs ? ntombs : 1), sizeof(coax_t_NsdTomb)), ntombs};
  tomb_slots = ints(2 * ntombs + 1);
  if (!arena.data || !recs || !by_id.data || !by_ak.data || !by_time.data || !tag_key.data || !tag_rec.data || !count_out.data || !count_seen.data || !items.data
      || !ids.data || !addrs.data || !tombs.data || !tomb_slots.data) {
    fprintf(stderr, "relay: cannot allocate a store of %lld events\n", (long long)events); return 1;
  }
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)512 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) { fprintf(stderr, "relay: no thread\n"); return 1; }
  pthread_join(t, NULL);
  return exit_code;
}
