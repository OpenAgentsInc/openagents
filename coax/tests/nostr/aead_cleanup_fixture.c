#include "aead-cleanup.h"
#include <stdint.h>
static int valid(coax_t_AeadCleanup s){
 if(s.phase==1)return s.reason==0 && s.pending==0;
 if(s.reason<1||s.reason>3)return 0;
 if(s.phase==2)return s.reason==1?s.pending==1:(s.pending==2||s.pending==3);
 if(s.phase==3)return s.pending==0;
 return s.phase==4&&s.pending==0&&s.reason==1;
}
static int64_t next(coax_t_AeadCleanup s){
 if(!valid(s))return -1;if(s.phase==1)return -2;if(s.phase==4)return -3;if(s.phase==3)return 0;
 return s.pending&1?1:2;
}
static int same(coax_t_AeadCleanup a,coax_t_AeadCleanup b){return a.phase==b.phase&&a.reason==b.reason&&a.pending==b.pending;}
int aead_cleanup_fixture(void){
 coax_t_AeadCleanup s=ac_begin();if(s.phase!=1||s.reason||s.pending)return 1;
 for(int phase=-1;phase<=5;phase++)for(int reason=-1;reason<=5;reason++)for(int pending=-1;pending<=5;pending++){
  s=(coax_t_AeadCleanup){.phase=phase,.reason=reason,.pending=pending};
  if(ac_valid(s)!=valid(s)||ac_next(s)!=next(s)||ac_visible(s)!=(valid(s)&&phase==4))return 2;
  for(int end=-1;end<=5;end++){
   coax_t_AeadCleanup want=s;
   if(valid(s)&&end>=1&&end<=3){
    if(phase==1){want.phase=2;want.reason=end;want.pending=end==1?1:3;}
    else if((phase==2||phase==3)&&reason==1&&end!=1){want.phase=2;want.reason=end;want.pending+=2;}
   }
   if(!same(ac_stop(s,end),want))return 3;
  }
  for(int region=-3;region<=4;region++)for(int erased=0;erased<=1;erased++){
   coax_t_AeadCleanup want=s;
   if(valid(s)&&phase==2&&erased&&region==next(s)){want.pending-=region;if(!want.pending)want.phase=3;}
   if(!same(ac_ack(s,region,erased),want))return 4;
  }
  coax_t_AeadCleanup want=s;if(valid(s)&&phase==3&&reason==1)want.phase=4;
  if(!same(ac_publish(s),want))return 5;
 }
 return 0;
}
#ifdef NATIVE_MAIN
int main(void){return aead_cleanup_fixture();}
#endif
