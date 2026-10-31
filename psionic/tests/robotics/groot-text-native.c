/* Firm reason2: independent numerical/ABI oracle; not shipped. */
#include "groot-text.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static long cases;
static void demand(int yes){cases++;if(!yes){fprintf(stderr,"text native case%ld failed\n",cases);exit(1);}}
static coax_v_s64 si(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_f32 sf(float *p,int64_t n){return(coax_v_f32){p,n};}
int main(int argc,char **argv){
 demand(argc==2);int64_t *p=calloc(1539,sizeof(*p));float *y=malloc(65537*sizeof(*y)),*work=malloc(65536*sizeof(*work));demand(p&&y&&work);
 for(int t=-2;t<=514;t++)demand(gt_rope_admit(t,65536,65536,0)==(t<1||t>512?COAX_GtStatus_shape:COAX_GtStatus_ok));
 int sizes[]={1,2,7,140,512};
 for(int c=0;c<5;c++){int n=sizes[c];for(int a=0;a<3;a++)for(int t=0;t<n;t++)p[a*n+t]=(a*137+t*17)%512;
  for(int i=0;i<=65536;i++)y[i]=-713.25f;
  coax_t_GtRead r=gt_rope(si(p,3*n),sf(y,65537),sf(work,65536),0);demand(!r.status&&r.written==128*n&&y[128*n]==-713.25f);
  for(int t=0;t<n;t++)for(int j=0;j<64;j++){
   int axis=j%3;if(j>=60)axis=0;float power=powf(5000000.f,(float)j/64.f);float frequency=1.f/power;float phase=(float)p[axis*n+t]*frequency;
   float co=(float)cos((double)phase),sn=(float)sin((double)phase);
   demand(fabsf(y[t*128+2*j]-co)<=5e-5f+2e-6f*fabsf(co));demand(fabsf(y[t*128+2*j+1]-sn)<=5e-5f+2e-6f*fabsf(sn));
  }
 }
 FILE *f=fopen(argv[1],"wb");demand(f!=NULL);demand(fwrite(y,4,65536,f)==65536);demand(!fclose(f));
 int64_t ids[513]={0},out[513],scratch[513];for(int i=0;i<513;i++)out[i]=791;
 for(int n=1;n<=512;n++){for(int i=0;i<n;i++)ids[i]=i%4==0&&i<512?151655:0;
  coax_t_GtRead r=gt_images(si(ids,n),si(out,513),si(scratch,513),0);demand(!r.status&&r.written==n);int image=0;
  for(int i=0;i<n;i++)demand(out[i]==(ids[i]==151655?image++:-1));
 }
 for(int invalid=0;invalid<4;invalid++){for(int i=0;i<513;i++)out[i]=791;ids[511]=invalid==0?-1:151936;
  coax_t_GtRead r=gt_images(si(ids,512),si(out,513),si(scratch,513),invalid==1);
  demand(r.status== (invalid==1?COAX_GtStatus_alias:COAX_GtStatus_value));for(int i=0;i<513;i++)demand(out[i]==791);
 }
 free(p);free(y);free(work);printf("GR00T text native: %ld cases passed\n",cases);return 0;
}
