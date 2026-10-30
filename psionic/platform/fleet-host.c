/* Fleet's platform effects only. Coax owns HTTP, routing, discovery and replies.
   Socket handles belong to this single Coax worker; they are not OS descriptors.
   Listen is IPv4 loopback only. Read exposes the last <=64KiB recv until the
   next read/owning close. Each stream stages <=1MiB; flush retains its unsent
   offset after timeout, and close discards pending bytes. Return -2 for a
   deadline, -1 for read EOF, -3 for an invalid handle/argument or syscall error.
   Build Coax C with -Dmain=coax_fleet_main; no generated header is required. */
#if defined(__APPLE__)
#define _DARWIN_C_SOURCE
#endif
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define FH_MAX_SOCKETS 32
#define FH_MAX_IN ((size_t)65536)
#define FH_MAX_OUT ((size_t)1048576)
#define FH_LISTENER 1
#define FH_STREAM 2
int coax_fleet_main(void);
typedef struct {
  int fd, kind;
  unsigned char *out;
  size_t used, sent;
} fh_slot;
static fh_slot fh_slots[FH_MAX_SOCKETS];
static int fh_ready, fh_argc_value, fh_exit_status;
static char **fh_argv;
static volatile sig_atomic_t fh_stopping;
static unsigned char fh_input[FH_MAX_IN];
static size_t fh_input_used;
static int64_t fh_input_owner = -1;
static void fh_init(void) {
  if (fh_ready) return;
  for (int i = 0; i < FH_MAX_SOCKETS; i++) fh_slots[i].fd = -1;
  signal(SIGPIPE, SIG_IGN); fh_ready = 1;
}
static fh_slot *fh_get(int64_t handle, int kind) {
  fh_init();
  if (handle < 0 || handle >= FH_MAX_SOCKETS || fh_slots[handle].fd < 0 ||
      (kind && fh_slots[handle].kind != kind)) return NULL;
  return &fh_slots[handle];
}
static int fh_free_slot(void) {
  fh_init();
  for (int i = 0; i < FH_MAX_SOCKETS; i++) if (fh_slots[i].fd < 0) return i;
  return -1;
}
static int64_t fh_mono_ms(void) {
  struct timespec t;
  if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) return -3;
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static int64_t fh_deadline(int64_t timeout_ms) {
  if (timeout_ms < 0 || timeout_ms > INT_MAX) return -3;
  int64_t now = fh_mono_ms(); return now < 0 ? -3 : now + timeout_ms;
}
static int fh_wait(int fd, short events, int64_t deadline) {
  for (;;) {
    if (fh_stopping) return -3;
    int64_t now = fh_mono_ms(); if (now < 0) return -3;
    int64_t left = deadline > now ? deadline - now : 0;
    struct pollfd p = { fd, events, 0 };
    /* Signals land on the main thread; bounded slices let this worker stop. */
    int n = poll(&p, 1, left > 100 ? 100 : (int)left);
    if (n < 0 && errno == EINTR) continue;
    if (n < 0 || (p.revents & POLLNVAL)) return -3;
    if (!n) {
      if (fh_stopping) return -3;
      if (fh_mono_ms() >= deadline) return -2;
      continue;
    }
    return 0;
  }
}
static int fh_prepare(int fd) {
  int flags = fcntl(fd, F_GETFL, 0);
  if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) != 0 ||
      fcntl(fd, F_SETFD, FD_CLOEXEC) != 0) return -3;
#ifdef SO_NOSIGPIPE
  int one = 1;
  if (setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one) != 0) return -3;
