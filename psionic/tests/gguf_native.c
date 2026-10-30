/* Independent struct-oracle expectations and bounded host file effects only. */
#include "gguf-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "gguf-goldens.h"
#define V_META(p) (coax_v_t_PgMeta){p,4096}
#define V_TENSOR(p) (coax_v_t_PgTensor){p,4096}
static void text(coax_string actual,const char *expected){assert(actual.count==(int64_t)strlen(expected));assert(!memcmp(actual.data,expected,(size_t)actual.count));}
static void run(const char *path,const int64_t *summary,const char **names,const int64_t expected[][9],const char **keys,const int64_t metadata[][3]){
    FILE *f=fopen(path,"rb");assert(f && !fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0 && !fseek(f,0,SEEK_SET));unsigned char *bytes=malloc((size_t)size);assert(bytes && fread(bytes,1,(size_t)size,f)==(size_t)size && !fclose(f));
    coax_t_PgMeta *m=calloc(4096,sizeof(*m)),*mw=calloc(4096,sizeof(*mw));coax_t_PgTensor *t=calloc(4096,sizeof(*t)),*tw=calloc(4096,sizeof(*tw));assert(m && mw && t && tw);
    coax_t_PgResult r=psionic_pg_read((coax_v_u8){bytes,size},size,V_META(m),V_META(mw),V_TENSOR(t),V_TENSOR(tw));
    if(!summary){assert(r.status==COAX_PgStatus_unsupported && r.metadata==0 && r.tensors==0);puts("GGUF: pinned big-endian file refuses explicitly");goto done;}
    if(r.status){fprintf(stderr,"%s refused with status %u\n",path,r.status);}assert(r.status==COAX_PgStatus_ok && r.metadata==summary[0] && r.tensors==summary[1] && r.data==summary[2] && r.alignment==summary[3]);
    for(int64_t i=0;i<r.metadata;i++){text(m[i].key,keys[i]);assert(m[i].kind==metadata[i][0] && m[i].begin==metadata[i][1] && m[i].end==metadata[i][2] && m[i].end<=size);}
    for(int64_t i=0;i<r.tensors;i++){text(t[i].name,names[i]);assert(t[i].rank==expected[i][0] && t[i].kind==expected[i][1] && t[i].elements==expected[i][2] && t[i].offset==expected[i][3] && t[i].bytes==expected[i][4]);for(int k=0;k<4;k++)assert(t[i].dims.e[k]==expected[i][5+k]);assert(t[i].begin==r.data+t[i].offset && t[i].end==t[i].begin+t[i].bytes && t[i].end<=size);}
    assert(psionic_pg_read((coax_v_u8){bytes,size},size,V_META(m),V_META(m),V_TENSOR(t),V_TENSOR(tw)).status==COAX_PgStatus_alias);
    assert(psionic_pg_read((coax_v_u8){(unsigned char *)m,24},size,V_META(m),V_META(mw),V_TENSOR(t),V_TENSOR(tw)).status==COAX_PgStatus_alias);
    /* Full public arrays, including unused tails, survive a late header fault. */
    if(r.tensors){coax_t_PgMeta *before_m=malloc(4096*sizeof(*m));coax_t_PgTensor *before_t=malloc(4096*sizeof(*t));assert(before_m && before_t);memcpy(before_m,m,4096*sizeof(*m));memcpy(before_t,t,4096*sizeof(*t));
        coax_t_PgResult fail=psionic_pg_read((coax_v_u8){bytes,r.data-1},size,V_META(m),V_META(mw),V_TENSOR(t),V_TENSOR(tw));assert(fail.status==COAX_PgStatus_truncated && fail.metadata==0 && fail.tensors==0 && !memcmp(before_m,m,4096*sizeof(*m)) && !memcmp(before_t,t,4096*sizeof(*t)));free(before_m);free(before_t);
    }
    printf("GGUF: %lld metadata, %lld tensors; all original/owned names, types, dimensions, spans and independent byte extents agree\n",(long long)r.metadata,(long long)r.tensors);
done:free(bytes);free(m);free(mw);free(t);free(tw);
}
int main(int argc,char **argv){assert(argc==6);run(argv[1],pg_owned_summary,pg_owned_names,pg_owned_tensors,pg_owned_keys,pg_owned_meta);run(argv[2],pg_layouts_summary,pg_layouts_names,pg_layouts_tensors,pg_layouts_keys,pg_layouts_meta);run(argv[3],pg_tiny_summary,pg_tiny_names,pg_tiny_tensors,pg_tiny_keys,pg_tiny_meta);run(argv[4],pg_vocab_summary,pg_vocab_names,pg_vocab_tensors,pg_vocab_keys,pg_vocab_meta);run(argv[5],0,0,0,0,0);return 0;}
