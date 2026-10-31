/* OS effects only: spawn, pipes, lines, signals, reaping. What to run and
   how to talk to it is Coax (coax/lib/agent/claude/). */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "process-native.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#if defined(__linux__)
#include <sys/prctl.h>
#endif
extern char **environ;

typedef struct { char *data; size_t count, cap; } Bytes;
typedef struct { Bytes *items; size_t count, cap; Bytes open; } List;

typedef struct {
  int used, started, reaped, status, quiet;
  pid_t pid;
  int in, out, err;
  List args, env, unset;
  Bytes cwd;
  Bytes buf;           /* stdout bytes read, not yet returned */
  size_t line_start, line_len, scan;
  int eof, skipping;
  Bytes wbuf;
  char keep[CXPROC_STDERR_KEEP];
  int64_t kept;        /* total stderr bytes seen; the ring holds the last ones */
} Proc;

static Proc procs[CXPROC_MAX];

static int put(Bytes *b, char c) {
  if (b->count == b->cap) {
    size_t cap = b->cap ? b->cap * 2 : 64;
    char *d = realloc(b->data, cap);
    if (!d) return -1;
    b->data = d; b->cap = cap;
  }
  b->data[b->count++] = c;
  return 0;
}
static char *cstr(Bytes *b) { if (put(b, 0)) return NULL; b->count--; return b->data; }
static void drop(Bytes *b) { free(b->data); b->data = NULL; b->count = b->cap = 0; }
static int close_entry(List *l) {
  if (l->count == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 8;
    Bytes *d = realloc(l->items, cap * sizeof(Bytes));
    if (!d) return -1;
    l->items = d; l->cap = cap;
  }
  if (!cstr(&l->open)) return -1;
  l->items[l->count++] = l->open;
  memset(&l->open, 0, sizeof l->open);
  return 0;
}
static void drop_list(List *l) {
  for (size_t i = 0; i < l->count; i++) drop(&l->items[i]);
  free(l->items); drop(&l->open);
  memset(l, 0, sizeof *l);
}
static Proc *get(int64_t h) {
  if (h < 0 || h >= CXPROC_MAX || !procs[h].used) return NULL;
  return &procs[h];
}

int64_t cxproc_now_ms(void) {
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}

int64_t cxproc_new(void) {
  for (int i = 0; i < CXPROC_MAX; i++) if (!procs[i].used) {
    memset(&procs[i], 0, sizeof procs[i]);
    procs[i].used = 1; procs[i].in = procs[i].out = procs[i].err = -1;
    return i;
  }
  return -1;
}

int64_t cxproc_quiet(int64_t h) {
  Proc *p = get(h);
  if (!p || p->started) return -1;
  p->quiet = 1;
  return 0;
}

int64_t cxproc_sleep_ms(int64_t ms) {
  if (ms < 0 || ms > 86400000) return -1;
  struct timespec t = {(time_t)(ms / 1000), (long)(ms % 1000) * 1000000}, left;
  while (nanosleep(&t, &left) != 0) { if (errno != EINTR) return -1; t = left; }
  return 0;
}

int64_t cxproc_push(int64_t h, int64_t field, int64_t byte) {
  Proc *p = get(h);
  if (!p || p->started || byte <= 0 || byte > 255) return -1;
  Bytes *b = field == 0 ? &p->args.open : field == 1 ? &p->env.open
           : field == 2 ? &p->unset.open : field == 3 ? &p->cwd : NULL;
  return b ? put(b, (char)byte) : -1;
}

int64_t cxproc_end(int64_t h, int64_t field) {
  Proc *p = get(h);
  if (!p || p->started) return -1;
  if (field == 0) return close_entry(&p->args);
  if (field == 1) return close_entry(&p->env);
  if (field == 2) return close_entry(&p->unset);
  return field == 3 ? 0 : -1;
}

static int exists(const char *path) { return access(path, X_OK) == 0; }

/* The program's path: as given when it names a file, else the first PATH
   entry that holds it, else (for claude only) where installers put it. */
