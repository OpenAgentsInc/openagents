/* Generated-code lifecycle checks against an independent finite model. */
#include "scrypt-cleanup.h"
#include <stdint.h>

static int same(coax_t_ScryptCleanup a,coax_t_ScryptCleanup b){
  return a.phase==b.phase && a.reason==b.reason && a.pending==b.pending;
}
static int valid(coax_t_ScryptCleanup s){
  if(s.phase==1)return s.reason==0 && s.pending==3;
  if(s.reason<1 || s.reason>3)return 0;
  if(s.phase==2)return s.pending>=1 && s.pending<=7 && (s.reason!=1 || s.pending<=3);
  if(s.phase==3)return s.pending==0;
  return s.phase==4 && s.reason==1 && s.pending==0;
}
int cleanup_fixture(void){
  coax_t_ScryptCleanup begin=sc_begin();
  if(begin.phase!=1 || begin.reason!=0 || begin.pending!=3)return 1;
  for(int phase=-1;phase<=5;phase++)for(int reason=-1;reason<=5;reason++)for(int pending=-1;pending<=9;pending++){
    coax_t_ScryptCleanup s={.phase=phase,.reason=reason,.pending=pending};
    if(sc_valid(s)!=valid(s))return 2;
    if(sc_visible(s)!=(valid(s)&&phase==4))return 3;
    for(int end=-1;end<=5;end++){
      coax_t_ScryptCleanup want=s;
      if(valid(s)&&end>=1&&end<=3){
        if(phase==1){want.phase=2;want.reason=end;want.pending=end==1?3:7;}
        else if((phase==2||phase==3)&&reason==1&&end!=1){want.reason=end;want.pending+=4;want.phase=2;}
      }
      if(!same(sc_stop(s,end),want))return 4;
    }
    coax_t_ScryptCleanup published=s;
    if(valid(s)&&phase==3&&reason==1)published.phase=4;
    if(!same(sc_publish(s),published))return 6;
    for(int region=-1;region<=4;region++)for(int erased=0;erased<=1;erased++){
      coax_t_ScryptCleanup want=s;
      if(valid(s)&&phase==2&&erased&&(region==1||region==2||region==4)&&(pending&region)){
        want.pending-=region;if(!want.pending)want.phase=3;
      }
      if(!same(sc_ack(s,region,erased),want))return 5;
    }
  }
  return 0;
}
#ifdef NATIVE_MAIN
int main(void){return cleanup_fixture();}
#endif
