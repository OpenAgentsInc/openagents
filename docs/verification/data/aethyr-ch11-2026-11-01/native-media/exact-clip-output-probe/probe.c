#include "vks.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
int main(void){FILE *f=fopen("probe.spv","rb");assert(f);fseek(f,0,SEEK_END);size_t n=(size_t)ftell(f);rewind(f);uint32_t *words=malloc(n);assert(fread(words,1,n,f)==n&&!fclose(f));
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,48);VksImage target=vks_image(&v,320,180,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
 VksBinding binding={.kind=VKS_STORAGE,.buffer=&u,.range=48,.stages=VK_SHADER_STAGE_VERTEX_BIT};VksPass pass={.words=words,.count=n/4,.bindings=&binding,.nb=1,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1,.clear={-1,-1,-1,-1}};
 float *pixels=malloc(320*180*16);f=fopen("cases.bin","rb");assert(f);FILE *raw=fopen("observations.rgba32f","wb");assert(raw);printf("{\"device\":\"%s\",\"subPixelPrecisionBits\":%u,\"observations\":[",v.props.deviceName,v.props.limits.subPixelPrecisionBits);
 for(int k=0;k<8;k++){uint32_t xy[2];assert(fread(xy,4,2,f)==2&&fread(u.map,4,12,f)==12);VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,320*180*16);float *q=pixels+(xy[1]*320+xy[0])*4;assert(fwrite(q,4,4,raw)==4);printf("%s[%.17g,%.17g,%.17g,%.17g]",k?",":"",(double)q[0],(double)q[1],(double)q[2],(double)q[3]);vks_pipe_close(&v,&pipe);}
 assert(fgetc(f)==EOF&&!fclose(f)&&!fclose(raw));printf("],\"validation_errors\":%d,\"validation_warnings\":%d}\n",vkc_validation_errors,vkc_validation_warnings);free(pixels);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_close(&v);free(words);return vkc_validation_errors||vkc_validation_warnings?1:0;}