static char *resolve(const char *name, char *out, size_t cap) {
  if (strchr(name, '/')) { snprintf(out, cap, "%s", name); return out; }
  const char *path = getenv("PATH");
  while (path && *path) {
    const char *end = strchr(path, ':');
    size_t n = end ? (size_t)(end - path) : strlen(path);
    if (n && snprintf(out, cap, "%.*s/%s", (int)n, path, name) < (int)cap && exists(out)) return out;
    path = end ? end + 1 : NULL;
  }
  if (strcmp(name, "claude") == 0) {
    const char *home = getenv("HOME");
    const char *rel[] = {"/.local/bin/claude", "/.claude/local/claude"};
    for (int i = 0; home && i < 2; i++)
      if (snprintf(out, cap, "%s%s", home, rel[i]) < (int)cap && exists(out)) return out;
    const char *abs[] = {"/usr/local/bin/claude", "/opt/homebrew/bin/claude"};
    for (int i = 0; i < 2; i++) if (exists(abs[i])) { snprintf(out, cap, "%s", abs[i]); return out; }
  }
  return NULL;
}

int64_t cxproc_spawn(int64_t h) {
  Proc *p = get(h);
  if (!p || p->started || p->args.count == 0) return EINVAL;
  char path[4096];
  if (!resolve(p->args.items[0].data, path, sizeof path)) return ENOENT;
  char **argv = calloc(p->args.count + 1, sizeof *argv);
  if (!argv) return ENOMEM;
  for (size_t i = 0; i < p->args.count; i++) argv[i] = p->args.items[i].data;
  char *cwd = p->cwd.count ? cstr(&p->cwd) : NULL;
  int in[2], out[2], err[2];
  if (pipe(in)) { free(argv); return errno; }
  if (pipe(out)) { int e = errno; close(in[0]); close(in[1]); free(argv); return e; }
  if (pipe(err)) { int e = errno; close(in[0]); close(in[1]); close(out[0]); close(out[1]); free(argv); return e; }
  /* A child that exits early must not kill the host with SIGPIPE. */
  signal(SIGPIPE, SIG_IGN);
  int report[2];
  if (pipe(report)) { int e = errno; free(argv); close(in[0]); close(in[1]); close(out[0]); close(out[1]); close(err[0]); close(err[1]); return e; }
  fcntl(report[1], F_SETFD, FD_CLOEXEC);
  pid_t pid = fork();
  if (pid < 0) {
    int e = errno; free(argv);
    int fds[] = {in[0], in[1], out[0], out[1], err[0], err[1], report[0], report[1]};
    for (int i = 0; i < 8; i++) close(fds[i]);
    return e;
  }
  if (pid == 0) {
    setpgid(0, 0);
#if defined(__linux__)
    prctl(PR_SET_PDEATHSIG, SIGKILL);
#endif
    signal(SIGPIPE, SIG_DFL);
    dup2(in[0], 0); dup2(out[1], 1); dup2(err[1], 2);
    if (p->quiet) {
      int null = open("/dev/null", O_WRONLY);
      if (null >= 0) { dup2(null, 1); dup2(null, 2); if (null > 2) close(null); }
    }
    close(in[0]); close(in[1]); close(out[0]); close(out[1]); close(err[0]); close(err[1]);
    close(report[0]);
    for (size_t i = 0; i < p->unset.count; i++) unsetenv(p->unset.items[i].data);
    for (size_t i = 0; i < p->env.count; i++) {
      char *kv = p->env.items[i].data, *eq = strchr(kv, '=');
      if (eq && eq != kv) { *eq = 0; setenv(kv, eq + 1, 1); }
    }
    int e = 0;
    if (cwd && chdir(cwd)) e = errno;
    if (!e) { execve(path, argv, environ); e = errno; }
    ssize_t w = write(report[1], &e, sizeof e); (void)w;
    _exit(127);
  }
  free(argv);
  setpgid(pid, pid); /* also from the parent, so the group exists before any kill */
  close(in[0]); close(out[1]); close(err[1]); close(report[1]);
  int e = 0;
  ssize_t got;
  do got = read(report[0], &e, sizeof e); while (got < 0 && errno == EINTR);
  close(report[0]);
  p->started = 1; p->pid = pid; p->in = in[1]; p->out = out[0]; p->err = err[0];
  fcntl(p->in, F_SETFD, FD_CLOEXEC); fcntl(p->out, F_SETFD, FD_CLOEXEC); fcntl(p->err, F_SETFD, FD_CLOEXEC);
  fcntl(p->err, F_SETFL, fcntl(p->err, F_GETFL) | O_NONBLOCK);
#if defined(F_SETNOSIGPIPE)
  fcntl(p->in, F_SETNOSIGPIPE, 1);
#endif
  if (got == (ssize_t)sizeof e && e) {
    waitpid(pid, &p->status, 0); p->reaped = 1;
    return e;
  }
  return 0;
}

