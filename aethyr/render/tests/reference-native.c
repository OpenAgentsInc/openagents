/* Verification host: file/word transport and independent observations only. */
#include "reference-native.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static uint32_t *read_tree(const char *path,int64_t *count) {
 FILE *f=fopen(path,"r");assert(f);
 size_t cap=65536;uint32_t *tree=calloc(cap,sizeof(*tree));assert(tree);
 uint32_t n;*count=0;
 while(fscanf(f,"%u",&n)==1) {assert((size_t)*count<cap);tree[(*count)++]=n;}
 assert(feof(f));assert(fclose(f)==0);return tree;
}
int main(int argc,char **argv) {
 if(argc==2&&!strcmp(argv[1],"--assert-probe")){rq_native_assert_probe(false);return 9;}
 assert(argc==2||argc==3);int64_t count;uint32_t *tree=read_tree(argv[1],&count);
 int64_t nmeta=rq_native_meta_words();int64_t *meta=calloc((size_t)nmeta,sizeof(*meta));assert(meta);
 uint32_t *scratch=calloc(65536,sizeof(*scratch));assert(scratch);
 uint32_t input[16]={0x40000000,0,0x40000000,0x3f800000,0x40c00000,0,0x40000000,0,
  0x41200000,0,0x40000000,0x3f800000,0x41b00000,0,0x40000000,0x3f800000};
 if(argc==3&&!strcmp(argv[2],"--drop-helper"))input[4]=0;
 uint32_t out[16]={0};coax_t_RQResources resources={0};
 coax_t_RQExecution r=rq_native_run((coax_v_u32){tree,count},(coax_v_u32){input,16},resources,
  (coax_v_s64){meta,nmeta},(coax_v_u32){scratch,65536},(coax_v_u32){out,16},1,100000,15,COAX_RQDerivative_fine);
 assert(r.status==COAX_RQStatus_ok&&r.steps>0&&r.discarded==2&&r.publish_mask==13&&r.output_words==16);
 const uint32_t expected[16]={0x40800000,0x41000000,0x41400000,0x41000000,
  0x40800000,0x41800000,0x41a00000,0x41000000,
  0x41400000,0x41000000,0x41a00000,0x41000000,
  0x41400000,0x41800000,0x41e00000,0x41000000};
 for(int i=0;i<16;i++){if(out[i]!=expected[i])fprintf(stderr,"word%d got%08x want%08x\n",i,out[i],expected[i]);assert(out[i]==expected[i]);}
 puts("quad real checked IR: fine derivatives, discarded helper, calls/for/while/break/continue, exact16 words passed");
 free(tree);free(meta);free(scratch);return 0;
}
