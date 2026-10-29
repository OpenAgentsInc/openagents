/* One-token experiment. Authenticate the original bundle before running. */
#include "elephant-runtime.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_string str(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_f32 span(float *p,int64_t n) {return (coax_v_f32){p,n};}
int main(int argc,char **argv) {
    if(argc!=2) {fprintf(stderr,"usage: elephant-token MODEL_SAFETENSORS\n");return 2;}
    int fd=open(argv[1],O_RDONLY);if(fd<0){perror("open");return 2;}struct stat st;
    if(fstat(fd,&st) || st.st_size!=18819722392LL)return 2;
    void *map=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(map==MAP_FAILED)return 2;
    coax_t_P5Tensor *inv=calloc(4096,sizeof(*inv)),*draft=calloc(4096,sizeof(*draft));
    float *params=calloc(32*50000,sizeof(float)),*temporary=calloc(50000,sizeof(float));coax_t_QeLayer *layers=calloc(32,sizeof(*layers));
    const int64_t states=24*(3*8192+32*128*128)+8*2*1*4*256;
    const int64_t work_count=states+5*1024*1024;
    float *state=calloc((size_t)states,sizeof(float)),*work=calloc((size_t)work_count,sizeof(float));
    float *input=calloc(4096,sizeof(float)),*hidden=calloc(4096,sizeof(float)),*norm=calloc(4096,sizeof(float)),*scale=calloc(4096,sizeof(float)),*scratch=calloc(4096,sizeof(float));
    if(!inv||!draft||!params||!temporary||!layers||!state||!work||!input||!hidden||!norm||!scale||!scratch){fprintf(stderr,"allocation failed\n");return 2;}
    coax_v_u8 bytes={map,st.st_size};coax_t_PwResult scan=p5_inspect_checked(bytes,(coax_v_t_P5Tensor){inv,4096},(coax_v_t_P5Tensor){draft,4096},0);
    if(scan.status || scan.written!=760){fprintf(stderr,"inventory %u\n",scan.status);return 1;}
    coax_t_PaFile file={bytes};coax_v_t_PaFile files={&file,1};coax_v_t_P5Tensor inventory={inv,scan.written};
    coax_t_PdShape delta={16,32,128,128,0.000001f};coax_t_QfShape attention={16,4,256,64,0.000001f};
    coax_t_QeShape shape={.linear={4096,12288,delta,COAX_QmDtype_bf16},.full={4096,12288,1,attention,COAX_QmDtype_bf16},.state={24,8,1,delta,attention}};
    for(int i=0;i<32;i++) {
        coax_t_PmResult r=pm_layer(shape,i,inventory,files,span(params+i*50000,50000),span(temporary,50000),0);
        if(r.status){fprintf(stderr,"layer %d mapping %u\n",i,r.status);return 1;}layers[i]=r.layer;
    }
    coax_t_P5Shape matrix={2,{{248320,4096,0,0,0}}},vector={1,{{4096,0,0,0,0}}};
    coax_t_PaView embedding=pa_view(str("model.language_model.embed_tokens.weight"),matrix,COAX_PwDtype_bf16,inventory,files);
    coax_t_PaView scale_view=pa_view(str("model.language_model.norm.weight"),vector,COAX_PwDtype_bf16,inventory,files);
    coax_t_PaView head=pa_view(str("lm_head.weight"),matrix,COAX_PwDtype_bf16,inventory,files);
    if(embedding.status||scale_view.status||head.status){fprintf(stderr,"embedding/norm/head mapping failed\n");return 1;}
    coax_t_PaView row=embedding;row.data.data+=32*4096*2;row.data.count=4096*2;row.elements=4096;
    coax_t_PwResult a=pa_decode(row,span(input,4096),span(scratch,4096),0);
    coax_t_PwResult b=pa_decode(scale_view,span(scale,4096),span(scratch,4096),0);
    if(a.status||b.status){fprintf(stderr,"decode failed\n");return 1;}
    float rope[64]={0};for(int i=0;i<64;i+=2)rope[i]=1;
    puts("executing one original Elephant token through 32 layers");fflush(stdout);
    coax_t_QeResult run=qe_block(shape,(coax_v_t_QeLayer){layers,32},span(input,4096),span(rope,64),0,1,span(state,states),span(hidden,4096),span(work,work_count),0);
    if(run.status){fprintf(stderr,"hybrid execution status %u\n",run.status);return 1;}
    coax_t_QmResult normalized=qb_begin(4096,0.000001f,span(hidden,4096),span(scale,4096),span(norm,4096),span(scratch,4096),0);
    if(normalized.status){fprintf(stderr,"final norm %u\n",normalized.status);return 1;}
    float logits[2],logit_work[2];coax_v_u8 rows={head.data.data+32*4096*2,2*4096*2};
    coax_t_QmResult projection=qm_project(COAX_QmDtype_bf16,2,4096,rows,span(norm,4096),span(logits,2),span(logit_work,2),0);
    if(projection.status){fprintf(stderr,"head projection %u\n",projection.status);return 1;}
    printf("token 32: hidden[0..3]=%.9g %.9g %.9g %.9g; slot A/B logits=%.9g %.9g\n",hidden[0],hidden[1],hidden[2],hidden[3],logits[0],logits[1]);
    free(inv);free(draft);free(params);free(temporary);free(layers);free(state);free(work);free(input);free(hidden);free(norm);free(scale);free(scratch);munmap(map,(size_t)st.st_size);return 0;
}
