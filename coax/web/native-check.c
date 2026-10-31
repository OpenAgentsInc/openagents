// The main of a native check suite: a .coax file whose header names a separately
// compiled unit with --native-link (see coax/web/suites.coax). It exports
// native_check(i, run): with run, check i runs and an #assert that fails aborts;
// without, it prints the check's name. Both return the number of checks.
// Firm reason: this is the C entry point the linker needs to join two Coax
// objects; each check runs in a forked child, on a thread with a large stack
// (a frame check keeps whole vertex buffers on it), so one failure can't hide
// the rest. It prints lines and a summary the way coax test does.
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

int64_t native_check(int64_t i, _Bool run);

static int64_t which;
static void *body(void *unused) { (void)unused; native_check(which, 1); return 0; }

static int child(int64_t i) {
  pthread_attr_t at; pthread_t th; which = i;
  pthread_attr_init(&at); pthread_attr_setstacksize(&at, (size_t)1 << 30);
  if (pthread_create(&th, &at, body, 0) != 0) return 2;
  pthread_join(th, 0);
  return 0;
}

int main(void) {
  int64_t n = native_check(-1, 0), passed = 0;
  for (int64_t i = 0; i < n; i++) {
    fflush(stdout); fflush(stderr);
    pid_t pid = fork();
    if (pid == 0) _exit(child(i));
    int status = 1;
    if (pid < 0 || waitpid(pid, &status, 0) != pid) status = 1;
    int ok = WIFEXITED(status) && WEXITSTATUS(status) == 0;
    passed += ok;
    printf(ok ? "ok   #native " : "FAIL #native ");
    fflush(stdout); native_check(i, 0); printf("\n");
  }
  printf("%lld checks, %lld passed, %lld failed\n", (long long)n, (long long)passed, (long long)(n - passed));
  return passed == n ? 0 : 1;
}
