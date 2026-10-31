/* Firm reason: 2 (independent numerical/ABI oracle). Long-double source
   formulas and separate row/head indexing test checked Coax primitives. */
#include "groot-dit.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned long cases;
static coax_v_f32 fs(float *p,int64_t n){return(coax_v_f32){p,n};}
static coax_v_u8 bs(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static float number(uint32_t b){float x;memcpy(&x,&b,4);return x;}
static void result(coax_t_GdtResult r,int status,int64_t n){assert(r.status==status&&r.written==n);}
static void fill(float *p,size_t n){for(size_t i=0;i<n;i++)p[i]=-713.25f;}
static void untouched(const float *p,size_t n){for(size_t i=0;i<n;i++)assert(p[i]==-713.25f);}
static void close_value(float actual,float expected){assert(isfinite(actual)&&fabs((double)actual-expected)<=0.000006+0.000003*fabs((double)expected));cases++;}
static float affine(float x,float scale,float bias){float product=(float)((long double)x*scale);return(float)((long double)product+bias);}
static void norm_ref(int kind,int rows,int width,const float *x,const float *params,float *out){
  const float epsilon=kind==3?1e-6f:1e-5f;
  for(int row=0;row<rows;row++){
    long double mean=0,var=0;for(int j=0;j<width;j++)mean+=x[row*width+j];mean/=width;
    for(int j=0;j<width;j++){long double c=(long double)x[row*width+j]-mean;var+=c*c;}
    long double divisor=sqrtl(var/width+epsilon);
    for(int j=0;j<width;j++){
      float normal=(float)(((long double)x[row*width+j]-mean)/divisor);
      if(kind==0)out[row*width+j]=normal;
      else {float scale=params[kind==3?width+j:j],bias=params[kind==3?j:width+j];if(kind>=2)scale=(float)((long double)scale+1);out[row*width+j]=affine(normal,scale,bias);}
    }
  }
}
static float activation_ref(int kind,float x){
  long double z=x;
  if(kind==0){long double e=expl(-fabsl(z));float probability=(float)(z<0?e/(1+e):1/(1+e));return(float)(z*probability);}
  long double argument=sqrtl(2/acosl(-1.0L))*(z+0.044715L*z*z*z);
  return(float)((z/2)*(1+tanhl(argument)));
}
static void attention_ref(int nq,int nk,int h,int d,const float *q,const float *k,const float *v,const uint8_t *mask,float *out){
  float score[1024],probability[1024];int active=0;for(int t=0;t<nk;t++)active+=mask[t];
  for(int row=0;row<nq;row++)for(int head=0;head<h;head++){
    if(!active){for(int j=0;j<d;j++)out[(row*h+head)*d+j]=0;continue;}
    long double maximum=-LDBL_MAX;
    for(int t=0;t<nk;t++){
      long double dot=0;for(int j=0;j<d;j++)dot+=(long double)q[(row*h+head)*d+j]*k[(t*h+head)*d+j];
      score[t]=(float)(dot/sqrtl(d));if(mask[t]&&score[t]>maximum)maximum=score[t];
    }
    long double sum=0;for(int t=0;t<nk;t++)if(mask[t])sum+=expl((long double)score[t]-maximum);
    for(int t=0;t<nk;t++)probability[t]=mask[t]?(float)(expl((long double)score[t]-maximum)/sum):0;
    for(int j=0;j<d;j++){long double dot=0;for(int t=0;t<nk;t++)dot+=(long double)probability[t]*v[(t*h+head)*d+j];out[(row*h+head)*d+j]=(float)dot;}
  }
}
int main(void){
  const size_t cap=131072;float *x=calloc(cap,sizeof(float)),*out=calloc(cap,sizeof(float)),*work=calloc(2*cap,sizeof(float)),*expected=calloc(cap,sizeof(float));assert(x&&out&&work&&expected);
  float *params=calloc(8192,sizeof(float));assert(params);
  const int widths[]={1,2,3,7,48,64,1536,2048},rows[]={1,2,3,41};
  for(int kind=0;kind<4;kind++)for(unsigned w=0;w<sizeof widths/sizeof *widths;w++)for(unsigned r=0;r<sizeof rows/sizeof *rows;r++){
    int width=widths[w],count=rows[r]*width;for(int i=0;i<count;i++)x[i]=(float)((i*17+i/width*3)%47-23)/8;
    for(int j=0;j<width;j++){params[j]=(float)(j%7-3)/4;params[width+j]=(float)(j%5-2)/8;}
    norm_ref(kind,rows[r],width,x,params,expected);fill(out,cap);fill(work,2*cap);
    result(gdt_norm((coax_t_GdtNorm)kind,rows[r],width,fs(x,count),fs(params,kind?2*width:0),fs(out,cap),fs(work,2*cap),0),0,count);
    for(int i=0;i<count;i++)close_value(out[i],expected[i]);untouched(out+count,cap-count);untouched(work+2*count,2*cap-2*count);
  }
  for(int kind=0;kind<4;kind++)for(int field=0;field<6;field++){
    fill(out,cap);fill(work,2*cap);int count=6,pc=kind?6:0,no=6,nw=12,alias=0;
    if(field==0)count=5;if(field==1)pc++;if(field==2)no=5;if(field==3)nw=11;if(field==4)alias=1;
    if(field==5){x[0]=number(0x7fc00000);count=6;pc=kind?6:0;}
    int status=field<4?3:field==4?4:5;
    result(gdt_norm((coax_t_GdtNorm)kind,2,3,fs(x,count),fs(params,pc),fs(out,no),fs(work,nw),alias),status,0);untouched(out,cap);cases++;x[0]=0;
  }
  const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001,0xff800001};
  for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int field=0;field<4;field++){
    float values[]={-1,0,1,-1,0,1},p[]={1,1,1,0,0,0};if(field<2)values[field?5:0]=number(bad[b]);else p[field==2?0:5]=number(bad[b]);fill(out,cap);fill(work,2*cap);
    result(gdt_norm(COAX_GdtNorm_affine,2,3,fs(values,6),fs(p,6),fs(out,6),fs(work,12),0),5,0);untouched(out,cap);untouched(work,2*cap);cases++;
  }
  for(int kind=1;kind<4;kind++){
    float values[]={-1,1},p[]={1,FLT_MAX,0,FLT_MAX};if(kind==3){p[1]=FLT_MAX;p[2]=1;p[3]=FLT_MAX;}
    fill(out,cap);result(gdt_norm((coax_t_GdtNorm)kind,1,2,fs(values,2),fs(p,4),fs(out,2),fs(work,4),0),7,0);untouched(out,cap);cases++;
  }
  const float special[]={-FLT_MAX,-80,-10,-1,-0.1f,-0.001f,-0.0f,0,0.001f,0.1f,1,10,80,FLT_MAX};
  for(int kind=0;kind<2;kind++){
    for(int i=0;i<65536;i++)x[i]=(float)(i%601-300)/16;
    for(unsigned i=0;i<sizeof special/sizeof *special;i++)x[i]=special[i];fill(out,cap);fill(work,2*cap);
    result(gdt_activate((coax_t_GdtActivation)kind,fs(x,65536),fs(out,cap),fs(work,2*cap),0),0,65536);
    for(int i=0;i<65536;i++)close_value(out[i],activation_ref(kind,x[i]));untouched(out+65536,cap-65536);untouched(work+65536,2*cap-65536);
    for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int tail=0;tail<2;tail++){
      float values[]={0,1};values[tail]=number(bad[b]);fill(out,cap);fill(work,2*cap);
      result(gdt_activate((coax_t_GdtActivation)kind,fs(values,2),fs(out,2),fs(work,2),0),5,0);untouched(out,cap);untouched(work,2*cap);cases++;
    }
  }
  for(int block=-1;block<=32;block++){coax_t_GdtRoute route=gdt_block_route(block);if(block<0||block>31)assert(route.status==2);else assert(route.status==0&&route.mask==(block&1?1:block%4==0?0:2));cases++;}
  uint8_t image[1025],valid[1025],mask_out[1025];
  for(int count=1;count<=1024;count*=2)for(int images=0;images<2;images++){
    memset(mask_out,9,sizeof mask_out);for(int i=0;i<count;i++){image[i]=(uint8_t)(i&1);valid[i]=(uint8_t)(i%3!=0);}
    result(gdt_cross_mask(images,bs(image,count),bs(valid,count),bs(mask_out,1025),0),0,count);
    for(int i=0;i<count;i++){assert(mask_out[i]==(valid[i]&&(image[i]!=0)==images));cases++;}assert(mask_out[count]==9);
  }
  for(int images=0;images<2;images++)for(int field=0;field<5;field++){
    memset(mask_out,9,sizeof mask_out);memset(image,0,sizeof image);memset(valid,1,sizeof valid);
    int count=field==0?0:field==1?1025:4,vc=field==2?3:count,no=field==3?3:1025,alias=field==4;
    result(gdt_cross_mask(images,bs(image,count),bs(valid,vc),bs(mask_out,no),alias),field<3?2:field==3?3:4,0);for(int i=0;i<1025;i++)assert(mask_out[i]==9);cases++;
  }
  const int qns[]={1,2,7},kns[]={1,2,5,13},heads[]={1,2,32},dims[]={1,2,3,48,64};
  float *k=calloc(cap,sizeof(float)),*v=calloc(cap,sizeof(float));assert(k&&v);
  for(unsigned qi=0;qi<3;qi++)for(unsigned ki=0;ki<4;ki++)for(unsigned hi=0;hi<3;hi++)for(unsigned di=0;di<5;di++){
    int nq=qns[qi],nk=kns[ki],h=heads[hi],d=dims[di],qn=nq*h*d,kn=nk*h*d;
    coax_t_GdtAttention s={nq,nk,h,d};coax_t_GdtAttentionPlan layout=gdt_attention_plan(s);assert(layout.status==0&&layout.query==qn&&layout.key==kn&&layout.work==3*nk+qn);
    for(int i=0;i<qn;i++)x[i]=(float)(i*7%13-6)/4;for(int i=0;i<kn;i++){k[i]=(float)(i*11%17-8)/4;v[i]=(float)(i*13%19-9)/2;}
    for(int pattern=0;pattern<5;pattern++){
      for(int t=0;t<nk;t++)valid[t]=(uint8_t)(pattern==0?1:pattern==1?(t&1):pattern==2?0:pattern==3?t==0:t==nk-1);
      fill(out,cap);fill(work,2*cap);attention_ref(nq,nk,h,d,x,k,v,valid,expected);
      result(gdt_attention(s,fs(x,qn),fs(k,kn),fs(v,kn),bs(valid,nk),fs(out,cap),fs(work,2*cap),0),0,qn);
      for(int i=0;i<qn;i++)close_value(out[i],expected[i]);untouched(out+qn,cap-qn);untouched(work+layout.work,2*cap-layout.work);
    }
  }
  coax_t_GdtAttention s={1,2,1,2};float qv[]={1,1},kv[]={0,0,1,1},vv[]={1,2,5,10};uint8_t good[]={1,1};
  for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int field=0;field<6;field++){
    float q2[2],k2[4],v2[4];memcpy(q2,qv,sizeof q2);memcpy(k2,kv,sizeof k2);memcpy(v2,vv,sizeof v2);
    if(field<2)q2[field?1:0]=number(bad[b]);else if(field<4)k2[field==2?0:3]=number(bad[b]);else v2[field==4?0:3]=number(bad[b]);fill(out,cap);fill(work,2*cap);
    result(gdt_attention(s,fs(q2,2),fs(k2,4),fs(v2,4),bs(good,2),fs(out,2),fs(work,8),0),5,0);untouched(out,cap);untouched(work,2*cap);cases++;
  }
  for(int field=0;field<8;field++){
    fill(out,cap);fill(work,2*cap);uint8_t m[]={1,1};int qn=2,kn=4,vn=4,mn=2,no=2,nw=8,alias=0;
    if(field==0)qn--;if(field==1)kn--;if(field==2)vn--;if(field==3)mn--;if(field==4)no--;if(field==5)nw--;if(field==6)alias=1;if(field==7)m[1]=2;
    result(gdt_attention(s,fs(qv,qn),fs(kv,kn),fs(vv,vn),bs(m,mn),fs(out,no),fs(work,nw),alias),field<6?3:field==6?4:6,0);untouched(out,cap);untouched(work,2*cap);cases++;
  }
  qv[0]=FLT_MAX;kv[2]=FLT_MAX;fill(out,cap);result(gdt_attention(s,fs(qv,2),fs(kv,4),fs(vv,4),bs(good,2),fs(out,2),fs(work,8),0),7,0);untouched(out,cap);cases++;
  coax_t_GdtAttention uniform={1,3,1,1};float uq[]={0},uk[]={0,0,0},uv[]={FLT_MAX,FLT_MAX,FLT_MAX};uint8_t um[]={1,1,1};fill(out,cap);
  result(gdt_attention(uniform,fs(uq,1),fs(uk,3),fs(uv,3),bs(um,3),fs(out,1),fs(work,10),0),7,0);untouched(out,cap);cases++;
  free(x);free(out);free(work);free(expected);free(params);free(k);free(v);
  printf("GR00T DiT primitive native: %lu independent normalization, activation, mask, attention and ABI cases passed\n",cases);return 0;
}
