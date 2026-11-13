#include "render-media.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
void rhgm_close(RhgMediaRects *media){free(media->rects);memset(media,0,sizeof*media);}
static int rhgm_error(char *error,size_t bytes,const char *message){if(bytes)snprintf(error,bytes,"%s",message);return 0;}
int rhgm_rects(int64_t profile,const RfiFrame *raw,RhgTextures *textures,int64_t width,int64_t height,RhgMediaRects *out,char *error,size_t error_bytes){
  int64_t capacity=aethyr_render_media_capacity();coax_t_RenderMediaRequest *requests=rfi_array((size_t)capacity,sizeof*requests);
  coax_t_RenderHostMediaImage *images=rfi_array(raw->media_image_count,sizeof*images);
  coax_t_RenderHostMediaTexture *physical=rfi_array(raw->texture_count,sizeof*physical);RhgMediaRects candidate={0};candidate.rects=rfi_array((size_t)capacity,sizeof*candidate.rects);
  if(!requests||!images||!physical||!candidate.rects){free(requests);free(images);free(physical);rhgm_close(&candidate);return 0;}
  coax_t_RenderMediaParse parsed=aethyr_render_media_parse((coax_string){(char*)raw->media_text.data,(int64_t)raw->media_text.size},(coax_v_t_RenderMediaRequest){requests,capacity});
  if(parsed.status!=COAX_RenderMediaStatus_ok){free(requests);free(images);free(physical);rhgm_close(&candidate);return rhgm_error(error,error_bytes,"media text capacity refused by Coax");}
  for(size_t i=0;i<raw->media_image_count;i++){
    RfiMediaImage image=raw->media_images[i];coax_t_RenderHostMediaObserved observed=aethyr_render_media_observe(image.kind,(coax_string){image.url,(int64_t)strlen(image.url)},image.ready,image.width,image.height);
    if(observed.status!=COAX_RenderHostMediaStatus_ok){free(requests);free(images);free(physical);rhgm_close(&candidate);return rhgm_error(error,error_bytes,"raw media observation refused by Coax");}images[i]=observed.image;
  }
  coax_v_t_RenderHostNamedTexture names=rhgt_names(textures);
  for(size_t i=0;i<raw->texture_count;i++){
    const RfiTexture *texture=&raw->textures[i];physical[i]=(coax_t_RenderHostMediaTexture){.name=names.data[i].name,.width=texture->levels[0].width,.height=texture->levels[0].height,.layers=names.data[i].layers,.depth=names.data[i].depth};
  }
  candidate.receipt=aethyr_render_media_draws(profile,(coax_v_t_RenderMediaRequest){requests,parsed.count},(coax_v_t_RenderHostMediaImage){images,(int64_t)raw->media_image_count},width,height,
    (coax_v_t_RenderHostMediaTexture){physical,(int64_t)raw->texture_count},(coax_v_t_RenderMediaRect){candidate.rects,capacity});
  free(requests);free(images);free(physical);
  if(candidate.receipt.status!=COAX_RenderHostMediaStatus_ok){rhgm_close(&candidate);return rhgm_error(error,error_bytes,"media identity/texture candidate refused by Coax");}
  *out=candidate;return 1;
}
