/* The Wayland server's effects for Coax (os/compositor.coax): a listening Unix socket, its clients'
   bytes and file descriptors (SCM_RIGHTS), shared-memory pools mapped read-only, and sealed memory
   files for the keymap. Effects only; the protocol, every object and every decision is Coax.
   See wayland-native.h. */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "wayland-native.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#ifndef MSG_CMSG_CLOEXEC
#define MSG_CMSG_CLOEXEC 0
#endif
#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif
#define WL_CLIENTS 32
#define WL_FDS 28
#define WL_MAPS 256

static int lis = -1;
static int cl_fd[WL_CLIENTS];
static int cl_used[WL_CLIENTS];
static int got_fds[WL_FDS];
static int got_n;
static void *map_at[WL_MAPS];
static size_t map_len[WL_MAPS];
static int map_used[WL_MAPS];

int64_t cxwl_listen(coax_v_u8 path) {
  signal(SIGPIPE, SIG_IGN);
  struct sockaddr_un a; memset(&a, 0, sizeof a); a.sun_family = AF_UNIX;
  if (path.count <= 0 || (size_t)path.count >= sizeof a.sun_path) return -ENAMETOOLONG;
  memcpy(a.sun_path, path.data, (size_t)path.count);
  unlink(a.sun_path);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return -(int64_t)errno;
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  if (bind(fd, (struct sockaddr *)&a, sizeof a) < 0 || listen(fd, 16) < 0) { int e = errno; close(fd); return -(int64_t)e; }
  fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK);
  lis = fd;
  return 0;
}
int64_t cxwl_unlisten(coax_v_u8 path) {
  if (lis >= 0) close(lis);
  lis = -1;
  char p[108]; if (path.count > 0 && path.count < 108) { memcpy(p, path.data, (size_t)path.count); p[path.count] = 0; unlink(p); }
  return 0;
}
int64_t cxwl_accept(void) {
  if (lis < 0) return -1;
  int fd = accept(lis, NULL, NULL);
  if (fd < 0) return -1;
  for (int i = 0; i < WL_CLIENTS; i++) if (!cl_used[i]) {
    fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK); fcntl(fd, F_SETFD, FD_CLOEXEC);
    cl_used[i] = 1; cl_fd[i] = fd; return i;
  }
  close(fd);
  return -2;
}
/* What a client sent: the byte count into out (0 when nothing is ready, -1 at its end or on an
   error). The descriptors that came with them wait for cxwl_fd_count and cxwl_fd. */
int64_t cxwl_recv(int64_t h, coax_v_u8 out) {
  got_n = 0;
  if (h < 0 || h >= WL_CLIENTS || !cl_used[h]) return -1;
  char ctl[CMSG_SPACE(sizeof(int) * WL_FDS)];
  struct iovec iov = {out.data, (size_t)out.count};
  struct msghdr m; memset(&m, 0, sizeof m);
  m.msg_iov = &iov; m.msg_iovlen = 1; m.msg_control = ctl; m.msg_controllen = sizeof ctl;
  ssize_t n;
  do { n = recvmsg(cl_fd[h], &m, MSG_DONTWAIT | MSG_CMSG_CLOEXEC); } while (n < 0 && errno == EINTR);
  if (n < 0) return (errno == EAGAIN || errno == EWOULDBLOCK) ? 0 : -1;
  for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
    if (c->cmsg_level == SOL_SOCKET && c->cmsg_type == SCM_RIGHTS) {
      int k = (int)((c->cmsg_len - CMSG_LEN(0)) / sizeof(int));
      int *fds = (int *)CMSG_DATA(c);
      for (int i = 0; i < k; i++) { if (got_n < WL_FDS) got_fds[got_n++] = fds[i]; else close(fds[i]); }
    }
  }
  return n == 0 ? -1 : (int64_t)n;
}
int64_t cxwl_fd_count(void) { return got_n; }
int64_t cxwl_fd(int64_t i) { return (i >= 0 && i < got_n) ? got_fds[i] : -1; }
/* All of s to a client, with fd (or -1) riding on its first byte: 0, or -1 when it broke or
   would not take it within a second. */
