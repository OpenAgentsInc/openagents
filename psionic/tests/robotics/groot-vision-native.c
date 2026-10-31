/* Firm reason: 2 (independent formula/layout oracle). Authored NCHW pixels,
   nested temporal/block/intra/channel indexing, C math vision RoPE and learned
   position interpolation. No third-party code, model weights or image decoder. */
#include "groot-vision.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_f32 span(float *p,int64_t n){return(coax_v_f32){p,n};}
static float number(uint32_t u){float f;memcpy(&f,&u,4);return f;}
static int valid(coax_t_GvGrid g){return g.t>=1&&g.t<=4&&g.h>=2&&g.h<=32&&g.h%2==0&&g.w>=2&&g.w<=32&&g.w%2==0;}
static void unchanged(float *p,int n){for(int i=0;i<n;i++)assert(p[i]==-713.25f);}
static void poison(float *p,int n){for(int i=0;i<n;i++)p[i]=-713.25f;}
static void close_value(float got,float want){assert(isfinite(got)&&fabs((double)got-want)<=2e-5+1e-5*fabs((double)want));cases++;}
static void packing(int frames,int h,int w){
 int n=frames*3*h*w,nt=(frames+1)/2,gh=h/16,gw=w/16,m=nt*gh*gw*1536;
 float *pixels=malloc((size_t)n*4),*out=malloc((size_t)(m+1)*4);assert(pixels&&out);
 for(int i=0;i<n;i++)pixels[i]=(float)i;poison(out,m+1);
 coax_t_GvRead result=gv_pack(frames,h,w,span(pixels,n),span(out,m+1),0);assert(result.status==0&&result.written==m&&out[m]==-713.25f);
 int index=0;
 for(int t=0;t<nt;t++)for(int br=0;br<gh/2;br++)for(int bc=0;bc<gw/2;bc++)for(int ir=0;ir<2;ir++)for(int ic=0;ic<2;ic++)
  for(int c=0;c<3;c++)for(int dt=0;dt<2;dt++)for(int y=0;y<16;y++)for(int x=0;x<16;x++){
   int frame=2*t+dt;if(frame>=frames)frame=frames-1;int row=(br*2+ir)*16+y,col=(bc*2+ic)*16+x;
   assert(out[index++]==pixels[((frame*3+c)*h+row)*w+col]);cases++;
  }
 assert(index==m);poison(out,m+1);
 result=gv_pack(frames,h,w,span(pixels,n-1),span(out,m+1),0);assert(result.status==2&&result.written==0);unchanged(out,m+1);cases++;
 result=gv_pack(frames,h,w,span(pixels,n),span(out,m-1),0);assert(result.status==2);unchanged(out,m+1);cases++;
 result=gv_pack(frames,h,w,span(pixels,n),span(out,m+1),1);assert(result.status==3);unchanged(out,m+1);cases++;
 const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001};
 for(int b=0;b<4;b++)for(int last=0;last<2;last++){int at=last?n-1:0;float saved=pixels[at];pixels[at]=number(bad[b]);result=gv_pack(frames,h,w,span(pixels,n),span(out,m+1),0);assert(result.status==4&&result.written==0);unchanged(out,m+1);pixels[at]=saved;cases++;}
 free(out);free(pixels);
}
static void layouts(void){
 const int64_t values[]={-1,0,1,2,3,4,6,16,32,34,INT64_MAX};
 for(unsigned ti=0;ti<11;ti++)for(unsigned hi=0;hi<11;hi++)for(unsigned wi=0;wi<11;wi++){
  coax_t_GvGrid g={values[ti],values[hi],values[wi]};assert(gv_grid(g)==valid(g));cases++;
 }
 coax_t_GvGrid g={2,4,6};int token=0;
 for(int t=0;t<2;t++)for(int br=0;br<2;br++)for(int bc=0;bc<3;bc++)for(int ir=0;ir<2;ir++)for(int ic=0;ic<2;ic++){
  coax_t_GvPosition p=gv_position(g,token++);assert(p.status==0&&p.time==t&&p.row==br*2+ir&&p.column==bc*2+ic);cases++;
 }
 for(int post=0;post<2;post++){coax_t_GvMerge m=gv_merge_layout(g,1024,post);assert(m.status==0&&m.rows==12&&m.width==4096&&m.values==49152&&m.norm_width==(post?4096:1024));cases++;}
 for(int layer=-1;layer<25;layer++){int want=layer==5?0:layer==11?1:layer==17?2:-1;assert(gv_deepstack(layer)==want);cases++;}
 assert(gv_position(g,-1).status==1&&gv_position(g,48).status==1);cases++;
 for(int gh=2;gh<=16;gh+=2)for(int gw=2;gw<=16;gw+=2){g=(coax_t_GvGrid){1,gh,gw};token=0;
  for(int br=0;br<gh/2;br++)for(int bc=0;bc<gw/2;bc++)for(int ir=0;ir<2;ir++)for(int ic=0;ic<2;ic++){
   int row=2*br+ir,col=2*bc+ic;double fy=(double)row*47/(gh-1),fx=(double)col*47/(gw-1);float y=(float)fy,x=(float)fx;
   int yl=(int)y,xl=(int)x,yh=yl<47?yl+1:47,xh=xl<47?xl+1:47;float dy=y-yl,dx=x-xl;
   const int indices[]={yl*48+xl,yl*48+xh,yh*48+xl,yh*48+xh};float weights[]={(1-dy)*(1-dx),(1-dy)*dx,dy*(1-dx),dy*dx};
   coax_t_GvMix m=gv_posmix(g,token++);assert(m.status==0);for(int i=0;i<4;i++){assert(m.indices.e[i]==indices[i]);close_value(m.weights.e[i],weights[i]);}
  }
 }
}
static void rotary(void){
 coax_t_GvGrid g={2,4,6};int heads=2,tokens=48,n=tokens*heads*64;
 float *x=malloc((size_t)n*4),*out=malloc((size_t)(n+1)*4),*work=malloc((size_t)n*4);assert(x&&out&&work);
 for(int i=0;i<n;i++)x[i]=(float)(i%23-11)/4;poison(out,n+1);poison(work,n);
 coax_t_GvRead result=gv_rope(g,heads,span(x,n),span(out,n+1),span(work,n),0);assert(result.status==0&&result.written==n&&out[n]==-713.25f);
 int token=0;
 for(int t=0;t<2;t++)for(int br=0;br<2;br++)for(int bc=0;bc<3;bc++)for(int ir=0;ir<2;ir++)for(int ic=0;ic<2;ic++,token++)for(int head=0;head<heads;head++)for(int pair=0;pair<32;pair++){
  int row=2*br+ir,col=2*bc+ic,position=pair<16?row:col;float inv=1.0f/powf(10000.0f,(float)(pair%16)/16.0f),phase=(float)position*inv,co=cosf(phase),si=sinf(phase);int base=(token*heads+head)*64;
  float a=x[base+pair]*co,b=x[base+pair+32]*si,c=x[base+pair+32]*co,d=x[base+pair]*si;
  close_value(out[base+pair],a-b);close_value(out[base+pair+32],c+d);
 }
 poison(out,n+1);result=gv_rope(g,heads,span(x,n),span(out,n-1),span(work,n),0);assert(result.status==2);unchanged(out,n+1);cases++;
 result=gv_rope(g,heads,span(x,n),span(out,n+1),span(work,n-1),0);assert(result.status==2);unchanged(out,n+1);cases++;
 result=gv_rope(g,heads,span(x,n-1),span(out,n+1),span(work,n),0);assert(result.status==2);unchanged(out,n+1);cases++;
 result=gv_rope(g,heads,span(x,n),span(out,n+1),span(work,n),1);assert(result.status==3);unchanged(out,n+1);cases++;
 for(int last=0;last<2;last++){int at=last?n-1:0;float saved=x[at];x[at]=NAN;result=gv_rope(g,heads,span(x,n),span(out,n+1),span(work,n),0);assert(result.status==4&&result.written==0);unchanged(out,n+1);x[at]=saved;cases++;}
 memset(x,0,(size_t)n*4);x[80]=FLT_MAX;x[112]=-FLT_MAX; /* token0head1 has position0: safe, choose token1head0 below */
 x[144]=FLT_MAX;x[176]=-FLT_MAX;result=gv_rope(g,heads,span(x,n),span(out,n+1),span(work,n),0);assert(result.status==5&&result.written==0);unchanged(out,n+1);cases++;
 free(work);free(out);free(x);
}
int main(void){layouts();packing(1,32,64);packing(3,32,32);packing(4,32,32);rotary();printf("Qwen3VL vision native: %u independent patch/position/RoPE/layout/refusal cases passed\n",cases);return 0;}
