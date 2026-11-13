/* A stand-in for the NDK's android/log.h, for crash-reports-check.sh on a desktop: the log goes to stderr. */
#pragma once
#include <stdarg.h>
#include <stdio.h>
#define ANDROID_LOG_INFO 4
static inline int __android_log_print(int prio, const char *tag, const char *fmt, ...) {
  (void)prio; va_list a; va_start(a, fmt); fprintf(stderr, "%s: ", tag); vfprintf(stderr, fmt, a); fputc('\n', stderr); va_end(a); return 0;
}