static void keep_stderr(Proc *p, const char *d, ssize_t n) {
  for (ssize_t i = 0; i < n; i++) p->keep[(p->kept + i) % CXPROC_STDERR_KEEP] = d[i];
  p->kept += n;
}
static void drain_stderr(Proc *p) {
  char d[4096];
  for (;;) {
    ssize_t n = read(p->err, d, sizeof d);
    if (n > 0) { keep_stderr(p, d, n); continue; }
    if (n < 0 && errno == EINTR) continue;
    if (n == 0) { close(p->err); p->err = -1; }
    return;
  }
}

/* A complete line in buf from scan on, or 0. */
static int find_line(Proc *p) {
  char *nl = p->scan < p->buf.count ? memchr(p->buf.data + p->scan, '\n', p->buf.count - p->scan) : NULL;
  /* No newline yet: nothing is consumed (scan marks what the next call forgets,
     so a partial line read before a timeout must stay in the buffer). */
  if (!nl) return 0;
  size_t end = (size_t)(nl - p->buf.data);
  p->line_start = 0; p->line_len = end;
  if (p->line_len && p->buf.data[p->line_len - 1] == '\r') p->line_len--;
  p->scan = end + 1;
  return 1;
}

int64_t cxproc_read_line(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  if (!p || !p->started) return CXPROC_ERROR;
  /* Forget the line returned last time. */
  if (p->scan) {
    memmove(p->buf.data, p->buf.data + p->scan, p->buf.count - p->scan);
    p->buf.count -= p->scan; p->scan = 0;
  }
  int64_t deadline = timeout_ms < 0 ? -1 : cxproc_now_ms() + timeout_ms;
  int polled = 0; /* a 0 timeout still takes what the pipe already holds */
  for (;;) {
    if (find_line(p)) {
      if (p->skipping) {
        p->skipping = 0;
        memmove(p->buf.data, p->buf.data + p->scan, p->buf.count - p->scan);
        p->buf.count -= p->scan; p->scan = 0;
        return CXPROC_TOO_LONG;
      }
      return (int64_t)p->line_len;
    }
    if (p->buf.count > CXPROC_MAX_LINE) { p->skipping = 1; p->buf.count = 0; p->scan = 0; }
    if (p->eof) {
      if (p->buf.count && !p->skipping) { /* a last line with no newline */
        p->line_start = 0; p->line_len = p->buf.count; p->scan = p->buf.count;
        return (int64_t)p->line_len;
      }
      return CXPROC_EOF;
    }
    struct pollfd fds[2] = {{p->out, POLLIN, 0}, {p->err, POLLIN, 0}};
    int wait = -1;
    if (deadline >= 0) {
      int64_t left = deadline - cxproc_now_ms();
      if (left <= 0 && polled) return CXPROC_TIMEOUT;
      wait = left <= 0 ? 0 : left > 1000000 ? 1000000 : (int)left;
    }
    polled = 1;
    int r = poll(fds, p->err >= 0 ? 2 : 1, wait);
    if (r < 0) { if (errno == EINTR) continue; return CXPROC_ERROR; }
    if (r == 0) { if (wait == 0) return CXPROC_TIMEOUT; continue; }
    if (p->err >= 0 && (fds[1].revents & (POLLIN | POLLHUP | POLLERR))) drain_stderr(p);
    if (fds[0].revents & (POLLIN | POLLHUP | POLLERR)) {
      char d[65536];
      ssize_t n = read(p->out, d, sizeof d);
      if (n < 0) { if (errno == EINTR) continue; return CXPROC_ERROR; }
      if (n == 0) { p->eof = 1; continue; }
      for (ssize_t i = 0; i < n; i++) if (put(&p->buf, d[i])) return CXPROC_ERROR;
    }
  }
}

