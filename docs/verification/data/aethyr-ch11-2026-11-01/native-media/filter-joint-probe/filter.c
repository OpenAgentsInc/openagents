#include "vks.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <assert.h>
#include <string.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
static void *load(const char *path,size_t *size){FILE*f=fopen(path,"rb");assert(f);fseek(f,0,SEEK_END);long end=ftell(f);assert(end>=0);*size=(size_t)end;rewind(f);void*data=malloc(*size?*size:1);assert(data&&fread(data,1,*size,f)==*size&&!fclose(f));return data;}
int main(void){
 size_t n=0,case_bytes=0;uint32_t*w=load("filter.spv",&n);float *cases=load("cases.bin",&case_bytes);assert(case_bytes==20*16);
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,16);VksImage target=vks_image(&v,64,64,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);VkSampler sampler=vks_sampler_with(&v,VKS_LINEAR,0,0,VK_COMPARE_OP_LESS_OR_EQUAL,1);
 FILE *raw=fopen("observations.rgba32f","wb");assert(raw);float*pixels=malloc(64*64*16);
 printf("{\"device\":\"%s\",\"driverVersion\":%u,\"subPixelPrecisionBits\":%u,\"subTexelPrecisionBits\":%u,\"mipmapPrecisionBits\":%u,\"observations\":[",v.props.deviceName,v.props.driverVersion,v.props.limits.subPixelPrecisionBits,v.props.limits.subTexelPrecisionBits,v.props.limits.mipmapPrecisionBits);
 for(int format=0;format<2;format++){
  const char *format_name=format?"rgba16float":"rgba8unorm";size_t size=0;void *data=load(format?"rgba16float.bin":"rgba8unorm.bin",&size);assert(size==(format?40:20));VksImage image=vks_image_levels(&v,2,2,1,0,format?VK_FORMAT_R16G16B16A16_SFLOAT:VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,2);vks_upload(&v,&image,data,size);free(data);
  VksBinding bindings[3]={{.kind=VKS_STORAGE,.buffer=&u,.range=16,.stages=VK_SHADER_STAGE_FRAGMENT_BIT},{.kind=VKS_IMAGE,.image=&image,.stages=VK_SHADER_STAGE_FRAGMENT_BIT},{.kind=VKS_SAMPLER,.sampler=sampler,.stages=VK_SHADER_STAGE_FRAGMENT_BIT}};VksPass pass={.words=w,.count=n/4,.bindings=bindings,.nb=3,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1};VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);
  for(int i=0;i<20;i++){
   memcpy(u.map,cases+i*4,16);Probe probe={&pipe,&pass};vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,64*64*16);float *pixel=pixels+(9*64+9)*4;assert(fwrite(pixel,4,4,raw)==4);
   printf("%s{\"format\":\"%s\",\"case\":%d,\"uniform\":[%.17g,%.17g,%.17g,%.17g],\"rgba\":[%.17g,%.17g,%.17g,%.17g]}",(i||format)?",":"",format_name,i,(double)cases[i*4],(double)cases[i*4+1],(double)cases[i*4+2],(double)cases[i*4+3],(double)pixel[0],(double)pixel[1],(double)pixel[2],(double)pixel[3]);
  }vks_pipe_close(&v,&pipe);vks_image_free(&v,&image);
 }
 printf("],\"validation_errors\":%d,\"validation_warnings\":%d}\n",vkc_validation_errors,vkc_validation_warnings);assert(!fclose(raw));free(pixels);free(cases);vkDestroySampler(v.dev,sampler,NULL);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_close(&v);free(w);return vkc_validation_errors||vkc_validation_warnings?1:0;
}
