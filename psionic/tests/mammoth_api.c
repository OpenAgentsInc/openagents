/* Exercise the public typed request against the authenticated original Mammoth. */
#define _POSIX_C_SOURCE 200809L
#include "decision-plan.h"
#include "mammoth-inventory.h"
#include "mammoth-request.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
coax_t_PsSession psionic_session_open(uint8_t,int64_t,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_t_PaFile,coax_v_t_P5Spec,coax_v_u8,coax_t_PsWorkspace);
coax_t_PrResult psionic_prepare(coax_v_u8,coax_t_PtEntry,coax_t_PtQuestion,coax_v_t_PtOption,coax_v_s64,coax_t_PrWorkspace);
coax_t_PsAnswer psionic_answer(coax_t_PsSession,coax_t_PrResult,coax_v_s64,coax_v_t_PtOption,float,coax_v_f32,coax_v_f64,coax_v_f64,coax_v_f32);
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map_file(const char *path){int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;if(fstat(fd,&st)||st.st_size<=0)exit(2);void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};}
static coax_v_u8 named(const char *dir,const char *name){char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);if(n<0||(size_t)n>=sizeof(path))exit(2);return map_file(path);}
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (double)t.tv_sec+1e-9*(double)t.tv_nsec;}
static long peak_kib(void){struct rusage u;getrusage(RUSAGE_SELF,&u);return u.ru_maxrss;}
static void *zeroed(int64_t count,size_t width){void *p=calloc((size_t)count,width);if(!p){fprintf(stderr,"out of memory\n");exit(2);}return p;}
int main(int argc,char **argv){
 if(argc!=3){fprintf(stderr,"usage: mammoth-api ORIGINAL_DIRECTORY TOKENIZER_PST\n");return 2;}
 double began=now();
 coax_v_u8 config=named(argv[1],"config.json"),tokenizer=named(argv[1],"tokenizer.json"),readout=named(argv[1],"readout.json"),index=named(argv[1],"model.safetensors.index.json"),table=map_file(argv[2]);
 coax_t_PaFile files[2]={{.data=named(argv[1],"model-00001-of-00002.safetensors")},{.data=named(argv[1],"model-00002-of-00002.safetensors")}};
 /* Prepare first: the token stream is independent of the weights and sizes the request. */
 unsigned char *prompt=zeroed(65536,1),*render_work=zeroed(65536,1),*normal=zeroed(262144,1);
 int64_t *candidate=zeroed(16384,8),*full=zeroed(16384,8),*token_stage=zeroed(16384,8),*scalars=zeroed(262144,8),*symbols=zeroed(1024,8),*tokens=zeroed(16384,8);
 coax_t_PrWorkspace prep={{prompt,65536},{render_work,65536},{candidate,16384},{full,16384},{token_stage,16384},{normal,262144},{scalars,262144},{symbols,1024}};
 coax_t_PtEntry entry={COAX_PtEntryKind_text,str("Water is wet.")};coax_t_PtQuestion q={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Is the water wet?")}};
 coax_t_PtOption opts[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
 coax_t_PrResult p=psionic_prepare(table,entry,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){tokens,16384},prep);
 if(p.status){fprintf(stderr,"prepare status=%u cause=%lld\n",p.status,(long long)p.cause);return 1;}
 if(p.prefix!=mammoth_request_prefix || p.written!=(int64_t)(sizeof(mammoth_request_tokens)/sizeof(*mammoth_request_tokens)) || memcmp(tokens,mammoth_request_tokens,sizeof(mammoth_request_tokens))){fprintf(stderr,"prepared token stream differs from reference\n");return 1;}
 printf("prepared tokens=%lld prefix=%lld suffix=%lld\n",(long long)p.written,(long long)p.prefix,(long long)(p.written-p.prefix));
 /* Every caller allocation comes from the Coax plan for this target and request. */
 coax_t_PsPlan plan=ps_plan(1,p.written,p.written,p.prefix);
 if(plan.status){fprintf(stderr,"plan status=%u cause=%lld\n",plan.status,(long long)plan.cause);return 1;}
 printf("plan state=%lld work=%lld params=%lld weight_bytes=%lld\n",(long long)plan.state,(long long)plan.work,(long long)plan.params,(long long)plan.weight_bytes);
 size_t count=sizeof(mammoth_specs)/sizeof(*mammoth_specs);if((int64_t)count!=plan.tensors)return 1;
 coax_t_P5Spec *specs=zeroed(plan.tensors,sizeof(coax_t_P5Spec));coax_t_P5Tensor *inventory=zeroed(plan.tensors,sizeof(coax_t_P5Tensor)),*draft=zeroed(plan.tensors,sizeof(coax_t_P5Tensor));
 coax_t_QeLayer *layers=zeroed(plan.layers,sizeof(coax_t_QeLayer));
 float *params=zeroed(plan.params,4),*stage=zeroed(plan.params,4),*work=zeroed(plan.layer_work,4),*norm=zeroed(plan.norm,4),*norm_work=zeroed(plan.norm,4);
 for(size_t i=0;i<count;i++){specs[i].name=str(mammoth_specs[i].name);specs[i].shape.rank=mammoth_specs[i].rank;for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=mammoth_specs[i].dims[j];specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=mammoth_specs[i].shard;}
 coax_t_PsWorkspace memory={{inventory,plan.tensors},{draft,plan.tensors},{layers,plan.layers},{params,plan.params},{stage,plan.params},{work,plan.layer_work},{norm,plan.norm},{norm_work,plan.norm}};
 puts("authenticating original Mammoth");fflush(stdout);
 double t0=now();
 coax_t_PsSession s=psionic_session_open(1,p.written,config,tokenizer,readout,index,(coax_v_t_PaFile){files,2},(coax_v_t_P5Spec){specs,(int64_t)count},table,memory);
 if(s.status){fprintf(stderr,"session status=%u cause=%lld\n",s.status,(long long)s.cause);return 1;}
 double t1=now();printf("phase open seconds=%.1f peak_rss_kib=%ld\n",t1-t0,peak_kib());
 float *state=zeroed(plan.state,4),*scratch=zeroed(plan.work,4);double out[2],prob[4];
 puts("executing typed decision");fflush(stdout);
 coax_t_PsAnswer a=psionic_answer(s,p,(coax_v_s64){tokens,p.written},(coax_v_t_PtOption){opts,2},1,(coax_v_f32){state,plan.state},(coax_v_f64){out,2},(coax_v_f64){prob,4},(coax_v_f32){scratch,plan.work});
 if(a.status){fprintf(stderr,"answer status=%u cause=%lld\n",a.status,(long long)a.cause);return 1;}
 double t2=now();printf("phase answer seconds=%.1f peak_rss_kib=%ld total_seconds=%.1f\n",t2-t1,peak_kib(),t2-began);
 printf("choice=%.*s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n",(int)a.choice.count,a.choice.data,out[0],out[1],a.statistics.mass,a.statistics.confidence);
 return 0;
}
