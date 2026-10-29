/* Original-weight request experiment. Authenticate the original bundle first. */
#include "elephant-decision-runtime.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_f32 fs(float *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_u8 map_file(const char *path,int64_t expected){
    int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;
    if(fstat(fd,&st) || st.st_size<=0 || (expected && st.st_size!=expected))exit(2);
    void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED){perror("mmap");exit(2);}
    return (coax_v_u8){p,st.st_size};
}
int main(int argc,char **argv){
    if(argc!=4){fprintf(stderr,"usage: elephant-decision MODEL_SAFETENSORS TOKENIZER_PST TOKENS_OUT\n");return 2;}
    coax_v_u8 table=map_file(argv[2],0);if(!pt_admit(table)){fprintf(stderr,"tokenizer identity failed\n");return 1;}
    unsigned char *prompt=calloc(65536,1),*render_work=calloc(65536,1),*normal=calloc(262144,1);
    int64_t *candidate=calloc(16384,sizeof(int64_t)),*tokens=calloc(16384,sizeof(int64_t)),*stage=calloc(16384,sizeof(int64_t)),*scalars=calloc(262144,sizeof(int64_t)),*symbols=calloc(1024,sizeof(int64_t));
    if(!prompt||!render_work||!normal||!candidate||!tokens||!stage||!scalars||!symbols)return 2;
    coax_t_PtEntry entry={COAX_PtEntryKind_text,str("Water is wet.")};
    coax_t_PtQuestion question={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Is the water wet?")}};
    coax_t_PtOption options[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
    coax_t_PtPrompt rendered=pt_render_checked(entry,question,(coax_v_t_PtOption){options,2},(coax_v_u8){prompt,65536},(coax_v_u8){render_work,65536},0);
    if(rendered.status){fprintf(stderr,"render status %u\n",rendered.status);return 1;}
    coax_t_PtResult a=pt_encode_checked(table,(coax_string){(char *)prompt,rendered.prefix_bytes},(coax_v_s64){candidate,16384},(coax_v_s64){stage,16384},(coax_v_u8){normal,262144},(coax_v_s64){scalars,262144},(coax_v_s64){symbols,1024},0);
    coax_t_PtResult b=pt_encode_checked(table,(coax_string){(char *)prompt,rendered.written},(coax_v_s64){tokens,16384},(coax_v_s64){stage,16384},(coax_v_u8){normal,262144},(coax_v_s64){scalars,262144},(coax_v_s64){symbols,1024},0);
    if(a.status||b.status || b.written>512){fprintf(stderr,"encoding status %u/%u tokens %lld\n",a.status,b.status,(long long)b.written);return 1;}
    int64_t end=b.written;coax_t_PtBranch branch;
    coax_t_PtSplit split=pt_split_checked((coax_v_s64){candidate,a.written},(coax_v_s64){tokens,b.written},(coax_v_s64){&end,1},(coax_v_t_PtBranch){&branch,1},0);
    if(split.status){fprintf(stderr,"prefix split status %u\n",split.status);return 1;}
    FILE *token_out=fopen(argv[3],"w");if(!token_out)return 2;
    fprintf(token_out,"%lld\n",(long long)split.prefix);for(int64_t i=0;i<b.written;i++)fprintf(token_out,"%lld\n",(long long)tokens[i]);if(fclose(token_out))return 2;
    printf("decision tokens: prefix=%lld suffix=%lld\n",(long long)split.prefix,(long long)(b.written-split.prefix));fflush(stdout);
    coax_v_u8 bytes=map_file(argv[1],18819722392LL);coax_t_P5Tensor *inv=calloc(4096,sizeof(*inv)),*draft=calloc(4096,sizeof(*draft));
    float *params=calloc(32*50000,sizeof(float)),*temp=calloc(50000,sizeof(float)),*scale=calloc(4096,sizeof(float)),*scale_work=calloc(4096,sizeof(float));coax_t_QeLayer *layers=calloc(32,sizeof(*layers));
    if(!inv||!draft||!params||!temp||!scale||!scale_work||!layers)return 2;
    coax_t_PwResult scan=p5_inspect_checked(bytes,(coax_v_t_P5Tensor){inv,4096},(coax_v_t_P5Tensor){draft,4096},0);
    if(scan.status || scan.written!=760){fprintf(stderr,"inventory %u\n",scan.status);return 1;}
    coax_t_PaFile file={bytes};coax_v_t_PaFile files={&file,1};coax_v_t_P5Tensor inventory={inv,scan.written};
    coax_t_PdShape delta={16,32,128,128,0.000001f};coax_t_QfShape att={16,4,256,64,0.000001f};
    coax_t_QeShape shape={.linear={4096,12288,delta,COAX_QmDtype_bf16},.full={4096,12288,b.written,att,COAX_QmDtype_bf16},.state={24,8,b.written,delta,att}};
    for(int i=0;i<32;i++){
        coax_t_PmResult m=pm_layer(shape,i,inventory,files,fs(params+i*50000,50000),fs(temp,50000),0);
        if(m.status){fprintf(stderr,"layer %d mapping %u\n",i,m.status);return 1;}layers[i]=m.layer;
    }
    coax_t_P5Shape matrix={2,{{248320,4096,0,0,0}}},vector={1,{{4096,0,0,0,0}}};
    coax_t_PaView embedding=pa_view(str("model.language_model.embed_tokens.weight"),matrix,COAX_PwDtype_bf16,inventory,files);
    coax_t_PaView head=pa_view(str("lm_head.weight"),matrix,COAX_PwDtype_bf16,inventory,files);
    coax_t_PaView scale_view=pa_view(str("model.language_model.norm.weight"),vector,COAX_PwDtype_bf16,inventory,files);
    if(embedding.status||head.status||scale_view.status || pa_decode(scale_view,fs(scale,4096),fs(scale_work,4096),0).status)return 1;
    int64_t states=24*(3*8192+32*128*128)+8*2*b.written*4*256;
    int64_t work_count=5*states+b.written*100000+5*1048576;
    float *state=calloc((size_t)states,sizeof(float)),*work=calloc((size_t)work_count,sizeof(float));if(!state||!work)return 2;
    coax_t_QxModel model={shape,{layers,32},248320,embedding.data,head.data,{scale,4096}};
    int64_t slots[2]={qr_pinned_slot(0),qr_pinned_slot(1)};double result[2],prob_work[4];
    puts("executing original Elephant decision");fflush(stdout);
    coax_t_QxResult answer=qs_decide(model,(coax_v_s64){tokens,b.written},split.prefix,fs(state,states),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){result,2},(coax_v_f64){prob_work,4},fs(work,work_count),0);
    if(answer.status){fprintf(stderr,"decision status %u cause %lld\n",answer.status,(long long)answer.cause);return 1;}
    printf("decision: choice=%s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n",answer.answer.best==0?"yes":"no",result[0],result[1],answer.answer.mass,answer.answer.confidence);
    return 0;
}
