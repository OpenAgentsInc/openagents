// Verification effects only: load authored raw fixtures and call checked Coax.
#include "Filter.h"
#include <assert.h>
#include <stdbool.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *load(const char *path,size_t *bytes){
 FILE *file=fopen(path,"rb");assert(file);assert(!fseek(file,0,SEEK_END));long end=ftell(file);assert(end>=0);*bytes=(size_t)end;assert(!fseek(file,0,SEEK_SET));
 void *data=malloc(*bytes?*bytes:1);assert(data&&fread(data,1,*bytes,file)==*bytes&&!fclose(file));return data;
}
static coax_string text(const char *value){return(coax_string){value,(int64_t)strlen(value)};}
int main(int argc,char **argv){
 assert(argc==10);_Static_assert(sizeof(coax_t_RenderTextureLevel)==40,"fixture metadata is five 64-bit words");
 size_t bytes=0,level_bytes=0;unsigned char *source=load(argv[3],&bytes);coax_t_RenderTextureLevel *levels=load(argv[4],&level_bytes);assert(level_bytes%sizeof*levels==0);
 coax_t_RQColor colors[5];coax_t_RQMip mips[2];for(int i=0;i<5;i++)colors[i]=(coax_t_RQColor){-23,-23,-23,-23};for(int i=0;i<2;i++)mips[i]=(coax_t_RQMip){-23,-23,-23};
 int64_t used=(int64_t)bytes;if(!strcmp(argv[9],"short-bytes"))used--;
 coax_t_RFTextureDecoded decoded=rnf_decode(text(argv[2]),1,(coax_v_t_RenderTextureLevel){levels,(int64_t)(level_bytes/sizeof*levels)},(coax_v_u8){source,used},(coax_v_t_RQColor){colors,5},(coax_v_t_RQMip){mips,2});
 if(decoded.status!=COAX_RenderTextureStatus_ok){
  int preserved=1;for(int i=0;i<5;i++)if(colors[i].r!=-23||colors[i].g!=-23||colors[i].b!=-23||colors[i].a!=-23)preserved=0;for(int i=0;i<2;i++)if(mips[i].width!=-23||mips[i].height!=-23||mips[i].offset!=-23)preserved=0;
  printf("{\"decoded\":%u,\"sampled\":-1,\"output_preserved\":%s}\n",decoded.status,preserved?"true":"false");free(source);free(levels);return 0;
 }
 coax_t_RQSampler sampler;coax_default_t_RQSampler(&sampler);sampler.min_linear=true;sampler.mag_linear=true;sampler.mip_linear=true;
 if(!strcmp(argv[8],"min-nearest"))sampler.min_linear=false;
 if(!strcmp(argv[8],"mag-nearest"))sampler.mag_linear=false;
 if(!strcmp(argv[8],"mip-nearest"))sampler.mip_linear=false;
 if(!strcmp(argv[8],"anisotropic"))sampler.max_anisotropy=2;
 if(!strcmp(argv[8],"fractional-min"))sampler.lod_min=0.25f;
 if(!strcmp(argv[8],"clamp-integer"))sampler.lod_max=1;
 if(!strcmp(argv[9],"storage"))decoded.texture.storage=COAX_RQStorageFormat_unclassified;
 if(!strcmp(argv[9],"mips"))decoded.texture.levels.count=1;
 if(!strcmp(argv[9],"color"))colors[0].r+=0.125f;
 const char *profile=!strcmp(argv[9],"profile")?"analytic":argv[1];
 float x=strtof(argv[5],NULL),y=strtof(argv[6],NULL),lod=strtof(argv[7],NULL);coax_t_RQSample sampled;
 if(!strcmp(argv[8],"compare")){sampler.compare=COAX_RQCompare_less;sampled=rnf_compare(decoded.texture,text(profile),sampler,x,y,lod,0.5f);}
 else sampled=rnf_sample(decoded.texture,text(profile),sampler,x,y,lod);
 printf("{\"decoded\":%u,\"sampled\":%u,\"storage\":%u,\"rgba\":[%.17g,%.17g,%.17g,%.17g]}\n",decoded.status,sampled.status,decoded.texture.storage,(double)sampled.color.r,(double)sampled.color.g,(double)sampled.color.b,(double)sampled.color.a);
 free(source);free(levels);return 0;
}
