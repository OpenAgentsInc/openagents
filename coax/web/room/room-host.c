/* The Orbit League room server's host (ol-room): TCP, the WebSocket
   handshake and frames (verse/voice-host/ws.c), and the 60 Hz beat. Every
   room decision is room.coax's: the host hands it each new socket's request
   (rm_join), each text frame (rm_heard) and each close (rm_left), calls
   rm_tick 60 times a second and sends each player what rm_state writes.
     room [--listen 127.0.0.1] [--port 8830]     PORT in the environment wins (Cloud Run: 0.0.0.0)
   GET /health answers 200 "ok" with the number of players. */
#define _POSIX_C_SOURCE 200809L
#include "room.h"
#include "ws.h"
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

#define CONNS 32
#define IN_CAP 16384
typedef struct { int fd; bool ws; size_t n; uint8_t in[IN_CAP]; int64_t heard; } conn;
static conn conns[CONNS];
static coax_t_RmServer *server;
static coax_v_t_RmServer view(void) { return (coax_v_t_RmServer){server, 1}; }

static int64_t now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static bool send_all(int fd, const void *p, size_t n) {
  const uint8_t *b = p;
  while (n) {
    ssize_t w = send(fd, b, n, MSG_NOSIGNAL);
    if (w < 0 && errno == EINTR) continue;
    if (w < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      struct pollfd q = {fd, POLLOUT, 0};
      if (poll(&q, 1, 50) <= 0) return false;
      continue;
    }
    if (w <= 0) return false;
    b += w; n -= (size_t)w;
  }
  return true;
}
static void shut(int i);
// The relay's frames (room-relay.coax), each to the connections it names.
static void relay_flush(void) {
  int64_t len;
  while ((len = rm_relay_next(view())) > 0) {
    static uint8_t out[4300];
    size_t k = ws_build(out, sizeof out, WS_TEXT, server->rout.e, (size_t)len, false);
    if (!k) continue;
    for (int j = 0; j < CONNS; j++) if (conns[j].fd >= 0 && conns[j].ws && rm_relay_to(view(), j) && !send_all(conns[j].fd, out, k)) shut(j);
  }
}
static void shut(int i) {
  if (conns[i].fd < 0) return;
  bool was = conns[i].ws;
  if (was) rm_left(view(), i);
  close(conns[i].fd);
  conns[i].fd = -1; conns[i].ws = false; conns[i].n = 0;
  if (was) relay_flush();
}
static void http(int i, const char *status, const char *body) {
  char out[512];
  int n = snprintf(out, sizeof out, "HTTP/1.1 %s\r\nContent-Type: text/plain\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s", status, strlen(body), body);
  send_all(conns[i].fd, out, (size_t)n);
  shut(i);
}
// The request is in: a room's WebSocket, the health check, or nothing.
static void opening(int i) {
  conn *c = &conns[i];
  uint8_t *end = NULL;
  for (size_t k = 3; k < c->n; k++) if (!memcmp(c->in + k - 3, "\r\n\r\n", 4)) { end = c->in + k + 1; break; }
  if (!end) { if (c->n >= IN_CAP) http(i, "431 Request Header Fields Too Large", "too big"); return; }
  size_t len = (size_t)(end - c->in);
  if (!strncmp((char *)c->in, "GET /health ", 12)) {
    char body[64]; snprintf(body, sizeof body, "ok %lld players\n", (long long)rm_players(view()));
    http(i, "200 OK", body); return;
  }
  char out[512];
  size_t hn = ws_server_handshake((char *)c->in, len, out, sizeof out);
  if (!hn) { http(i, "400 Bad Request", "Rooms: open a WebSocket to /api/room/orbit-league/NAME, or to /api/room/relay/GAME/CODE.\n"); return; }
  int64_t seat = rm_join(view(), i, (coax_string){(const char *)c->in, (int64_t)len});
  if (seat < 0) { http(i, "404 Not Found", "No such room, or it is full.\n"); return; }
  if (!send_all(c->fd, out, hn)) { rm_left(view(), i); close(c->fd); c->fd = -1; c->n = 0; return; }
  c->ws = true;
  memmove(c->in, end, c->n - len); c->n -= len;
  relay_flush();
}
static void frames(int i) {
  conn *c = &conns[i];
  while (c->fd >= 0 && c->n) {
    ws_frame f;
    int r = ws_parse(c->in, c->n, 4096, &f);
    if (r == 0) return;
    if (r < 0) { shut(i); return; }
    if (f.opcode == WS_TEXT) { rm_heard(view(), i, (coax_string){(const char *)f.payload, (int64_t)f.length}); relay_flush(); if (c->fd < 0) return; }
    else if (f.opcode == WS_PING) {
      uint8_t out[160]; size_t k = ws_build(out, sizeof out, WS_PONG, f.payload, f.length, false);
      if (!k || !send_all(c->fd, out, k)) { shut(i); return; }
    } else if (f.opcode == WS_CLOSE) {
      uint8_t out[8]; size_t k = ws_build(out, sizeof out, WS_CLOSE, (const uint8_t *)"\x03\xe8", 2, false);
      if (k) send_all(c->fd, out, k);
      shut(i); return;
    }
    memmove(c->in, c->in + f.consumed, c->n - f.consumed); c->n -= f.consumed;
  }
}

