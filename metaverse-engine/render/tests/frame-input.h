// Raw measurement-file transport shared by CPU and Vulkan drivers. This
// header does no resource-role lookup, Coax admission, decoding or rendering.
// Paths are tokens relative to frame.config (or absolute); '-' is empty.
#ifndef AETHYR_FRAME_INPUT_H
#define AETHYR_FRAME_INPUT_H
#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {char *path;unsigned char *data;size_t size;} RfiBytes;
typedef struct {int64_t index;RfiBytes bytes;} RfiShader;
typedef struct {
  char *name;RfiBytes bytes;
  uint64_t source_capacity,logical_payload,initialized_highwater,upload_bytes,gpu_bytes;
  int64_t zero_initialized;
} RfiBuffer;
typedef struct {int64_t width,height;uint64_t offset,row_bytes,layer_bytes;} RfiLevel;
typedef struct {char *name,*format;int64_t layers;size_t level_count;RfiLevel *levels;RfiBytes bytes;} RfiTexture;
typedef struct {int64_t kind;char *url;int64_t ready,width,height;} RfiMediaImage;
typedef struct {
  // profile,width,height,hud,stride,draws,water,grass,particles,surface,
  // caster_vertices,caster_instances,coarse; these remain raw observations.
  int64_t counts[13];RfiBytes uniforms,hud,draws,indices,package,media_text;
  size_t shader_count,buffer_count,texture_count;
  RfiShader *shaders;RfiBuffer *buffers;RfiTexture *textures;
  size_t media_image_count;RfiMediaImage *media_images;
} RfiFrame;

