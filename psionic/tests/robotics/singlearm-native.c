/* Firm reason: 2 (independent oracle). Original Parquet-derived C tables
   independently check every Coax-decoded sample word. No Parquet runtime. */
#include "singlearm.h"
#include "singlearm-goldens.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static uint32_t bits(float value){uint32_t b;memcpy(&b,&value,4);return b;}
static coax_string read_input(const char *path,char *buffer,size_t capacity){
  FILE *f=fopen(path,"rb");assert(f);size_t n=fread(buffer,1,capacity,f);
  assert(!ferror(f)&&fgetc(f)==EOF);fclose(f);return (coax_string){buffer,(int64_t)n};
}
int main(void){
  _Static_assert(sizeof(float)==4,"reference requires binary32");unsigned cases=0;
  char info[8192],samples[8192];
  coax_string metadata=read_input("psionic/fixtures/robotics/singlearm-panda-open-drawer/info.json",info,sizeof info);
  coax_string excerpt=read_input("psionic/fixtures/robotics/singlearm-panda-open-drawer/samples.json",samples,sizeof samples);
  coax_t_SrInfo profile=sr_info(metadata);
  assert(profile.status==0&&profile.states==25&&profile.actions==8&&profile.fps==30&&profile.episodes==1273&&profile.frames==154256);cases++;
  coax_t_SrSample out[6]={0};out[5].frame=731;out[5].action.e[7]=-3;
  coax_t_SrRead result=sr_samples(excerpt,(coax_v_t_SrSample){out,6});
  assert(result.status==0&&result.written==5);
  for(int i=0;i<5;i++){
    assert(out[i].frame==sa_reference_frame[i]);cases++;
    assert(bits(out[i].timestamp)==sa_reference_timestamp_bits[i]);cases++;
    for(int j=0;j<25;j++){assert(bits(out[i].state.e[j])==sa_reference_state_bits[i][j]);cases++;}
    for(int j=0;j<8;j++){assert(bits(out[i].action.e[j])==sa_reference_action_bits[i][j]);cases++;}
  }
  assert(out[5].frame==731&&out[5].action.e[7]==-3);cases++;
  coax_t_SrSample before[6];memcpy(before,out,sizeof out);
  for(int count=-1;count<=4;count+=5){
    result=sr_samples(excerpt,(coax_v_t_SrSample){out,count});
    assert(result.status==4&&result.written==0&&!memcmp(before,out,sizeof out));cases++;
  }
  assert(sr_info((coax_string){NULL,0}).status==1);cases++;
  result=sr_samples((coax_string){"{]",2},(coax_v_t_SrSample){out,6});
  assert(result.status==1&&result.written==0&&!memcmp(before,out,sizeof out));cases++;
  printf("SingleArm original-data native: %u cases; all170 F32 words and5 frame IDs exact\n",cases);
  return 0;
}
