#include "vks.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
static void *load(const char *name,size_t *bytes){FILE *f=fopen(name,"rb");assert(f);fseek(f,0,SEEK_END);*bytes=(size_t)ftell(f);rewind(f);void *p=malloc(*bytes);assert(p&&fread(p,1,*bytes,f)==*bytes&&!fclose(f));return p;}
int main(void){size_t spv_bytes,u_bytes,object_bytes;uint32_t *words=load("probe.spv",&spv_bytes);void *ub=load("u.bin",&u_bytes),*ob=load("objects.bin",&object_bytes);
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,u_bytes),objects=vkc_buffer(&v,object_bytes),params=vkc_buffer(&v,16);memcpy(u.map,ub,u_bytes);memcpy(objects.map,ob,object_bytes);VksImage target=vks_image(&v,64,64,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
 VksBinding bindings[3]={{.kind=VKS_STORAGE,.buffer=&u,.range=u_bytes,.stages=VK_SHADER_STAGE_VERTEX_BIT},{.kind=VKS_STORAGE,.buffer=&objects,.range=object_bytes,.stages=VK_SHADER_STAGE_VERTEX_BIT},{.kind=VKS_STORAGE,.buffer=&params,.range=16,.stages=VK_SHADER_STAGE_VERTEX_BIT|VK_SHADER_STAGE_FRAGMENT_BIT}};
 VksPass pass={.words=words,.count=spv_bytes/4,.bindings=bindings,.nb=3,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1,.clear={-1,-1,-1,-1}};VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};
 float *pixels=malloc(64*64*16);FILE *f=fopen("cases.bin","rb"),*raw=fopen("observations.rgba32f","wb");assert(f&&raw);printf("{\"device\":\"%s\",\"subPixelPrecisionBits\":%u,\"observations\":[",v.props.deviceName,v.props.limits.subPixelPrecisionBits);
 for(int k=0;k<96;k++){assert(fread(params.map,4,4,f)==4);vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,64*64*16);float *q=pixels+(9*64+9)*4;assert(fwrite(q,4,4,raw)==4);printf("%s[%.17g,%.17g,%.17g,%.17g]",k?",":"",(double)q[0],(double)q[1],(double)q[2],(double)q[3]);}
 assert(fgetc(f)==EOF&&!fclose(f)&&!fclose(raw));printf("],\"validation_errors\":%d,\"validation_warnings\":%d}\n",vkc_validation_errors,vkc_validation_warnings);free(pixels);vks_pipe_close(&v,&pipe);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_free(&v,&objects);vkc_free(&v,&params);vkc_close(&v);free(words);free(ub);free(ob);return vkc_validation_errors||vkc_validation_warnings?1:0;}
