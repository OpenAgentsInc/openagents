/* Time the public typed request against the authenticated original Elephant.
   Usage: elephant-bench ORIGINAL_DIRECTORY TOKENIZER_PST [REPEAT [RUNS]]
   REPEAT copies of "Water is wet." form the state (1 reproduces chapter 14's
   request); RUNS decisions reuse one authenticated session. */
#define _POSIX_C_SOURCE 200809L
#include "decision-answer.h"
#include "elephant-inventory.h"
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
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (double)t.tv_sec+(double)t.tv_nsec*1e-9;}
static long peak_mib(void){struct rusage u;getrusage(RUSAGE_SELF,&u);return u.ru_maxrss/1024;}
static coax_v_u8 map_file(const char *path){int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;if(fstat(fd,&st)||st.st_size<=0)exit(2);void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};}
static coax_v_u8 named(const char *dir,const char *name){char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);if(n<0||(size_t)n>=sizeof(path))exit(2);return map_file(path);}
int main(int argc,char **argv){
 if(argc<3||argc>5){fprintf(stderr,"usage: elephant-bench ORIGINAL_DIRECTORY TOKENIZER_PST [REPEAT [RUNS]]\n");return 2;}
 int repeat=argc>3?atoi(argv[3]):1,runs=argc>4?atoi(argv[4]):1;if(repeat<1||repeat>2000||runs<1||runs>16)return 2;
 coax_v_u8 table=map_file(argv[2]);
 char *text=calloc((size_t)repeat*15+1,1);if(!text)return 2;
 for(int i=0;i<repeat;i++)strcat(text,i?" Water is wet.":"Water is wet.");
 unsigned char *prompt=calloc(65536,1),*render_work=calloc(65536,1),*normal=calloc(262144,1);
 int64_t *candidate=calloc(16384,sizeof(int64_t)),*full=calloc(16384,sizeof(int64_t)),*token_stage=calloc(16384,sizeof(int64_t)),*scalars=calloc(262144,sizeof(int64_t)),*symbols=calloc(1024,sizeof(int64_t)),*tokens=calloc(16384,sizeof(int64_t));
 if(!prompt||!render_work||!normal||!candidate||!full||!token_stage||!scalars||!symbols||!tokens)return 2;
 coax_t_PrWorkspace prep={{prompt,65536},{render_work,65536},{candidate,16384},{full,16384},{token_stage,16384},{normal,262144},{scalars,262144},{symbols,1024}};
 coax_t_PtEntry entry={COAX_PtEntryKind_text,str(text)};coax_t_PtQuestion q={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Is the water wet?")}};
 coax_t_PtOption opts[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
 double t0=now();
 coax_t_PrResult p=psionic_prepare(table,entry,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){tokens,16384},prep);
 double t_prepare=now()-t0;
 if(p.status){fprintf(stderr,"prepare status=%u cause=%lld\n",p.status,(long long)p.cause);return 1;}
 coax_v_u8 config=named(argv[1],"config.json"),tokenizer=named(argv[1],"tokenizer.json"),readout=named(argv[1],"readout.json"),index={NULL,0};
 coax_t_PaFile file={.data=named(argv[1],"model.safetensors")};size_t count=sizeof(elephant_specs)/sizeof(*elephant_specs);
 coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *inventory=calloc(count,sizeof(*inventory)),*draft=calloc(count,sizeof(*draft));
 coax_t_QeLayer *layers=calloc(32,sizeof(*layers));float *params=calloc(1057280,sizeof(float)),*stage=calloc(1057280,sizeof(float)),*work=calloc(50000,sizeof(float)),*norm=calloc(4096,sizeof(float)),*norm_work=calloc(4096,sizeof(float));
 if(!specs||!inventory||!draft||!layers||!params||!stage||!work||!norm||!norm_work)return 2;
 for(size_t i=0;i<count;i++){specs[i].name=str(elephant_specs[i].name);specs[i].shape.rank=elephant_specs[i].rank;for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=elephant_specs[i].dims[j];specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=0;}
 coax_t_PsWorkspace memory={{inventory,(int64_t)count},{draft,(int64_t)count},{layers,32},{params,1057280},{stage,1057280},{work,50000},{norm,4096},{norm_work,4096}};
 t0=now();
 coax_t_PsSession s=psionic_session_open(0,p.written,config,tokenizer,readout,index,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,(int64_t)count},table,memory);
 double t_open=now()-t0;
 if(s.status){fprintf(stderr,"session status=%u cause=%lld\n",s.status,(long long)s.cause);return 1;}
 printf("tokens=%lld prefix=%lld suffix=%lld prepare_s=%.6f open_s=%.3f peak_mib=%ld\n",(long long)p.written,(long long)p.prefix,(long long)(p.written-p.prefix),t_prepare,t_open,peak_mib());fflush(stdout);
 int64_t states=24*(3*8192+32*128*128)+8*2*p.written*4*256, work_count=5*states+p.written*100000+5*1048576;
 for(int r=0;r<runs;r++){
  float *state=calloc((size_t)states,sizeof(float)),*scratch=calloc((size_t)work_count,sizeof(float));double out[2],prob[4];if(!state||!scratch)return 2;
  t0=now();
  coax_t_PsAnswer a=psionic_answer(s,p,(coax_v_s64){tokens,p.written},(coax_v_t_PtOption){opts,2},1,(coax_v_f32){state,states},(coax_v_f64){out,2},(coax_v_f64){prob,4},(coax_v_f32){scratch,work_count});
  double t_answer=now()-t0;
  if(a.status){fprintf(stderr,"answer status=%u cause=%lld\n",a.status,(long long)a.cause);return 1;}
  printf("run=%d answer_s=%.3f tokens_per_s=%.2f choice=%.*s p_yes=%.9g p_no=%.9g peak_mib=%ld\n",r,t_answer,(double)p.written/t_answer,(int)a.choice.count,a.choice.data,out[0],out[1],peak_mib());fflush(stdout);
  free(state);free(scratch);
 }
 return 0;
}
