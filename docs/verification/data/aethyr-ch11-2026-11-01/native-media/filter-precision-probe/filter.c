#include "vks.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <assert.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
int main(void){
 FILE*f=fopen("filter.spv","rb");assert(f);fseek(f,0,SEEK_END);size_t n=(size_t)ftell(f);rewind(f);uint32_t*w=malloc(n);assert(fread(w,1,n,f)==n);fclose(f);
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,16);VksImage target=vks_image(&v,64,64,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET),image=vks_image(&v,2,1,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE);unsigned char texels[8]={0,0,0,255,255,255,255,255};vks_upload(&v,&image,texels,8);VkSampler sampler=vks_sampler(&v,VKS_LINEAR,0,0);
 VksBinding bindings[3]={{.kind=VKS_STORAGE,.buffer=&u,.range=16,.stages=VK_SHADER_STAGE_FRAGMENT_BIT},{.kind=VKS_IMAGE,.image=&image,.stages=VK_SHADER_STAGE_FRAGMENT_BIT},{.kind=VKS_SAMPLER,.sampler=sampler,.stages=VK_SHADER_STAGE_FRAGMENT_BIT}};
 VksPass pass={.words=w,.count=n/4,.bindings=bindings,.nb=3,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1};
 float fractions[14]={0,1.0f/1024,2.0f/1024,3.0f/1024,6.0f/1024,7.0f/1024,.1f,.2f,1.0f/3,.5f-1.0f/1024,.5f,.5f+1.0f/1024,1.0f-1.0f/1024,1};float*pixels=malloc(64*64*16);
 printf("{\"device\":\"%s\",\"driverVersion\":%u,\"subPixelPrecisionBits\":%u,\"subTexelPrecisionBits\":%u,\"mipmapPrecisionBits\":%u,\"rgba\":[",v.props.deviceName,v.props.driverVersion,v.props.limits.subPixelPrecisionBits,v.props.limits.subTexelPrecisionBits,v.props.limits.mipmapPrecisionBits);
 for(int i=0;i<14;i++){((float*)u.map)[0]=fractions[i];VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,64*64*16);printf("%s[%.17g,%.17g,%.17g,%.17g]",i?",":"",(double)pixels[(9*64+9)*4],(double)pixels[(9*64+9)*4+1],(double)pixels[(9*64+9)*4+2],(double)pixels[(9*64+9)*4+3]);vks_pipe_close(&v,&pipe);}
 printf("],\"validation_errors\":%d,\"validation_warnings\":%d}\n",vkc_validation_errors,vkc_validation_warnings);
 free(pixels);vkDestroySampler(v.dev,sampler,NULL);vks_image_free(&v,&image);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_close(&v);free(w);return vkc_validation_errors||vkc_validation_warnings?1:0;
}
