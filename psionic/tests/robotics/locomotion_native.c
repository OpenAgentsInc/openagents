/* New GAE/PPO ABI, admission, publication and alias witnesses. No simulator. */
#include "locomotion.h"
#include <float.h>
#include <inttypes.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(double)==8 && DBL_MANT_DIG==53, "IEEE F64");
_Static_assert(sizeof(coax_t_RgaReward)==8, "nominal reward");
_Static_assert(sizeof(coax_t_RgaStep)==40 && sizeof(coax_t_RgaOutput)==40, "GAE ABI");
_Static_assert(sizeof(coax_t_RcpOutput)==40, "PPO ABI");
_Static_assert(offsetof(coax_t_RgaStep,terminated)==24 && offsetof(coax_t_RgaStep,truncated)==32, "input flags");
_Static_assert(offsetof(coax_t_RgaOutput,bootstrap)==24 && offsetof(coax_t_RgaOutput,trace)==32, "output masks");
_Static_assert(offsetof(coax_t_RcpOutput,clipped_ratio)==24 && offsetof(coax_t_RcpOutput,clipped_active)==32, "PPO fields");

static uint64_t assertions, refusals, aliases, ieee_patterns;
#define NEED(x) do { assertions++; if(!(x)){fprintf(stderr,"locomotion witness:%d: %s\n",__LINE__,#x);exit(1);} } while(0)
static double word(uint64_t b){double x;memcpy(&x,&b,8);return x;}
static uint64_t bits(double x){uint64_t b;memcpy(&b,&x,8);return b;}
static void *read_file(const char *name,size_t *n){FILE*f=fopen(name,"rb");NEED(f);NEED(!fseek(f,0,SEEK_END));long z=ftell(f);NEED(z>=0&&z<=32*1024*1024);rewind(f);void*p=malloc((size_t)z+1);NEED(p);NEED(fread(p,1,(size_t)z,f)==(size_t)z&&!fclose(f));*n=(size_t)z;return p;}
static void dump(const char *dir,const char *name,const void *p,size_t n){char path[4096];NEED(snprintf(path,sizeof path,"%s/%s",dir,name)<(int)sizeof path);FILE*f=fopen(path,"wb");NEED(f);NEED(fwrite(p,1,n,f)==n&&!fclose(f));}
static uint8_t ga(int typed,int64_t w,int64_t h,double g,double l,coax_t_RgaStep*s,int64_t n,coax_t_RgaOutput*out,int64_t m){
  coax_v_t_RgaStep v={s,n};coax_v_t_RgaOutput o={out,m};
  return typed?rga_returns(w,h,g,l,v,o):rga_returns_raw(w,h,g,l,v,o);
}
static uint8_t pp(int typed,double rho,double a,double eps,coax_t_RcpOutput*out,int64_t m){
  coax_v_t_RcpOutput o={out,m};coax_t_RgaReward reward={a};
  return typed?rcp_surrogate(rho,reward,eps,o):rcp_surrogate_raw(rho,a,eps,o);
}
static void ga_poison(coax_t_RgaOutput *o,size_t n){for(size_t i=0;i<n;i++)o[i]=(coax_t_RgaOutput){{1001+(double)i},{2001+(double)i},{3001+(double)i},71,73};}
static void pp_poison(coax_t_RcpOutput *o,size_t n){for(size_t i=0;i<n;i++)o[i]=(coax_t_RcpOutput){{1001+(double)i},{2001+(double)i},{3001+(double)i},4001+(double)i,79};}
static void ga_finite(const coax_t_RgaOutput*o,size_t n){for(size_t i=0;i<n;i++){NEED(isfinite(o[i].delta.value)&&isfinite(o[i].advantage.value)&&isfinite(o[i].lambda_return.value));NEED((o[i].bootstrap==0||o[i].bootstrap==1)&&(o[i].trace==0||o[i].trace==1));}}
static void pp_finite(const coax_t_RcpOutput*o,size_t n){for(size_t i=0;i<n;i++){NEED(isfinite(o[i].surrogate.value)&&isfinite(o[i].loss.value)&&isfinite(o[i].dloss_dlogp.value)&&isfinite(o[i].clipped_ratio));NEED(o[i].clipped_active==0||o[i].clipped_active==1);}}
static void fixture(coax_t_RgaStep s[4]){
  for(int i=0;i<4;i++)s[i]=(coax_t_RgaStep){{.125*(i+1)},{-.25*(i+1)},{.375*(i+1)},0,0};
  s[1].truncated=1;s[2].terminated=1;s[2].truncated=1;
}
static void ga_refuse(uint8_t expected,int64_t w,int64_t h,double g,double l,coax_t_RgaStep s[4],int64_t n,int64_t m){
  for(int typed=0;typed<2;typed++){
    struct {uint64_t before;coax_t_RgaOutput v[6];uint64_t after;} out={0};
    out.before=UINT64_C(0x3141592653589793);out.after=UINT64_C(0x2718281828459045);ga_poison(out.v,6);
    unsigned char saved[sizeof out],inputs[4*sizeof*s];memcpy(saved,&out,sizeof out);memcpy(inputs,s,sizeof inputs);
    NEED(ga(typed,w,h,g,l,s,n,out.v,m)==expected);NEED(!memcmp(saved,&out,sizeof out));NEED(!memcmp(inputs,s,sizeof inputs));refusals++;
  }
}
static void pp_refuse(uint8_t expected,double r,double a,double e,int64_t m){
  for(int typed=0;typed<2;typed++){
    struct {uint64_t before;coax_t_RcpOutput v[3];uint64_t after;} out={0};out.before=1234567;out.after=7654321;pp_poison(out.v,3);
    unsigned char saved[sizeof out];memcpy(saved,&out,sizeof out);NEED(pp(typed,r,a,e,out.v,m)==expected);NEED(!memcmp(saved,&out,sizeof out));refusals++;
  }
}
static void admission(void){
  coax_t_RgaStep s[4];fixture(s);
  int64_t bad[]={INT64_MIN,-1,0,17,INT64_MAX};
  for(size_t i=0;i<sizeof bad/sizeof*bad;i++)ga_refuse(1,bad[i],4,.99,.95,s,4,4);
  int64_t badh[]={INT64_MIN,-1,0,129,INT64_MAX};for(size_t i=0;i<sizeof badh/sizeof*badh;i++)ga_refuse(1,1,badh[i],.99,.95,s,4,4);
  int64_t spans[]={INT64_MIN,-1,0,3,5,INT64_MAX};
  for(size_t i=0;i<sizeof spans/sizeof*spans;i++){ga_refuse(1,1,4,.99,.95,s,spans[i],4);ga_refuse(1,1,4,.99,.95,s,4,spans[i]);pp_refuse(1,1,1,.2,spans[i]);}
  pp_refuse(1,1,1,.2,2);
  uint64_t nonfinite[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff0000000000001),UINT64_C(0xfff8000000000013)};
  for(size_t k=0;k<sizeof nonfinite/sizeof*nonfinite;k++){
    double x=word(nonfinite[k]);ga_refuse(2,1,4,x,.95,s,4,4);ga_refuse(2,1,4,.99,x,s,4,4);ieee_patterns+=2;
    for(int row=0;row<4;row++)for(int col=0;col<3;col++){
      fixture(s);if(col==0)s[row].reward.value=x;else if(col==1)s[row].current_value.value=x;else s[row].next_value.value=x;
      ga_refuse(2,1,4,.99,.95,s,4,4);ieee_patterns++;
    }
    fixture(s);pp_refuse(2,x,1,.2,1);pp_refuse(2,1,x,.2,1);pp_refuse(2,1,1,x,1);ieee_patterns+=3;
  }
  fixture(s);double below0=nextafter(0.,-INFINITY),above1=nextafter(1.,INFINITY);
  ga_refuse(3,1,4,below0,.5,s,4,4);ga_refuse(3,1,4,above1,.5,s,4,4);ga_refuse(3,1,4,.5,below0,s,4,4);ga_refuse(3,1,4,.5,above1,s,4,4);
  for(int row=0;row<4;row++)for(int col=0;col<3;col++)for(int sign=-1;sign<=1;sign+=2){
    fixture(s);double x=sign*nextafter(1e6,INFINITY);if(col==0)s[row].reward.value=x;else if(col==1)s[row].current_value.value=x;else s[row].next_value.value=x;ga_refuse(3,1,4,.99,.95,s,4,4);
  }
  int64_t flag[]={INT64_MIN,-1,2,INT64_MAX};
  for(size_t k=0;k<sizeof flag/sizeof*flag;k++)for(int row=0;row<4;row++)for(int col=0;col<2;col++){
    fixture(s);if(col)s[row].truncated=flag[k];else s[row].terminated=flag[k];ga_refuse(3,1,4,.99,.95,s,4,4);
  }
  fixture(s);double nan=word(UINT64_C(0x7ff8000000000001));
  ga_refuse(1,0,4,nan,nan,s,4,4);ga_refuse(2,1,4,2,nan,s,4,4);s[3].next_value.value=nan;ga_refuse(3,1,4,2,.5,s,4,4);
  s[0].reward.value=nextafter(1e6,INFINITY);s[0].terminated=-1;ga_refuse(2,1,4,.5,.5,s,4,4);
  pp_refuse(3,nextafter(0x1p-20,0.),1,.2,1);pp_refuse(3,nextafter(16.,INFINITY),1,.2,1);
  pp_refuse(3,1,nextafter(1e9,INFINITY),.2,1);pp_refuse(3,1,-nextafter(1e9,INFINITY),.2,1);pp_refuse(3,1,1,below0,1);pp_refuse(3,1,1,above1,1);
  pp_refuse(1,nan,nan,nan,0);pp_refuse(2,0,nan,.2,1);pp_refuse(2,1,1,nan,1);
}
static void endpoints(void){
  for(int typed=0;typed<2;typed++){
    coax_t_RgaStep s[4];fixture(s);coax_t_RgaOutput out[5];
    for(int sign=-1;sign<=1;sign+=2)for(int field=0;field<3;field++){
      fixture(s);if(field==0)s[3].reward.value=sign*1e6;else if(field==1)s[3].current_value.value=sign*1e6;else s[3].next_value.value=sign*1e6;
      ga_poison(out,5);coax_t_RgaOutput spare=out[4];NEED(!ga(typed,1,4,1,1,s,4,out,4));ga_finite(out,4);NEED(!memcmp(&spare,out+4,sizeof spare));
    }
    double hyper[]={-0.,0.,1.};for(size_t i=0;i<3;i++)for(size_t j=0;j<3;j++){fixture(s);NEED(!ga(typed,1,4,hyper[i],hyper[j],s,4,out,4));ga_finite(out,4);}
    double rho[]={0x1p-20,16.},adv[]={-1e9,1e9,-0.,0.},eps[]={-0.,0.,1.};
    for(size_t i=0;i<2;i++)for(size_t j=0;j<4;j++)for(size_t k=0;k<3;k++){
      coax_t_RcpOutput o[2];pp_poison(o,2);coax_t_RcpOutput spare=o[1];NEED(!pp(typed,rho[i],adv[j],eps[k],o,1));pp_finite(o,1);NEED(!memcmp(&spare,o+1,sizeof spare));
      if(adv[j]==0){NEED(bits(o[0].surrogate.value)==0&&bits(o[0].loss.value)==0&&bits(o[0].dloss_dlogp.value)==0&&o[0].clipped_active==0);}
    }
    /* Exactly rounded knots select the unclipped formal log-prob derivative. */
    double es[]={0.,.2,1.,0x1p-53};for(size_t j=0;j<4;j++)for(int sign=-1;sign<=1;sign+=2){
      double r=sign>0?1.+es[j]:1.-es[j];if(r<0x1p-20)continue;
      coax_t_RcpOutput o;NEED(!pp(typed,r,(double)sign,es[j],&o,1));NEED(o.clipped_active==0&&o.dloss_dlogp.value==-r*sign);
    }
  }
}
static void aliasing(void){
  const int inoff[]={0,0,1,0,2},outoff[]={0,1,0,2,0};
  for(int typed=0;typed<2;typed++)for(size_t k=0;k<5;k++){
    union {coax_t_RgaStep in[10];coax_t_RgaOutput out[10];unsigned char bytes[400];} u;
    memset(&u,0x3c,sizeof u);coax_t_RgaStep input[4];fixture(input);memcpy(u.bytes+40*inoff[k],input,sizeof input);
    unsigned char saved[sizeof u];memcpy(saved,&u,sizeof u);coax_t_RgaOutput baseline[4];NEED(!ga(typed,1,4,.875,.75,input,4,baseline,4));
    NEED(!ga(typed,1,4,.875,.75,u.in+inoff[k],4,u.out+outoff[k],4));NEED(!memcmp(u.bytes+40*outoff[k],baseline,sizeof baseline));
    for(size_t b=0;b<sizeof u;b++)if(b<(size_t)(40*outoff[k])||b>=(size_t)(40*(outoff[k]+4)))NEED(u.bytes[b]==saved[b]);aliases++;
    memcpy(&u,saved,sizeof u);u.in[inoff[k]+3].next_value.value=word(UINT64_C(0x7ff8000000000001));memcpy(saved,&u,sizeof u);
    NEED(ga(typed,1,4,.875,.75,u.in+inoff[k],4,u.out+outoff[k],4)==2);NEED(!memcmp(&u,saved,sizeof u));aliases++;
  }
  for(int typed=0;typed<2;typed++){
    union {double values[10];coax_t_RcpOutput out[2];unsigned char bytes[80];} u;
    for(int i=0;i<10;i++)u.values[i]=100+i;u.values[0]=1.5;u.values[1]=-.75;u.values[2]=.125;
    unsigned char saved[sizeof u];memcpy(saved,&u,sizeof u);coax_t_RcpOutput baseline;NEED(!pp(typed,u.values[0],u.values[1],u.values[2],&baseline,1));
    NEED(!pp(typed,u.values[0],u.values[1],u.values[2],u.out,1));NEED(!memcmp(u.out,&baseline,sizeof baseline));NEED(!memcmp(u.bytes+40,saved+40,40));aliases++;
  }
}
int main(int argc,char **argv){
  NEED(argc==5);uint16_t endian=1;NEED(*(unsigned char*)&endian==1);
#if defined(__aarch64__)
  uint64_t fpcr;__asm__ volatile("mrs %0, fpcr":"=r"(fpcr));NEED(fpcr==0);
#endif
  admission();endpoints();aliasing();
  size_t nc,ns,np;unsigned char*cfg=read_file(argv[1],&nc);coax_t_RgaStep*steps=read_file(argv[2],&ns);double*ppo=read_file(argv[3],&np);
  NEED(nc&&nc%48==0&&nc/48<=256&&ns%40==0&&ns/40<=524288&&np&&np%24==0&&np/24<=8192);
  size_t cases=nc/48,rows=ns/40,pr=np/24,offset=0;coax_t_RgaOutput*results=calloc(rows?rows:1,40);coax_t_RcpOutput*pres=calloc(pr,40);NEED(results&&pres);
  for(size_t c=0;c<cases;c++){
    int64_t h[4];double x[2];memcpy(h,cfg+48*c,32);memcpy(x,cfg+48*c+32,16);
    NEED(h[0]>=1&&h[0]<=16&&h[1]>=1&&h[1]<=128&&h[3]==h[0]*h[1]&&h[2]==(int64_t)offset&&offset+(size_t)h[3]<=rows);
    size_t n=(size_t)h[3];coax_t_RgaOutput*a=calloc(n+2,40),*b=calloc(n+2,40);coax_t_RgaStep*saved=malloc(n*40);NEED(a&&b&&saved);ga_poison(a,n+2);ga_poison(b,n+2);memcpy(saved,steps+offset,n*40);
    coax_t_RgaOutput before=a[0],after=a[n+1];NEED(!ga(1,h[0],h[1],x[0],x[1],steps+offset,h[3],a+1,h[3]));NEED(!ga(0,h[0],h[1],x[0],x[1],steps+offset,h[3],b+1,h[3]));
    NEED(!memcmp(a,b,(n+2)*40));NEED(!memcmp(&before,a,40)&&!memcmp(&after,a+n+1,40));NEED(!memcmp(saved,steps+offset,n*40));ga_finite(a+1,n);memcpy(results+offset,a+1,n*40);offset+=n;free(a);free(b);free(saved);
  }
  NEED(offset==rows);
  for(size_t i=0;i<pr;i++){
    coax_t_RcpOutput a[3],b[3];pp_poison(a,3);pp_poison(b,3);coax_t_RcpOutput before=a[0],after=a[2];double input[3];memcpy(input,ppo+3*i,24);
    NEED(!pp(1,input[0],input[1],input[2],a+1,1));NEED(!pp(0,input[0],input[1],input[2],b+1,1));NEED(!memcmp(a,b,sizeof a));NEED(!memcmp(a,&before,40)&&!memcmp(a+2,&after,40));NEED(!memcmp(input,ppo+3*i,24));pp_finite(a+1,1);pres[i]=a[1];
  }
  dump(argv[4],"gae-actual.bin",results,rows*40);dump(argv[4],"ppo-actual.bin",pres,pr*40);
  printf("{\"assertions\":%"PRIu64",\"refusal_queries\":%"PRIu64",\"alias_queries\":%"PRIu64",\"IEEE_patterns\":%"PRIu64",\"gae_cases\":%zu,\"gae_rows\":%zu,\"ppo_rows\":%zu,\"fpcr_zero\":true}\n",assertions,refusals,aliases,ieee_patterns,cases,rows,pr);
  free(cfg);free(steps);free(ppo);free(results);free(pres);return 0;
}
