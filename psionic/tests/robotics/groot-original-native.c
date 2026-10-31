/* Firm reason: 2 (component effect/parity harness). Reads four selected
   original BF16 tensors, calls Coax packing/bias/state arithmetic, and writes
   bounded F32 intermediates/results for an independent reference. No full
   model, Python runtime, simulator or robot command is linked here. */
#include "groot-linear.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static coax_v_u8 bytes(uint8_t *p,int64_t n){return (coax_v_u8){p,n};}
static coax_v_f32 floats(float *p,int64_t n){return (coax_v_f32){p,n};}
static uint8_t *part(const char *root,const char *name,size_t expected){
  char path[8192];int count=snprintf(path,sizeof path,"%s/%s.bf16",root,name);assert(count>0&&(size_t)count<sizeof path);
  FILE *file=fopen(path,"rb");assert(file);uint8_t *data=malloc(expected);assert(data);
  assert(fread(data,1,expected,file)==expected&&fgetc(file)==EOF&&!ferror(file));assert(fclose(file)==0);return data;
}
static void ok(coax_t_QmResult r,int64_t written){assert(r.status==COAX_QmStatus_ok&&r.written==written);}
int main(int argc,char **argv){
  _Static_assert(sizeof(float)==4,"component profile requires binary32");
  if(argc!=3){fputs("usage: groot-original-native WEIGHT_DIRECTORY NEW_OUTPUT.bin\n",stderr);return 2;}
  uint16_t endian=1;assert(*(uint8_t *)&endian==1);
  uint8_t *w1=part(argv[1],"state-w1",270336),*b1=part(argv[1],"state-b1",2048);
  uint8_t *w2=part(argv[1],"state-w2",3145728),*b2=part(argv[1],"state-b2",3072);
  uint8_t *packed1=malloc(270336),*packed2=malloc(3145728);assert(packed1&&packed2);
  float bias1[1024],bias2[1536],input[132],output[1537];
  coax_t_GlLayout layout=gl_state_layout(1024,1536);assert(layout.status==COAX_QmStatus_ok&&layout.total==7680);
  float *work=malloc((size_t)layout.total*sizeof(float));assert(work);
  ok(gl_pack(COAX_QmDtype_bf16,132,1024,bytes(w1,270336),bytes(packed1,270336),0),270336);
  ok(gl_pack(COAX_QmDtype_bf16,1024,1536,bytes(w2,3145728),bytes(packed2,3145728),0),3145728);
  ok(gl_bias(COAX_QmDtype_bf16,1024,bytes(b1,2048),floats(bias1,1024),0),1024);
  ok(gl_bias(COAX_QmDtype_bf16,1536,bytes(b2,3072),floats(bias2,1536),0),1536);
  FILE *record=fopen(argv[2],"wbx");assert(record);
  const float seed[8]={-0.75f,-0.5f,-0.25f,0,0.25f,0.5f,0.75f,1};
  for(int example=0;example<2;example++){
    for(int i=0;i<132;i++)input[i]=example?(float)(i%17-8)/16.0f:(i<8?seed[i]:0);
    output[1536]=-713.25f;
    ok(gl_state(COAX_QmDtype_bf16,132,1024,1536,bytes(packed1,270336),floats(bias1,1024),bytes(packed2,3145728),floats(bias2,1536),floats(input,132),floats(output,1537),floats(work,layout.total),0),1536);
    assert(output[1536]==-713.25f);
    assert(fwrite(work,sizeof(float),1024,record)==1024&&fwrite(output,sizeof(float),1536,record)==1536);
  }
  assert(fclose(record)==0);
  free(work);free(packed1);free(packed2);free(w1);free(b1);free(w2);free(b2);
  puts("GR00T original selected state encoder: 2 authored inputs, 5120 F32 intermediate/output values written");return 0;
}
