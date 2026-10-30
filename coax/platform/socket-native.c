/* TCP client sockets for Coax; see socket-native.h for the contract. */
#define _POSIX_C_SOURCE 200809L
#define _DARWIN_C_SOURCE 1
#include "socket-native.h"
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>
#include <netinet/in.h>
#include <netinet/tcp.h>

typedef struct {
  int fd;                 /* -1 when the slot is free */
  int64_t in_count;
  unsigned char in[CXSOCK_MAX_IN];
  int64_t out_count;
  unsigned char *out;     /* CXSOCK_MAX_OUT bytes, allocated on connect */
} cxsock_slot;

static cxsock_slot cxsock_slots[CXSOCK_MAX];
static int cxsock_ready = 0;
static char cxsock_host[CXSOCK_MAX_HOST + 1];
static int cxsock_host_len = 0;
static int cxsock_host_bad = 0;

static void cxsock_init(void) {
  if (cxsock_ready) return;
  for (int i = 0; i < CXSOCK_MAX; i++) { cxsock_slots[i].fd = -1; cxsock_slots[i].out = NULL; }
  signal(SIGPIPE, SIG_IGN);
  cxsock_ready = 1;
}

static cxsock_slot *cxsock_get(int64_t h) {
  cxsock_init();
  if (h < 0 || h >= CXSOCK_MAX || cxsock_slots[h].fd < 0) return NULL;
  return &cxsock_slots[h];
}

int64_t cxsock_now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}

int64_t cxsock_host_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || cxsock_host_len >= CXSOCK_MAX_HOST) { cxsock_host_bad = 1; return CXSOCK_FULL; }
  cxsock_host[cxsock_host_len++] = (char)byte;
  return 0;
}

static int cxsock_wait(int fd, short events, int64_t deadline) {
  for (;;) {
    int64_t left = deadline < 0 ? -1 : deadline - cxsock_now_ms();
    if (deadline >= 0 && left < 0) left = 0;
    struct pollfd p = { fd, events, 0 };
    int r = poll(&p, 1, left > 2147483647 ? 2147483647 : (int)left);
    if (r < 0 && errno == EINTR) continue;
    if (r < 0) return CXSOCK_ERROR;
    if (r == 0) return CXSOCK_TIMEOUT;
    return 0;
  }
}

int64_t cxsock_connect(int64_t port, int64_t timeout_ms) {
  cxsock_init();
  int bad = cxsock_host_bad || cxsock_host_len == 0 || port < 1 || port > 65535 || timeout_ms < 0;
  cxsock_host[cxsock_host_len] = 0;
  char host[CXSOCK_MAX_HOST + 1];
  memcpy(host, cxsock_host, sizeof host);
  cxsock_host_len = 0; cxsock_host_bad = 0;
  if (bad) return CXSOCK_ERROR;
  int slot = -1;
  for (int i = 0; i < CXSOCK_MAX; i++) if (cxsock_slots[i].fd < 0) { slot = i; break; }
  if (slot < 0) return CXSOCK_FULL;
  char service[8];
  snprintf(service, sizeof service, "%d", (int)port);
  struct addrinfo hints, *list = NULL;
  memset(&hints, 0, sizeof hints);
  hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM; hints.ai_flags = AI_NUMERICSERV;
  if (getaddrinfo(host, service, &hints, &list) != 0 || list == NULL) return CXSOCK_RESOLVE;
  int64_t deadline = cxsock_now_ms() + timeout_ms;
  int64_t result = CXSOCK_REFUSED;
  int fd = -1;
  for (struct addrinfo *a = list; a != NULL; a = a->ai_next) {
    fd = socket(a->ai_family, a->ai_socktype, a->ai_protocol);
    if (fd < 0) { result = CXSOCK_ERROR; continue; }
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) { close(fd); fd = -1; result = CXSOCK_ERROR; continue; }
#ifdef SO_NOSIGPIPE
    int one = 1; setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one);
#endif
    int nodelay = 1; setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &nodelay, sizeof nodelay);
    if (connect(fd, a->ai_addr, a->ai_addrlen) == 0) break;
    if (errno != EINPROGRESS) { close(fd); fd = -1; result = CXSOCK_REFUSED; continue; }
    int w = cxsock_wait(fd, POLLOUT, deadline);
    if (w != 0) { close(fd); fd = -1; result = w; if (w == CXSOCK_TIMEOUT) break; continue; }
    int err = 0; socklen_t len = sizeof err;
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) == 0 && err == 0) break;
    close(fd); fd = -1; result = CXSOCK_REFUSED;
  }
  freeaddrinfo(list);
  if (fd < 0) return result;
  cxsock_slot *s = &cxsock_slots[slot];
  if (s->out == NULL) s->out = (unsigned char *)malloc(CXSOCK_MAX_OUT);
  if (s->out == NULL) { close(fd); return CXSOCK_ERROR; }
  s->fd = fd; s->in_count = 0; s->out_count = 0;
  return slot;
}

int64_t cxsock_write(int64_t h, int64_t byte) {
  cxsock_slot *s = cxsock_get(h);
  if (s == NULL || byte < 0 || byte > 255) return CXSOCK_ERROR;
  if (s->out_count >= CXSOCK_MAX_OUT) return CXSOCK_FULL;
  s->out[s->out_count++] = (unsigned char)byte;
  return 0;
}

int64_t cxsock_flush(int64_t h, int64_t timeout_ms) {
  cxsock_slot *s = cxsock_get(h);
  if (s == NULL) return CXSOCK_ERROR;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  int64_t at = 0;
  while (at < s->out_count) {
#ifdef MSG_NOSIGNAL
    ssize_t n = send(s->fd, s->out + at, (size_t)(s->out_count - at), MSG_NOSIGNAL);
#else
    ssize_t n = send(s->fd, s->out + at, (size_t)(s->out_count - at), 0);
#endif
    if (n > 0) { at += n; continue; }
    if (n < 0 && errno == EINTR) continue;
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      int w = cxsock_wait(s->fd, POLLOUT, deadline);
      if (w != 0) { s->out_count = 0; return w; }
      continue;
    }
    s->out_count = 0;
    return CXSOCK_ERROR;
  }
  s->out_count = 0;
  return 0;
}

int64_t cxsock_read(int64_t h, int64_t timeout_ms) {
  cxsock_slot *s = cxsock_get(h);
  if (s == NULL) return CXSOCK_ERROR;
  s->in_count = 0;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  for (;;) {
    ssize_t n = recv(s->fd, s->in, CXSOCK_MAX_IN, 0);
    if (n > 0) { s->in_count = n; return n; }
    if (n == 0) return CXSOCK_EOF;
    if (errno == EINTR) continue;
    if (errno != EAGAIN && errno != EWOULDBLOCK) return CXSOCK_ERROR;
    int w = cxsock_wait(s->fd, POLLIN, deadline);
    if (w != 0) return w;
  }
}

int64_t cxsock_byte(int64_t h, int64_t i) {
  cxsock_slot *s = cxsock_get(h);
  if (s == NULL || i < 0 || i >= s->in_count) return CXSOCK_ERROR;
  return s->in[i];
}

int64_t cxsock_close(int64_t h) {
  cxsock_slot *s = cxsock_get(h);
  if (s == NULL) return CXSOCK_ERROR;
  close(s->fd);
  s->fd = -1; s->in_count = 0; s->out_count = 0;
  return 0;
}
