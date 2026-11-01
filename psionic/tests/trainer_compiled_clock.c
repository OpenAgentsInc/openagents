/* Chapter 131: the one host procedure the bench needs, the monotonic clock
   in nanoseconds (tcb_now in psionic/tests/trainer-compiled-bench.coax). */
#define _POSIX_C_SOURCE 200112L
#include <stdint.h>
#include <time.h>

int64_t tcb_now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}