int main(int argc, char **argv) {
  const char *address = "127.0.0.1"; int port = 8830;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--listen") && i + 1 < argc) address = argv[++i];
    else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
  }
  const char *env = getenv("PORT");
  if (env && *env) { port = atoi(env); address = "0.0.0.0"; }
  signal(SIGPIPE, SIG_IGN);
  setvbuf(stdout, NULL, _IOLBF, 0);
  server = calloc(1, sizeof *server);
  if (!server) { fprintf(stderr, "room: out of memory\n"); return 1; }
  rm_init(view());
  int lfd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1; setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0};
  a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, address, &a.sin_addr);
  if (bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 64)) { perror("room: listen"); return 1; }
  fcntl(lfd, F_SETFL, O_NONBLOCK);
  for (int i = 0; i < CONNS; i++) conns[i].fd = -1;
  printf("room: listening on %s:%d\n", address, port);
  const double tick = 1000.0 / 60.0;
  double next = (double)now_ms();
  int64_t ticks = 0, last_report = now_ms();
  for (;;) {
    struct pollfd fds[CONNS + 1]; int map[CONNS + 1], n = 0;
    fds[n] = (struct pollfd){lfd, POLLIN, 0}; map[n++] = -1;
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0) { fds[n] = (struct pollfd){conns[i].fd, POLLIN, 0}; map[n++] = i; }
    int wait = (int)(next - (double)now_ms());
    if (poll(fds, (nfds_t)n, wait > 0 ? wait : 0) < 0 && errno != EINTR) { perror("room: poll"); return 1; }
    int64_t now = now_ms();
    for (int k = 0; k < n; k++) {
      if (!(fds[k].revents & (POLLIN | POLLHUP | POLLERR))) continue;
      if (map[k] < 0) {
        for (;;) {
          int fd = accept(lfd, NULL, NULL);
          if (fd < 0) break;
          int slot = -1;
          for (int i = 0; i < CONNS; i++) if (conns[i].fd < 0) { slot = i; break; }
          if (slot < 0) { close(fd); continue; }
          fcntl(fd, F_SETFL, O_NONBLOCK);
          setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
          conns[slot] = (conn){.fd = fd, .heard = now};
        }
        continue;
      }
      int i = map[k]; conn *c = &conns[i];
      ssize_t r = recv(c->fd, c->in + c->n, IN_CAP - c->n, 0);
      if (r <= 0) { if (r < 0 && (errno == EAGAIN || errno == EINTR)) continue; shut(i); continue; }
      c->n += (size_t)r; c->heard = now;
      if (!c->ws) opening(i);
      if (c->fd >= 0 && c->ws) frames(i);
      if (c->fd >= 0 && c->n >= IN_CAP) shut(i);
    }
    for (int i = 0; i < CONNS; i++) if (conns[i].fd >= 0 && now - conns[i].heard > 20000) shut(i);
    if ((double)now >= next) {
      rm_tick(view());
      for (int i = 0; i < CONNS; i++) {
        if (conns[i].fd < 0 || !conns[i].ws) continue;
        int64_t len = rm_state(view(), i);
        if (len <= 0) continue;
        uint8_t out[2100]; size_t k = ws_build(out, sizeof out, WS_TEXT, server->out.e, (size_t)len, false);
        if (!k || !send_all(conns[i].fd, out, k)) shut(i);
      }
      ticks++;
      next += tick;
      if ((double)now - next > 250.0) next = (double)now + tick;   // a stall: start the beat again
      if (now - last_report >= 60000) { printf("room: %lld ticks in %.1f s, %lld players\n", (long long)ticks, (now - last_report) / 1000.0, (long long)rm_players(view())); ticks = 0; last_report = now; }
    }
  }
}
