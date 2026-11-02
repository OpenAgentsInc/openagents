#include "pipeline-native.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(coax_t_RQColor)==16,"four float color channels");
_Static_assert(sizeof(coax_t_RRPixel)==36,"depth and two four-channel targets");
typedef struct {uint32_t before[4];coax_t_RRPixel pixel[16];uint32_t after[4];} Guarded;
static uint32_t bits(float value) {uint32_t out;memcpy(&out,&value,4);return out;}
int main(int argc,char **argv) {
 assert(argc==3||argc==4);int scenario=atoi(argv[2]);
 FILE *f=fopen(argv[1],"r");assert(f);uint32_t *tree=calloc(65536,4);assert(tree);int64_t count=0;uint32_t word;
 while(fscanf(f,"%" SCNu32,&word)==1){assert(count<65536);tree[count++]=word;}assert(feof(f));assert(fclose(f)==0);
 Guarded g;for(int k=0;k<4;k++){g.before[k]=0x719ac23d;g.after[k]=0xa1634e79;}
 for(int k=0;k<16;k++)g.pixel[k]=(coax_t_RRPixel){.depth=1,.colour={.r=scenario==9?3.0e38f:0.125f,.g=0.25f,.b=0.5f,.a=0.75f},.aux={.r=0.25f,.g=0.5f,.b=0.75f,.a=1}};
 int short_view=argc==4&&!strcmp(argv[3],"short");
 coax_t_RRPassResult result=rr_native_fixture((coax_v_u32){tree,count},scenario,(coax_v_t_RRPixel){g.pixel,short_view?3:16});
 for(int k=0;k<4;k++){assert(g.before[k]==0x719ac23d&&g.after[k]==0xa1634e79);}
 const char *fault=argc==4?argv[3]:"";
 if(!strcmp(fault,"status"))result.status=COAX_RQStatus_malformed;
 if(!strcmp(fault,"work"))result.steps++;
 if(!strcmp(fault,"publication"))result.written++;
 if(!strcmp(fault,"coverage"))result.fragments++;
 if(!strcmp(fault,"colour"))g.pixel[0].colour.r+=0.25f;
 if(!strcmp(fault,"depth"))g.pixel[0].depth+=0.25f;
 if(!strcmp(fault,"aux"))g.pixel[0].aux.g+=0.25f;
 if(!strcmp(fault,"masked_aux"))g.pixel[0].aux.r=0;
 if(!strcmp(fault,"vertex_colour"))g.pixel[0].colour.r+=0.25f;
 if(!strcmp(fault,"vertex_depth"))g.pixel[0].depth=1;
 if(!strcmp(fault,"void_depth"))g.pixel[1].depth=0.50000011920928955078125f;
 printf("{\"result\":[%u,%" PRId64 ",%" PRId64 ",%" PRId64 ",%" PRId64 ",%" PRId64 "],\"pixels\":[",(unsigned)result.status,result.steps,result.triangles,result.quads,result.fragments,result.written);
 for(int k=0;k<16;k++) {
  coax_t_RRPixel p=g.pixel[k];if(k)putchar(',');
  printf("[%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 "]",
   bits(p.depth),bits(p.colour.r),bits(p.colour.g),bits(p.colour.b),bits(p.colour.a),bits(p.aux.r),bits(p.aux.g),bits(p.aux.b),bits(p.aux.a));
 }
 puts("]}");free(tree);return 0;
}
