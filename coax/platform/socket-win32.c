/* Client sockets on Windows (GitHub #13782): socket-native.h's TCP and local stream half over
   Winsock, for coax/lib/net/socket_foreign.coax. Effects only, as socket-native.c on POSIX (which
   stays the POSIX build's). A local socket is Winsock's AF_UNIX (Windows 10 1803 and later),
   so the desktop core reaches the daemon at the same HOST/service.sock path as on a Mac or Linux.
   cxsock_peer_self asks Winsock for the peer's process (SIO_AF_UNIX_GETPEERPID) and compares
   that process's user with this one's, in place of POSIX's peer uid. Unix listeners and UDP are
   not provided here (nothing on Windows serves them yet). */
#include "win32-native.h"
#include <winsock2.h>
#include <ws2tcpip.h>
#include <afunix.h>
#include "socket-native.h"
#include <stdio.h>

typedef struct {
  SOCKET fd;              /* INVALID_SOCKET when the slot is free */
  int64_t in_count;
  unsigned char in[CXSOCK_MAX_IN];
  int64_t out_count;
  unsigned char *out;
  int port;
  char peer_ip[64];
  int64_t peer_len;
} cxsock_slot;

static cxsock_slot cxsock_slots[CXSOCK_MAX];
static int cxsock_ready;
static char cxsock_host[CXSOCK_MAX_HOST + 1];
static int cxsock_host_len, cxsock_host_bad;

static void cxsock_init(void) {
  if (cxsock_ready) return;
  WSADATA wsa;
  WSAStartup(MAKEWORD(2, 2), &wsa);
  for (int i = 0; i < CXSOCK_MAX; i++) { cxsock_slots[i].fd = INVALID_SOCKET; cxsock_slots[i].out = NULL; }
  cxsock_ready = 1;
}
static cxsock_slot *cxsock_get(int64_t h) {
  cxsock_init();
  if (h < 0 || h >= CXSOCK_MAX || cxsock_slots[h].fd == INVALID_SOCKET) return NULL;
  return &cxsock_slots[h];
}
int64_t cxsock_now_ms(void) { return (int64_t)GetTickCount64(); }
int64_t cxsock_host_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || cxsock_host_len >= CXSOCK_MAX_HOST) { cxsock_host_bad = 1; return CXSOCK_FULL; }
  cxsock_host[cxsock_host_len++] = (char)byte;
  return 0;
}
/* Waits for fd to be readable (write 0) or writable (write 1) until deadline (-1 for ever). */
static int cxsock_wait(SOCKET fd, int write, int64_t deadline) {
  WSAPOLLFD p = {fd, write ? POLLWRNORM : POLLRDNORM, 0};
  int64_t left = deadline < 0 ? -1 : deadline - cxsock_now_ms();
  if (deadline >= 0 && left < 0) left = 0;
  int r = WSAPoll(&p, 1, left > 2147483647 ? 2147483647 : (int)left);
  if (r < 0) return CXSOCK_ERROR;
  if (r == 0) return CXSOCK_TIMEOUT;
  return 0;
}
static int64_t cxsock_take(SOCKET fd, int port) {
  for (int i = 0; i < CXSOCK_MAX; i++) if (cxsock_slots[i].fd == INVALID_SOCKET) {
    if (!cxsock_slots[i].out) cxsock_slots[i].out = malloc(CXSOCK_MAX_OUT);
    if (!cxsock_slots[i].out) { closesocket(fd); return CXSOCK_ERROR; }
    cxsock_slots[i].fd = fd; cxsock_slots[i].in_count = 0; cxsock_slots[i].out_count = 0; cxsock_slots[i].port = port;
    return i;
  }
  closesocket(fd);
  return CXSOCK_FULL;
}
static int cxsock_nonblocking(SOCKET fd) { u_long on = 1; return ioctlsocket(fd, FIONBIO, &on) == 0; }
/* connect, bounded by deadline: 0, or a CXSOCK_ error. */
static int cxsock_dial(SOCKET fd, const struct sockaddr *a, int len, int64_t deadline) {
  if (connect(fd, a, len) == 0) return 0;
  if (WSAGetLastError() != WSAEWOULDBLOCK) return CXSOCK_REFUSED;
  int w = cxsock_wait(fd, 1, deadline);
  if (w) return w;
  int err = 0, n = sizeof err;
  if (getsockopt(fd, SOL_SOCKET, SO_ERROR, (char *)&err, &n) != 0 || err) return CXSOCK_REFUSED;
  return 0;
}

