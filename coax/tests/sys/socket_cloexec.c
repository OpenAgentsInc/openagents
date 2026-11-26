/* Regression test (#14095): listening sockets created by cxsock_listen,
   cxsock_listen_ip, and cxsock_listen_unix must set FD_CLOEXEC and must never
   leak into child processes spawned via cxproc_spawn. */
#define _GNU_SOURCE
#include "../../platform/process-native.c"
#include "../../platform/socket-native.c"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void push_host(const char *s) {
  for (const char *p = s; *p; p++) assert(cxsock_host_push((unsigned char)*p) == 0);
}

static void push_arg(int64_t h, const char *s) {
  for (const char *p = s; *p; p++) assert(cxproc_push(h, 0, (unsigned char)*p) == 0);
  assert(cxproc_end(h, 0) == 0);
}

int main(int argc, char **argv) {
  if (argc >= 5 && strcmp(argv[1], "--check-child") == 0) {
    for (int i = 2; i < 5; i++) {
      int fd = atoi(argv[i]);
      assert(fd >= 3);
      errno = 0;
      int r = fcntl(fd, F_GETFD);
      if (r != -1 || errno != EBADF) {
        fprintf(stderr, "FAIL: child inherited listening socket fd=%d (fcntl=%d errno=%d)\n", fd, r, errno);
        return 2;
      }
    }
    printf("child_ok\n");
    return 0;
  }

  int64_t tcp = cxsock_listen(0);
  assert(tcp >= 0);
  int tcp_fd = cxsock_fd(tcp);
  assert(tcp_fd >= 3);
  assert((fcntl(tcp_fd, F_GETFD) & FD_CLOEXEC) != 0);

  push_host("127.0.0.1");
  int64_t ip = cxsock_listen_ip(0);
  assert(ip >= 0);
  int ip_fd = cxsock_fd(ip);
  assert(ip_fd >= 3);
  assert((fcntl(ip_fd, F_GETFD) & FD_CLOEXEC) != 0);

  char dir_tpl[] = "/tmp/coax-sock-cloexec-XXXXXX";
  char *dir = mkdtemp(dir_tpl);
  assert(dir != NULL);
  char sock_path[256];
  snprintf(sock_path, sizeof sock_path, "%s/service.sock", dir);
  push_host(sock_path);
  int64_t unix_h = cxsock_listen_unix();
  assert(unix_h >= 0);
  int unix_fd = cxsock_fd(unix_h);
  assert(unix_fd >= 3);
  assert((fcntl(unix_fd, F_GETFD) & FD_CLOEXEC) != 0);

  int64_t proc = cxproc_new();
  assert(proc >= 0);
  push_arg(proc, argv[0]);
  push_arg(proc, "--check-child");
  char fd_buf[32];
  snprintf(fd_buf, sizeof fd_buf, "%d", tcp_fd); push_arg(proc, fd_buf);
  snprintf(fd_buf, sizeof fd_buf, "%d", ip_fd); push_arg(proc, fd_buf);
  snprintf(fd_buf, sizeof fd_buf, "%d", unix_fd); push_arg(proc, fd_buf);
  assert(cxproc_spawn(proc) == 0);

  int64_t n = cxproc_read_line(proc, 3000);
  assert(n == 8);
  char line[16];
  for (int64_t i = 0; i < n; i++) line[i] = (char)cxproc_line_byte(proc, i);
  line[n] = 0;
  assert(strcmp(line, "child_ok") == 0);
  assert(cxproc_wait(proc, 2000) == 0);
  assert(cxproc_release(proc) == 0);

  assert(cxsock_close(unix_h) == 0);
  assert(cxsock_close(ip) == 0);
  assert(cxsock_close(tcp) == 0);
  unlink(sock_path);
  rmdir(dir);
  printf("socket_cloexec: verified FD_CLOEXEC on tcp, ip, and unix listeners + child exec isolation\n");
  return 0;
}
