// Firm reason: independent oracle (2): recomputes latency buckets/counts in C and checks emitted-C ABI/canaries.
// Never calls a model; policy and telemetry implementation remain in Coax.
#include "Guard-decision.h"
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
static coax_v_t_GaSession view(coax_t_GaSession *s) { return (coax_v_t_GaSession){s,1}; }
static coax_string str(const char *s) { return (coax_string){s,(int64_t)strlen(s)}; }
static int equal(coax_string s,const char *text) { return s.count==(int64_t)strlen(text) && !memcmp(s.data,text,(size_t)s.count); }
static const char good[] = "200\n{\"model\":\"elephant-1\",\"answers\":{\"ambient\":{\"type\":\"choice\",\"choice\":\"greeting\",\"confidence\":0.8,\"probabilities\":{\"quiet\":0.1,\"greeting\":0.8,\"weather\":0.1}}},\"service\":{\"version\":\"psionic-pylon-v1\",\"door\":\"pylon:test\",\"identity\":\"elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba\",\"provider\":\"59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7\"}}";
static int oracle_bin(int64_t milliseconds) {
  const int64_t upper[] = {10,50,100,250,500,1000,2000,4000,8000,30000};
  for (int n=0;n<10;n++) if (milliseconds<upper[n]) return n;
  return 10;
}
static void histogram(coax_t_GaSession *s,int field,int64_t elapsed,int count) {
  for (int bin=0;bin<11;bin++) assert(ga_probe(view(s),field+bin)==(bin==oracle_bin(elapsed)?count:0));
}
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
    if(fault<2)assert(ga_probe(view(&s),6)==1);
    const int reasons[]={2,3,9,6,9,6,5};
    assert(ga_probe(view(&s),7)==1&&ga_probe(view(&s),8)==1&&ga_probe(view(&s),9)==1&&ga_probe(view(&s),10)==1);
    assert(ga_probe(view(&s),11)==reasons[fault]&&ga_probe(view(&s),19+reasons[fault])==1);
    ga_receive(view(&s),1,8002,str(good));ga_receive(view(&s),1,8003,str(good));
    assert(ga_probe(view(&s),10)==1&&ga_probe(view(&s),12)==1);cases++;
  }
  for(int mode=0;mode<3;mode++) {
    coax_t_GaSession s={0};coax_t_GaInput i=input();i.url=str(mode==0?"/realm":mode==1?"?guard-ai=ambient":"?guard-ai=shadow&guard-ai=shadow");ga_page(view(&s),i,0);
    assert(ga_probe(view(&s),1)==0&&ga_effect(view(&s),1).body.count==0);cases++;
  }
  const int64_t delays[]={0,9,10,49,50,99,100,249,250,499,500,999,1000,1999,2000,3999,4000,7999};
  for (unsigned n=0;n<sizeof delays/sizeof delays[0];n++) for(int channel=0;channel<2;channel++) {
    struct {uint64_t before[2];coax_t_GaSession s;uint64_t after[2];} box={0};
    box.before[0]=box.after[1]=UINT64_C(0x13579bdf02468ace);box.before[1]=box.after[0]=UINT64_C(0xfedcba9876543210);
    coax_t_GaInput i=input();int64_t start=123,network=channel==0?delays[n]:0,queue=channel==1?delays[n]:0;
    ga_page(view(&box.s),i,start);ga_receive(view(&box.s),1001,start+1,str(good));assert(ga_probe(view(&box.s),9)==0);
    ga_receive(view(&box.s),1,start+network,str(good));ga_receive(view(&box.s),1,start+network,str(good));
    assert(ga_probe(view(&box.s),9)==1&&ga_probe(view(&box.s),12)==0);
    ga_page(view(&box.s),i,start+network+queue);
    assert(ga_probe(view(&box.s),10)==1&&ga_probe(view(&box.s),11)==1&&ga_probe(view(&box.s),20)==1);
    assert(ga_probe(view(&box.s),13)==network&&ga_probe(view(&box.s),14)==queue&&ga_probe(view(&box.s),15)==network+queue);
    histogram(&box.s,40,network,1);histogram(&box.s,60,queue,1);histogram(&box.s,80,network+queue,1);
    assert(box.before[0]==UINT64_C(0x13579bdf02468ace)&&box.after[1]==box.before[0]&&box.before[1]==UINT64_C(0xfedcba9876543210)&&box.after[0]==box.before[1]);cases++;
  }
  {
    coax_t_GaSession s={0};coax_t_GaInput i=input();ga_page(view(&s),i,0);ga_receive(view(&s),1,30000,str(good));
    assert(ga_probe(view(&s),11)==5&&ga_probe(view(&s),13)==30000&&ga_probe(view(&s),16)==1);
    histogram(&s,40,30000,1);histogram(&s,60,0,0);histogram(&s,80,0,0);cases++;
  }
  printf("guard native: %u lifecycle/transport/metrics oracle cases passed\n",cases);return 0;
}
