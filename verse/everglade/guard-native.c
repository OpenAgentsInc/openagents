// Platform-free ABI checks against independently emitted Coax, never a model call.
#include "Guard-decision.h"
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
static coax_v_t_GaSession view(coax_t_GaSession *s) { return (coax_v_t_GaSession){s,1}; }
static coax_string str(const char *s) { return (coax_string){s,(int64_t)strlen(s)}; }
static int equal(coax_string s,const char *text) { return s.count==(int64_t)strlen(text) && !memcmp(s.data,text,(size_t)s.count); }
static const char good[] = "200\n{\"model\":\"elephant-1\",\"answers\":{\"ambient\":{\"type\":\"choice\",\"choice\":\"greeting\",\"confidence\":0.8,\"probabilities\":{\"quiet\":0.1,\"greeting\":0.8,\"weather\":0.1}}},\"service\":{\"version\":\"psionic-pylon-v1\",\"door\":\"pylon:test\",\"identity\":\"elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba\",\"provider\":\"59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7\"}}";
static coax_t_GaInput input(void) {
  coax_t_GaInput i={0};i.observation=(coax_t_GaObservation){true,true,true,true,true,true,1,0};
  i.configured=true;i.url=str("?guard-ai=shadow");i.epoch=1;i.generation=2;i.revision=3;i.boundary=true;return i;
}
int main(void) {
  unsigned cases=0;
  {
    coax_t_GaSession s={0};coax_t_GaInput i=input();ga_page(view(&s),i,0);
    coax_t_GaEffect e=ga_effect(view(&s),1);assert(equal(e.url,"/api/v1/systemone")&&e.flags==5&&e.timeout_ms==7999&&e.body.count>0&&e.body.count<1024);
    assert(ga_pending(view(&s),1));ga_sent(view(&s),1,true);ga_receive(view(&s),1,2,str(good));assert(ga_probe(view(&s),2)==2&&ga_probe(view(&s),3)==0);
    i.boundary=false;ga_page(view(&s),i,3);assert(ga_probe(view(&s),3)==0);
    i.boundary=true;ga_page(view(&s),i,4);assert(ga_probe(view(&s),3)==1&&ga_probe(view(&s),5)==1);
    ga_receive(view(&s),1,5,str(good));assert(ga_probe(view(&s),3)==1);
    for(int n=1;n<6;n++){ga_page(view(&s),i,(int64_t)n*60000);assert(ga_probe(view(&s),1)==n+1);ga_sent(view(&s),n+1,false);}
    ga_page(view(&s),i,360000);assert(ga_probe(view(&s),1)==6&&!ga_pending(view(&s),6));cases++;
  }
  for(int flag=0;flag<10;flag++) {
    coax_t_GaSession s={0};coax_t_GaInput i=input();ga_page(view(&s),i,0);ga_receive(view(&s),1,1,str(good));
    switch(flag){case 0:i.observation.calm=false;break;case 1:i.observation.alive=false;break;case 2:i.observation.near=false;break;case 3:i.observation.visible=false;break;case 4:i.observation.safe=false;break;case 5:i.observation.playable=false;break;case 6:i.generation++;break;case 7:i.epoch++;break;case 8:i.suspended=true;break;case 9:i.blocked=true;break;}
    ga_page(view(&s),i,2);assert(ga_probe(view(&s),3)==0&&ga_probe(view(&s),2)==0);cases++;
  }
  for(int fault=0;fault<7;fault++) {
    coax_t_GaSession s={0};coax_t_GaInput i=input();ga_page(view(&s),i,0);
    const char *reply=fault==0?"401\n":fault==1?"429\n":fault==2?"503\n":fault==3?"200\n{}":fault==4?"0\n":fault==5?"200\n[":good;
    ga_receive(view(&s),1,fault==6?8000:1,str(reply));ga_page(view(&s),i,8001);assert(ga_probe(view(&s),3)==0&&ga_probe(view(&s),2)==0);
    if(fault<2)assert(ga_probe(view(&s),6)==1);cases++;
  }
  for(int mode=0;mode<3;mode++) {
    coax_t_GaSession s={0};coax_t_GaInput i=input();i.url=str(mode==0?"/realm":mode==1?"?guard-ai=ambient":"?guard-ai=shadow&guard-ai=shadow");ga_page(view(&s),i,0);
    assert(ga_probe(view(&s),1)==0&&ga_effect(view(&s),1).body.count==0);cases++;
  }
  printf("guard native: %u lifecycle/transport cases passed\n",cases);return 0;
}
