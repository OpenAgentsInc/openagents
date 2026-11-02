// Actual Vulkan effects for the ordinary Coax pipeline fixture. Expected
// pixels below are independently authored affine/finite-difference values.
#include "vks.h"
#include "RenderCaps.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t *read_words(const char *name, size_t *count) {
  FILE *f=fopen(name,"rb"); if(!f){perror(name);exit(2);}
  fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);
  if(n<20 || n%4){fprintf(stderr,"invalid shader bytes\n");exit(2);}
  uint32_t *words=malloc((size_t)n);
  if(!words || fread(words,1,(size_t)n,f)!=(size_t)n){fprintf(stderr,"cannot read shader\n");exit(2);}
  fclose(f);*count=(size_t)n/4;return words;
}
typedef struct{VksPipe *pipe;VksPass *pass;} Draw;
static void record(VkCommandBuffer cmd,void *user){Draw *d=user;vks_pipe_record(d->pipe,cmd,d->pass);}
static int close_value(float actual,float expected){return isfinite(actual)&&fabs((double)actual-expected)<=0.00001;}

int main(int argc,char **argv){
  if(argc!=3){fprintf(stderr,"usage: render-helper-gpu HELPER.spv RANGE.spv\n");return 2;}
  size_t hn,rn;uint32_t *helper=read_words(argv[1],&hn),*range=read_words(argv[2],&rn);
  Vkc v;vkc_open(&v);
  printf("device=%s vendor=%u device_id=%u driver=%u helper_supported=%d helper_enabled=%d int64_enabled=%d\n",
    v.props.deviceName,v.props.vendorID,v.props.deviceID,v.props.driverVersion,v.demote_helper_supported,v.demote_helper_enabled,v.shader_int64_enabled);
  int negative=(int)render_test_vulkan_features(3,2,2,0,0,v.shader_int64_enabled);
  if(!negative){fprintf(stderr,"unsupported helper feature was admitted\n");return 1;}
  // No pipeline has been created at this point; source admission is Coax.
  printf("unsupported helper feature refused before pipeline creation: %d\n",negative);
  negative=(int)render_test_vulkan_features(2,0,0,1,v.demote_helper_enabled,0);
  if(!negative){fprintf(stderr,"unsupported native Int64 feature was admitted\n");return 1;}
  printf("unsupported Int64 feature refused before pipeline creation: %d\n",negative);
  int admitted=(int)render_test_vulkan_features(3,2,2,0,v.demote_helper_enabled,v.shader_int64_enabled);
  if(admitted){fprintf(stderr,"helper profile refused: %d (supported=%d enabled=%d)\n",admitted,v.demote_helper_supported,v.demote_helper_enabled);vkc_close(&v);return 1;}

  typedef struct{float position[4];float value;uint32_t tag;} Attr;
  _Static_assert(sizeof(Attr)==24,"fixture attribute layout");
  Attr attrs[6]={{{-1,1,.5f,1},0,9},{{1,1,.5f,1},4,77},{{-1,-1,.5f,1},8,88},
                 {{1,1,.5f,1},4,19},{{1,-1,.5f,1},12,55},{{-1,-1,.5f,1},8,66}};
  VkcBuffer vb=vkc_buffer(&v,sizeof attrs),ub=vkc_buffer(&v,16);memcpy(vb.map,attrs,sizeof attrs);((float*)ub.map)[0]=1;
  VksImage colour=vks_image(&v,2,2,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
  VksImage aux=vks_image(&v,2,2,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
  VksImage depth=vks_image(&v,2,2,1,0,VK_FORMAT_D32_SFLOAT,VKS_TARGET);
  VksBinding binding={.kind=VKS_STORAGE,.buffer=&ub,.range=4};
  uint32_t draws[4]={0,6,0x80000002u,1};
  VksPass pass={.words=helper,.count=hn,.bindings=&binding,.nb=1,.vertices=&vb,.stride=24,.nattrs=3,
    .attrs={{0,0,VK_FORMAT_R32G32B32A32_SFLOAT,0},{1,0,VK_FORMAT_R32_SFLOAT,16},{2,0,VK_FORMAT_R32_UINT,20}},
    .draws=draws,.ndraws=1,.targets={&colour,&aux},.ntargets=2,.depth=&depth,.depth_test=1,.depth_write=1,
    .clear={.125f,.25f,.5f,.75f}};
  VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Draw d={&pipe,&pass};vks_submit(&v,record,&d);
  float pixels[16],others[16],z[4];vks_read(&v,&colour,pixels,sizeof pixels);vks_read(&v,&aux,others,sizeof others);vks_read(&v,&depth,z,sizeof z);
  int wrong=0;
  for(int k=0;k<4;k++){
    float expected[4],expected_aux[4];
    if(k%2==0){float e[4]={10,4,(float)(k*2+4),1};float a[4]={k==0?11.f:21.f,.5f,(float)(k/2)+.5f,1};memcpy(expected,e,sizeof e);memcpy(expected_aux,a,sizeof a);}
    else {float clear[4]={.125f,.25f,.5f,.75f};memcpy(expected,clear,sizeof clear);memcpy(expected_aux,clear,sizeof clear);}
    for(int c=0;c<4;c++)if(!close_value(pixels[k*4+c],expected[c])||!close_value(others[k*4+c],expected_aux[c]))wrong++;
    if(!close_value(z[k],k%2==0?.5f:1.f))wrong++;
  }
  vks_pipe_close(&v,&pipe);vks_image_free(&v,&colour);vks_image_free(&v,&aux);vks_image_free(&v,&depth);vkc_free(&v,&vb);vkc_free(&v,&ub);

  admitted=(int)render_test_vulkan_features(2,0,0,1,v.demote_helper_enabled,v.shader_int64_enabled);
  if(admitted){fprintf(stderr,"range fixture refused: %d\n",admitted);return 1;}
  VkcBuffer physical=vkc_buffer(&v,9200);float *values=physical.map;values[27]=7;values[91]=17;values[2299]=575;
  const VkDeviceSize offsets[4]={0,0,256,0};
  const float counts[4]={28,2300,28,2300},last[4]={7,575,17,575};
  for(int test=0;test<4;test++){
    VksImage target=vks_image(&v,2,2,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
    coax_t_RenderBufferView view=render_test_uniform_view(test==1?575:7,9200,offsets[test]);
    if(!view.bytes){fprintf(stderr,"uniform view was not admitted\n");return 1;}
    binding=(VksBinding){.kind=VKS_STORAGE,.buffer=&physical,.offset=view.offset,.range=test==3?0:view.bytes};
    pass=(VksPass){.words=range,.count=rn,.bindings=&binding,.nb=1,.vertex_count=3,.targets={&target},.ntargets=1};
    vks_pipe_open(&v,&pipe,&pass);d=(Draw){&pipe,&pass};vks_submit(&v,record,&d);vks_read(&v,&target,pixels,sizeof pixels);
    for(int k=0;k<4;k++)if(!close_value(pixels[k*4],counts[test])||!close_value(pixels[k*4+1],last[test])||!close_value(pixels[k*4+2],(float)(k%2)+.5f)||!close_value(pixels[k*4+3],1))wrong++;
    vks_pipe_close(&v,&pipe);vks_image_free(&v,&target);
  }
  vkc_free(&v,&physical);vkc_close(&v);free(helper);free(range);
  printf("mixed helper publication/depth and four byte views: wrong=%d validation_errors=%d validation_warnings=%d\n",wrong,vkc_validation_errors,vkc_validation_warnings);
  return wrong||vkc_validation_errors||vkc_validation_warnings?1:0;
}