#endif
  return 0;
}
int64_t fh_argc(void) { return fh_argc_value; }
int64_t fh_arg_len(int64_t i) { return i >= 0 && i < fh_argc_value ? (int64_t)strlen(fh_argv[i]) : -1; }
int64_t fh_arg_byte(int64_t i, int64_t k) {
  if (i < 0 || i >= fh_argc_value || k < 0 || (size_t)k >= strlen(fh_argv[i])) return -1;
  return (unsigned char)fh_argv[i][k];
}
int64_t fh_now_s(void) { return (int64_t)time(NULL); }
int64_t fh_stop(void) { return fh_stopping ? 1 : 0; }
int64_t fh_status(int64_t code) { fh_exit_status = (int)(code & 255); return 0; }
int64_t fh_listen(int64_t port) {
  if (port < 0 || port > 65535) return -3;
  int slot = fh_free_slot(); if (slot < 0) return -3;
  int fd = socket(AF_INET, SOCK_STREAM, 0); if (fd < 0) return -3;
  int one = 1;
  struct sockaddr_in address; memset(&address, 0, sizeof address);
  address.sin_family = AF_INET; address.sin_port = htons((uint16_t)port);
  address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  if (fh_prepare(fd) != 0 || setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one) != 0 ||
      bind(fd, (struct sockaddr *)&address, sizeof address) != 0 || listen(fd, 16) != 0) {
    close(fd); return -3;
  }
  fh_slots[slot].fd = fd; fh_slots[slot].kind = FH_LISTENER; return slot;
}
int64_t fh_accept(int64_t listener, int64_t timeout_ms) {
  fh_slot *s = fh_get(listener, FH_LISTENER); int64_t deadline = fh_deadline(timeout_ms);
  int slot = fh_free_slot(); if (!s || slot < 0 || deadline < 0) return -3;
  for (;;) {
    int ready = fh_wait(s->fd, POLLIN, deadline); if (ready < 0) return ready;
    int fd = accept(s->fd, NULL, NULL);
    if (fd < 0 && (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK)) continue;
    if (fd < 0) return -3;
    if (fh_prepare(fd) != 0) { close(fd); return -3; }
    fh_slots[slot].fd = fd; fh_slots[slot].kind = FH_STREAM; return slot;
  }
}
int64_t fh_read(int64_t handle, int64_t timeout_ms) {
  fh_input_used = 0; fh_input_owner = -1;
  fh_slot *s = fh_get(handle, FH_STREAM); int64_t deadline = fh_deadline(timeout_ms);
  if (!s || deadline < 0) return -3;
  for (;;) {
    int ready = fh_wait(s->fd, POLLIN, deadline); if (ready < 0) return ready;
    ssize_t n = recv(s->fd, fh_input, sizeof fh_input, 0);
    if (n < 0 && (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK)) continue;
    if (n < 0) return -3;
    if (!n) return -1;
    fh_input_used = (size_t)n; fh_input_owner = handle; return n;
  }
}
int64_t fh_byte(int64_t i) { return i >= 0 && (size_t)i < fh_input_used ? fh_input[i] : -1; }
int64_t fh_write(int64_t handle, int64_t byte) {
  fh_slot *s = fh_get(handle, FH_STREAM);
  if (!s || byte < 0 || byte > 255 || s->sent || s->used >= FH_MAX_OUT) return -3;
  if (!s->out) { s->out = malloc(FH_MAX_OUT); if (!s->out) return -3; }
  s->out[s->used++] = (unsigned char)byte; return 0;
}
int64_t fh_flush(int64_t handle, int64_t timeout_ms) {
  fh_slot *s = fh_get(handle, FH_STREAM); int64_t deadline = fh_deadline(timeout_ms);
  if (!s || deadline < 0) return -3;
  while (s->sent < s->used) {
    int ready = fh_wait(s->fd, POLLOUT, deadline); if (ready < 0) return ready;
    int flags = 0;
#ifdef MSG_NOSIGNAL
    flags = MSG_NOSIGNAL;
#endif
    ssize_t n = send(s->fd, s->out + s->sent, s->used - s->sent, flags);
    if (n < 0 && (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK)) continue;
    if (n <= 0) return -3;
    s->sent += (size_t)n;
  }
  s->used = 0; s->sent = 0; return 0;
}
int64_t fh_close(int64_t handle) {
  fh_slot *s = fh_get(handle, 0); if (!s) return -3;
  int result = close(s->fd); free(s->out); memset(s, 0, sizeof *s); s->fd = -1;
  if (fh_input_owner == handle) { fh_input_owner = -1; fh_input_used = 0; }
  return result == 0 ? 0 : -3;
}
static void fh_signal(int sig) { (void)sig; fh_stopping = 1; }
static void *fh_run(void *arg) {
  (void)arg; sigset_t signals; sigemptyset(&signals);
  sigaddset(&signals, SIGTERM); sigaddset(&signals, SIGINT);
  pthread_sigmask(SIG_BLOCK, &signals, NULL); coax_fleet_main(); return NULL;
}
int main(int argc, char **argv) {
  fh_argc_value = argc; fh_argv = argv; fh_stopping = 0; fh_exit_status = 0; fh_init();
  setvbuf(stdout, NULL, _IOLBF, 0);
  struct sigaction action; memset(&action, 0, sizeof action);
  action.sa_handler = fh_signal; action.sa_flags = SA_RESTART; sigemptyset(&action.sa_mask);
  if (sigaction(SIGINT, &action, NULL) != 0 || sigaction(SIGTERM, &action, NULL) != 0) return 1;
  pthread_attr_t attributes;
  if (pthread_attr_init(&attributes) != 0) return 1;
  if (pthread_attr_setstacksize(&attributes, (size_t)512 << 20) != 0) { pthread_attr_destroy(&attributes); return 1; }
  pthread_t thread; int result = pthread_create(&thread, &attributes, fh_run, NULL);
  pthread_attr_destroy(&attributes);
  if (result != 0) { fputs("fleet: no worker thread\n", stderr); return 1; }
  if (pthread_join(thread, NULL) != 0) return 1;
  for (int i = 0; i < FH_MAX_SOCKETS; i++) if (fh_slots[i].fd >= 0) fh_close(i);
  return fh_exit_status;
}
