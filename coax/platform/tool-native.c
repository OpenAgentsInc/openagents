/* The host of a Coax command-line tool: argv, stderr, the OS and a big
   stack. Effects only; see tool-native.h. Flags, usage text and every
   decision are Coax (coax/lib/sys/args.coax). */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "tool-native.h"
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

static int g_argc;
static char **g_argv;
static int64_t g_status;

int64_t cxargs_count(void) { return g_argc; }
int64_t cxargs_len(int64_t i) {
  if (i < 0 || i >= g_argc) return -1;
  return (int64_t)strlen(g_argv[i]);
}
int64_t cxargs_byte(int64_t i, int64_t j) {
  if (i < 0 || i >= g_argc || j < 0 || (size_t)j >= strlen(g_argv[i])) return -1;
  return (unsigned char)g_argv[i][j];
}

int64_t cxtool_err(int64_t byte) {
  if (byte < 0 || byte > 255) return -1;
  return fputc((int)byte, stderr) == EOF ? -1 : 0;
}
int64_t cxtool_err_flush(void) { return fflush(stderr) == 0 ? 0 : -1; }

/* The terminal on stdin and stdout, for a full-screen tool. */
static struct termios g_term_saved;
static int g_term_raw;
static volatile sig_atomic_t g_term_stop, g_term_winch;
int64_t cxterm_raw(int64_t on) {
  if (on) {
    struct termios t;
    if (tcgetattr(0, &t) != 0) return -1;
    if (!g_term_raw) g_term_saved = t;
    t.c_lflag &= ~(tcflag_t)(ECHO | ICANON);
    t.c_cc[VMIN] = 0; t.c_cc[VTIME] = 0;
    if (tcsetattr(0, TCSANOW, &t) != 0) return -1;
    g_term_raw = 1; return 0;
  }
  if (!g_term_raw) return 0;
  g_term_raw = 0;
  return tcsetattr(0, TCSANOW, &g_term_saved) == 0 ? 0 : -1;
}
/* cxterm_keys(1) is cxterm_raw(1) for a key-driven shell: Ctrl-C, Ctrl-Z,
   Ctrl-S, Ctrl-Q and Ctrl-V arrive as bytes instead of signals or flow
   control, and Enter arrives as CR. cxterm_keys(0) puts back what was there. */
int64_t cxterm_keys(int64_t on) {
  if (!on) return cxterm_raw(0);
  if (cxterm_raw(1) != 0) return -1;
  struct termios t;
  if (tcgetattr(0, &t) != 0) return -1;
  t.c_lflag &= ~(tcflag_t)(ISIG | IEXTEN);
  t.c_iflag &= ~(tcflag_t)(IXON | ICRNL | INLCR);
  return tcsetattr(0, TCSANOW, &t) == 0 ? 0 : -1;
}
int64_t cxterm_size(void) {
  struct winsize w;
  if (ioctl(0, TIOCGWINSZ, &w) != 0 || w.ws_row == 0 || w.ws_col == 0) return -1;
  return (int64_t)w.ws_row * 65536 + w.ws_col;
}
int64_t cxterm_byte(int64_t timeout_ms) {
  struct pollfd f = {0, POLLIN, 0};
  for (;;) {
    int r = poll(&f, 1, timeout_ms < 0 ? -1 : (int)timeout_ms);
    if (r < 0 && errno == EINTR) return -1;
    if (r <= 0) return -1;
    unsigned char c;
    ssize_t n = read(0, &c, 1);
    if (n < 0 && errno == EINTR) return -1;
    return n == 1 ? c : -2;
  }
}
int64_t cxterm_write(coax_v_u8 s) {
  fflush(stdout);
  int64_t done = 0;
  while (done < s.count) {
    ssize_t n = write(1, s.data + done, (size_t)(s.count - done));
    if (n < 0 && errno == EINTR) continue;
    if (n <= 0) return -1;
    done += n;
  }
  return 0;
}
static void term_on_stop(int sig) { g_term_stop = sig; }
static void term_on_winch(int sig) { (void)sig; g_term_winch = 1; }
int64_t cxterm_watch(void) {
  struct sigaction a; memset(&a, 0, sizeof a); sigemptyset(&a.sa_mask);
  a.sa_handler = term_on_stop;
  if (sigaction(SIGINT, &a, 0) || sigaction(SIGTERM, &a, 0) || sigaction(SIGHUP, &a, 0)) return -1;
  a.sa_handler = term_on_winch; a.sa_flags = SA_RESTART;
  return sigaction(SIGWINCH, &a, 0) == 0 ? 0 : -1;
}
int64_t cxterm_stopped(void) { return g_term_stop; }
int64_t cxterm_resized(void) { int r = g_term_winch; g_term_winch = 0; return r; }

int64_t cxtool_os(void) {
#if defined(__APPLE__)
  return 1;
#elif defined(__linux__)
  return 2;
#else
  return 0;
#endif
}

static void *run(void *unused) {
  (void)unused;
  g_status = tool_main();
  return NULL;
}

int main(int argc, char **argv) {
  g_argc = argc; g_argv = argv;
  pthread_attr_t attr;
  pthread_t thread;
  if (pthread_attr_init(&attr) || pthread_attr_setstacksize(&attr, COAX_TOOL_STACK) ||
      pthread_create(&thread, &attr, run, NULL) || pthread_join(thread, NULL)) {
    fputs("coax tool: could not start the program thread\n", stderr);
    return 70;
  }
  fflush(stdout); fflush(stderr);
  return g_status < 0 || g_status > 255 ? 1 : (int)g_status;
}