int64_t cxproc_line_byte(int64_t h, int64_t i) {
  Proc *p = get(h);
  if (!p || i < 0 || (size_t)i >= p->line_len) return -1;
  return (unsigned char)p->buf.data[p->line_start + (size_t)i];
}

int64_t cxproc_write(int64_t h, int64_t byte) {
  Proc *p = get(h);
  if (!p || p->in < 0 || byte < 0 || byte > 255) return -1;
  return put(&p->wbuf, (char)byte);
}

int64_t cxproc_flush(int64_t h) {
  Proc *p = get(h);
  if (!p || p->in < 0) return -1;
  size_t done = 0;
  while (done < p->wbuf.count) {
    ssize_t n = write(p->in, p->wbuf.data + done, p->wbuf.count - done);
    if (n < 0) { if (errno == EINTR) continue; p->wbuf.count = 0; return -1; }
    done += (size_t)n;
  }
  p->wbuf.count = 0;
  return 0;
}

/* A bounded write to this handle's owned child-stdin pipe. Unlike the
   legacy flush, a retry keeps only the suffix that has not been sent. */
int64_t cxproc_flush_timeout(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  struct stat info;
  if (!p || !p->started || p->in < 3 || timeout_ms < 0 ||
      timeout_ms > INT_MAX || fstat(p->in, &info) || !S_ISFIFO(info.st_mode))
    return CXPROC_ERROR;
  int flags = fcntl(p->in, F_GETFL);
  if (flags < 0 || (flags & O_ACCMODE) != O_WRONLY ||
      fcntl(p->in, F_SETFL, flags | O_NONBLOCK) < 0)
    return CXPROC_ERROR;
  int64_t start = cxproc_now_ms(), result = 0;
  size_t done = 0;
  if (start < 0 || start > INT64_MAX - timeout_ms) result = CXPROC_ERROR;
  int64_t deadline = result ? 0 : start + timeout_ms;
  int attempted = 0;
  while (!result && done < p->wbuf.count) {
    if (attempted && cxproc_now_ms() >= deadline) {
      result = CXPROC_TIMEOUT;
      break;
    }
    attempted = 1; /* timeout 0 permits one nonblocking attempt */
    size_t count = p->wbuf.count - done;
    if (count > 65536) count = 65536;
    ssize_t n = write(p->in, p->wbuf.data + done, count);
    if (n > 0) { done += (size_t)n; continue; }
    if (!n) { result = CXPROC_ERROR; break; }
    if (errno == EINTR) continue;
    if (errno != EAGAIN && errno != EWOULDBLOCK) {
      result = CXPROC_ERROR;
      break;
    }
    int64_t left = deadline - cxproc_now_ms();
    if (left <= 0) { result = CXPROC_TIMEOUT; break; }
    struct pollfd fds[2] = {{p->in, POLLOUT, 0}, {p->err, POLLIN, 0}};
    int ready = poll(fds, p->err >= 0 ? 2 : 1, (int)left);
    if (ready < 0) {
      if (errno == EINTR) continue;
      result = CXPROC_ERROR;
      break;
    }
    if (!ready) { result = CXPROC_TIMEOUT; break; }
    if (fds[0].revents & (POLLERR | POLLHUP | POLLNVAL)) {
      result = CXPROC_ERROR;
      break;
    }
    /* One bounded stderr read per turn: a noisy child cannot deadlock on
       its stderr pipe or extend this operation's total deadline. */
    if (p->err >= 0 &&
        (fds[1].revents & (POLLIN | POLLHUP | POLLERR))) {
      char bytes[4096];
      ssize_t got = read(p->err, bytes, sizeof bytes);
      if (got > 0) keep_stderr(p, bytes, got);
      else if (!got) { close(p->err); p->err = -1; }
      else if (errno != EINTR && errno != EAGAIN && errno != EWOULDBLOCK) {
        result = CXPROC_ERROR;
        break;
      }
    }
  }
  if (done) {
    size_t remain = p->wbuf.count - done;
    memmove(p->wbuf.data, p->wbuf.data + done, remain);
    volatile unsigned char *retired = (volatile unsigned char *)p->wbuf.data;
    for (size_t i = remain; i < p->wbuf.count; i++) retired[i] = 0;
    p->wbuf.count = remain;
  }
  if (fcntl(p->in, F_SETFL, flags) < 0) result = CXPROC_ERROR;
  return result;
}