int64_t cxwl_send(int64_t h, coax_v_u8 s, int64_t fd) {
  if (h < 0 || h >= WL_CLIENTS || !cl_used[h]) return -1;
  int64_t done = 0; int tries = 0;
  while (done < s.count) {
    struct iovec iov = {s.data + done, (size_t)(s.count - done)};
    struct msghdr m; memset(&m, 0, sizeof m); m.msg_iov = &iov; m.msg_iovlen = 1;
    char ctl[CMSG_SPACE(sizeof(int))];
    if (fd >= 0 && done == 0) {
      memset(ctl, 0, sizeof ctl);
      m.msg_control = ctl; m.msg_controllen = sizeof ctl;
      struct cmsghdr *c = CMSG_FIRSTHDR(&m);
      c->cmsg_level = SOL_SOCKET; c->cmsg_type = SCM_RIGHTS; c->cmsg_len = CMSG_LEN(sizeof(int));
      int f = (int)fd; memcpy(CMSG_DATA(c), &f, sizeof f);
    }
    ssize_t n = sendmsg(cl_fd[h], &m, MSG_NOSIGNAL | MSG_DONTWAIT);
    if (n < 0 && errno == EINTR) continue;
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      if (++tries > 1000) return -1;
      struct pollfd p = {cl_fd[h], POLLOUT, 0}; poll(&p, 1, 1); continue;
    }
    if (n <= 0) return -1;
    done += n;
  }
  return 0;
}
int64_t cxwl_hangup(int64_t h) {
  if (h < 0 || h >= WL_CLIENTS || !cl_used[h]) return -1;
  close(cl_fd[h]); cl_used[h] = 0;
  return 0;
}
int64_t cxwl_close_fd(int64_t fd) { return fd >= 0 ? close((int)fd) : -1; }
/* Waits up to timeout_ms for the listener, a client, or stdin when watch_stdin: the ready count. */
int64_t cxwl_poll(int64_t timeout_ms, int64_t watch_stdin) {
  struct pollfd p[WL_CLIENTS + 2]; int n = 0;
  if (lis >= 0) { p[n].fd = lis; p[n].events = POLLIN; p[n].revents = 0; n++; }
  if (watch_stdin) { p[n].fd = 0; p[n].events = POLLIN; p[n].revents = 0; n++; }
  for (int i = 0; i < WL_CLIENTS; i++) if (cl_used[i]) { p[n].fd = cl_fd[i]; p[n].events = POLLIN; p[n].revents = 0; n++; }
  int r = poll(p, (nfds_t)n, (int)timeout_ms);
  return r < 0 ? 0 : r;
}
/* A client's pool, mapped read-only: a map handle, or -errno. */
int64_t cxwl_map(int64_t fd, int64_t size) {
  if (size <= 0) return -EINVAL;
  void *at = mmap(NULL, (size_t)size, PROT_READ, MAP_SHARED, (int)fd, 0);
  if (at == MAP_FAILED) return -(int64_t)errno;
  for (int i = 0; i < WL_MAPS; i++) if (!map_used[i]) { map_used[i] = 1; map_at[i] = at; map_len[i] = (size_t)size; return i; }
  munmap(at, (size_t)size);
  return -ENOMEM;
}
int64_t cxwl_unmap(int64_t m) {
  if (m < 0 || m >= WL_MAPS || !map_used[m]) return -1;
  munmap(map_at[m], map_len[m]); map_used[m] = 0;
  return 0;
}
/* The pool grew (wl_shm_pool.resize): the same handle over the new size, or -errno. */
int64_t cxwl_remap(int64_t m, int64_t fd, int64_t size) {
  if (m < 0 || m >= WL_MAPS || !map_used[m] || size <= 0) return -EINVAL;
  void *at = mmap(NULL, (size_t)size, PROT_READ, MAP_SHARED, (int)fd, 0);
  if (at == MAP_FAILED) return -(int64_t)errno;
  munmap(map_at[m], map_len[m]); map_at[m] = at; map_len[m] = (size_t)size;
  return 0;
}
/* Bytes of a mapped pool from offset into out: the count copied (fewer at the pool's end), -1 for
   a bad handle. A client that shrinks its file under the map would fault the reader: the copy
   stops at the file's current size. */
