/* Independent scalar residual equations; verification only, not product code. */
#include "residual-native.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void reference(const float*x,const float*a,const float*b,const float*c,const float*d,const float*dy,
                      float*y,float*gx,float*ga,float*gb,float*gc,float*gd,float*loss){
  float pre[18],hidden[18];double dh[18];
  for(int h=0;h<3;h++)for(int p=0;p<6;p++){
    float sum=0;for(int k=0;k<2;k++){float v=x[k*6+p]*a[h*2+k];sum=sum+v;}
    pre[h*6+p]=sum+b[h];hidden[h*6+p]=pre[h*6+p]>0?pre[h*6+p]:0;
  }
  for(int k=0;k<2;k++)for(int p=0;p<6;p++){
    float sum=0;for(int h=0;h<3;h++){float v=hidden[h*6+p]*c[k*3+h];sum=sum+v;}
    y[k*6+p]=(sum+d[k])+x[k*6+p];
  }
  for(int h=0;h<3;h++)for(int p=0;p<6;p++){
    double sum=0;for(int k=0;k<2;k++)sum+=(double)dy[k*6+p]*c[k*3+h];
    dh[h*6+p]=pre[h*6+p]>0?sum:0;
  }
  for(int k=0;k<2;k++)for(int p=0;p<6;p++){
    double sum=dy[k*6+p];for(int h=0;h<3;h++)sum+=dh[h*6+p]*a[h*2+k];gx[k*6+p]=(float)sum;
  }
  for(int h=0;h<3;h++){
    double sum=0;for(int p=0;p<6;p++)sum+=dh[h*6+p];gb[h]=(float)sum;
    for(int k=0;k<2;k++){sum=0;for(int p=0;p<6;p++)sum+=dh[h*6+p]*x[k*6+p];ga[h*2+k]=(float)sum;}
  }
  for(int k=0;k<2;k++){
    double sum=0;for(int p=0;p<6;p++)sum+=dy[k*6+p];gd[k]=(float)sum;
    for(int h=0;h<3;h++){sum=0;for(int p=0;p<6;p++)sum+=(double)dy[k*6+p]*hidden[h*6+p];gc[k*3+h]=(float)sum;}
  }
  float sum=0;for(int i=0;i<12;i++){float v=y[i]*dy[i];sum=sum+v;}*loss=sum;
}
static int run(float*x,float*a,float*b,float*c,float*d,float*dy,float*y,float*gx,float*ga,float*gb,float*gc,float*gd,float*loss){
  return residual_native_vjp((coax_v_f32){x,12},(coax_v_f32){a,6},(coax_v_f32){b,3},(coax_v_f32){c,6},(coax_v_f32){d,2},(coax_v_f32){dy,12},
    (coax_v_f32){y,12},(coax_v_f32){gx,12},(coax_v_f32){ga,6},(coax_v_f32){gb,3},(coax_v_f32){gc,6},(coax_v_f32){gd,2},(coax_v_f32){loss,1});
}
int main(void){
  float x[12],a[6],b[3],c[6],d[2],dy[12],y[12],gx[12],ga[6],gb[3],gc[6],gd[2],loss;
  float ey[12],ex[12],ea[6],eb[3],ec[6],ed[2],el;
  for(int seed=0;seed<4;seed++){
    for(int i=0;i<12;i++){x[i]=(float)((i*7+3+seed)%17-8)/8;dy[i]=(float)((i*3+seed)%9-4)/8;}
    for(int i=0;i<6;i++){a[i]=(float)((i*5+2+seed)%11-5)/16;c[i]=(float)((i*3+1+seed)%13-6)/16;}
    b[0]=-.125f;b[1]=.25f;b[2]=.0625f;d[0]=.125f;d[1]=-.25f;
    reference(x,a,b,c,d,dy,ey,ex,ea,eb,ec,ed,&el);
    assert(run(x,a,b,c,d,dy,y,gx,ga,gb,gc,gd,&loss)==COAX_AdStatus_ok);
    for(int i=0;i<12;i++){assert(y[i]==ey[i]);assert(gx[i]==ex[i]);}
    for(int i=0;i<6;i++){assert(ga[i]==ea[i]);assert(gc[i]==ec[i]);}
    for(int i=0;i<3;i++)assert(gb[i]==eb[i]);for(int i=0;i<2;i++)assert(gd[i]==ed[i]);assert(loss==el);
  }
  /* Zero residual branch: every output is x, every input VJP is dy. */
  memset(c,0,sizeof c);memset(d,0,sizeof d);assert(run(x,a,b,c,d,dy,y,gx,ga,gb,gc,gd,&loss)==COAX_AdStatus_ok);
  for(int i=0;i<12;i++){assert(y[i]==x[i]);assert(gx[i]==dy[i]);}for(int i=0;i<6;i++)assert(ga[i]==0);for(int i=0;i<3;i++)assert(gb[i]==0);
  const uint32_t bad[3]={UINT32_C(0x7fc00001),UINT32_C(0x7f800000),UINT32_C(0xff800000)};
  for(int source=0;source<6;source++)for(int kind=0;kind<3;kind++){
    float*at=source==0?x:source==1?a:source==2?b:source==3?c:source==4?d:dy;float old=at[0];memcpy(at,&bad[kind],4);
    for(int i=0;i<12;i++){y[i]=91;gx[i]=92;}for(int i=0;i<6;i++){ga[i]=93;gc[i]=94;}for(int i=0;i<3;i++)gb[i]=95;gd[0]=gd[1]=96;loss=97;
    assert(run(x,a,b,c,d,dy,y,gx,ga,gb,gc,gd,&loss)==COAX_AdStatus_number);
    for(int i=0;i<12;i++){assert(y[i]==91);assert(gx[i]==92);}for(int i=0;i<6;i++){assert(ga[i]==93);assert(gc[i]==94);}for(int i=0;i<3;i++)assert(gb[i]==95);assert(gd[0]==96&&gd[1]==96&&loss==97);at[0]=old;
  }
  assert(residual_native_state());
  puts("residual native: four independent full forward/input/17parameter VJPs, identity skip,18 IEEE refusals, named state and freeze pass");return 0;
}
