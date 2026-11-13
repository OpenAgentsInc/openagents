/* The room host's wait on its sockets (#13275, #13384): one kernel readiness set a waiter, so a wake
   costs what is ready, not every socket there is. poll() over the whole connection table and the
   host's loop round it were about 38% of a full shard's CPU (#13384's profile), paid whether or not a
   socket had anything to say.
   A set is one waiter's: today the host's one loop holds one for every socket; a zone's thread
   (docs/decisions/0007) holds its own for its zone's sockets, with the same five calls.
   The kernel's set is epoll on Linux (the room VM, Cloud Run, the build hosts) and kqueue on macOS
   (a local --dev room). Both are level-triggered here: a socket with bytes still unread, or room to
   write while its output waits, is reported again at the next wait.
     wait_open(w)                 the set; false when the kernel gives none
     wait_add(w, fd, tag)         fd is watched for reading; tag comes back with each of its events
     wait_out(w, fd, tag, on)     and for room to write, while its output waits (on) or not
     wait_for(w, out, cap, us)    up to cap events, waiting at most us microseconds; -1 a failure that
                                  is not a signal
   The wait is to the microsecond (epoll_pwait2, kevent's timespec). A wait in whole milliseconds
   rounds the time to the next beat down, to 0 for its last part: the loop then asked the kernel again
   and again with no wait until the beat came, about 0.7 ms of every 16.7, and an idle room took 5% of a
   core for it (#13384: seven rooms on the VM's two cores). A kernel without epoll_pwait2 (before 5.11,
   or a sandbox's) waits whole milliseconds rounded up instead: late by under one, never spinning.
   A closed descriptor leaves its set by itself (the host closes each socket once; a WebTransport
   session's descriptor is a dup that is never added). */
#ifndef ROOM_WAIT_H
#define ROOM_WAIT_H
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <unistd.h>
typedef struct { int q; } wait_set;
typedef struct { int tag; bool in, out; } wait_event;
#define WAIT_BATCH 256   /* events taken a wait; the rest are still ready at the next */

#if defined(__linux__)
#include <sys/epoll.h>
#include <sys/syscall.h>
#include <time.h>
static bool wait_open(wait_set *w) { w->q = epoll_create1(EPOLL_CLOEXEC); return w->q >= 0; }
static bool wait_ctl(wait_set *w, int op, int fd, int tag, bool out) {
  struct epoll_event e = {.events = EPOLLIN | (out ? EPOLLOUT : 0), .data = {.u64 = (uint64_t)(uint32_t)tag}};
  return epoll_ctl(w->q, op, fd, &e) == 0;
}
static bool wait_add(wait_set *w, int fd, int tag) { return wait_ctl(w, EPOLL_CTL_ADD, fd, tag, false); }
static bool wait_out(wait_set *w, int fd, int tag, bool on) { return wait_ctl(w, EPOLL_CTL_MOD, fd, tag, on); }
static int wait_for(wait_set *w, wait_event *out, int cap, int64_t us) {
  struct epoll_event ev[WAIT_BATCH];
  static bool whole_ms;   /* this kernel has no epoll_pwait2 */
  int n = -1;
  if (us < 0) us = 0;
#ifdef SYS_epoll_pwait2
  if (!whole_ms) {
    struct timespec t = {us / 1000000, (long)(us % 1000000) * 1000L};
    n = (int)syscall(SYS_epoll_pwait2, w->q, ev, cap < WAIT_BATCH ? cap : WAIT_BATCH, &t, NULL, (size_t)0);
    if (n < 0 && errno == ENOSYS) whole_ms = true;
  }
#else
  whole_ms = true;
#endif
  if (whole_ms) n = epoll_wait(w->q, ev, cap < WAIT_BATCH ? cap : WAIT_BATCH, (int)((us + 999) / 1000));
  if (n < 0) return errno == EINTR ? 0 : -1;
  for (int k = 0; k < n; k++)
    out[k] = (wait_event){(int)(int32_t)(uint32_t)ev[k].data.u64, (ev[k].events & (EPOLLIN | EPOLLHUP | EPOLLERR)) != 0, (ev[k].events & EPOLLOUT) != 0};
  return n;
}
#else
#include <sys/types.h>
#include <sys/event.h>
#include <sys/time.h>
#include <fcntl.h>
static bool wait_open(wait_set *w) { w->q = kqueue(); if (w->q >= 0) fcntl(w->q, F_SETFD, FD_CLOEXEC); return w->q >= 0; }
static bool wait_add(wait_set *w, int fd, int tag) {
  struct kevent e; EV_SET(&e, fd, EVFILT_READ, EV_ADD, 0, 0, (void *)(intptr_t)tag);
  return kevent(w->q, &e, 1, NULL, 0, NULL) == 0;
}
static bool wait_out(wait_set *w, int fd, int tag, bool on) {
  struct kevent e; EV_SET(&e, fd, EVFILT_WRITE, on ? EV_ADD : EV_DELETE, 0, 0, (void *)(intptr_t)tag);
  return kevent(w->q, &e, 1, NULL, 0, NULL) == 0 || (!on && errno == ENOENT);
}
static int wait_for(wait_set *w, wait_event *out, int cap, int64_t us) {
  struct kevent ev[WAIT_BATCH];
  if (us < 0) us = 0;
  struct timespec t = {us / 1000000, (long)(us % 1000000) * 1000L};
  int n = kevent(w->q, NULL, 0, ev, cap < WAIT_BATCH ? cap : WAIT_BATCH, &t);
  if (n < 0) return errno == EINTR ? 0 : -1;
  for (int k = 0; k < n; k++)
    out[k] = (wait_event){(int)(intptr_t)ev[k].udata, ev[k].filter == EVFILT_READ, ev[k].filter == EVFILT_WRITE};
  return n;
}
#endif
#endif
