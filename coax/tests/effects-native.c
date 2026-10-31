/* Independent policy ABI transport; no classification or closure in C. */
#include "effects-policy.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#define POISON INT64_C(1431655765)
static void get(FILE *f,void *p,size_t n){assert(fread(p,1,n,f)==n);}
static void put(FILE *f,const void *p,size_t n){assert(fwrite(p,1,n,f)==n);}
int main(int argc,char **argv){
 assert(argc==3);FILE *in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");assert(in&&out);uint32_t cases;get(in,&cases,4);
 for(uint32_t c=0;c<cases;c++){
  int64_t n,e,s,root,wrapping;get(in,&n,8);get(in,&e,8);get(in,&s,8);get(in,&root,8);get(in,&wrapping,8);assert(n>=0&&n<=8&&e>=0&&e<100&&s>=0&&s<100);
  coax_t_EF_Edge *edges=calloc((size_t)e+1,sizeof(*edges));coax_t_EF_Site *sites=calloc((size_t)s+1,sizeof(*sites));assert(edges&&sites);get(in,edges,(size_t)e*sizeof(*edges));get(in,sites,(size_t)s*sizeof(*sites));
  int64_t p[9],v[9],q[9],d[9],w[10];for(int i=0;i<9;i++)p[i]=v[i]=q[i]=d[i]=POISON;for(int i=0;i<10;i++)w[i]=POISON;
  coax_t_EF_Result r=EF_trace(n,(coax_v_t_EF_Edge){edges,e},(coax_v_t_EF_Site){sites,s},root,(coax_v_s64){p,n},(coax_v_s64){v,n},(coax_v_s64){q,n},(coax_v_s64){d,n},(coax_v_s64){w,9},wrapping!=0,0);
  if(r.status){for(int i=0;i<9;i++)assert(p[i]==POISON&&v[i]==POISON&&q[i]==POISON&&d[i]==POISON);for(int i=0;i<10;i++)assert(w[i]==POISON);}
  assert(p[n]==POISON&&v[n]==POISON&&q[n]==POISON&&d[n]==POISON&&w[9]==POISON);
  uint32_t rec=r.recursive;put(out,&r.status,4);put(out,&r.effects,4);put(out,&r.reached,8);put(out,&rec,4);put(out,&r.acyclic_removed,8);put(out,p,(size_t)n*8);put(out,v,(size_t)n*8);put(out,w,9*8);free(edges);free(sites);
 }
 assert(fgetc(in)==EOF);assert(!fclose(in)&&!fclose(out));printf("effects policy native: %u independently checked cases\n",cases);return 0;
}
