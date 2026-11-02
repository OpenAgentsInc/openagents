// File/allocation transport only. Coax authenticates/decodes the trees and
// chooses every pass, draw, resource, sample, framebuffer and refusal.
#include "reference-frame.h"
#include <assert.h>
#include <stdbool.h>
#include "frame-input.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(float)==4 && sizeof(coax_t_RQColor)==16,"recorded four-f32 colour carrier");
_Static_assert(__BYTE_ORDER__==__ORDER_LITTLE_ENDIAN__,"recorded producer word profile is little endian");
typedef struct Allocation {void *pointer;struct Allocation *next;} Allocation;
static Allocation *allocations;
static RfiFrame raw;
static void close_all(void){while(allocations){Allocation *a=allocations;allocations=a->next;free(a->pointer);free(a);}rfi_close(&raw);}
static void *owned(int64_t count,size_t stride) {assert(count>=0);void *p=calloc(count?(size_t)count:1,stride);assert(p);Allocation *a=malloc(sizeof*a);assert(a);*a=(Allocation){p,allocations};allocations=a;return p;}
int main(int argc,char **argv) {
 assert(argc==11);char *bits_end=NULL;int64_t bits=strtoll(argv[5],&bits_end,10);assert(bits_end!=argv[5]&&*bits_end==0);int64_t guard=strtoll(argv[7],&bits_end,10);assert(bits_end!=argv[7]&&*bits_end==0);assert(!strcmp(argv[8],"rgba8")||!strcmp(argv[8],"rgba16f"));assert(!atexit(close_all));char error[256];if(!rfi_load(argv[1],&raw,error,sizeof error)){fprintf(stderr,"%s\n",error);return 2;}
 coax_t_RFInput input={.profile=raw.counts[0],.counts={raw.counts[1],raw.counts[2],raw.counts[3],raw.counts[4],raw.counts[5],raw.counts[6],raw.counts[7],raw.counts[8],raw.counts[9],raw.counts[10],raw.counts[11]},.ready=COAX_RenderReadiness_ready,.particles=true};coax_t_RenderHostBool coarse_flag=rfn_bool(raw.counts[12]);if(!coarse_flag.ok){fprintf(stderr,"noncanonical coarse flag\n");return 2;}bool coarse=coarse_flag.value;
 assert(raw.uniforms.size%4==0&&raw.hud.size%4==0&&raw.draws.size%4==0&&raw.indices.size%4==0);
 input.uniforms=(coax_v_f32){(float*)raw.uniforms.data,(int64_t)(raw.uniforms.size/4)};input.hud=(coax_v_f32){(float*)raw.hud.data,(int64_t)(raw.hud.size/4)};
 input.draw_words=(coax_v_u32){(uint32_t*)raw.draws.data,(int64_t)(raw.draws.size/4)};input.indices=(coax_v_u32){(uint32_t*)raw.indices.data,(int64_t)(raw.indices.size/4)};
 int64_t programs_count=(int64_t)raw.shader_count;coax_t_RFShader *programs=owned(programs_count,sizeof*programs);int64_t *metadata=owned(8192,sizeof*metadata);
 for(int64_t k=0;k<programs_count;k++) {
  RfiShader *s=&raw.shaders[k];uint32_t *words=owned((int64_t)s->bytes.size,sizeof*words);
  coax_t_RFPackageTree admitted=rfn_load((coax_string){(char*)raw.package.data,(int64_t)raw.package.size},input.profile,s->index,(coax_v_u8){s->bytes.data,(int64_t)s->bytes.size},(coax_v_u32){words,(int64_t)s->bytes.size},(coax_v_s64){metadata,8192});
  if(admitted.status!=COAX_RQStatus_ok){fprintf(stderr,"tree admission refused: input=%" PRId64 " status=%u package=%u transport=%u\n",k,(unsigned)admitted.status,(unsigned)admitted.package_status,(unsigned)admitted.transport.status);return 3;}
  programs[k]=(coax_t_RFShader){.index=s->index,.tree={words,admitted.transport.count}};
 }
 int64_t buffer_count=(int64_t)raw.buffer_count;coax_t_RenderHostNamedBuffer *buffers=owned(buffer_count,sizeof*buffers);
 for(int64_t k=0;k<buffer_count;k++) {
  RfiBuffer *b=&raw.buffers[k];assert(b->bytes.size%4==0);coax_t_RenderHostBool zero_flag=rfn_bool(b->zero_initialized);if(!zero_flag.ok){fprintf(stderr,"noncanonical buffer initialized flag\n");return 2;}buffers[k].name=(coax_string){b->name,(int64_t)strlen(b->name)};buffers[k].words=(coax_v_u32){(uint32_t*)b->bytes.data,(int64_t)(b->bytes.size/4)};
  buffers[k].observed=(coax_t_RenderBufferObserved){.source_capacity=b->source_capacity,.logical_payload=b->logical_payload,.initialized_highwater=b->initialized_highwater,.upload_bytes=b->upload_bytes,.gpu_bytes=b->gpu_bytes,.alignment=4,.zero_initialized=zero_flag.value};
 }
 input.buffers=(coax_v_t_RenderHostNamedBuffer){buffers,buffer_count};
 int64_t texture_count=(int64_t)raw.texture_count;coax_t_RFNamedTexture *textures=owned(texture_count,sizeof*textures);
 for(int64_t k=0;k<texture_count;k++) {
  RfiTexture *t=&raw.textures[k];coax_t_RenderTextureLevel *source_levels=owned((int64_t)t->level_count,sizeof*source_levels);
  for(size_t l=0;l<t->level_count;l++)source_levels[l]=(coax_t_RenderTextureLevel){.width=t->levels[l].width,.height=t->levels[l].height,.offset=t->levels[l].offset,.row_bytes=t->levels[l].row_bytes,.layer_bytes=t->levels[l].layer_bytes};
  coax_string format={t->format,(int64_t)strlen(t->format)};coax_v_t_RenderTextureLevel levels={source_levels,(int64_t)t->level_count};coax_t_RFTextureNeed need=rfn_texture_need(format,t->layers,levels);
  if(need.status!=COAX_RenderTextureStatus_ok){fprintf(stderr,"texture allocation refused: input=%" PRId64 " status=%u\n",k,(unsigned)need.status);return 5;}
  coax_t_RQColor *pixels=owned(need.pixels,sizeof*pixels);coax_t_RQMip *mips=owned((int64_t)t->level_count,sizeof*mips);
  coax_t_RFTextureDecoded decoded=rfn_texture_decode(format,t->layers,levels,(coax_v_u8){t->bytes.data,(int64_t)t->bytes.size},(coax_v_t_RQColor){pixels,need.pixels},(coax_v_t_RQMip){mips,(int64_t)t->level_count});
  if(decoded.status!=COAX_RenderTextureStatus_ok){fprintf(stderr,"texture decode refused: input=%" PRId64 " status=%u level=%" PRId64 " texel=%" PRId64 "\n",k,(unsigned)decoded.status,decoded.bad_level,decoded.bad_texel);return 6;}
  textures[k]=(coax_t_RFNamedTexture){.name={t->name,(int64_t)strlen(t->name)},.texture=decoded.texture};
 }
 input.textures=(coax_v_t_RFNamedTexture){textures,texture_count};
 coax_t_RenderHostMediaImage *media_images=owned((int64_t)raw.media_image_count,sizeof*media_images);
 for(size_t k=0;k<raw.media_image_count;k++){RfiMediaImage *m=&raw.media_images[k];coax_t_RenderHostMediaObserved observed=rfn_media_observe_raw(m->kind,(coax_string){m->url,(int64_t)strlen(m->url)},m->ready,m->width,m->height);if(observed.status!=COAX_RenderHostMediaStatus_ok){fprintf(stderr,"raw media image refused: input=%zu\n",k);return 8;}media_images[k]=observed.image;}
 coax_t_RenderMediaRect *media=owned(112,sizeof*media);
 if(raw.media_text.size){coax_t_RFMediaPrepared prepared=rfn_prepare_media(input.profile,(coax_string){(char*)raw.media_text.data,(int64_t)raw.media_text.size},input.counts.width,input.counts.height,input.textures,(coax_v_t_RenderHostMediaImage){media_images,(int64_t)raw.media_image_count},(coax_v_t_RenderMediaRect){media,112});if(prepared.status!=COAX_RQStatus_ok){fprintf(stderr,"media preparation refused status=%u texture=%" PRId64 "\n",(unsigned)prepared.status,prepared.bad_bitmap);return 7;}input.media=(coax_v_t_RenderMediaRect){media,prepared.count};}
 coax_t_RFRequirements req=rfn_require(input.profile,input.counts,coarse!=0);
 if(req.status!=COAX_RQStatus_ok||req.loading){fprintf(stderr,"workspace refusal status=%u loading=%u\n",(unsigned)req.status,(unsigned)req.loading);return 4;}
 coax_t_RFWorking work={.images={owned(req.image_pixels,sizeof(coax_t_RQColor)),req.image_pixels},.target={owned(req.pass_pixels,sizeof(coax_t_RRPixel)),req.pass_pixels},.scratch={owned(req.pass_pixels,sizeof(coax_t_RRPixel)),req.pass_pixels},.words={owned(req.word_words,sizeof(uint32_t)),req.word_words},.meta={owned(req.meta_words,sizeof(int64_t)),req.meta_words},.domains={owned(input.counts.draw_count,sizeof(coax_t_RenderDrawDomain)),input.counts.draw_count},.decoded={owned(input.counts.draw_count,sizeof(coax_t_RenderDecodedDraw)),input.counts.draw_count}};
 int64_t pixels=input.counts.width*input.counts.height;coax_t_RQColor *out=owned(pixels+1,sizeof*out);out[pixels]=(coax_t_RQColor){91,92,93,94};
 coax_t_RFPolicy policy=rfn_work_policy((coax_string){argv[9],(int64_t)strlen(argv[9])},(coax_string){argv[4],(int64_t)strlen(argv[4])},bits,(coax_string){argv[6],(int64_t)strlen(argv[6])},guard);policy=rfn_filter_policy(policy,(coax_string){argv[10],(int64_t)strlen(argv[10])});coax_t_RFResult result=(!strcmp(argv[8],"rgba16f")?rfn_render_half:rfn_render)(input,(coax_v_t_RFShader){programs,programs_count},coarse!=0,policy,work,(coax_v_t_RQColor){out,pixels+1});
 assert(out[pixels].r==91&&out[pixels].g==92&&out[pixels].b==93&&out[pixels].a==94);
 FILE *image=fopen(argv[2],"wb");assert(image);assert(fwrite(out,sizeof*out,(size_t)pixels,image)==(size_t)pixels);assert(!fclose(image));
 FILE *record=fopen(argv[3],"w");assert(record);fprintf(record,"{\"filter_profile\":%u,\"work_profile\":%u,\"shader_ceiling\":%" PRId64 ",\"pixel_ceiling\":%" PRId64 ",\"status\":%u,\"geometry\":%u,\"frame\":%u,\"bad_op\":%" PRId64 ",\"bad_shader\":%" PRId64 ",\"bad_media\":%" PRId64 ",\"shader_steps\":%" PRId64 ",\"pixel_steps\":%" PRId64 ",\"draws\":%" PRId64 ",\"written\":%" PRId64 ",\"pass_steps\":[",(unsigned)policy.filter,(unsigned)policy.work_profile,policy.shader_work,policy.pixel_work,(unsigned)result.status,(unsigned)result.geometry,(unsigned)result.frame,result.bad_op,result.bad_shader,result.bad_media,result.shader_steps,result.pixel_steps,result.draws,result.written);
 for(int k=0;k<14;k++)fprintf(record,"%s%" PRId64,k?",":"",result.pass_steps.e[k]);fputs("]}\n",record);assert(!fclose(record));
 return 0;
}