int64_t cxproc_close_stdin(int64_t h) {
  Proc *p = get(h);
  if (!p) return -1;
  if (p->in >= 0) { close(p->in); p->in = -1; }
  return 0;
}

int64_t cxproc_kill(int64_t h, int64_t sig) {
  Proc *p = get(h);
  if (!p || !p->started) return -1;
  /* The group outlives a reaped leader while its other members run. */
  if (kill(-p->pid, (int)sig) == 0) return 0;
  return errno == ESRCH ? 0 : -1;
}

int64_t cxproc_wait(int64_t h, int64_t timeout_ms) {
  Proc *p = get(h);
  if (!p || !p->started) return -1;
  int64_t deadline = timeout_ms < 0 ? -1 : cxproc_now_ms() + timeout_ms;
  while (!p->reaped) {
    pid_t r = waitpid(p->pid, &p->status, WNOHANG);
    if (r == p->pid) { p->reaped = 1; break; }
    if (r < 0 && errno != EINTR) return -1;
    if (deadline >= 0 && cxproc_now_ms() >= deadline) return CXPROC_RUNNING;
    struct timespec t = {0, 5 * 1000000}; nanosleep(&t, NULL);
  }
  if (WIFEXITED(p->status)) return WEXITSTATUS(p->status);
  if (WIFSIGNALED(p->status)) return 128 + WTERMSIG(p->status);
  return -1;
}

int64_t cxproc_pid(int64_t h) { Proc *p = get(h); return p && p->started ? p->pid : -1; }

int64_t cxproc_group_alive(int64_t pgid) {
  if (pgid <= 0) return 0;
  return kill(-(pid_t)pgid, 0) == 0 || errno == EPERM;
}

int64_t cxproc_stderr_count(int64_t h) {
  Proc *p = get(h);
  if (!p) return 0;
  return p->kept < CXPROC_STDERR_KEEP ? p->kept : CXPROC_STDERR_KEEP;
}

int64_t cxproc_stderr_byte(int64_t h, int64_t i) {
  Proc *p = get(h);
  if (!p) return -1;
  int64_t n = cxproc_stderr_count(h);
  if (i < 0 || i >= n) return -1;
  return (unsigned char)p->keep[(p->kept - n + i) % CXPROC_STDERR_KEEP];
}

int64_t cxproc_release(int64_t h) {
  Proc *p = get(h);
  if (!p) return -1;
  if (p->started) {
    kill(-p->pid, SIGKILL);
    if (!p->reaped) { waitpid(p->pid, &p->status, 0); p->reaped = 1; }
  }
  if (p->in >= 0) close(p->in);
  if (p->out >= 0) close(p->out);
  if (p->err >= 0) close(p->err);
  drop_list(&p->args); drop_list(&p->env); drop_list(&p->unset);
  drop(&p->cwd); drop(&p->buf); drop(&p->wbuf);
  memset(p, 0, sizeof *p);
  return 0;
}

static Bytes env_name;
static const char *env_value;
int64_t cxproc_getenv_push(int64_t byte) {
  if (byte <= 0 || byte > 255) return -1;
  return put(&env_name, (char)byte);
}
int64_t cxproc_getenv(void) {
  char *name = cstr(&env_name);
  env_value = name ? getenv(name) : NULL;
  env_name.count = 0;
  return env_value ? (int64_t)strlen(env_value) : -1;
}
int64_t cxproc_getenv_byte(int64_t i) {
  if (!env_value || i < 0 || (size_t)i >= strlen(env_value)) return -1;
  return (unsigned char)env_value[i];
}
