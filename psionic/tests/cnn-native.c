/* Independent pull-indexed convolution/VJP oracle. Verification only. */
#include "cnn-native.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int xi(int n,int c,int h,int w){return ((n*2+c)*3+h)*4+w;}
static int wi(int o,int c,int h,int w){return ((o*2+c)*2+h)*3+w;}
static int yi(int n,int o,int h,int w){return ((n*2+o)*2+h)*2+w;}
static void reference(const float*x,const float*w,const float*b,const float*dy,
                      float*y,float*gx,float*gw,float*gb,float*loss){
  for(int n=0;n<2;n++)for(int o=0;o<2;o++)for(int h=0;h<2;h++)for(int z=0;z<2;z++){
    float total=0;
    for(int c=0;c<2;c++)for(int kh=0;kh<2;kh++)for(int kw=0;kw<3;kw++){
      float product=x[xi(n,c,h+kh,z+kw)]*w[wi(o,c,kh,kw)];total=total+product;
    }
    y[yi(n,o,h,z)]=total+b[o];
  }
  /* Pull each input's Jacobian row independently; Coax uses scatter-add. */
  for(int n=0;n<2;n++)for(int c=0;c<2;c++)for(int h=0;h<3;h++)for(int z=0;z<4;z++){
    double total=0;
    for(int o=0;o<2;o++)for(int oh=0;oh<2;oh++)for(int ow=0;ow<2;ow++){
      int kh=h-oh,kw=z-ow;
      if(kh>=0&&kh<2&&kw>=0&&kw<3)total+=(double)dy[yi(n,o,oh,ow)]*w[wi(o,c,kh,kw)];
    }
    gx[xi(n,c,h,z)]=(float)total;
  }
  for(int o=0;o<2;o++)for(int c=0;c<2;c++)for(int kh=0;kh<2;kh++)for(int kw=0;kw<3;kw++){
    double total=0;
    for(int n=0;n<2;n++)for(int h=0;h<2;h++)for(int z=0;z<2;z++)total+=(double)dy[yi(n,o,h,z)]*x[xi(n,c,h+kh,z+kw)];
    gw[wi(o,c,kh,kw)]=(float)total;
  }
  for(int o=0;o<2;o++){double total=0;for(int n=0;n<2;n++)for(int h=0;h<2;h++)for(int z=0;z<2;z++)total+=dy[yi(n,o,h,z)];gb[o]=(float)total;}
  float total=0;for(int i=0;i<16;i++){float product=y[i]*dy[i];total=total+product;}*loss=total;
}
static int run(float*x,float*w,float*b,float*dy,float*y,float*gx,float*gw,float*gb,float*loss){
  return cnn_native_vjp((coax_v_f32){x,48},(coax_v_f32){w,24},(coax_v_f32){b,2},(coax_v_f32){dy,16},
       (coax_v_f32){y,16},(coax_v_f32){gx,48},(coax_v_f32){gw,24},(coax_v_f32){gb,2},(coax_v_f32){loss,1});
}
int main(void){
  float x[48],w[24],b[2],dy[16],y[16],gx[48],gw[24],gb[2],loss;
  float ey[16],ex[48],ew[24],eb[2],el;
  for(int seed=0;seed<4;seed++){
    for(int i=0;i<48;i++)x[i]=(float)((i*7+3+seed)%17-8)/8;
    for(int i=0;i<24;i++)w[i]=(float)((i*5+2+seed)%11-5)/16;
    for(int i=0;i<16;i++)dy[i]=(float)((i*3+4+seed)%9-4)/8;
    b[0]=-.125f;b[1]=.25f;reference(x,w,b,dy,ey,ex,ew,eb,&el);
    assert(run(x,w,b,dy,y,gx,gw,gb,&loss)==COAX_AdStatus_ok);
    for(int i=0;i<16;i++)assert(y[i]==ey[i]);
    for(int i=0;i<48;i++)assert(gx[i]==ex[i]);
    for(int i=0;i<24;i++)assert(gw[i]==ew[i]);
    assert(gb[0]==eb[0]&&gb[1]==eb[1]&&loss==el);
  }
  const uint32_t bad[3]={UINT32_C(0x7fc00001),UINT32_C(0x7f800000),UINT32_C(0xff800000)};
  for(int source=0;source<4;source++)for(int k=0;k<3;k++){
    float*at=source==0?x:source==1?w:source==2?b:dy;float old=at[0];memcpy(at,&bad[k],4);
    for(int i=0;i<16;i++)y[i]=91;for(int i=0;i<48;i++)gx[i]=92;
    for(int i=0;i<24;i++)gw[i]=93;gb[0]=gb[1]=94;loss=95;
    assert(run(x,w,b,dy,y,gx,gw,gb,&loss)==COAX_AdStatus_number);
    for(int i=0;i<16;i++)assert(y[i]==91);for(int i=0;i<48;i++)assert(gx[i]==92);
    for(int i=0;i<24;i++)assert(gw[i]==93);assert(gb[0]==94&&gb[1]==94&&loss==95);at[0]=old;
  }
  assert(cnn_native_state());
  puts("CNN native: four independent NCHW/OIHW forward/input/kernel/bias VJPs; IEEE refusals; named state reload and frozen convolution pass");
  return 0;
}
