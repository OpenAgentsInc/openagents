/* Independent plan/publication/kernel oracle. CPU-emitted
   helpers only; actual Metal execution has its separate required-device gate. */
#include "plan.h"
#include "bias.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static unsigned cases;
static coax_v_t_GmsCursor one(coax_t_GmsCursor *s){return(coax_v_t_GmsCursor){s,1};}
static coax_v_u32 words(uint32_t *p,int n){return(coax_v_u32){p,n};}
static coax_v_f32 floats(float *p,int n){return(coax_v_f32){p,n};}
static coax_v_u8 bytes(uint8_t *p,int n){return(coax_v_u8){p,n};}
static void poison(float *p){for(int i=0;i<2561;i++)p[i]=-713.25f;}
static void unchanged(float *p){for(int i=0;i<2561;i++)assert(p[i]==-713.25f);}
static coax_t_GmsCursor ready(void){coax_t_GmsLayout p=gms_layout();coax_t_GmsCursor s=gms_begin((coax_v_s64){p.bytes.e,8},0);assert(s.status==0);for(int i=0;i<4;i++)assert(gms_sent(one(&s),i,0)==0);assert(gms_waited(one(&s),0)==0);return s;}
int main(void){
 const int64_t sizes[]={270336,3145728,4096,6144,528,4096,6144,2560};coax_t_GmsLayout p=gms_layout();assert(p.nodes==4&&!memcmp(p.bytes.e,sizes,sizeof sizes));cases++;
 const int kernel[]={0,1,0,2},items[]={16,1024,24,1536},lanes[]={128,1,128,1},argc[]={6,4,6,4};
 for(int i=0;i<4;i++){coax_t_GmsNode n=gms_node(i);assert(n.status==0&&n.kernel==kernel[i]&&n.items==items[i]&&n.lanes==lanes[i]&&n.argc==argc[i]&&n.block==128);cases++;}
 for(int at=0;at<8;at++){int64_t bad[8];memcpy(bad,sizes,sizeof bad);bad[at]--;assert(gms_admit((coax_v_s64){bad,8},0)==2);cases++;}
 uint32_t h[1024]={0},y[1536]={0};uint8_t flags[2560]={0};float out[2561];
 for(int source=0;source<2;source++)for(int last=0;last<2;last++)for(int b=0;b<4;b++){
  const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001};coax_t_GmsCursor s=ready();uint32_t *v=source?y:h;int n=source?1536:1024;int index=last?n-1:0;v[index]=bad[b];poison(out);
  coax_t_GmsRead r=gms_publish(one(&s),bytes(flags,2560),words(h,1024),words(y,1536),floats(out,2561),0);assert(r.status==7&&r.written==0&&s.status==7);unchanged(out);v[index]=0;cases++;
 }
 for(int i=0;i<4;i++){
  coax_t_GmsCursor s=ready();poison(out);flags[i?2559:0]=(uint8_t)(i+1);coax_t_GmsRead r=gms_publish(one(&s),bytes(flags,2560),words(h,1024),words(y,1536),floats(out,2561),0);assert(r.status==7&&r.written==0);unchanged(out);flags[0]=flags[2559]=0;cases++;
 }
 for(int mode=0;mode<6;mode++){
  coax_t_GmsCursor s=ready();poison(out);coax_t_GmsRead r=gms_publish(one(&s),bytes(flags,mode==0?2559:2560),words(h,mode==1?1023:1024),words(y,mode==2?1535:1536),floats(out,mode==3?2559:2561),mode==4);
  if(mode<5){assert(r.status!=0&&r.written==0);unchanged(out);}else{assert(r.status==0&&r.written==2560&&s.published&&out[2560]==-713.25f);assert(gms_publish(one(&s),bytes(flags,2560),words(h,1024),words(y,1536),floats(out,2561),0).status==4);}cases++;
 }
 for(int status=1;status<=7;status++){
  coax_t_GmsCursor s=ready();s.waited=0;assert(gms_waited(one(&s),status)==(status==7?6:5));poison(out);assert(gms_publish(one(&s),bytes(flags,2560),words(h,1024),words(y,1536),floats(out,2561),0).status!=0);unchanged(out);cases++;
 }
 const float a[]={-2,-1,0,2},b[]={1,2,0,-1};float values[4];uint8_t f[8];memcpy(values,a,sizeof a);memset(f,0xff,sizeof f);
 for(int i=0;i<4;i++)gms_bias_relu(i,floats(values,4),floats((float*)b,4),bytes(f,8),0);
 const float wanted[]={0,1,0,1};assert(!memcmp(values,wanted,sizeof wanted));for(int i=0;i<4;i++){assert(f[i]==0&&f[i+4]==255);cases++;}
 for(int mode=0;mode<4;mode++){float x[1]={mode==0?FLT_MAX:mode==1?NAN:mode==2?INFINITY:-INFINITY};float bias[1]={mode==0?FLT_MAX:0};uint8_t flag=255;gms_bias(0,floats(x,1),floats(bias,1),bytes(&flag,1),0);assert(flag==1);cases++;}
 float x[1]={3},bzero[1]={0};uint8_t untouched=255;gms_bias_relu(0,floats(x,1),floats(bzero,1),bytes(&untouched,1),1);assert(x[0]==3&&untouched==255);cases++;
 printf("GR00T Metal helper native: %u independent layout/receipt/finite/publication cases passed\n",cases);return 0;
}
