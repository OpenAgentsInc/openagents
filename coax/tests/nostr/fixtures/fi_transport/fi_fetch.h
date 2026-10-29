#ifndef COAX_FI_FETCH_H
#define COAX_FI_FETCH_H
#include <stdint.h>
/* One in-flight operation per process. Public HTTPS only. The handle owns its
   result; it is not an installed provider or an FI authorization. Poll/free
   for a handle must be serialized by its caller. 0=pending, 1=complete,
   2=unavailable. A short destination returns unavailable with no copy.
   The adapter is disabled in normal builds because a host-wide child reaper
   can race parent waitpid. Only an isolated test build may define
   FI_FETCH_TEST_UNSAFE_REAPER. FI_DNS_HELPER_PATH must be an absolute, trusted
   build-time executable path;
   an unset or relative path denies. The caller must serialize poll/free, own
   valid pointer extents and avoid freeing a handle during poll. A timeout
   reports unavailable; resource cleanup assumes the operating system can
   schedule and reap the dedicated helper and its resolver child. */
typedef struct FiNativeFetch FiNativeFetch;
FiNativeFetch *fi_fetch_start(const char *uri, int64_t length, int64_t transport, int64_t deadline_ms);
int fi_fetch_poll(FiNativeFetch *fetch, uint8_t *out, int64_t capacity, int64_t *written);
void fi_fetch_free(FiNativeFetch *fetch);
#endif
