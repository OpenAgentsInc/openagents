// Native GPU observation with an independently authored stripe average.
#include "vks.h"
#include "RenderCaps.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static uint32_t *read_words(const char *name,size_t *count){
  FILE *f=fopen(name,"rb");if(!f){perror(name);exit(2);}fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);
  if(n<20||n%4){fprintf(stderr,"invalid shader length\n");exit(2);}uint32_t *p=malloc((size_t)n);
  if(!p||fread(p,1,(size_t)n,f)!=(size_t)n){fprintf(stderr,"shader read failed\n");exit(2);}fclose(f);*count=(size_t)n/4;return p;
}
typedef struct{VksPipe *pipe;VksPass *pass;} Draw;
static void record(VkCommandBuffer cmd,void *user){Draw *d=user;vks_pipe_record(d->pipe,cmd,d->pass);}
int main(int argc,char **argv){
  if(argc!=3){fprintf(stderr,"usage: render-sampling-gpu FILTERED.spv INVALID.spv\n");return 2;}
  Vkc v;vkc_open(&v);printf("device=%s helper=%d int64=%d\n",v.props.deviceName,v.demote_helper_enabled,v.shader_int64_enabled);
  int admitted=(int)render_test_vulkan_features(3,2,2,1,v.demote_helper_enabled,v.shader_int64_enabled);
  if(admitted){fprintf(stderr,"sampling feature profile refused: %d\n",admitted);return 2;}
  VksImage texture=vks_image(&v,8,1,1,0,VK_FORMAT_R16G16B16A16_SFLOAT,VKS_TEXTURE);
  // These half words are exact powers/fractions. The CPU oracle does not call
  // the footprint or texture library to calculate its expected output.
  const uint16_t pixels[32]={0,0,0x3400,0x3c00,0,0x3000,0x3400,0x3c00,
    0,0x3400,0x3400,0x3c00,0x3c00,0x3600,0x3400,0x3c00,
    0x3c00,0x3800,0x3400,0x3c00,0,0x3900,0x3400,0x3c00,
    0,0x3a00,0x3400,0x3c00,0,0x3b00,0x3400,0x3c00};
  vks_upload(&v,&texture,pixels,sizeof pixels);VkSampler sampler=vks_sampler_with(&v,VKS_LINEAR,1,0,VK_COMPARE_OP_ALWAYS,1);
  VksBinding bindings[2]={{.kind=VKS_IMAGE,.image=&texture},{.kind=VKS_SAMPLER,.sampler=sampler}};
  int wrong=0;
  for(int test=0;test<2;test++){
    size_t count;uint32_t *words=read_words(argv[test+1],&count);
    VksImage target=vks_image(&v,2,2,1,0,VK_FORMAT_R32G32B32A32_SFLOAT,VKS_TARGET);
    VksPass pass={.words=words,.count=count,.bindings=bindings,.nb=2,.vertex_count=3,.targets={&target},.ntargets=1,.clear={.125f,.25f,.5f,.75f}};
    VksPipe pipe;vks_pipe_open(&v,&pipe,&pass);Draw draw={&pipe,&pass};vks_submit(&v,record,&draw);
    float actual[16];vks_read(&v,&target,actual,sizeof actual);
    const float average[4]={.25f,.4375f,.25f,1},clear[4]={.125f,.25f,.5f,.75f};
    const float *expected=test?clear:average;
    for(int i=0;i<16;i++)if(!isfinite(actual[i])||fabs((double)actual[i]-expected[i%4])>.00001){wrong++;fprintf(stderr,"sample%d word%d got%.9g expected%.9g\n",test,i,actual[i],expected[i%4]);}
    vks_pipe_close(&v,&pipe);vks_image_free(&v,&target);free(words);
  }
  vkDestroySampler(v.dev,sampler,NULL);vks_image_free(&v,&texture);vkc_close(&v);
  printf("eight taps and invalid-footprint publication: wrong=%d validation_errors=%d validation_warnings=%d\n",wrong,vkc_validation_errors,vkc_validation_warnings);
  return wrong||vkc_validation_errors||vkc_validation_warnings?1:0;
}