int64_t cxsock_connect_host(const char *host, int64_t port, int64_t timeout_ms) {
  cxsock_init();
  if (port <= 0 || port > 65535 || timeout_ms < 0) return CXSOCK_ERROR;
  char service[8];
  snprintf(service, sizeof service, "%d", (int)port);
  struct addrinfo hints, *found = NULL;
  memset(&hints, 0, sizeof hints);
  hints.ai_family = AF_UNSPEC; hints.ai_socktype = SOCK_STREAM;
  if (getaddrinfo(host, service, &hints, &found) != 0 || !found) return CXSOCK_RESOLVE;
  int64_t deadline = cxsock_now_ms() + timeout_ms, result = CXSOCK_REFUSED;
  for (struct addrinfo *a = found; a; a = a->ai_next) {
    SOCKET fd = socket(a->ai_family, a->ai_socktype, a->ai_protocol);
    if (fd == INVALID_SOCKET) continue;
    if (!cxsock_nonblocking(fd)) { closesocket(fd); continue; }
    int r = cxsock_dial(fd, a->ai_addr, (int)a->ai_addrlen, deadline);
    if (r == 0) {
      int one = 1; setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, (const char *)&one, sizeof one);
      result = cxsock_take(fd, 0);
      break;
    }
    closesocket(fd);
    result = r;
    if (r == CXSOCK_TIMEOUT) break;
  }
  freeaddrinfo(found);
  return result;
}
int64_t cxsock_connect(int64_t port, int64_t timeout_ms) {
  int bad = cxsock_host_bad || cxsock_host_len == 0;
  cxsock_host[cxsock_host_len] = 0;
  cxsock_host_len = 0; cxsock_host_bad = 0;
  if (bad) return CXSOCK_ERROR;
  char host[CXSOCK_MAX_HOST + 1];
  memcpy(host, cxsock_host, sizeof host);
  return cxsock_connect_host(host, port, timeout_ms);
}
int64_t cxsock_connect_unix(int64_t timeout_ms) {
  cxsock_init();
  struct sockaddr_un addr;
  int bad = cxsock_host_bad || cxsock_host_len == 0 || cxsock_host_len >= (int)sizeof addr.sun_path || timeout_ms < 0;
  cxsock_host[cxsock_host_len] = 0;
  memset(&addr, 0, sizeof addr);
  addr.sun_family = AF_UNIX;
  if (!bad) memcpy(addr.sun_path, cxsock_host, (size_t)cxsock_host_len);
  cxsock_host_len = 0; cxsock_host_bad = 0;
  if (bad) return CXSOCK_ERROR;
  SOCKET fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd == INVALID_SOCKET) return CXSOCK_REFUSED;   /* Windows before 1803: no local sockets */
  if (!cxsock_nonblocking(fd)) { closesocket(fd); return CXSOCK_ERROR; }
  int r = cxsock_dial(fd, (struct sockaddr *)&addr, (int)sizeof addr, cxsock_now_ms() + timeout_ms);
  if (r) { closesocket(fd); return r; }
  return cxsock_take(fd, 0);
}
int64_t cxsock_listen_unix(void) {
  cxsock_init();
  struct sockaddr_un addr;
  int bad = cxsock_host_bad || cxsock_host_len == 0 || cxsock_host_len >= (int)sizeof addr.sun_path;
  cxsock_host[cxsock_host_len] = 0;
  memset(&addr, 0, sizeof addr);
  addr.sun_family = AF_UNIX;
  if (!bad) memcpy(addr.sun_path, cxsock_host, (size_t)cxsock_host_len);
  cxsock_host_len = 0; cxsock_host_bad = 0;
  if (bad) return CXSOCK_ERROR;
  DeleteFileA(addr.sun_path);
  SOCKET fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd == INVALID_SOCKET) return CXSOCK_REFUSED;
  if (bind(fd, (struct sockaddr *)&addr, (int)sizeof addr) != 0 ||
      listen(fd, 16) != 0 || !cxsock_nonblocking(fd)) {
    closesocket(fd);
    return CXSOCK_REFUSED;
  }
  return cxsock_take(fd, 0);
}

