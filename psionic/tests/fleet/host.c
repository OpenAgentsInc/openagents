/* Real loopback byte effects; protocol decisions are not host code. */
#define main fleet_host_main
#include "../../platform/fleet-host.c"
#undef main
#include <assert.h>
#include <stdlib.h>
#include <sys/time.h>

static int connect_client(int64_t listener) {
  struct sockaddr_in address; socklen_t n = sizeof address;
  assert(getsockname(fh_slots[listener].fd, (struct sockaddr *)&address, &n) == 0);
  assert(address.sin_addr.s_addr == htonl(INADDR_LOOPBACK));
  int client = socket(AF_INET, SOCK_STREAM, 0); assert(client >= 0);
  struct timeval limit = { 5, 0 };
  assert(setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &limit, sizeof limit) == 0);
  assert(connect(client, (struct sockaddr *)&address, n) == 0);
  return client;
}
static void stage(int64_t handle, const char *text) {
  for (const unsigned char *p = (const unsigned char *)text; *p; p++) assert(fh_write(handle, *p) == 0);
}
static void receive_exact(int client, const char *text) {
  char got[128]; size_t size = strlen(text), at = 0; assert(size < sizeof got);
  while (at < size) { ssize_t n = recv(client, got + at, size - at, 0); assert(n > 0); at += (size_t)n; }
  assert(memcmp(got, text, size) == 0);
}
static void *receive_big(void *arg) {
  int client = *(int *)arg; unsigned char buffer[8192]; size_t at = 0;
  while (at < FH_MAX_OUT) {
    ssize_t n = recv(client, buffer, sizeof buffer, 0); assert(n > 0);
    for (ssize_t i = 0; i < n; i++) assert(buffer[i] == (unsigned char)((at + (size_t)i) % 251));
    at += (size_t)n; assert(at <= FH_MAX_OUT);
  }
  return NULL;
}
static void *request_stop(void *arg) {
  (void)arg; struct timespec delay = { 0, 20000000 }; nanosleep(&delay, NULL);
  assert(kill(getpid(), SIGTERM) == 0); return NULL;
}
int coax_fleet_main(void) {
  assert(fh_argc() == 3 && fh_arg_len(0) == 10 && fh_arg_len(3) == -1);
  assert(fh_arg_byte(1, 0) == '-' && fh_arg_byte(1, 99) == -1 && fh_arg_byte(-1, 0) == -1);
  assert(fh_now_s() > 0 && fh_stop() == 0);
  int64_t listener = fh_listen(0); assert(listener >= 0);
  pthread_t stopper; assert(pthread_create(&stopper, NULL, request_stop, NULL) == 0);
  int64_t started = fh_mono_ms(); assert(fh_accept(listener, 2000) == -3);
  assert(fh_mono_ms() - started < 1000 && fh_stop() == 1);
  assert(pthread_join(stopper, NULL) == 0); assert(fh_close(listener) == 0); assert(fh_status(23) == 0); return 0;
}
int main(void) {
  assert(fh_listen(-1) == -3 && fh_listen(65536) == -3);
  int64_t listener = fh_listen(0); assert(listener >= 0);
  assert((fcntl(fh_slots[listener].fd, F_GETFD) & FD_CLOEXEC) != 0);
  assert(fh_accept(listener, 0) == -2 && fh_accept(-1, 1) == -3);
  int a = connect_client(listener); int64_t ah = fh_accept(listener, 1000); assert(ah >= 0);
  assert((fcntl(fh_slots[ah].fd, F_GETFL) & O_NONBLOCK) != 0);
  assert(fh_read(ah, 0) == -2 && fh_byte(0) == -1);
  const char request[] = "POST /v1/systemone HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
  assert(send(a, request, sizeof request - 1, 0) == (ssize_t)(sizeof request - 1));
  int64_t n = fh_read(ah, 1000); assert(n == (int64_t)sizeof request - 1);
  for (int64_t i = 0; i < n; i++) assert(fh_byte(i) == request[i]);
  assert(fh_byte(n) == -1 && fh_byte(-1) == -1);
  assert(fh_read(-1, 0) == -3 && fh_byte(0) == -1);
  unsigned char incoming[70000];
  for (size_t i = 0; i < sizeof incoming; i++) incoming[i] = (unsigned char)(i % 251);
  size_t at = 0;
  while (at < sizeof incoming) { ssize_t sent = send(a, incoming + at, sizeof incoming - at, 0); assert(sent > 0); at += (size_t)sent; }
  at = 0;
  while (at < sizeof incoming) {
    n = fh_read(ah, 1000); assert(n > 0 && n <= 65536 && at + (size_t)n <= sizeof incoming);
    for (int64_t i = 0; i < n; i++) assert(fh_byte(i) == incoming[at + (size_t)i]);
    assert(fh_byte(n) == -1); at += (size_t)n;
  }
  assert(fh_read(ah, -1) == -3 && fh_byte(0) == -1);
  const char response[] = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
  stage(ah, response); assert(fh_flush(ah, 1000) == 0); receive_exact(a, response);
  int b = connect_client(listener); int64_t bh = fh_accept(listener, 1000); assert(bh >= 0);
  stage(ah, "A"); stage(bh, "B"); assert(fh_flush(bh, 1000) == 0); receive_exact(b, "B");
  assert(fh_flush(ah, 1000) == 0); receive_exact(a, "A");
  assert(fh_write(ah, -1) == -3 && fh_write(ah, 256) == -3);
  assert(shutdown(a, SHUT_WR) == 0 && fh_read(ah, 1000) == -1 && fh_byte(0) == -1);
  stage(ah, "never sent"); assert(fh_close(ah) == 0); close(a);
  int c = connect_client(listener); int64_t ch = fh_accept(listener, 1000); assert(ch >= 0);
  assert(fh_flush(ch, 1000) == 0); stage(ch, "new"); assert(fh_flush(ch, 1000) == 0); receive_exact(c, "new");
  int small = 1024; assert(setsockopt(fh_slots[ch].fd, SOL_SOCKET, SO_SNDBUF, &small, sizeof small) == 0);
  for (size_t i = 0; i < FH_MAX_OUT; i++) assert(fh_write(ch, (int64_t)(i % 251)) == 0);
  assert(fh_write(ch, 0) == -3);
  assert(fh_flush(ch, 0) == -2);
  pthread_t reader; assert(pthread_create(&reader, NULL, receive_big, &c) == 0);
  assert(fh_flush(ch, 10000) == 0); assert(pthread_join(reader, NULL) == 0);
  assert(fh_flush(ch, 0) == 0 && fh_close(ch) == 0); close(c);
  assert(fh_close(bh) == 0); close(b); assert(fh_close(listener) == 0);
  assert(fh_close(listener) == -3 && fh_flush(-1, 0) == -3 && fh_read(-1, 0) == -3);
  char *args[] = { "fleet-test", "--flag", "value", NULL };
  assert(fleet_host_main(3, args) == 23);
  puts("fleet host: loopback bytes, deadlines, bounds, stream isolation, retry and lifecycle passed");
  return 0;
}
