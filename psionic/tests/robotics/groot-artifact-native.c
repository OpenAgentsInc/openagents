/* Firm reason: 2 (independent oracle). Check actual header/range ABI and emit
   typed facts for an independent stdlib JSON reference; never fake payload views. */
#include "groot-artifact.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *read_header(const char *path,int64_t *size){
  FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0&&n<=1048576);rewind(f);
  char *s=malloc((size_t)n);assert(s&&fread(s,1,(size_t)n,f)==(size_t)n);fclose(f);*size=n;return s;
}
int main(int argc,char **argv){
  assert(argc==3);FILE *report=fopen(argv[2],"wb");assert(report);
  const int64_t sizes[2]={INT64_C(4990519232),INT64_C(1919980184)};const int counts[2]={774,257};
  coax_t_P5Tensor *out=calloc(1024,sizeof *out),*work=calloc(1024,sizeof *work),*saved=calloc(1024,sizeof *saved);assert(out&&work&&saved);
  unsigned cases=0;
  for(int shard=0;shard<2;shard++){
    char path[4096];assert(snprintf(path,sizeof path,"%s/model-%05d-of-00002.header.json",argv[1],shard+1)<(int)sizeof path);
    int64_t length;char *text=read_header(path,&length);coax_t_PwResult r=gca_header((coax_string){text,length},sizes[shard],shard,(coax_v_t_P5Tensor){out,1024},(coax_v_t_P5Tensor){work,1024},0);
    assert(r.status==0&&r.written==counts[shard]&&r.bytes==sizes[shard]-length-8);cases++;
    for(int64_t i=0;i<r.written;i++){
      coax_t_P5Tensor *t=&out[i];int64_t elements=1;assert(t->dtype==0&&t->shape.rank>=0&&t->shape.rank<=5&&t->shard==shard);
      for(int j=0;j<t->shape.rank;j++){assert(t->shape.dims.e[j]>0);elements*=t->shape.dims.e[j];}
      assert(elements==t->elements&&t->end-t->begin==2*elements&&t->begin>=length+8&&t->end<=sizes[shard]);cases++;
      fprintf(report,"tensor|%d|%.*s|%d|%"PRId64"",shard,(int)t->name.count,t->name.data,(int)t->dtype,t->shape.rank);
      for(int j=0;j<5;j++)fprintf(report,"|%"PRId64,t->shape.dims.e[j]);
      fprintf(report,"|%"PRId64"|%"PRId64"|%"PRId64"\n",t->elements,t->begin,t->end);
    }
    if(shard==1)for(int part=0;part<4;part++)for(int cat=0;cat<32;cat++){
      coax_t_GcaRange p=gca_state_range(part,cat,(coax_v_t_P5Tensor){out,r.written},sizes[shard]);assert(p.status==0&&p.category==cat&&p.end-p.begin==p.bytes);cases++;
      fprintf(report,"range|%d|%d|%.*s|%"PRId64"|%"PRId64"|%"PRId64"|%"PRId64"|%"PRId64"\n",part,cat,(int)p.name.count,p.name.data,p.begin,p.end,p.bytes,p.inputs,p.outputs);
    }
    memcpy(saved,out,1024*sizeof *out);
    coax_t_PwResult bad=gca_header((coax_string){text,length},sizes[shard],shard,(coax_v_t_P5Tensor){out,1},(coax_v_t_P5Tensor){work,1024},0);
    assert(bad.status==2&&bad.written==0&&!memcmp(out,saved,1024*sizeof *out));cases++;
    free(text);
  }
  fclose(report);free(saved);free(work);free(out);
  printf("GR00T header/range ABI: %u cases,1031 tensors and128 category ranges\n",cases);return 0;
}
