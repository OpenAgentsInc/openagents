/* Independent scalar oracle for new vision Metal helpers, compiled to native
   C for debug/release/ASan+UBSan. This is tooling, never a model CPU fallback. */
#include "vision-plan.h"
#include "vision-device.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_f32 F(float *x,int64_t n){return(coax_v_f32){x,n};}
static coax_v_u32 U(uint32_t *x,int64_t n){return(coax_v_u32){x,n};}
static coax_v_u8 B(uint8_t *x,int64_t n){return(coax_v_u8){x,n};}
static float word(uint32_t b){float x;memcpy(&x,&b,4);return x;}
static void close_to(float actual,double expected,double atol,double rtol){assert(isfinite(actual)&&isfinite(expected));assert(fabs((double)actual-expected)<=atol+rtol*fabs(expected));cases++;}
static float bf16(const uint32_t *p,int64_t i){uint32_t b=(i%2?p[i/2]>>16:p[i/2]&65535)<<16;return word(b);}
static float coordinate(int p){float step=47.0f/15.0f;return p<8?(float)p*step:47.0f-(float)(15-p)*step;}
int main(void){
 float x=0,bias=0,out=7;uint8_t flag=9;
 for(int i=0;i<=49152;i++){x=(float)(i-24576)/2048.0f;flag=9;gvi_element(0,1,2,F(&x,1),F(&bias,1),F(NULL,0),F(&out,1),B(&flag,1));assert(flag==0);double expected=.5*(double)x*(1+erf((double)x/sqrt(2.0)));close_to(out,expected,2e-6,2e-6);}
 const float edge[]={FLT_MAX,-FLT_MAX,8,-8,0,-0.0f};for(unsigned i=0;i<sizeof edge/sizeof*edge;i++){x=edge[i];gvi_element(0,1,2,F(&x,1),F(&bias,1),F(NULL,0),F(&out,1),B(&flag,1));assert(flag==0);close_to(out,x>=8?x:x<=-8?0:0,2e-6,2e-6);}
 const uint32_t nonfinite[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001};for(int i=0;i<4;i++){x=word(nonfinite[i]);flag=9;gvi_element(0,1,2,F(&x,1),F(&bias,1),F(NULL,0),F(&out,1),B(&flag,1));assert(flag==1);cases++;}
 size_t n=524288;float *values=malloc(n*3*4),*biases=malloc(3072*4),*q=malloc(n*4),*k=malloc(n*4),*v=malloc(n*4);uint8_t *flags=malloc(n);assert(values&&biases&&q&&k&&v&&flags);
 for(size_t i=0;i<n*3;i++)values[i]=(float)((int)(i%127)-63)/64;for(int i=0;i<3072;i++)biases[i]=(float)(i%11-5)/32;
 for(size_t i=0;i<n;i+=127){flags[i]=9;gvi_qkv((int64_t)i,F(values,n*3),F(biases,3072),F(q,n),F(k,n),F(v,n),B(flags,n));assert(flags[i]==0);int t=(int)(i/1024),a=(int)(i%1024),d=a%64,local=t%256,row=2*(local/32)+(local%4)/2,col=2*((local/4)%8)+local%2,other=a+(d<32?32:-32);float power=(float)pow(10000.0,(double)(d%16)/16),frequency=(float)(1.0/(double)power),phase=(float)(d%32<16?row:col)*frequency,c=(float)cos((double)phase),s=(float)sin((double)phase),sign=d<32?-1:1;
  float aa=values[t*3072+a]+biases[a],ar=values[t*3072+other]+biases[other],bb=values[t*3072+1024+a]+biases[1024+a],br=values[t*3072+1024+other]+biases[1024+other];close_to(q[i],aa*c+(sign*ar)*s,2e-6,2e-6);close_to(k[i],bb*c+(sign*br)*s,2e-6,2e-6);close_to(v[i],values[t*3072+2048+a]+biases[2048+a],0,0);
 }
 uint32_t *pos=malloc(1179648*4);assert(pos);for(int64_t i=0;i<1179648;i++){uint32_t low=0x3d00+(uint32_t)(i%257),high=0xbd00+(uint32_t)((i*7)%251);pos[i]=low|(high<<16);}for(size_t i=0;i<n;i++)values[i]=(float)((int)(i%31)-15)/16;for(int i=0;i<1024;i++)biases[i]=(float)(i%7-3)/8;
 for(int token=0;token<512;token++)for(int sample=0;sample<6;sample++){const int axes[]={0,1,2,31,32,1023};int axis=axes[sample],i=token*1024+axis;flags[i]=9;gvi_posadd(i,F(values,n),F(biases,1024),U(pos,1179648),F(q,n),B(flags,n));assert(flags[i]==0);int local=token%256,row=2*(local/32)+(local%4)/2,col=2*((local/4)%8)+local%2;float yy=coordinate(row),xx=coordinate(col);int y0=(int)yy,x0=(int)xx,y1=y0<47?y0+1:y0,x1=x0<47?x0+1:x0;float dy=yy-y0,dx=xx-x0;int ids[]={y0*48+x0,y0*48+x1,y1*48+x0,y1*48+x1};float weights[]={(1-dy)*(1-dx),(1-dy)*dx,dy*(1-dx),dy*dx},p=0;for(int z=0;z<4;z++)p+=bf16(pos,(int64_t)ids[z]*1024+axis)*weights[z];close_to(q[i],(values[i]+biases[axis])+p,2e-6,2e-6);}
 pos[0]=(pos[0]&0xffff0000u)|0x7f80u;q[0]=7;flags[0]=9;gvi_posadd(0,F(values,n),F(biases,1024),U(pos,1179648),F(q,n),B(flags,n));assert(flags[0]==1&&q[0]==7);cases++;
 uint32_t raw[]={0x3f800000,0x80000000,0x7f7fffff};float published[]={7,7,7,7};assert(gvp_copy(U(raw,3),F(published,4),0).written==3&&published[0]==1&&published[3]==7);for(int i=0;i<4;i++){raw[2]=nonfinite[i];for(int j=0;j<4;j++)published[j]=7;assert(gvp_copy(U(raw,3),F(published,4),0).status==COAX_GvpStatus_number);for(int j=0;j<4;j++)assert(published[j]==7);cases++;}
 assert(gvp_stage_count()==29&&gvp_weight_count()==315&&gvp_buffer_count()==336);int nodes=0;for(int stage=0;stage<29;stage++){coax_t_GvpStage s=gvp_stage(stage);int64_t total=0;for(int at=0;at<s.nodes;at++){coax_t_GvpNode node=gvp_node(stage,at);assert(!node.status&&node.flag_offset==total);total+=node.flag_count;nodes++;}assert(total==s.flags);cases++;}assert(nodes==359);
 free(values);free(biases);free(q);free(k);free(v);free(flags);free(pos);printf("GR00T vision native: %u independent erf/position/RoPE/publication/graph cases passed\n",cases);return 0;
}