/* The user a process runs as is this process's user. */
static int same_user(DWORD pid) {
  int same = 0;
  HANDLE them = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid), tt = NULL, mt = NULL;
  if (!them) return 0;
  unsigned char a[512], b[512];
  DWORD n = 0;
  if (OpenProcessToken(them, TOKEN_QUERY, &tt) && OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mt) &&
      GetTokenInformation(tt, TokenUser, a, sizeof a, &n) && GetTokenInformation(mt, TokenUser, b, sizeof b, &n))
    same = EqualSid(((TOKEN_USER *)a)->User.Sid, ((TOKEN_USER *)b)->User.Sid) != 0;
  if (tt) CloseHandle(tt);
  if (mt) CloseHandle(mt);
  CloseHandle(them);
  return same;
}
int64_t cxsock_peer_self(int64_t h) {
  cxsock_slot *s = cxsock_get(h);
  if (!s) return 0;
  ULONG pid = 0; DWORD got = 0;
  if (WSAIoctl(s->fd, SIO_AF_UNIX_GETPEERPID, NULL, 0, &pid, sizeof pid, &got, NULL, NULL) != 0 || got != sizeof pid || !pid) return 0;
  return same_user(pid);
}

int64_t cxsock_listen(int64_t port) {
  cxsock_init();
  if (port < 0 || port > 65535) return CXSOCK_ERROR;
  SOCKET fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd == INVALID_SOCKET) return CXSOCK_ERROR;
  int one = 1; setsockopt(fd, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (const char *)&one, sizeof one);
  struct sockaddr_in a; memset(&a, 0, sizeof a);
  a.sin_family = AF_INET; a.sin_port = htons((u_short)port); a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  if (bind(fd, (struct sockaddr *)&a, sizeof a) != 0 || listen(fd, 16) != 0 || !cxsock_nonblocking(fd)) { closesocket(fd); return CXSOCK_REFUSED; }
  int len = sizeof a;
  getsockname(fd, (struct sockaddr *)&a, &len);
  return cxsock_take(fd, ntohs(a.sin_port));
}
int64_t cxsock_listen_ip(int64_t port) {
  cxsock_init();
  int bad = cxsock_host_bad || cxsock_host_len == 0 || cxsock_host_len >= 64 || port < 0 || port > 65535;
  char ip[64];
  if (!bad) { memcpy(ip, cxsock_host, (size_t)cxsock_host_len); ip[cxsock_host_len] = 0; }
  cxsock_host_len = 0; cxsock_host_bad = 0;
  if (bad) return CXSOCK_ERROR;
  struct sockaddr_in a; memset(&a, 0, sizeof a);
  a.sin_family = AF_INET; a.sin_port = htons((u_short)port);
  if (InetPtonA(AF_INET, ip, &a.sin_addr) != 1) return CXSOCK_ERROR;
  SOCKET fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd == INVALID_SOCKET) return CXSOCK_ERROR;
  int one = 1; setsockopt(fd, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (const char *)&one, sizeof one);
  if (bind(fd, (struct sockaddr *)&a, sizeof a) != 0 || listen(fd, 16) != 0 || !cxsock_nonblocking(fd)) { closesocket(fd); return CXSOCK_REFUSED; }
  int len = sizeof a;
  getsockname(fd, (struct sockaddr *)&a, &len);
  return cxsock_take(fd, ntohs(a.sin_port));
}
int64_t cxsock_port(int64_t h) { cxsock_slot *s = cxsock_get(h); return s ? s->port : CXSOCK_ERROR; }
int64_t cxsock_accept(int64_t h, int64_t timeout_ms) {
  cxsock_slot *s = cxsock_get(h);
  if (!s || !s->port) return CXSOCK_ERROR;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  for (;;) {
    struct sockaddr_storage sa; int salen = sizeof sa;
    SOCKET fd = accept(s->fd, (struct sockaddr *)&sa, &salen);
    if (fd != INVALID_SOCKET) {
      if (!cxsock_nonblocking(fd)) { closesocket(fd); return CXSOCK_ERROR; }
      int64_t slot = cxsock_take(fd, 0);
      if (slot >= 0) {
        cxsock_slot *cs = &cxsock_slots[slot];
        cs->peer_len = 0; cs->peer_ip[0] = 0;
        if (sa.ss_family == AF_INET) {
          InetNtopA(AF_INET, &((struct sockaddr_in *)&sa)->sin_addr, cs->peer_ip, sizeof cs->peer_ip);
          cs->peer_len = (int64_t)strlen(cs->peer_ip);
        }
      }
      return slot;
    }
    if (WSAGetLastError() != WSAEWOULDBLOCK) return CXSOCK_ERROR;
    int w = cxsock_wait(s->fd, 0, deadline);
    if (w) return w;
  }
}
int64_t cxsock_peer_ip(int64_t h) { cxsock_slot *s = cxsock_get(h); return s ? s->peer_len : 0; }
int64_t cxsock_peer_byte(int64_t h, int64_t i) { cxsock_slot *s = cxsock_get(h); return (!s || i < 0 || i >= s->peer_len) ? 0 : (unsigned char)s->peer_ip[i]; }
int64_t cxsock_write(int64_t h, int64_t byte) {
  cxsock_slot *s = cxsock_get(h);
  if (!s || !s->out || byte < 0 || byte > 255) return CXSOCK_ERROR;
  if (s->out_count >= CXSOCK_MAX_OUT) return CXSOCK_FULL;
  s->out[s->out_count++] = (unsigned char)byte;
  return 0;
}
int64_t cxsock_flush(int64_t h, int64_t timeout_ms) {
  cxsock_slot *s = cxsock_get(h);
  if (!s) return CXSOCK_ERROR;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms, at = 0;
  while (at < s->out_count) {
    int n = send(s->fd, (const char *)s->out + at, (int)(s->out_count - at), 0);
    if (n > 0) { at += n; continue; }
    if (n < 0 && WSAGetLastError() == WSAEWOULDBLOCK) {
      int w = cxsock_wait(s->fd, 1, deadline);
      if (w) { s->out_count = 0; return w; }
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
  if (!s) return CXSOCK_ERROR;
  s->in_count = 0;
  int64_t deadline = timeout_ms < 0 ? -1 : cxsock_now_ms() + timeout_ms;
  for (;;) {
    int n = recv(s->fd, (char *)s->in, CXSOCK_MAX_IN, 0);
    if (n > 0) { s->in_count = n; return n; }
    if (n == 0) return CXSOCK_EOF;
    if (WSAGetLastError() != WSAEWOULDBLOCK) return CXSOCK_ERROR;
    int w = cxsock_wait(s->fd, 0, deadline);
    if (w) return w;
  }
}
int64_t cxsock_byte(int64_t h, int64_t i) {
  cxsock_slot *s = cxsock_get(h);
  if (!s || i < 0 || i >= s->in_count) return CXSOCK_ERROR;
  return s->in[i];
}
int cxsock_fd(int64_t h) { (void)h; return -1; }
int64_t cxsock_close(int64_t h) {
  cxsock_slot *s = cxsock_get(h);
  if (!s) return CXSOCK_ERROR;
  closesocket(s->fd);
  s->fd = INVALID_SOCKET; s->in_count = 0; s->out_count = 0; s->port = 0;
  return 0;
}
