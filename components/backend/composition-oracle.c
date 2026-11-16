// Firm reason: 2 (independent pixel oracle over the actual SDK GPU; not shipped).
#include "composition.h"
#include "ui-host.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {coax_t_UcFixture s;UiHost *host;int fault,accepted;} Fixture;
static int frame(void *u,float *v,float w,float h,float d,float t) {return (int)uc_fixture((coax_v_t_UcFixture){&((Fixture*)u)->s,1},(coax_v_f32){v,32768},(int)w,(int)h);}
static int plan(void *u,UiComposition *out) {Fixture*f=u;if(f->fault==1)return -1;coax_t_UcPlan *p=((Fixture*)u)->s.plan.e;*out=(UiComposition){p->ops.e,p->n,p->targets,p->words};if(f->fault==2)out->count=4097;if(f->fault==3)out->targets=0;return p->ok?1:-1;}
static int image(void *u,int slot,UiImage *out) {if(slot||((Fixture*)u)->fault==4)return 0;*out=(UiImage){((Fixture*)u)->s.image.e,2,2,1};return 1;}
static void accepted(void*u,int ok){((Fixture*)u)->accepted=ok;}
static int channel(const unsigned char*p,int x,int y,int c) {return p[(y*64+x)*4+c];}
int main(int argc,char**argv) {
 assert(argc==4);Fixture *f=calloc(1,sizeof *f);assert(f);
 UiApp a={.title="Composition oracle",.max_vertices=32768/28,.shader=argv[1],.composition_shader=argv[2],.user=f,.frame=frame,.composition=plan,.image=image,.presented=accepted};
 f->host=ui_open(&a,1);assert(f->host);unsigned char mono=255,white[4]={255,255,255,255},pixels[64*64*4];ui_atlas(f->host,&mono,1,1,white,1,1);
 for(int mode=0;mode<6;mode++) {
  f->s.mode=mode;assert(ui_shot_frame(f->host,64,64,0));assert(ui_readback(f->host,pixels,sizeof pixels));
  if(mode==0) {assert(abs(channel(pixels,12,16,0)-128)<=1);assert(abs(channel(pixels,25,16,0)-128)<=1);assert(channel(pixels,50,16,0)==0);}
  if(mode==1) {assert(channel(pixels,30,16,0)>250);assert(channel(pixels,12,16,0)==0);assert(channel(pixels,30,30,0)==0);}
  if(mode==2) {assert(channel(pixels,32,32,0)>245);assert(channel(pixels,22,32,0)>0);assert(channel(pixels,16,32,0)==0);}
  if(mode==3) {assert(channel(pixels,32,32,0)>0);assert(channel(pixels,43,27,0)>250);assert(channel(pixels,50,27,0)==0);}
  if(mode==4) {assert(channel(pixels,24,24,0)>220);assert(channel(pixels,40,20,1)==0);assert(channel(pixels,24,40,2)>100 && channel(pixels,24,40,2)<140);}
  if(mode==5) {assert(abs(channel(pixels,12,16,0)-140)<=2);assert(abs(channel(pixels,25,16,0)-160)<=2);}
  char path[2048];snprintf(path,sizeof path,"%s-%d.png",argv[3],mode);assert(ui_shot_write(f->host,path));printf("ok GPU pixels mode %d\n",mode);
 }
 f->s.mode=4;assert(ui_shot_frame(f->host,64,64,0));assert(ui_readback(f->host,pixels,sizeof pixels));
 unsigned char retained[sizeof pixels];memcpy(retained,pixels,sizeof pixels);
 for(int fault=1;fault<=4;fault++){f->fault=fault;f->accepted=1;assert(!ui_shot_frame(f->host,64,64,0));assert(f->accepted==0);assert(ui_readback(f->host,pixels,sizeof pixels));assert(!memcmp(pixels,retained,sizeof pixels));}
 f->fault=0;assert(ui_shot_frame(f->host,64,64,0));assert(f->accepted==1);printf("ok four SDK refusals retain the last pixels and recover\n");
 assert(ui_readback(f->host,retained,sizeof retained));assert(ui_shot_frame(f->host,80,80,0));assert(ui_shot_frame(f->host,64,64,0));assert(ui_readback(f->host,pixels,sizeof pixels));assert(!memcmp(pixels,retained,sizeof pixels));
 ui_close(f->host);f->host=ui_open(&a,1);assert(f->host);ui_atlas(f->host,&mono,1,1,white,1,1);assert(ui_shot_frame(f->host,64,64,0));assert(ui_readback(f->host,pixels,sizeof pixels));assert(!memcmp(pixels,retained,sizeof pixels));printf("ok resize and new-device host replay retained image bytes\n");
 ui_close(f->host);free(f);return 0;
}
