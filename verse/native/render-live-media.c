// Firm reason: 6 (SDK: physical files, canonical borrowed pixels and Vulkan uploads).
#define _GNU_SOURCE
#include "render-live-media.h"
#include "render-images.h"
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
typedef struct {RhgiImage cpu;VksImage gpu;RhgHandle handle;int failed;} RhglmImage;
struct RhgLiveMedia {
 Vkc *device;RhgiImages *cache;char *mount;size_t capacity;
 RhglmImage *images,**lines;
 coax_t_RenderMediaRequest *requests;coax_t_RenderHostMediaImage *observed;
 coax_t_RenderHostMediaTexture *physical;coax_t_HostMediaNotice *notices;
 char (*names)[32];uint8_t *notice,*last_notice;size_t notice_capacity,notice_bytes,last_bytes;
};
static int lm_same(coax_string a,coax_string b){return a.count==b.count&&(a.count==0||!memcmp(a.data,b.data,(size_t)a.count));}
static int lm_error(char *out,size_t n,const char *text){if(n)snprintf(out,n,"%s",text);return 0;}
// Check the actual opened descriptor's canonical location, not a path checked
// before a symlink race. No bytes or metadata publish before containment.
static int lm_file(RhgLiveMedia *m,coax_string url){
 uint8_t relative[12288];int64_t n=hmb_path(url,(coax_v_u8){relative,sizeof relative});if(n<1)return -1;
 size_t prefix=strlen(m->mount);char *path=malloc(prefix+(size_t)n+1);if(!path)return -1;memcpy(path,m->mount,prefix);memcpy(path+prefix,relative,(size_t)n);path[prefix+(size_t)n]=0;
 int fd=open(path,O_RDONLY|O_CLOEXEC);free(path);if(fd<0)return -1;
 char link[64];snprintf(link,sizeof link,"/proc/self/fd/%d",fd);char *actual=realpath(link,NULL);bool inside=actual&&hmb_inside((coax_string){m->mount,(int64_t)prefix},(coax_string){actual,actual?(int64_t)strlen(actual):0});free(actual);
 if(!inside){close(fd);return -1;}return fd;
}
static int64_t lm_size(void *context,coax_string url){RhgLiveMedia *m=context;int fd=lm_file(m,url);if(fd<0)return -1;struct stat s;int64_t result=fstat(fd,&s)==0&&S_ISREG(s.st_mode)&&s.st_size>=0&&s.st_size<=INT64_MAX?(int64_t)s.st_size:-1;close(fd);return result;}
static int lm_read(void *context,coax_string url,uint8_t *out,size_t bytes){
 RhgLiveMedia *m=context;int fd=lm_file(m,url);if(fd<0)return 0;struct stat before,after;int ok=fstat(fd,&before)==0&&S_ISREG(before.st_mode)&&before.st_size>=0&&(uint64_t)before.st_size==bytes;size_t at=0;
 while(ok&&at<bytes){ssize_t n=read(fd,out+at,bytes-at);if(n<1)ok=0;else at+=(size_t)n;}
 uint8_t extra;if(ok&&read(fd,&extra,1)!=0)ok=0;
 if(ok&&(fstat(fd,&after)||before.st_dev!=after.st_dev||before.st_ino!=after.st_ino||before.st_size!=after.st_size||before.st_mtim.tv_sec!=after.st_mtim.tv_sec||before.st_mtim.tv_nsec!=after.st_mtim.tv_nsec||before.st_ctim.tv_sec!=after.st_ctim.tv_sec||before.st_ctim.tv_nsec!=after.st_ctim.tv_nsec))ok=0;
 close(fd);return ok;
}
RhgLiveMedia *rhglm_open(Vkc *device,const char *mount,size_t notice_capacity){
 if(!device||!mount||notice_capacity>INT64_MAX)return NULL;RhgLiveMedia *m=calloc(1,sizeof*m);if(!m)return NULL;m->device=device;m->capacity=(size_t)hic_capacity();m->notice_capacity=notice_capacity;
 m->mount=realpath(mount,NULL);m->cache=rhgi_open();m->images=rfi_array(m->capacity,sizeof*m->images);m->lines=rfi_array(m->capacity,sizeof*m->lines);m->requests=rfi_array(m->capacity,sizeof*m->requests);m->observed=rfi_array(m->capacity,sizeof*m->observed);m->physical=rfi_array(m->capacity,sizeof*m->physical);m->notices=rfi_array(m->capacity,sizeof*m->notices);m->names=rfi_array(m->capacity,sizeof*m->names);m->notice=rfi_array(notice_capacity,1);m->last_notice=rfi_array(notice_capacity,1);
 if(!m->mount||!m->cache||m->capacity!=(size_t)aethyr_render_media_capacity()||!m->images||!m->lines||!m->requests||!m->observed||!m->physical||!m->notices||!m->names||!m->notice||!m->last_notice){rhglm_close(m);return NULL;}return m;
}
static RhglmImage *lm_upload(RhgLiveMedia *m,RhgiImage image){
 if(image.slot<0||(uint64_t)image.slot>=m->capacity)return NULL;RhglmImage *p=&m->images[image.slot];p->cpu=image;if(!image.decoded||p->gpu.image||p->failed)return p;
 coax_t_RenderAssetMip extent=aethyr_render_asset_mip(image.width,image.height,1,0,m->device->props.limits.maxImageDimension2D,m->device->props.limits.maxImageArrayLayers);
 if(!extent.valid||!rhg_ledger_acquire(RHG_TEXTURE,aethyr_render_texture_words(image.width,image.height,1,1,4),&p->handle)){p->failed=1;return p;}
 p->gpu=vks_image_levels(m->device,(uint32_t)extent.width,(uint32_t)extent.height,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,1);vks_upload(m->device,&p->gpu,image.pixels,image.bytes);return p;
}
int rhglm_prepare(RhgLiveMedia *m,int64_t profile,coax_string packet,int64_t width,int64_t height,RhgMediaRects *out,char *error,size_t error_bytes){
 if(!m)return 0;m->notice_bytes=0;memset(m->lines,0,m->capacity*sizeof*m->lines);
 coax_t_RenderMediaParse parsed=aethyr_render_media_parse(packet,(coax_v_t_RenderMediaRequest){m->requests,(int64_t)m->capacity});if(parsed.status!=COAX_RenderMediaStatus_ok)return lm_error(error,error_bytes,"Coax refused live media packet");
 size_t count=0,physical_count=0;
 for(int64_t i=0;i<parsed.count;i++){
  coax_t_RenderMediaRequest request=m->requests[i];RhgiImage cpu=rhgi_get(m->cache,(int64_t)request.kind,request.url,(RhgiSource){lm_size,lm_read,m});RhglmImage *image=lm_upload(m,cpu);bool ready=image&&image->gpu.image&&rhg_ledger_live(&image->handle);bool failed=cpu.failed||(image&&image->failed);coax_string url=image?image->cpu.url:request.url;
  bool duplicate=false;for(size_t j=0;j<count;j++)if(m->observed[j].kind==request.kind&&lm_same(m->observed[j].url,url)){duplicate=true;break;}
  if(!duplicate){m->observed[count]=(coax_t_RenderHostMediaImage){.kind=request.kind,.url=url,.ready=ready,.width=ready?cpu.width:0,.height=ready?cpu.height:0};m->notices[count]=(coax_t_HostMediaNotice){.url=url,.ready=ready,.failed=failed,.width=ready?cpu.width:0,.height=ready?cpu.height:0};count++;}
  if(ready){if(request.line<0||(uint64_t)request.line>=m->capacity)return lm_error(error,error_bytes,"Coax media line exceeds physical SDK storage");m->lines[request.line]=image;coax_string name=aethyr_render_texture_name((coax_t_RenderBindingSource){.kind=COAX_RenderSourceKind_media_image,.index=request.line,.valid=true},(coax_v_u8){(uint8_t*)m->names[physical_count],32});m->physical[physical_count++]=(coax_t_RenderHostMediaTexture){.name=name,.width=cpu.width,.height=cpu.height,.layers=1};}
 }
 RhgMediaRects candidate={0};candidate.rects=rfi_array(m->capacity,sizeof*candidate.rects);if(!candidate.rects)return 0;
 candidate.receipt=aethyr_render_media_draws(profile,(coax_v_t_RenderMediaRequest){m->requests,parsed.count},(coax_v_t_RenderHostMediaImage){m->observed,(int64_t)count},width,height,(coax_v_t_RenderHostMediaTexture){m->physical,(int64_t)physical_count},(coax_v_t_RenderMediaRect){candidate.rects,(int64_t)m->capacity});
 if(candidate.receipt.status!=COAX_RenderHostMediaStatus_ok){rhgm_close(&candidate);return lm_error(error,error_bytes,"Coax refused actual live media composition");}
 int64_t n=rhgi_notice(m->notices,count,m->notice,m->notice_capacity);if(n<0){rhgm_close(&candidate);return lm_error(error,error_bytes,"Coax refused complete live media notice");}
 if(n&&(m->last_bytes!=(size_t)n||memcmp(m->last_notice,m->notice,(size_t)n)))m->notice_bytes=(size_t)n;*out=candidate;return 1;
}
const VksImage *rhglm_binding(RhgLiveMedia *m,int64_t line){if(!m||line<0||(uint64_t)line>=m->capacity)return NULL;RhglmImage *p=m->lines[line];return p&&p->gpu.image&&rhg_ledger_live(&p->handle)?&p->gpu:NULL;}
const uint8_t *rhglm_notice(RhgLiveMedia *m,size_t *bytes){*bytes=m?m->notice_bytes:0;return m?m->notice:NULL;}
void rhglm_notice_done(RhgLiveMedia *m){if(!m||!m->notice_bytes)return;memcpy(m->last_notice,m->notice,m->notice_bytes);m->last_bytes=m->notice_bytes;m->notice_bytes=0;}
void rhglm_drop_gpu(RhgLiveMedia *m){if(!m)return;for(size_t i=0;i<m->capacity;i++){RhglmImage *p=&m->images[i];if(p->gpu.image)vks_image_free(m->device,&p->gpu);memset(&p->gpu,0,sizeof p->gpu);rhg_ledger_release(&p->handle);p->failed=0;}m->last_bytes=0;m->notice_bytes=0;if(m->lines)memset(m->lines,0,m->capacity*sizeof*m->lines);}
void rhglm_device(RhgLiveMedia *m,Vkc *device){if(m)m->device=device;}
const VksImage *rhglm_image(RhgLiveMedia *m,coax_string url){if(!m)return NULL;for(size_t i=0;i<m->capacity;i++){RhglmImage *p=&m->images[i];if(lm_same(p->cpu.url,url)&&p->gpu.image&&rhg_ledger_live(&p->handle))return &p->gpu;}return NULL;}
int64_t rhglm_info(RhgLiveMedia *m,coax_string url,int field){if(!m)return 0;if(field==5)return(int64_t)rhgi_count(m->cache);if(field==6)return(int64_t)rhgi_bytes(m->cache);if(field==7||field==8){int64_t n=0;for(size_t i=0;i<m->capacity;i++){RhglmImage *p=&m->images[i];if(field==7){if(p->gpu.image&&rhg_ledger_live(&p->handle))n++;}else if(p->cpu.failed||p->failed)n++;}return n;}for(size_t i=0;i<m->capacity;i++){RhglmImage *p=&m->images[i];if(p->cpu.url.count&&lm_same(p->cpu.url,url)){switch(field){case 0:return 1;case 1:return p->gpu.image&&rhg_ledger_live(&p->handle);case 2:return p->cpu.failed||p->failed;case 3:return p->gpu.image?p->cpu.width:0;case 4:return p->gpu.image?p->cpu.height:0;}}}return 0;}
int64_t rhglm_close(RhgLiveMedia *m){if(!m)return 0;if(m->images)rhglm_drop_gpu(m);int64_t remaining=rhgi_close(m->cache);free(m->mount);free(m->images);free(m->lines);free(m->requests);free(m->observed);free(m->physical);free(m->notices);free(m->names);free(m->notice);free(m->last_notice);free(m);return remaining;}
