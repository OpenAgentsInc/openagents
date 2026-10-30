/* The chapter-19 Pylon's C host: effects only. It runs the Coax main
   (psionic/lib/pylon/run.coax, compiled with -Dmain=coax_pylon_main) on a
   thread with a 512 MiB stack, hands the arguments over one byte at a time,
   reads the wall clock, and reports a stop request: SIGTERM, SIGINT, or a
   stop file the Coax side named. The relay connection, the WebSocket, NIP-42,
   NIP-44, admission, the queue, the engine's HTTP request and every answer
   are Coax. Sockets are coax/platform/socket-native.c, entropy
   entropy-foreign.c, the signing double's process process-native.c. */
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

int coax_pylon_main(void);

static int g_argc;
static char **g_argv;
static volatile sig_atomic_t g_stop;
static char g_stop_path[1024];
static size_t g_stop_used;

int64_t pyh_argc(void) { return g_argc; }
int64_t pyh_arg_len(int64_t i) { return i >= 0 && i < g_argc ? (int64_t)strlen(g_argv[i]) : -1; }
int64_t pyh_arg_byte(int64_t i, int64_t k) {
  if (i < 0 || i >= g_argc || k < 0 || (size_t)k >= strlen(g_argv[i])) return -1;
  return (unsigned char)g_argv[i][k];
}
int64_t pyh_now_s(void) { return (int64_t)time(NULL); }
/* Names the stop file, one byte at a time (at most 1023). */
int64_t pyh_stop_path_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || g_stop_used + 1 >= sizeof g_stop_path) return -1;
  g_stop_path[g_stop_used++] = (char)byte; g_stop_path[g_stop_used] = 0;
  return 0;
}
int64_t pyh_stop(void) {
  if (g_stop) return 1;
  return g_stop_used > 0 && access(g_stop_path, F_OK) == 0 ? 1 : 0;
}

static void on_signal(int sig) { (void)sig; g_stop = 1; }
/* The worker blocks the stop signals, so the main thread (parked in
   pthread_join) takes them and no socket wait in the worker sees EINTR. */
static void *run(void *arg) {
  (void)arg; sigset_t s; sigemptyset(&s); sigaddset(&s, SIGTERM); sigaddset(&s, SIGINT);
  pthread_sigmask(SIG_BLOCK, &s, NULL);
  coax_pylon_main(); return NULL;
}

int main(int argc, char **argv) {
  g_argc = argc; g_argv = argv;
  setvbuf(stdout, NULL, _IOLBF, 0); /* the log is read while the Pylon runs */
  struct sigaction sa; memset(&sa, 0, sizeof sa);
  sa.sa_handler = on_signal; sa.sa_flags = SA_RESTART; sigemptyset(&sa.sa_mask);
  sigaction(SIGTERM, &sa, NULL); sigaction(SIGINT, &sa, NULL);
  pthread_attr_t attr; pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)512 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) { fprintf(stderr, "pylon: no thread\n"); return 1; }
  pthread_join(t, NULL);
  return 0;
}
