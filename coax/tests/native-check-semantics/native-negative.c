/* Deliberately non-Coax transport fixtures; never language qualification. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include "native-forall-observer.h"
#include "native-semantics-observer.h"
#ifndef SEMANTICS_NEGATIVE_MODE
#define SEMANTICS_NEGATIVE_MODE 0
#endif
static Coax_VerificationTrapHandler_v1 callback;
void coax_verification_set_trap_v1(Coax_VerificationTrapHandler_v1 value) { callback=value; }
static int64_t body(const uint64_t *values,int64_t count) {
  (void)values;
  if (count!=0) return -1;
  if (SEMANTICS_NEGATIVE_MODE==1) { raise(SIGSEGV); abort(); }
  if (SEMANTICS_NEGATIVE_MODE==2) {
    fputs("declared-negative.coax:1: assertion failed\n",stderr); abort();
  }
  if (SEMANTICS_NEGATIVE_MODE==3) { fwrite("COAXST1\0",1,8,stdout); fflush(stdout); abort(); }
  if (SEMANTICS_NEGATIVE_MODE==4) {
    const unsigned char malformed[32]={'C','O','A','X','S','T','1',0,31,0,0,0,255,255,255,255,
      0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0};
    fwrite(malformed,1,sizeof malformed,stdout); fflush(stdout); abort();
  }
  if (SEMANTICS_NEGATIVE_MODE==5) {
    if(callback) callback("assertion failed",16,"declared-negative.coax",22,1);
    return 0;
  }
  if (SEMANTICS_NEGATIVE_MODE==6) {
    if(callback) callback("not a Coax trap",15,"declared-negative.coax",22,1);
    abort();
  }
  abort();
}
static const Coax_VerificationCheck_v2 check={0,
  "declared native protocol negative",33,"declared-negative.coax",22,1,1,
  1,0,0,1,body};
const Coax_VerificationCheck_v2 *coax_verification_checks_v2(int64_t *count) { *count=1;return &check; }
