/* See socket-server-native.h. Effects only; no protocol or policy here. */
#define _POSIX_C_SOURCE 200809L
#include "socket-server-native.h"
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

typedef struct {
  int fd;
  unsigned char in[CXSRV_MAX_IN];
  int64_t in_n;
  unsigned char *out;
  int64_t out_n;
} cxsrv_conn;

static int listen_fd = -1;
static int bound_port = 0;
static char address[64] = "127.0.0.1";
static cxsrv_conn conns[CXSRV_MAX];
static int ready_init = 0;

static void init(void) {
  if (ready_init) return;
  ready_init = 1;
  signal(SIGPIPE, SIG_IGN);
  for (int i = 0; i < CXSRV_MAX; i++) conns[i].fd = -1;
}

static int nonblocking(int fd) {
  int flags = fcntl(fd, F_GETFL, 0);
  return flags < 0 ? -1 : fcntl(fd, F_SETFL, flags | O_NONBLOCK);
}

void cxsrv_set_address(const char *a) {
  if (a && strlen(a) < sizeof address) {
    strncpy(address, a, sizeof address - 1);
    address[sizeof address - 1] = 0;
  }
}

int64_t cxsrv_listen(int64_t port) {
  init();
  if (port < 0 || port > 65535 || listen_fd >= 0) return CXSRV_ERROR;
  struct sockaddr_storage ss;
  socklen_t len;
  memset(&ss, 0, sizeof ss);
  struct sockaddr_in *v4 = (struct sockaddr_in *)&ss;
  struct sockaddr_in6 *v6 = (struct sockaddr_in6 *)&ss;
  int family;
  if (inet_pton(AF_INET, address, &v4->sin_addr) == 1) {
    family = AF_INET; v4->sin_family = AF_INET; v4->sin_port = htons((uint16_t)port); len = sizeof *v4;
  } else if (inet_pton(AF_INET6, address, &v6->sin6_addr) == 1) {
    family = AF_INET6; v6->sin6_family = AF_INET6; v6->sin6_port = htons((uint16_t)port); len = sizeof *v6;
  } else {
    return CXSRV_ERROR;
  }
  int fd = socket(family, SOCK_STREAM, 0);
  if (fd < 0) return CXSRV_ERROR;
  int one = 1;
  setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  if (bind(fd, (struct sockaddr *)&ss, len) != 0 || listen(fd, 64) != 0 || nonblocking(fd) != 0) {
    close(fd);
    return CXSRV_ERROR;
  }
  struct sockaddr_storage got;
  socklen_t got_len = sizeof got;
  if (getsockname(fd, (struct sockaddr *)&got, &got_len) == 0) {
    bound_port = got.ss_family == AF_INET ? ntohs(((struct sockaddr_in *)&got)->sin_port)
                                          : ntohs(((struct sockaddr_in6 *)&got)->sin6_port);
  }
  listen_fd = fd;
  return 0;
}

int64_t cxsrv_bound_port(void) { return bound_port; }

int64_t cxsrv_poll(int64_t timeout_ms) {
  init();
  struct pollfd p[CXSRV_MAX + 1];
  int n = 0;
  if (listen_fd >= 0) { p[n].fd = listen_fd; p[n].events = POLLIN; p[n].revents = 0; n++; }
  for (int i = 0; i < CXSRV_MAX; i++) {
    if (conns[i].fd < 0) continue;
    p[n].fd = conns[i].fd; p[n].events = POLLIN; p[n].revents = 0; n++;
  }
  int r = poll(p, (nfds_t)n, timeout_ms < 0 ? -1 : (int)timeout_ms);
  return r < 0 ? (errno == EINTR ? 0 : CXSRV_ERROR) : r;
}

int64_t cxsrv_accept(void) {
  init();
  if (listen_fd < 0) return CXSRV_ERROR;
  int fd = accept(listen_fd, NULL, NULL);
  if (fd < 0) return CXSRV_NONE;
  for (int i = 0; i < CXSRV_MAX; i++) {
    if (conns[i].fd >= 0) continue;
    if (nonblocking(fd) != 0) break;
    int one = 1;
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
    conns[i].out = malloc(CXSRV_MAX_OUT);
    if (!conns[i].out) break;
    conns[i].fd = fd; conns[i].in_n = 0; conns[i].out_n = 0;
    return i + 1;
  }
  close(fd);
  return CXSRV_FULL;
}

static cxsrv_conn *conn(int64_t h) {
  if (h < 1 || h > CXSRV_MAX || conns[h - 1].fd < 0) return NULL;
  return &conns[h - 1];
}

int64_t cxsrv_read(int64_t h) {
  cxsrv_conn *c = conn(h);
  if (!c) return CXSRV_ERROR;
  ssize_t r = read(c->fd, c->in, sizeof c->in);
  if (r > 0) { c->in_n = r; return r; }
  c->in_n = 0;
  if (r == 0) return CXSRV_EOF;
  return (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR) ? 0 : CXSRV_ERROR;
}

int64_t cxsrv_byte(int64_t h, int64_t i) {
  cxsrv_conn *c = conn(h);
  if (!c || i < 0 || i >= c->in_n) return -1;
  return c->in[i];
}

int64_t cxsrv_write(int64_t h, int64_t byte) {
  cxsrv_conn *c = conn(h);
  if (!c || byte < 0 || byte > 255) return CXSRV_ERROR;
  if (c->out_n >= CXSRV_MAX_OUT) return CXSRV_FULL;
  c->out[c->out_n++] = (unsigned char)byte;
  return 0;
}

static int64_t now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}

int64_t cxsrv_flush(int64_t h, int64_t timeout_ms) {
  cxsrv_conn *c = conn(h);
  if (!c) return CXSRV_ERROR;
  int64_t sent = 0, deadline = now_ms() + timeout_ms;
  while (sent < c->out_n) {
    ssize_t w = write(c->fd, c->out + sent, (size_t)(c->out_n - sent));
    if (w > 0) { sent += w; continue; }
    if (w < 0 && errno != EAGAIN && errno != EWOULDBLOCK && errno != EINTR) { c->out_n = 0; return CXSRV_ERROR; }
    int64_t left = deadline - now_ms();
    if (left <= 0) { c->out_n = 0; return CXSRV_ERROR; }
    struct pollfd p = {c->fd, POLLOUT, 0};
    poll(&p, 1, (int)left);
  }
  c->out_n = 0;
  return 0;
}

int64_t cxsrv_close(int64_t h) {
  cxsrv_conn *c = conn(h);
  if (!c) return CXSRV_ERROR;
  close(c->fd);
  c->fd = -1; c->in_n = 0; c->out_n = 0;
  free(c->out); c->out = NULL;
  return 0;
}

int64_t cxsrv_now_s(void) { return (int64_t)time(NULL); }

int64_t cxsrv_log(int64_t byte) {
  if (byte < 0 || byte > 255) return -1;
  fputc((int)byte, stderr);
  if (byte == '\n') fflush(stderr);
  return 0;
}
