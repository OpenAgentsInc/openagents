/* Independent native boundary checks for the emitted local ticket gate. */
#include "decision.h"
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>

static const int64_t CLOCK_MAX = INT64_C(9007199254740991);
static const uint64_t GUARD = UINT64_C(0x9a571c4f1d8b2e30);
static coax_t_GdContext context(void) {
  return (coax_t_GdContext){.epoch=4,.actor=7,.generation=3,.revision=9,.policy=1,.options=2};
}
static coax_v_t_GdState view(coax_t_GdState *s) { return (coax_v_t_GdState){s,1}; }

int main(void) {
  struct { uint64_t before; coax_t_GdState state; uint64_t after; } box;
  memset(&box,0,sizeof box); box.before=box.after=GUARD;
  coax_t_GdState *s=&box.state, saved;
  coax_t_GdContext c=context();
  assert(gd_begin(view(s),c,100,8000).code==COAX_GdCode_ok);
  saved=*s;
  assert(gd_begin(view(s),c,200,1).code==COAX_GdCode_busy);
  assert(!memcmp(s,&saved,sizeof saved));
  for (int field=0;field<6;field++) {
    coax_t_GdContext changed=c;
    switch(field) {
      case 0: changed.epoch++;break;case 1: changed.actor++;break;
      case 2: changed.generation++;break;case 3: changed.revision++;break;
      case 4: changed.policy++;break;case 5: changed.options++;break;
    }
    assert(gd_take(view(s),1,changed,200,7).code==COAX_GdCode_stale);
    assert(!memcmp(s,&saved,sizeof saved));
  }
  assert(gd_offer(view(s),0,200,1)==COAX_GdCode_stale);
  assert(!gd_cancel(view(s),0)); assert(!memcmp(s,&saved,sizeof saved));
  assert(gd_offer(view(s),1,200,2)==COAX_GdCode_ok);
  saved=*s;
  assert(gd_offer(view(s),1,201,0)==COAX_GdCode_stale);
  assert(gd_take(view(s),1,c,201,3).code==COAX_GdCode_illegal);
  assert(gd_take(view(s),1,c,8100,7).code==COAX_GdCode_expired);
  assert(!memcmp(s,&saved,sizeof saved));
  assert(gd_take(view(s),1,c,8099,7).action==2);
  assert(gd_take(view(s),1,c,8099,7).code==COAX_GdCode_stale);
  assert(gd_begin(view(s),c,9000,1).ticket.sequence==2);
  assert(gd_cancel(view(s),2));
  assert(gd_begin(view(s),c,9000,1).ticket.sequence==3);
  saved=*s;
  assert(gd_offer(view(s),2,9000,0)==COAX_GdCode_stale);
  assert(!memcmp(s,&saved,sizeof saved));
  assert(gd_expire(view(s),9001)); assert(!gd_expire(view(s),9001));

  const int64_t bad_clock[]={INT64_MIN,-1,CLOCK_MAX+1,INT64_MAX};
  const int64_t bad_timeout[]={INT64_MIN,-1,0,30001,INT64_MAX};
  for (size_t i=0;i<sizeof bad_clock/sizeof *bad_clock;i++) {
    saved=*s;
    assert(gd_begin(view(s),c,bad_clock[i],1).code==COAX_GdCode_bounds);
    assert(!memcmp(s,&saved,sizeof saved));
  }
  for (size_t i=0;i<sizeof bad_timeout/sizeof *bad_timeout;i++) {
    saved=*s;
    assert(gd_begin(view(s),c,0,bad_timeout[i]).code==COAX_GdCode_bounds);
    assert(!memcmp(s,&saved,sizeof saved));
  }
  assert(gd_begin(view(s),c,CLOCK_MAX,1).code==COAX_GdCode_bounds);
  assert(gd_begin(view(s),c,CLOCK_MAX-30000,30000).code==COAX_GdCode_ok);
  assert(gd_offer(view(s),4,CLOCK_MAX-1,30)==COAX_GdCode_ok);
  saved=*s;
  assert(gd_take(view(s),4,c,CLOCK_MAX-1,INT64_MAX).code==COAX_GdCode_bounds);
  assert(gd_take(view(s),4,c,CLOCK_MAX-1,INT64_MIN).code==COAX_GdCode_bounds);
  assert(!memcmp(s,&saved,sizeof saved));
  assert(gd_take(view(s),4,c,CLOCK_MAX-1,INT32_MAX).action==30);

  const int64_t bad_sequence[]={INT64_MIN,-1,INT32_MAX,INT64_MAX};
  for (size_t i=0;i<sizeof bad_sequence/sizeof *bad_sequence;i++) {
    s->sequence=bad_sequence[i]; saved=*s;
    assert(gd_begin(view(s),c,0,1).code==COAX_GdCode_bounds);
    assert(!memcmp(s,&saved,sizeof saved));
  }
  memset(s,0,sizeof *s); s->sequence=INT32_MAX-1;
  assert(gd_begin(view(s),c,0,1).ticket.sequence==INT32_MAX);
  assert(gd_offer(view(s),INT32_MAX,0,0)==COAX_GdCode_ok);
  assert(gd_take(view(s),INT32_MAX,c,0,1).action==0);
  assert(gd_begin(view(s),c,0,1).code==COAX_GdCode_bounds);
  for (int64_t n=0;n<3;n+=2) {
    coax_v_t_GdState bad={NULL,n};
    assert(gd_begin(bad,c,0,1).code==COAX_GdCode_state);
    assert(gd_offer(bad,1,0,0)==COAX_GdCode_state);
    assert(gd_take(bad,1,c,0,1).code==COAX_GdCode_state);
    assert(!gd_cancel(bad,1)&&!gd_expire(bad,0));
  }
  assert(box.before==GUARD&&box.after==GUARD);
  puts("game decision native: stale context, once-only application, cancellation, expiry, bounds and guards passed");
  return 0;
}