int64_t cxwl_read(int64_t m, int64_t fd, int64_t offset, coax_v_u8 out) {
  if (m < 0 || m >= WL_MAPS || !map_used[m] || offset < 0) return -1;
  struct stat st; size_t len = map_len[m];
  if (fd >= 0 && fstat((int)fd, &st) == 0 && (size_t)st.st_size < len) len = (size_t)st.st_size;
  if ((size_t)offset >= len) return 0;
  size_t n = len - (size_t)offset; if ((int64_t)n > out.count) n = (size_t)out.count;
  memcpy(out.data, (char *)map_at[m] + offset, n);
  return (int64_t)n;
}
/* A sealed memory file holding s (the keymap): its fd, or -errno. */
int64_t cxwl_memfd(coax_v_u8 s) {
#if defined(__linux__)
  int fd = memfd_create("coax-keymap", MFD_CLOEXEC | MFD_ALLOW_SEALING);
#else
  int fd = -1; errno = ENOSYS;
#endif
  if (fd < 0) return -(int64_t)errno;
  int64_t done = 0;
  while (done < s.count) { ssize_t n = write(fd, s.data + done, (size_t)(s.count - done)); if (n <= 0) { int e = errno; close(fd); return -(int64_t)e; } done += n; }
#if defined(__linux__)
  fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE | F_SEAL_SEAL);
#endif
  return fd;
}
/* Milliseconds on the monotonic clock, as wl_callback.done and input events carry them. */
int64_t cxwl_ms(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000; }
/* What stdin has (the test channel): the count read into out, 0 when nothing is ready, -1 at its end. */
int64_t cxwl_stdin(coax_v_u8 out) {
  struct pollfd p = {0, POLLIN, 0};
  if (poll(&p, 1, 0) <= 0) return 0;
  ssize_t n = read(0, out.data, (size_t)out.count);
  return n > 0 ? (int64_t)n : -1;
}
/* All of s onto stdout (the test channel's answers): 0, or -1 when it broke. */
int64_t cxwl_out(coax_v_u8 s) {
  int64_t done = 0;
  while (done < s.count) { ssize_t n = write(1, s.data + done, (size_t)(s.count - done)); if (n < 0 && errno == EINTR) continue; if (n <= 0) return -1; done += n; }
  return 0;
}
/* Starts path detached, a client of the compositor at display (a full socket path): its pid, or
   -errno. Children are reaped by the kernel (SIGCHLD is ignored once this has run). */
int64_t cxwl_spawn(coax_v_u8 path, coax_v_u8 display) {
  char p[1024], d[1024];
  if (path.count <= 0 || path.count >= 1024 || display.count <= 0 || display.count >= 1024) return -ENAMETOOLONG;
  memcpy(p, path.data, (size_t)path.count); p[path.count] = 0;
  memcpy(d, display.data, (size_t)display.count); d[display.count] = 0;
  if (access(p, X_OK) != 0) return -(int64_t)errno;
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  if (pid < 0) return -(int64_t)errno;
  if (pid == 0) {
    setsid(); setenv("WAYLAND_DISPLAY", d, 1); unsetenv("DISPLAY");
    int nul = open("/dev/null", O_RDWR); if (nul >= 0) { dup2(nul, 0); }
    execl(p, p, (char *)NULL); _exit(127);
  }
  return pid;
}
/* Whether a pool's file still holds end bytes (a client may shrink it under the map): checked once
   per surface draw, so the row reads can pass fd -1 and skip the check. */
int64_t cxwl_fits(int64_t m, int64_t fd, int64_t end) {
  if (m < 0 || m >= WL_MAPS || !map_used[m] || end < 0 || (size_t)end > map_len[m]) return 0;
  struct stat st;
  return (fd >= 0 && fstat((int)fd, &st) == 0 && st.st_size >= end) ? 1 : 0;
}
