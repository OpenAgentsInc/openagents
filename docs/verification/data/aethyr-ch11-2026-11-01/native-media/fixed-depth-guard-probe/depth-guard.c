#include "vks.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <assert.h>
typedef struct {VksPipe *pipe;VksPass *pass;} Probe;
static void record(VkCommandBuffer cmd,void *user){Probe *p=user;vks_pipe_record(p->pipe,cmd,p->pass);}
int main(void){
 FILE*f=fopen("depth-guard.spv","rb");assert(f);fseek(f,0,SEEK_END);size_t n=(size_t)ftell(f);rewind(f);uint32_t*w=malloc(n);assert(fread(w,1,n,f)==n);fclose(f);
 Vkc v;vkc_open(&v);VkcBuffer u=vkc_buffer(&v,16);VksImage target=vks_image(&v,64,64,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
 VksBinding b={.kind=VKS_STORAGE,.buffer=&u,.range=16};VksPass pass={.words=w,.count=n/4,.bindings=&b,.nb=1,.vertex_count=3,.instance_count=1,.targets={&target},.ntargets=1};
 float*pixels=malloc(64*64*16);int deltas[3]={0,1,3};int ys[5]={4096,8192,16384,32768,65536};printf("{\"subPixelPrecisionBits\":%u,\"rgba\":[",v.props.limits.subPixelPrecisionBits);
 for(int y=0;y<5;y++)for(int mode=0;mode<2;mode++)for(int i=0;i<3;i++){((float*)u.map)[0]=(float)deltas[i]/1024.0f;((float*)u.map)[1]=(float)mode;((float*)u.map)[2]=(float)ys[y];VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Probe probe={&pipe,&pass};vks_submit(&v,record,&probe);vks_read(&v,&target,pixels,64*64*16);printf("%s[%.17g,%.17g,%.17g,%.17g]",(i||mode||y)?",":"",(double)pixels[(20*64+20)*4],(double)pixels[(20*64+20)*4+1],(double)pixels[(20*64+20)*4+2],(double)pixels[(20*64+20)*4+3]);vks_pipe_close(&v,&pipe);}
 printf("],\"validation_errors\":%d,\"validation_warnings\":%d}\n",vkc_validation_errors,vkc_validation_warnings);
 free(pixels);vks_image_free(&v,&target);vkc_free(&v,&u);vkc_close(&v);free(w);return vkc_validation_errors||vkc_validation_warnings?1:0;
}
