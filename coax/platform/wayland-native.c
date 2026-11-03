/* The Wayland server's effects for Coax (os/compositor.coax): a listening Unix socket, its clients'
   bytes and file descriptors (SCM_RIGHTS), shared-memory pools mapped read-only, and sealed memory
   files for the keymap. Effects only; the protocol, every object and every decision is Coax.
   See wayland-native.h. */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "wayland-native.h"
#include <dirent.h>
#include <errno.h>
#include <stdio.h>
#include <sys/wait.h>
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
#include "wayland-gpu.h"

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
/* The desk socket (os/compositor.coax --desk-socket): the desk protocol's clients, one JSON line a
   request (os/desk.coax is one). Bytes only; the protocol is the compositor's. */
#define WL_DESKS 8
static int desk_lis = -1;
static int desk_fd[WL_DESKS];
static int desk_used[WL_DESKS];
static int xwm_fd = -1, x_pending = -1, x_num = -1, x_lis = -1, x_slot = -1;   /* Xwayland: the window-manager end, its Wayland client before accept, its display */
static char x_display[16];
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
int64_t cxwl_desk_listen(coax_v_u8 path) {
  struct sockaddr_un a; memset(&a, 0, sizeof a); a.sun_family = AF_UNIX;
  if (path.count <= 0 || (size_t)path.count >= sizeof a.sun_path) return -ENAMETOOLONG;
  memcpy(a.sun_path, path.data, (size_t)path.count);
  unlink(a.sun_path);
  int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
  if (fd < 0) return -(int64_t)errno;
  if (bind(fd, (struct sockaddr *)&a, sizeof a) < 0 || listen(fd, 8) < 0) { int e = errno; close(fd); return -(int64_t)e; }
  chmod(a.sun_path, 0600);
  desk_lis = fd;
  return 0;
}
/* A desk client's handle, or -1 when none is waiting. */
int64_t cxwl_desk_accept(void) {
  if (desk_lis < 0) return -1;
  int fd = accept4(desk_lis, NULL, NULL, SOCK_CLOEXEC | SOCK_NONBLOCK);
  if (fd < 0) return -1;
  for (int i = 0; i < WL_DESKS; i++) if (!desk_used[i]) { desk_used[i] = 1; desk_fd[i] = fd; return i; }
  close(fd);
  return -1;
}
/* What a desk client sent: bytes into out, 0 when nothing is ready, -1 at its end. */
int64_t cxwl_desk_recv(int64_t h, coax_v_u8 out) {
  if (h < 0 || h >= WL_DESKS || !desk_used[h]) return -1;
  ssize_t n = read(desk_fd[h], out.data, (size_t)out.count);
  if (n > 0) return n;
  if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR)) return 0;
  return -1;
}
/* An answer line to a desk client: 0, or -1 when it is gone. */
int64_t cxwl_desk_send(int64_t h, coax_v_u8 s) {
  if (h < 0 || h >= WL_DESKS || !desk_used[h]) return -1;
  size_t done = 0; int64_t deadline = cxwl_ms() + 100;
  while (done < (size_t)s.count) {
    ssize_t w = send(desk_fd[h], s.data + done, (size_t)s.count - done, MSG_NOSIGNAL);
    if (w < 0 && (errno == EAGAIN || errno == EINTR)) {
      if (cxwl_ms() >= deadline) return -1;
      struct pollfd q = {desk_fd[h], POLLOUT, 0}; poll(&q, 1, 5); continue;
    }
    if (w <= 0) return -1;
    done += (size_t)w;
  }
  return 0;
}
int64_t cxwl_desk_close(int64_t h) {
  if (h < 0 || h >= WL_DESKS || !desk_used[h]) return -1;
  close(desk_fd[h]); desk_used[h] = 0;
  return 0;
}
int64_t cxwl_unlisten(coax_v_u8 path) {
  if (lis >= 0) close(lis);
  lis = -1;
  char p[108]; if (path.count > 0 && path.count < 108) { memcpy(p, path.data, (size_t)path.count); p[path.count] = 0; unlink(p); }
  return 0;
}
int64_t cxwl_accept(void) {
  if (x_pending >= 0) { int64_t h = x_pending; x_pending = -1; return h; }   /* Xwayland's own connection */
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
  struct pollfd p[WL_CLIENTS + WL_DESKS + 4]; int n = 0;
  if (lis >= 0) { p[n].fd = lis; p[n].events = POLLIN; p[n].revents = 0; n++; }
  if (watch_stdin) { p[n].fd = 0; p[n].events = POLLIN; p[n].revents = 0; n++; }
  if (desk_lis >= 0) { p[n].fd = desk_lis; p[n].events = POLLIN; p[n].revents = 0; n++; }
  for (int i = 0; i < WL_DESKS; i++) if (desk_used[i]) { p[n].fd = desk_fd[i]; p[n].events = POLLIN; p[n].revents = 0; n++; }
  if (xwm_fd >= 0) { p[n].fd = xwm_fd; p[n].events = POLLIN; p[n].revents = 0; n++; }
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
   -errno. It runs as init's child, so the compositor never waits for it. */
int64_t cxwl_spawn(coax_v_u8 path, coax_v_u8 display) {
  char p[1024], d[1024];
  if (path.count <= 0 || path.count >= 1024 || display.count <= 0 || display.count >= 1024) return -ENAMETOOLONG;
  memcpy(p, path.data, (size_t)path.count); p[path.count] = 0;
  memcpy(d, display.data, (size_t)display.count); d[display.count] = 0;
  if (access(p, X_OK) != 0) return -(int64_t)errno;
  int link[2]; if (pipe(link) < 0) return -(int64_t)errno;
  pid_t pid = fork();
  if (pid < 0) { close(link[0]); close(link[1]); return -(int64_t)errno; }
  if (pid == 0) {   // a second fork, so the program belongs to init and nothing here waits for it
    close(link[0]);
    pid_t g = fork();
    if (g == 0) {
      close(link[1]); setsid(); setenv("WAYLAND_DISPLAY", d, 1); if (x_display[0]) setenv("DISPLAY", x_display, 1); else unsetenv("DISPLAY");
      int nul = open("/dev/null", O_RDWR); if (nul >= 0) { dup2(nul, 0); }
      execl(p, p, (char *)NULL); _exit(127);
    }
    int64_t v = g; if (write(link[1], &v, sizeof v) < 0) {} _exit(0);
  }
  close(link[1]);
  int64_t g = -1; if (read(link[0], &g, sizeof g) != (ssize_t)sizeof g) g = -1;
  close(link[0]); waitpid(pid, NULL, 0);
  return g > 0 ? g : -ECHILD;
}
/* Whether a pool's file still holds end bytes (a client may shrink it under the map): checked once
   per surface draw, so the row reads can pass fd -1 and skip the check. */
int64_t cxwl_fits(int64_t m, int64_t fd, int64_t end) {
  if (m < 0 || m >= WL_MAPS || !map_used[m] || end < 0 || (size_t)end > map_len[m]) return 0;
  struct stat st;
  return (fd >= 0 && fstat((int)fd, &st) == 0 && st.st_size >= end) ? 1 : 0;
}
/* The pid of the process at the other end of client h (SO_PEERCRED), or -1. */
int64_t cxwl_peer_pid(int64_t h) {
#if defined(__linux__)
  if (h < 0 || h >= WL_CLIENTS || !cl_used[h]) return -1;
  struct ucred c; socklen_t n = sizeof c;
  if (getsockopt(cl_fd[h], SOL_SOCKET, SO_PEERCRED, &c, &n) < 0) return -1;
  return c.pid;
#else
  (void)h; return -1;
#endif
}
/* pid and every process under it, from /proc: the count, read by cxwl_tree_at(i). */
#define WL_TREE 512
static int64_t tree[WL_TREE]; static int tree_n;
int64_t cxwl_tree(int64_t pid) {
  tree_n = 0; if (pid <= 0) return 0;
  tree[tree_n++] = pid;
  DIR *d = opendir("/proc"); if (!d) return tree_n;
  int64_t pids[4096], par[4096]; int n = 0; struct dirent *e;
  while ((e = readdir(d)) && n < 4096) {
    char *end; long p = strtol(e->d_name, &end, 10); if (*end || p <= 0) continue;
    char path[64], buf[512]; snprintf(path, sizeof path, "/proc/%ld/stat", p);
    int fd = open(path, O_RDONLY | O_CLOEXEC); if (fd < 0) continue;
    ssize_t k = read(fd, buf, sizeof buf - 1); close(fd); if (k <= 0) continue; buf[k] = 0;
    char *rp = strrchr(buf, ')'); if (!rp) continue;
    long pp = 0; char st; if (sscanf(rp + 1, " %c %ld", &st, &pp) != 2) continue;
    pids[n] = p; par[n] = pp; n++;
  }
  closedir(d);
  for (int grew = 1; grew;) {
    grew = 0;
    for (int i = 0; i < n && tree_n < WL_TREE; i++) {
      int in = 0, under = 0;
      for (int j = 0; j < tree_n; j++) { if (tree[j] == pids[i]) in = 1; if (tree[j] == par[i]) under = 1; }
      if (!in && under) { tree[tree_n++] = pids[i]; grew = 1; }
    }
  }
  return tree_n;
}
int64_t cxwl_tree_at(int64_t i) { return (i >= 0 && i < tree_n) ? tree[i] : -1; }
/* One question to a program at a time, without blocking: cxwl_ask_start(args) runs it (args are
   NUL-separated; the first is looked up on PATH) with its output on a pipe; cxwl_ask_poll(out)
   says -1 while it runs, else its exit status (-2 when it could not start), with what it printed
   in out (cxwl_ask_len bytes). */
static pid_t ask_pid = -1; static int ask_fd = -1; static char ask_buf[4096]; static int ask_len;
int64_t cxwl_ask_start(coax_v_u8 args) {
  if (ask_pid > 0) return -EBUSY;
  char buf[8192]; char *argv[130]; int argc = 0;
  if (args.count <= 0 || args.count >= (int64_t)sizeof buf) return -EINVAL;
  memcpy(buf, args.data, (size_t)args.count); buf[args.count] = 0;
  for (int64_t i = 0; i < args.count && argc < 129;) { argv[argc++] = buf + i; i += (int64_t)strlen(buf + i) + 1; }
  argv[argc] = NULL;
  int p[2]; if (pipe(p) < 0) return -(int64_t)errno;
  pid_t pid = fork();
  if (pid < 0) { close(p[0]); close(p[1]); return -(int64_t)errno; }
  if (pid == 0) {
    signal(SIGCHLD, SIG_DFL);
    dup2(p[1], 1); int nul = open("/dev/null", O_RDWR); if (nul >= 0) { dup2(nul, 0); dup2(nul, 2); }
    close(p[0]); close(p[1]); execvp(argv[0], argv); _exit(127);
  }
  close(p[1]); fcntl(p[0], F_SETFL, fcntl(p[0], F_GETFL) | O_NONBLOCK);
  ask_pid = pid; ask_fd = p[0]; ask_len = 0;
  return 0;
}
int64_t cxwl_ask_poll(coax_v_u8 out) {
  if (ask_pid <= 0) return -2;
  for (;;) { ssize_t n = read(ask_fd, ask_buf + ask_len, sizeof ask_buf - 1 - (size_t)ask_len); if (n <= 0) break; ask_len += (int)n; if (ask_len >= (int)sizeof ask_buf - 1) break; }
  int st; pid_t r = waitpid(ask_pid, &st, WNOHANG);
  if (r == 0) return -1;
  for (;;) { ssize_t n = read(ask_fd, ask_buf + ask_len, sizeof ask_buf - 1 - (size_t)ask_len); if (n <= 0) break; ask_len += (int)n; }
  close(ask_fd); ask_fd = -1; ask_pid = -1;
  int64_t k = ask_len < out.count ? ask_len : out.count; memcpy(out.data, ask_buf, (size_t)k); ask_len = (int)k;
  if (r < 0) return -2;
  return WIFEXITED(st) ? WEXITSTATUS(st) : 128;
}
int64_t cxwl_ask_len(void) { return ask_len; }
/* Runs args (NUL-separated; the first looked up on PATH) detached, as a client of display, and
   forgets it: 0, or -errno. */
int64_t cxwl_run(coax_v_u8 args, coax_v_u8 display) {
  char buf[8192], d[1024]; char *argv[130]; int argc = 0;
  if (args.count <= 0 || args.count >= (int64_t)sizeof buf || display.count <= 0 || display.count >= 1024) return -EINVAL;
  memcpy(buf, args.data, (size_t)args.count); buf[args.count] = 0;
  memcpy(d, display.data, (size_t)display.count); d[display.count] = 0;
  for (int64_t i = 0; i < args.count && argc < 129;) { argv[argc++] = buf + i; i += (int64_t)strlen(buf + i) + 1; }
  argv[argc] = NULL;
  pid_t pid = fork();
  if (pid < 0) return -(int64_t)errno;
  if (pid == 0) {
    if (fork() == 0) {
      setsid(); setenv("WAYLAND_DISPLAY", d, 1); if (x_display[0]) setenv("DISPLAY", x_display, 1); else unsetenv("DISPLAY");
      int nul = open("/dev/null", O_RDWR); if (nul >= 0) { dup2(nul, 0); dup2(nul, 1); dup2(nul, 2); }
      execvp(argv[0], argv); _exit(127);
    }
    _exit(0);
  }
  waitpid(pid, NULL, 0);
  return 0;
}

/* Xwayland, rootless, as a client of this compositor: a free display :N from 50 on (its lock file and
   /tmp/.X11-unix socket made here), its Wayland connection a socketpair the next cxwl_accept returns,
   and its window-manager connection the other socketpair, read and written by cxx_recv and cxx_send.
   Programs started after this get DISPLAY=:N. Returns N, or -errno. */
int64_t cxwl_xwayland(void) {
  if (xwm_fd >= 0) return -EBUSY;
  char lock[64], path[64]; int n = -1, lf = -1;
  for (int i = 50; i < 100 && n < 0; i++) {
    snprintf(lock, sizeof lock, "/tmp/.X%d-lock", i);
    lf = open(lock, O_WRONLY | O_CREAT | O_EXCL, 0444);
    if (lf >= 0) n = i;
  }
  if (n < 0) return -EADDRINUSE;
  char pidtxt[16]; snprintf(pidtxt, sizeof pidtxt, "%10d\n", (int)getpid()); if (write(lf, pidtxt, 11) < 0) {} close(lf);
  mkdir("/tmp/.X11-unix", 01777);
  snprintf(path, sizeof path, "/tmp/.X11-unix/X%d", n); unlink(path);
  int l = socket(AF_UNIX, SOCK_STREAM, 0);
  struct sockaddr_un a; memset(&a, 0, sizeof a); a.sun_family = AF_UNIX; strncpy(a.sun_path, path, sizeof a.sun_path - 1);
  if (l < 0 || bind(l, (struct sockaddr *)&a, sizeof a) < 0 || listen(l, 16) < 0) { int e = errno; if (l >= 0) close(l); unlink(lock); return -(int64_t)e; }
  int wl[2], wm[2];
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, wl) < 0) { int e = errno; close(l); unlink(path); unlink(lock); return -(int64_t)e; }
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, wm) < 0) { int e = errno; close(l); close(wl[0]); close(wl[1]); unlink(path); unlink(lock); return -(int64_t)e; }
  int slot = -1; for (int i = 0; i < WL_CLIENTS; i++) if (!cl_used[i]) { slot = i; break; }
  if (slot < 0) { close(l); close(wl[0]); close(wl[1]); close(wm[0]); close(wm[1]); unlink(path); unlink(lock); return -EMFILE; }
  pid_t pid = fork();
  if (pid < 0) { int e = errno; close(l); close(wl[0]); close(wl[1]); close(wm[0]); close(wm[1]); unlink(path); unlink(lock); return -(int64_t)e; }
  if (pid == 0) {
    if (fork() == 0) {
      setsid(); close(wl[0]); close(wm[0]);
      fcntl(wl[1], F_SETFD, 0); fcntl(wm[1], F_SETFD, 0); fcntl(l, F_SETFD, 0);
      char ws[16], wf[16], lf2[16], disp[16];
      snprintf(ws, sizeof ws, "%d", wl[1]); snprintf(wf, sizeof wf, "%d", wm[1]); snprintf(lf2, sizeof lf2, "%d", l); snprintf(disp, sizeof disp, ":%d", n);
      setenv("WAYLAND_SOCKET", ws, 1); unsetenv("WAYLAND_DISPLAY"); unsetenv("DISPLAY");
      int nul = open("/dev/null", O_RDWR); if (nul >= 0) { dup2(nul, 0); }
      if (cxgpu_device() >= 0)
        execlp("Xwayland", "Xwayland", disp, "-rootless", "-core", "-listenfd", lf2, "-wm", wf, (char *)NULL);
      else
        execlp("Xwayland", "Xwayland", disp, "-rootless", "-core", "-shm", "-listenfd", lf2, "-wm", wf, (char *)NULL);
      _exit(127);
    }
    _exit(0);
  }
  waitpid(pid, NULL, 0);
  close(wl[1]); close(wm[1]); x_lis = l;
  fcntl(wl[0], F_SETFL, fcntl(wl[0], F_GETFL) | O_NONBLOCK); fcntl(wl[0], F_SETFD, FD_CLOEXEC);
  cl_used[slot] = 1; cl_fd[slot] = wl[0]; x_pending = slot; x_slot = slot;
  xwm_fd = wm[0]; fcntl(xwm_fd, F_SETFL, fcntl(xwm_fd, F_GETFL) | O_NONBLOCK); fcntl(xwm_fd, F_SETFD, FD_CLOEXEC);
  x_num = n; snprintf(x_display, sizeof x_display, ":%d", n);
  return n;
}
/* The client handle Xwayland's Wayland connection has, or -1. */
int64_t cxwl_xwayland_handle(void) { return x_slot; }
/* Writes all of b to the window-manager connection: 0, or -1 when it is gone. */
int64_t cxx_send(coax_v_u8 b) {
  if (xwm_fd < 0) return -1;
  int64_t at = 0;
  while (at < b.count) {
    ssize_t k = write(xwm_fd, b.data + at, (size_t)(b.count - at));
    if (k < 0 && (errno == EAGAIN || errno == EINTR)) { struct pollfd q = { xwm_fd, POLLOUT, 0 }; poll(&q, 1, 100); continue; }
    if (k <= 0) return -1;
    at += k;
  }
  return 0;
}
/* What the X server sent: the byte count into out, 0 when nothing waits, -1 at the end. */
int64_t cxx_recv(coax_v_u8 out) {
  if (xwm_fd < 0) return -1;
  ssize_t k = read(xwm_fd, out.data, (size_t)out.count);
  if (k < 0) return (errno == EAGAIN || errno == EINTR) ? 0 : -1;
  return k == 0 ? -1 : k;
}
/* Stops using Xwayland: closes the connection and removes the display's socket and lock. */
int64_t cxwl_xwayland_stop(void) {
  if (x_num < 0) return 0;
  char p[64];
  if (xwm_fd >= 0) close(xwm_fd);
  if (x_lis >= 0) close(x_lis);
  snprintf(p, sizeof p, "/tmp/.X11-unix/X%d", x_num); unlink(p);
  snprintf(p, sizeof p, "/tmp/.X%d-lock", x_num); unlink(p);
  xwm_fd = -1; x_lis = -1; x_num = -1; x_slot = -1; x_display[0] = 0;
  return 0;
}
