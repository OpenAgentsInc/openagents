/* Firm reason: It is a thin wrapper around another program and holds no logic.
   Coax owns the cohort, requests, pacing, process invocation, pins and records.
   These bindings expose argv, monotonic clock, sleep and an existing file effect. */
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <time.h>
#include "store-file-native.h"
static int count, result;
static char **values;
extern void gp_run(void);
int64_t gp_argc(void) { return count; }
int64_t gp_arg_len(int64_t i) { return i>=0 && i<count ? (int64_t)strlen(values[i]) : -1; }
int64_t gp_arg_byte(int64_t i,int64_t j) { return i>=0 && i<count && j>=0 && (size_t)j<strlen(values[i]) ? (unsigned char)values[i][j] : -1; }
int64_t gp_clock_ns(void) { struct timespec t; if(clock_gettime(CLOCK_MONOTONIC,&t))return -1;return (int64_t)t.tv_sec*1000000000+t.tv_nsec; }
int64_t gp_pause(void) { struct timespec t={0,20000000};return nanosleep(&t,0); }
int64_t gp_store_path(void) { return count>2 ? cxstore_set_path(values[2]) : -1; }
int64_t gp_exit(int64_t code) { result=(int)code;return code; }
int main(int argc,char **argv) { count=argc;values=argv;gp_run();return result; }
