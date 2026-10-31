/* Firm reason2: independent canonical-layout oracle and typed input stream
   for Python stdlib hashlib. These are owned codec fixtures, not robot logs. */
#include "panda-trace.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
static unsigned long checks;
#define CHECK(x) do{checks++;if(!(x)){fprintf(stderr,"trace native line%d\n",__LINE__);exit(1);}}while(0)
static coax_v_u8 span(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static uint64_t rng=169;
static uint64_t random_word(void){rng=rng*UINT64_C(6364136223846793005)+UINT64_C(1442695040888963407);return rng;}
static uint32_t fbits(float x){uint32_t b;memcpy(&b,&x,4);return b;}
static uint64_t dbits(double x){uint64_t b;memcpy(&b,&x,8);return b;}
static void put(uint8_t *p,uint64_t value,int count){for(int i=0;i<count;i++)p[i]=(uint8_t)(value>>(8*i));}
static void raw_input(FILE *f,coax_t_PrSample *s){
 uint8_t b[500];int at=0;int64_t counters[]={s->epoch,s->serial,s->tick,s->substep,s->flags};
 for(int i=0;i<5;i++){put(b+at,(uint64_t)counters[i],8);at+=8;}
 for(int i=0;i<7;i++){put(b+at,s->action.e[i],4);at+=4;}
 for(int i=0;i<9;i++){put(b+at,s->control.e[i],8);at+=8;}
 for(int i=0;i<45;i++){put(b+at,s->state.e[i],8);at+=8;}
 CHECK(at==500&&fwrite(b,1,sizeof(b),f)==sizeof(b));
}
static void independent_bytes(coax_t_PrSample *s,uint8_t *b){
 memset(b,0,512);memcpy(b,"PTR1",4);b[4]=1;b[6]=(uint8_t)s->flags;b[7]=(uint8_t)s->substep;
 put(b+8,(uint64_t)s->epoch,8);put(b+16,(uint64_t)s->serial,8);put(b+24,(uint64_t)s->tick,8);put(b+32,(uint64_t)(25*s->tick+s->substep),8);
 for(int i=0;i<7;i++)put(b+48+4*i,s->action.e[i],4);
 for(int i=0;i<9;i++)put(b+76+8*i,s->control.e[i],8);
 for(int i=0;i<45;i++)put(b+148+8*i,s->state.e[i],8);memcpy(b+508,"END1",4);
}
static void same(coax_t_PrSample a,coax_t_PrSample b){
 CHECK(a.epoch==b.epoch&&a.serial==b.serial&&a.tick==b.tick&&a.substep==b.substep&&a.flags==b.flags);
 CHECK(!memcmp(a.action.e,b.action.e,sizeof(a.action.e)));CHECK(!memcmp(a.control.e,b.control.e,sizeof(a.control.e)));CHECK(!memcmp(a.state.e,b.state.e,sizeof(a.state.e)));
}
static void refused(coax_t_PrSample s,int expected){uint8_t out[513],poison[513];memset(out,0xa5,sizeof(out));memcpy(poison,out,sizeof(out));CHECK(pr_encode(s,span(out,513),0)==expected);CHECK(!memcmp(out,poison,sizeof(out)));}
static FILE *open_output(const char *prefix,const char *suffix){char name[4096];CHECK(snprintf(name,sizeof(name),"%s%s",prefix,suffix)<(int)sizeof(name));FILE *f=fopen(name,"wb");CHECK(f!=NULL);return f;}
int main(int argc,char **argv){
 if(argc!=2){fprintf(stderr,"usage: panda_trace_native OUTPUT_PREFIX\n");return 2;}
 FILE *inputs=open_output(argv[1],".inputs"),*trace=open_output(argv[1],".trace"),*chains=open_output(argv[1],".chains");
 uint8_t previous[32];for(int i=0;i<32;i++)previous[i]=(uint8_t)(i+1);CHECK(fwrite(previous,1,32,trace)==32);
 coax_t_PrSample sample={0};
 for(int n=0;n<513;n++){
  sample.epoch=1+n;sample.serial=n;sample.tick=n/25;sample.substep=n%25+1;sample.flags=n%8;
  for(int i=0;i<6;i++)sample.action.e[i]=fbits((float)((double)(random_word()>>11)*0x1p-53*2-1));
  const uint32_t grip[]={0,UINT32_C(0x80000000),UINT32_C(0x3f800000),UINT32_C(0xbf800000)};sample.action.e[6]=grip[n%4];
  for(int i=0;i<7;i++){double limit=i<5?80:12;sample.control.e[i]=dbits(((double)(random_word()>>11)*0x1p-53*2-1)*limit);}
  sample.control.e[7]=dbits((double)(random_word()>>11)*0x1p-53*.04);sample.control.e[8]=dbits(-(double)(random_word()>>11)*0x1p-53*.04);
  for(int i=0;i<45;i++)sample.state.e[i]=random_word()&UINT64_C(0xffefffffffffffff);
  if(n==0){sample.action.e[0]=UINT32_C(0x80000000);sample.control.e[0]=UINT64_C(0x8000000000000000);sample.state.e[0]=UINT64_C(0x8000000000000000);sample.state.e[1]=1;sample.state.e[2]=UINT64_C(0x7fefffffffffffff);}
  uint8_t out[513],expected[512],next[32];memset(out,0xa5,sizeof(out));independent_bytes(&sample,expected);
  CHECK(pr_encode(sample,span(out,513),0)==0);CHECK(out[512]==0xa5&&!memcmp(out,expected,512));
  coax_t_PrRead read=pr_decode(span(out,512));CHECK(read.status==0);same(read.sample,sample);
  CHECK(pr_chain(span(previous,32),span(out,512),span(next,32),0)==0);
  raw_input(inputs,&sample);CHECK(fwrite(out,1,512,trace)==512);CHECK(fwrite(next,1,32,chains)==32);memcpy(previous,next,32);
 }
 CHECK(fwrite(previous,1,32,trace)==32);CHECK(!fclose(inputs)&&!fclose(trace)&&!fclose(chains));
 int64_t *fields[]={&sample.epoch,&sample.serial,&sample.tick,&sample.substep,&sample.flags};const int64_t bad[5][2]={{0,1000001},{-1,1000001},{-1,720},{0,26},{-1,8}};
 for(int i=0;i<5;i++){int64_t old=*fields[i];for(int k=0;k<2;k++){*fields[i]=bad[i][k];refused(sample,COAX_PrStatus_range);}*fields[i]=old;}
 const uint32_t bad32[]={UINT32_C(0x7f800000),UINT32_C(0xff800000),UINT32_C(0x7fc00001)};
 const uint64_t bad64[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001)};
 for(int i=0;i<7;i++){uint32_t old=sample.action.e[i];for(int k=0;k<3;k++){sample.action.e[i]=bad32[k];refused(sample,COAX_PrStatus_number);}sample.action.e[i]=old;}
 for(int kind=0;kind<2;kind++){uint64_t *a=kind?sample.state.e:sample.control.e;int count=kind?45:9;for(int i=0;i<count;i++){uint64_t old=a[i];for(int k=0;k<3;k++){a[i]=bad64[k];refused(sample,COAX_PrStatus_number);}a[i]=old;}}
 uint8_t bytes[513],poison[513],valid[512];memset(bytes,77,sizeof(bytes));memcpy(poison,bytes,sizeof(bytes));
 CHECK(pr_encode(sample,span(bytes,511),0)==COAX_PrStatus_shape);CHECK(pr_encode(sample,span(bytes,INT64_MIN),0)==COAX_PrStatus_shape);CHECK(pr_encode(sample,span(bytes,513),1)==COAX_PrStatus_overlap);CHECK(!memcmp(bytes,poison,sizeof(bytes)));
 independent_bytes(&sample,valid);const int indexes[]={0,1,2,3,4,5,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,508,509,510,511};
 for(unsigned i=0;i<sizeof(indexes)/sizeof(*indexes);i++){valid[indexes[i]]^=1;coax_t_PrRead r=pr_decode(span(valid,512));CHECK(r.status!=0&&r.sample.epoch==0&&r.sample.serial==0);valid[indexes[i]]^=1;}
 CHECK(pr_decode(span(valid,511)).status==COAX_PrStatus_shape);CHECK(pr_decode(span(valid,513)).status==COAX_PrStatus_shape);CHECK(pr_decode(span(valid,INT64_MIN)).status==COAX_PrStatus_shape);
 uint8_t next[33];memset(next,0xa5,sizeof(next));const int64_t counts[]={INT64_MIN,0,31,33};
 for(int i=0;i<4;i++){CHECK(pr_chain(span(previous,counts[i]),span(valid,512),span(next,32),0)==1);CHECK(pr_chain(span(previous,32),span(valid,512),span(next,counts[i]),0)==1);}
 CHECK(pr_chain(span(previous,32),span(valid,511),span(next,32),0)==1);CHECK(pr_chain(span(previous,32),span(valid,512),span(next,32),1)==2);for(int i=0;i<33;i++)CHECK(next[i]==0xa5);
 printf("panda trace: %lu assertions;513 canonical records and SHA chain steps\n",checks);return 0;
}