static inline void rfi_bytes_close(RfiBytes *b){free(b->path);free(b->data);memset(b,0,sizeof*b);}
static inline void rfi_close(RfiFrame *f){
  rfi_bytes_close(&f->uniforms);rfi_bytes_close(&f->hud);rfi_bytes_close(&f->draws);rfi_bytes_close(&f->indices);rfi_bytes_close(&f->package);rfi_bytes_close(&f->media_text);
  if(f->shaders)for(size_t i=0;i<f->shader_count;i++)rfi_bytes_close(&f->shaders[i].bytes);
  if(f->buffers)for(size_t i=0;i<f->buffer_count;i++){free(f->buffers[i].name);rfi_bytes_close(&f->buffers[i].bytes);}
  if(f->textures)for(size_t i=0;i<f->texture_count;i++){free(f->textures[i].name);free(f->textures[i].format);free(f->textures[i].levels);rfi_bytes_close(&f->textures[i].bytes);}
  if(f->media_images)for(size_t i=0;i<f->media_image_count;i++)free(f->media_images[i].url);
  free(f->shaders);free(f->buffers);free(f->textures);free(f->media_images);memset(f,0,sizeof*f);
}
static inline char *rfi_copy(const char *s){size_t n=strlen(s);char *p=malloc(n+1);if(p)memcpy(p,s,n+1);return p;}
static inline int rfi_token(FILE *f,char **out){char token[4096];if(fscanf(f,"%4095s",token)!=1)return 0;*out=rfi_copy(token);return *out!=NULL;}
static inline int rfi_i64(FILE *f,int64_t *out){char *token=NULL;if(!rfi_token(f,&token))return 0;errno=0;char *end;intmax_t value=strtoimax(token,&end,10);int ok=errno!=ERANGE&&end!=token&&*end==0&&value>=INT64_MIN&&value<=INT64_MAX;if(ok)*out=(int64_t)value;free(token);return ok;}
static inline int rfi_u64(FILE *f,uint64_t *out){char *token=NULL;if(!rfi_token(f,&token))return 0;errno=0;char *end;uintmax_t value=strtoumax(token,&end,10);int ok=token[0]!='-'&&errno!=ERANGE&&end!=token&&*end==0&&value<=UINT64_MAX;if(ok)*out=(uint64_t)value;free(token);return ok;}
static inline void *rfi_array(size_t count,size_t stride){if(count>SIZE_MAX/stride)return NULL;return calloc(count?count:1,stride);}
static inline int rfi_count(FILE *f,size_t *out){int64_t n;if(!rfi_i64(f,&n)||n<0||(uint64_t)n>SIZE_MAX)return 0;*out=(size_t)n;return 1;}
static inline char *rfi_path(const char *config,const char *token){
  if(token[0]=='/'||!strcmp(token,"-"))return rfi_copy(token);
  const char *slash=strrchr(config,'/');size_t prefix=slash?(size_t)(slash-config)+1:0,n=strlen(token);
  if(prefix>SIZE_MAX-n-1)return NULL;char *p=malloc(prefix+n+1);if(p){memcpy(p,config,prefix);memcpy(p+prefix,token,n+1);}return p;
}
static inline int rfi_file(FILE *config,const char *config_path,RfiBytes *out){
  char *token=NULL;if(!rfi_token(config,&token))return 0;out->path=rfi_path(config_path,token);free(token);if(!out->path)return 0;
  if(!strcmp(out->path,"-")){out->data=calloc(1,1);return out->data!=NULL;}
  FILE *f=fopen(out->path,"rb");if(!f)return 0;int ok=0;
  if(fseek(f,0,SEEK_END)==0){long size=ftell(f);if(size>=0&&(uint64_t)size<SIZE_MAX&&fseek(f,0,SEEK_SET)==0){
    out->size=(size_t)size;out->data=calloc(out->size+1,1);if(out->data&&fread(out->data,1,out->size,f)==out->size)ok=1;
  }}
  if(fclose(f)!=0)ok=0;return ok;
}
// The live host uses this same descriptor suffix for immutable GPU inputs,
// without loading a captured producer's counts, clock or mutable buffers.
static inline int rfi_suffix(FILE *f,const char *path,RfiFrame *candidate){
  char suffix[32];int trailing=fscanf(f,"%31s",suffix);
  if(trailing==1&&!strcmp(suffix,"textures")){
    if(!rfi_count(f,&candidate->texture_count)||(candidate->textures=rfi_array(candidate->texture_count,sizeof*candidate->textures))==NULL)return 0;
    for(size_t i=0;i<candidate->texture_count;i++){
      RfiTexture *t=&candidate->textures[i];
      if(!rfi_token(f,&t->name)||!rfi_token(f,&t->format)||!rfi_i64(f,&t->layers)||!rfi_count(f,&t->level_count)||!rfi_file(f,path,&t->bytes))return 0;
      t->levels=rfi_array(t->level_count,sizeof*t->levels);if(!t->levels)return 0;
      for(size_t k=0;k<t->level_count;k++){RfiLevel *l=&t->levels[k];if(!rfi_i64(f,&l->width)||!rfi_i64(f,&l->height)||!rfi_u64(f,&l->offset)||!rfi_u64(f,&l->row_bytes)||!rfi_u64(f,&l->layer_bytes))return 0;}
    }
    trailing=fscanf(f,"%31s",suffix);
  }
  // This is the exact borrowed command text. Coax owns parsing, URL/kind
  // matching, the112-line window and rectangle admission.
  if(trailing==1&&!strcmp(suffix,"media_text")){
    if(!rfi_file(f,path,&candidate->media_text))return 0;trailing=fscanf(f,"%31s",suffix);
  }
  if(trailing==1&&!strcmp(suffix,"media_images")){
    if(!rfi_count(f,&candidate->media_image_count)||(candidate->media_images=rfi_array(candidate->media_image_count,sizeof*candidate->media_images))==NULL)return 0;
    for(size_t i=0;i<candidate->media_image_count;i++){RfiMediaImage *m=&candidate->media_images[i];if(!rfi_i64(f,&m->kind)||!rfi_token(f,&m->url)||!rfi_i64(f,&m->ready)||!rfi_i64(f,&m->width)||!rfi_i64(f,&m->height))return 0;}
    trailing=fscanf(f,"%31s",suffix);
  }
  if(trailing!=EOF)return 0;
  return 1;
}
// No destination publication until every file and observation row was read.
// Negative/oversized array counts fail representability/allocation here;
// semantic counts, source ranges and formats are admitted later in Coax.
static inline int rfi_load(const char *path,RfiFrame *out,char *error,size_t error_size){
  RfiFrame candidate={0};FILE *f=fopen(path,"r");if(!f){if(error_size)snprintf(error,error_size,"cannot open frame.config");return 0;}
  for(size_t i=0;i<13;i++)if(!rfi_i64(f,&candidate.counts[i]))goto bad;
  if(!rfi_file(f,path,&candidate.uniforms)||!rfi_file(f,path,&candidate.hud)||!rfi_file(f,path,&candidate.draws)||!rfi_file(f,path,&candidate.indices)||!rfi_file(f,path,&candidate.package))goto bad;
  if(!rfi_count(f,&candidate.shader_count)||(candidate.shaders=rfi_array(candidate.shader_count,sizeof*candidate.shaders))==NULL)goto bad;
  for(size_t i=0;i<candidate.shader_count;i++)if(!rfi_i64(f,&candidate.shaders[i].index)||!rfi_file(f,path,&candidate.shaders[i].bytes))goto bad;
  if(!rfi_count(f,&candidate.buffer_count)||(candidate.buffers=rfi_array(candidate.buffer_count,sizeof*candidate.buffers))==NULL)goto bad;
  for(size_t i=0;i<candidate.buffer_count;i++){
    RfiBuffer *b=&candidate.buffers[i];if(!rfi_token(f,&b->name)||!rfi_file(f,path,&b->bytes))goto bad;
    if(!rfi_u64(f,&b->source_capacity)||!rfi_u64(f,&b->logical_payload)||!rfi_u64(f,&b->initialized_highwater)||!rfi_u64(f,&b->upload_bytes)||!rfi_u64(f,&b->gpu_bytes)||!rfi_i64(f,&b->zero_initialized))goto bad;
  }
  if(!rfi_suffix(f,path,&candidate))goto bad;
  if(fclose(f)!=0){f=NULL;goto bad;}*out=candidate;return 1;
bad:
  if(f)fclose(f);rfi_close(&candidate);if(error_size)snprintf(error,error_size,"truncated, unreadable or unrepresentable frame input");return 0;
}
static inline int rfi_load_textures(const char *path,RfiFrame *out,char *error,size_t error_size){
  RfiFrame candidate={0};FILE *f=fopen(path,"r");if(!f){if(error_size)snprintf(error,error_size,"cannot open texture descriptors");return 0;}
  int ok=rfi_suffix(f,path,&candidate);if(fclose(f)!=0)ok=0;
  if(!ok){rfi_close(&candidate);if(error_size)snprintf(error,error_size,"truncated, unreadable or unrepresentable texture input");return 0;}
  *out=candidate;return 1;
}
#endif
