/* wt-echo (#13175): the WebTransport prototype's server. It accepts any session, sends every
   datagram back on the session it came on, and echoes each bidirectional stream the page opens
   (its bytes back on the same stream, its end as the stream's end). On the same port over TCP it
   answers GET / with an empty page, the secure origin a headless Chrome test opens WebTransport from.
     wt-echo PORT CERT.pem KEY.pem [ADDRESS]
   Built and run by coax/web/room/wt-check.sh; the check is coax/web/room/wt-check.coax. */
#define _POSIX_C_SOURCE 200809L
#include "wt.h"
#include <arpa/inet.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static wt_server *srv;
static long long sessions, echoed, streamed;
static bool session(void *u, int sid, const char *path, const char *authority, const char *origin) {
  (void)u; sessions++;
  printf("wt-echo: session %d %s (authority %s, origin %s)\n", sid, path, authority, origin);
  return true;
}
static void datagram(void *u, int sid, const uint8_t *d, size_t n) { (void)u; if (!wt_send_datagram(srv, sid, d, n)) echoed++; }
static void stream(void *u, int sid, int64_t id, const uint8_t *d, size_t n, bool fin) {
  (void)u; streamed += (long long)n;
  if ((id & 3) == 0) wt_stream_send(srv, sid, id, d, n, fin);   /* the page's bidirectional streams */
}
static void closed(void *u, int sid) { (void)u; printf("wt-echo: session %d closed; %lld datagrams echoed, %lld stream bytes\n", sid, echoed, streamed); }

int main(int argc, char **argv) {
  if (argc < 4) { fprintf(stderr, "usage: wt-echo PORT CERT.pem KEY.pem [ADDRESS]\n"); return 2; }
  int port = atoi(argv[1]);
  const char *address = argc > 4 ? argv[4] : "127.0.0.1";
  signal(SIGPIPE, SIG_IGN);
  setvbuf(stdout, NULL, _IOLBF, 0);
  wt_events ev = {session, datagram, stream, closed};
  srv = wt_listen(address, port, argv[2], argv[3], &ev, NULL);
  if (!srv) { fprintf(stderr, "wt-echo: can't listen on udp %s:%d\n", address, port); return 1; }
  int lfd = socket(AF_INET, SOCK_STREAM, 0), one = 1;
  setsockopt(lfd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {0}; a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port); inet_pton(AF_INET, address, &a.sin_addr);
  if (bind(lfd, (struct sockaddr *)&a, sizeof a) || listen(lfd, 16)) { perror("wt-echo: tcp"); return 1; }
  printf("wt-echo: listening on udp and tcp %s:%d\n", address, port);
  for (;;) {
    struct pollfd fds[2] = {{wt_fd(srv), POLLIN, 0}, {lfd, POLLIN, 0}};
    poll(fds, 2, wt_timeout_ms(srv));
    if (fds[0].revents & POLLIN) wt_read(srv);
    if (fds[1].revents & POLLIN) {
      int fd = accept(lfd, NULL, NULL);
      if (fd >= 0) {
        char req[2048]; ssize_t r = recv(fd, req, sizeof req, 0); (void)r;
        const char *body = "<!doctype html><title>wt-echo</title>\n";
        char page[256]; int k = snprintf(page, sizeof page, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s", strlen(body), body);
        ssize_t w = send(fd, page, (size_t)k, 0); (void)w;
        close(fd);
      }
    }
    wt_flush(srv);
  }
}
