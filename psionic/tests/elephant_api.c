/* Exercise the public typed request against the authenticated original Elephant. */
#include "decision-answer.h"
#include "elephant-inventory.h"
#include "elephant-request.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
int main(int argc,char **argv){
 if(argc!=3){fprintf(stderr,"usage: elephant-api ORIGINAL_DIRECTORY TOKENIZER_PST\n");return 2;}
 coax_v_u8 config=named(argv[1],"config.json"),tokenizer=named(argv[1],"tokenizer.json"),readout=named(argv[1],"readout.json"),index={NULL,0},table=map_file(argv[2]);
 coax_t_PaFile file={named(argv[1],"model.safetensors")};size_t count=sizeof(elephant_specs)/sizeof(*elephant_specs);
 coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *inventory=calloc(count,sizeof(*inventory)),*draft=calloc(count,sizeof(*draft));
 coax_t_QeLayer *layers=calloc(32,sizeof(*layers));float *params=calloc(1057280,sizeof(float)),*stage=calloc(1057280,sizeof(float)),*work=calloc(50000,sizeof(float)),*norm=calloc(4096,sizeof(float)),*norm_work=calloc(4096,sizeof(float));
 if(!specs||!inventory||!draft||!layers||!params||!stage||!work||!norm||!norm_work)return 2;
 for(size_t i=0;i<count;i++){specs[i].name=str(elephant_specs[i].name);specs[i].shape.rank=elephant_specs[i].rank;for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=elephant_specs[i].dims[j];specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=0;}
 coax_t_PsWorkspace memory={{inventory,(int64_t)count},{draft,(int64_t)count},{layers,32},{params,1057280},{stage,1057280},{work,50000},{norm,4096},{norm_work,4096}};
 puts("authenticating original Elephant");fflush(stdout);
 coax_t_PsSession s=psionic_session_open(0,86,config,tokenizer,readout,index,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,(int64_t)count},table,memory);
 if(s.status){fprintf(stderr,"session status=%u cause=%lld\n",s.status,(long long)s.cause);return 1;}
 unsigned char *prompt=calloc(65536,1),*render_work=calloc(65536,1),*normal=calloc(262144,1);
 int64_t *candidate=calloc(16384,sizeof(int64_t)),*full=calloc(16384,sizeof(int64_t)),*token_stage=calloc(16384,sizeof(int64_t)),*scalars=calloc(262144,sizeof(int64_t)),*symbols=calloc(1024,sizeof(int64_t)),*tokens=calloc(16384,sizeof(int64_t));
 if(!prompt||!render_work||!normal||!candidate||!full||!token_stage||!scalars||!symbols||!tokens)return 2;
 coax_t_PrWorkspace prep={{prompt,65536},{render_work,65536},{candidate,16384},{full,16384},{token_stage,16384},{normal,262144},{scalars,262144},{symbols,1024}};
 coax_t_PtEntry entry={COAX_PtEntryKind_text,str("Water is wet.")};coax_t_PtQuestion q={COAX_PtQuestionKind_choice,{COAX_PtEntryKind_text,str("Is the water wet?")}};
 coax_t_PtOption opts[2]={{str("yes"),{COAX_PtEntryKind_none,str("")}},{str("no"),{COAX_PtEntryKind_none,str("")}}};
 coax_t_PrResult p=psionic_prepare(table,entry,q,(coax_v_t_PtOption){opts,2},(coax_v_s64){tokens,16384},prep);
 if(p.status||p.written!=86||p.prefix!=49){fprintf(stderr,"prepare status=%u cause=%lld count=%lld prefix=%lld\n",p.status,(long long)p.cause,(long long)p.written,(long long)p.prefix);return 1;}
 if(p.prefix!=elephant_request_prefix || p.written!=(int64_t)(sizeof(elephant_request_tokens)/sizeof(*elephant_request_tokens)) || memcmp(tokens,elephant_request_tokens,sizeof(elephant_request_tokens))){fprintf(stderr,"prepared token stream differs from reference\n");return 1;}
 int64_t states=24*(3*8192+32*128*128)+8*2*p.written*4*256, work_count=5*states+p.written*100000+5*1048576;
 float *state=calloc((size_t)states,sizeof(float)),*scratch=calloc((size_t)work_count,sizeof(float));double out[2],prob[4];if(!state||!scratch)return 2;
 puts("executing typed decision");fflush(stdout);
 coax_t_PsAnswer a=psionic_answer(s,p,(coax_v_s64){tokens,p.written},(coax_v_t_PtOption){opts,2},1,(coax_v_f32){state,states},(coax_v_f64){out,2},(coax_v_f64){prob,4},(coax_v_f32){scratch,work_count});
 if(a.status){fprintf(stderr,"answer status=%u cause=%lld\n",a.status,(long long)a.cause);return 1;}
 printf("choice=%.*s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n",(int)a.choice.count,a.choice.data,out[0],out[1],a.statistics.mass,a.statistics.confidence);
 return 0;
}
