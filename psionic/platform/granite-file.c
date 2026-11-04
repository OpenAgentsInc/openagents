/* See granite-file.h. Include the generated Coax header before this file's. */
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include GRANITE_COAX_HEADER
#include "granite-file.h"
#define GR_META_MAX 4096
#define GR_HEADER_MAX 67108864
static size_t gr_power(size_t n){size_t p=2;while(p<n)p*=2;return p;}
int gr_file_open(const char *path, GrFile *f, char *error, size_t error_bytes){
    memset(f,0,sizeof *f);f->fd=open(path,O_RDONLY);
    if(f->fd<0){snprintf(error,error_bytes,"cannot open %s",path);return 1;}
    struct stat st;if(fstat(f->fd,&st)){snprintf(error,error_bytes,"cannot stat %s",path);return 1;}
    f->size=(size_t)st.st_size;size_t page=(size_t)getpagesize();f->map_size=(f->size+page-1)/page*page;
    void *map=mmap(NULL,f->map_size,PROT_READ,MAP_PRIVATE,f->fd,0);
    if(map==MAP_FAILED){snprintf(error,error_bytes,"cannot map %s",path);return 1;}
    f->bytes=map;
    f->meta=calloc(GR_META_MAX,sizeof *f->meta);f->meta_work=calloc(GR_META_MAX,sizeof *f->meta_work);
    f->tensors=calloc(GR_META_MAX,sizeof *f->tensors);f->tensor_work=calloc(GR_META_MAX,sizeof *f->tensor_work);f->table=calloc(GR_META_MAX,sizeof *f->table);
    if(!f->meta||!f->meta_work||!f->tensors||!f->tensor_work||!f->table){snprintf(error,error_bytes,"out of memory");return 1;}
    size_t header=f->size<GR_HEADER_MAX?f->size:GR_HEADER_MAX;
    coax_v_u8 h={(uint8_t*)f->bytes,(int64_t)header};
    f->binding=gr_bind(h,(int64_t)f->size,(coax_v_t_PgMeta){f->meta,GR_META_MAX},(coax_v_t_PgMeta){f->meta_work,GR_META_MAX},
        (coax_v_t_PgTensor){f->tensors,GR_META_MAX},(coax_v_t_PgTensor){f->tensor_work,GR_META_MAX},(coax_v_t_GrTensor){f->table,GR_META_MAX});
    if(f->binding.status){snprintf(error,error_bytes,"binding refused: status %u cause %lld",f->binding.status,(long long)f->binding.cause);return 2;}
    int64_t n=f->binding.text.vocab,m=f->binding.text.merge_count;size_t vs=gr_power((size_t)(2*n)),ms=gr_power((size_t)(2*m));
    coax_t_GrTables t;memset(&t,0,sizeof t);
    t.pool=(coax_v_u8){malloc(header),(int64_t)header};t.spans=(coax_v_s64){malloc(16*(size_t)n),2*n};
    t.vocab_slots=(coax_v_s64){malloc(8*vs),(int64_t)vs};t.merges=(coax_v_s64){malloc(24*(size_t)m+8),3*m};
    t.merge_slots=(coax_v_s64){malloc(8*ms),(int64_t)ms};t.added=(coax_v_s64){malloc(8*(size_t)n),n};
    if(!t.pool.data||!t.spans.data||!t.vocab_slots.data||!t.merges.data||!t.merge_slots.data||!t.added.data){snprintf(error,error_bytes,"out of memory");return 1;}
    f->tables=gr_vocab_build(h,f->binding.text,t);
    if(f->tables.vocab!=n){snprintf(error,error_bytes,"tokenizer tables refused: %lld",(long long)f->tables.vocab);return 3;}
    return 0;
}
void gr_file_close(GrFile *f){
    if(f->bytes)munmap((void*)f->bytes,f->map_size);if(f->fd>0)close(f->fd);
    free(f->meta);free(f->meta_work);free(f->tensors);free(f->tensor_work);free(f->table);
    free(f->tables.pool.data);free(f->tables.spans.data);free(f->tables.vocab_slots.data);free(f->tables.merges.data);free(f->tables.merge_slots.data);free(f->tables.added.data);
    memset(f,0,sizeof *f);
}
