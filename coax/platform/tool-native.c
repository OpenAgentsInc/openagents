/* The host of a Coax command-line tool: argv, stderr, the OS and a big
   stack. Effects only; see tool-native.h. Flags, usage text and every
   decision are Coax (coax/lib/sys/args.coax). */
#include "tool-native.h"
#include <pthread.h>
#include <stdio.h>
#include <string.h>

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
