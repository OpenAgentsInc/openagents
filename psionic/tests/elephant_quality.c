/* Run the frozen chapter 12 held-out set through the authenticated original Elephant. */
#define _POSIX_C_SOURCE 200809L
#include "elephant-quality-runtime.h"
#include "elephant-inventory.h"
#include "quality-corpus.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/wait.h>
coax_t_PsSession psionic_session_open(uint8_t,int64_t,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_t_PaFile,coax_v_t_P5Spec,coax_v_u8,coax_t_PsWorkspace);
coax_t_PrResult psionic_prepare(coax_v_u8,coax_t_PtEntry,coax_t_PtQuestion,coax_v_t_PtOption,coax_v_s64,coax_t_PrWorkspace);
coax_t_PsAnswer psionic_answer(coax_t_PsSession,coax_t_PrResult,coax_v_s64,coax_v_t_PtOption,float,coax_v_f32,coax_v_f64,coax_v_f64,coax_v_f32);
coax_t_PjResult psionic_answer_json(coax_t_QrKind,uint8_t,coax_v_f64,coax_v_t_PtOption,coax_v_u8);
enum{CAPACITY=192,ITEMS=sizeof(quality_items)/sizeof(*quality_items)};
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 map_file(const char *path){int fd=open(path,O_RDONLY);if(fd<0){perror("open");exit(2);}struct stat st;if(fstat(fd,&st)||st.st_size<=0)exit(2);void *p=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)exit(2);return (coax_v_u8){p,st.st_size};}
static coax_v_u8 named(const char *dir,const char *name){char path[4096];int n=snprintf(path,sizeof(path),"%s/%s",dir,name);if(n<0||(size_t)n>=sizeof(path))exit(2);return map_file(path);}
static double seconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (double)t.tv_sec+(double)t.tv_nsec*1e-9;}
static coax_t_PtEntry text(const char *s){return s?(coax_t_PtEntry){COAX_PtEntryKind_text,str(s)}:(coax_t_PtEntry){COAX_PtEntryKind_none,str("")};}
int main(int argc,char **argv){
 if(argc!=3&&argc!=4){fprintf(stderr,"usage: elephant-quality ORIGINAL_DIRECTORY TOKENIZER_PST [WORKERS]\n");return 2;}
 coax_v_u8 config=named(argv[1],"config.json"),tokenizer=named(argv[1],"tokenizer.json"),readout=named(argv[1],"readout.json"),index={NULL,0},table=map_file(argv[2]);
 /* The pinned bundle has no calibration.json; its absence is the calibration. */
 char probe[4096];snprintf(probe,sizeof(probe),"%s/calibration.json",argv[1]);
 if(!access(probe,F_OK)){fprintf(stderr,"calibration.json is not part of the pinned bundle\n");return 1;}
 coax_t_PcCalibration calibration=pc_load(0,readout,(coax_v_u8){NULL,0});
 if(calibration.status){fprintf(stderr,"calibration status=%u\n",calibration.status);return 1;}
 printf("calibration: none in the pinned bundle; temperature %.1f for every kind\n",(double)pc_temperature(calibration,COAX_QrKind_choice,2));
 coax_t_PaFile file={named(argv[1],"model.safetensors")};size_t count=sizeof(elephant_specs)/sizeof(*elephant_specs);
 coax_t_P5Spec *specs=calloc(count,sizeof(*specs));coax_t_P5Tensor *inventory=calloc(count,sizeof(*inventory)),*draft=calloc(count,sizeof(*draft));
 coax_t_QeLayer *layers=calloc(32,sizeof(*layers));float *params=calloc(1057280,sizeof(float)),*stage=calloc(1057280,sizeof(float)),*work=calloc(50000,sizeof(float)),*norm=calloc(4096,sizeof(float)),*norm_work=calloc(4096,sizeof(float));
 if(!specs||!inventory||!draft||!layers||!params||!stage||!work||!norm||!norm_work)return 2;
 for(size_t i=0;i<count;i++){specs[i].name=str(elephant_specs[i].name);specs[i].shape.rank=elephant_specs[i].rank;for(int j=0;j<5;j++)specs[i].shape.dims.e[j]=elephant_specs[i].dims[j];specs[i].dtype=COAX_PwDtype_bf16;specs[i].shard=0;}
 coax_t_PsWorkspace memory={{inventory,(int64_t)count},{draft,(int64_t)count},{layers,32},{params,1057280},{stage,1057280},{work,50000},{norm,4096},{norm_work,4096}};
 puts("authenticating original Elephant");fflush(stdout);
 double t0=seconds();
 coax_t_PsSession s=psionic_session_open(0,CAPACITY,config,tokenizer,readout,index,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,(int64_t)count},table,memory);
 if(s.status){fprintf(stderr,"session status=%u cause=%lld\n",s.status,(long long)s.cause);return 1;}
 printf("session open in %.0f s\n",seconds()-t0);fflush(stdout);
 unsigned char *prompt=calloc(65536,1),*render_work=calloc(65536,1),*normal=calloc(262144,1),*json=calloc(65536,1);
 int64_t *candidate=calloc(16384,sizeof(int64_t)),*full=calloc(16384,sizeof(int64_t)),*token_stage=calloc(16384,sizeof(int64_t)),*scalars=calloc(262144,sizeof(int64_t)),*symbols=calloc(1024,sizeof(int64_t));
 coax_t_PrWorkspace prep={{prompt,65536},{render_work,65536},{candidate,16384},{full,16384},{token_stage,16384},{normal,262144},{scalars,262144},{symbols,1024}};
 /* Prepare every request here; workers then run independent decisions from the same session. */
 static coax_t_PtOption options[ITEMS][8];static int64_t tokens[ITEMS][CAPACITY];coax_t_PrResult prepared[ITEMS];
 int64_t *buffer=calloc(16384,sizeof(int64_t));if(!buffer)return 2;
 for(size_t k=0;k<ITEMS;k++){
  const quality_item *q=&quality_items[k];
  for(int i=0;i<q->count;i++){options[k][i].name=str(q->names[i]?q->names[i]:"");options[k][i].description=text(q->descriptions[i]);}
  coax_t_PtQuestion question={(coax_t_PtQuestionKind)q->kind,text(q->question)};
  coax_t_PrResult p=psionic_prepare(table,text(q->state),question,(coax_v_t_PtOption){options[k],q->count},(coax_v_s64){buffer,16384},prep);
  if(p.status||p.written>CAPACITY){fprintf(stderr,"%s: prepare status=%u cause=%lld tokens=%lld\n",q->id,p.status,(long long)p.cause,(long long)p.written);return 1;}
  memcpy(tokens[k],buffer,(size_t)p.written*sizeof(int64_t));prepared[k]=p;
 }
 int workers=argc>3?atoi(argv[3]):1;if(workers<1||workers>ITEMS)return 2;
 typedef struct {int status;int64_t best;double mass,seconds,p[8];} outcome;
 outcome *done=mmap(NULL,sizeof(outcome)*ITEMS,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);if(done==MAP_FAILED)return 2;
 for(size_t k=0;k<ITEMS;k++)done[k].status=-1;
 printf("running %zu decisions in %d processes\n",(size_t)ITEMS,workers);fflush(stdout);
 for(int w=0;w<workers;w++){
  pid_t pid=fork();if(pid<0)return 2;if(pid)continue;
  int64_t max_states=24*(3*8192+32*128*128)+8*2*CAPACITY*4*256,max_work=5*max_states+CAPACITY*100000+5*1048576;
  float *state=calloc((size_t)max_states,sizeof(float)),*scratch=calloc((size_t)max_work,sizeof(float));if(!state||!scratch)_exit(2);
  for(size_t k=(size_t)w;k<ITEMS;k+=(size_t)workers){
   const quality_item *q=&quality_items[k];coax_t_PrResult p=prepared[k];double prob[16];
   float temperature=pc_temperature(calibration,(coax_t_QrKind)q->kind,p.slots);
   /* State and workspace follow the session capacity, as in chapter 10 where it equalled the token count. */
   int64_t states=max_states,work_count=max_work;
   double t=seconds();
   coax_t_PsAnswer a=psionic_answer(s,p,(coax_v_s64){tokens[k],p.written},(coax_v_t_PtOption){options[k],q->count},temperature,(coax_v_f32){state,states},(coax_v_f64){done[k].p,p.slots},(coax_v_f64){prob,16},(coax_v_f32){scratch,work_count});
   done[k].best=a.statistics.best;done[k].mass=a.statistics.mass;done[k].seconds=seconds()-t;done[k].status=a.status;
   if(a.status)fprintf(stderr,"%s: answer status=%u cause=%lld\n",q->id,a.status,(long long)a.cause);
  }
  _exit(0);
 }
 for(int w=0;w<workers;w++){int st;if(wait(&st)<0||!WIFEXITED(st)||WEXITSTATUS(st))return 1;}
 double probabilities[ITEMS*8];coax_t_PqItem items[ITEMS];int64_t at=0;
 for(size_t k=0;k<ITEMS;k++){
  const quality_item *q=&quality_items[k];coax_t_PrResult p=prepared[k];
  if(done[k].status){fprintf(stderr,"%s did not finish\n",q->id);return 1;}
  memcpy(probabilities+at,done[k].p,(size_t)p.slots*sizeof(double));
  coax_t_PjResult j=psionic_answer_json((coax_t_QrKind)q->kind,0,(coax_v_f64){probabilities+at,p.slots},(coax_v_t_PtOption){options[k],q->count},(coax_v_u8){json,65536});
  if(j.status){fprintf(stderr,"%s: json status=%u\n",q->id,j.status);return 1;}
  printf("%s tokens=%lld prefix=%lld best=%lld gold=%d mass=%.6f seconds=%.0f\n  p=",q->id,(long long)p.written,(long long)p.prefix,(long long)done[k].best,q->gold,done[k].mass,done[k].seconds);
  for(int64_t i=0;i<p.slots;i++)printf("%s%.9f",i?",":"",probabilities[at+i]);
  printf("\n  %.*s\n",(int)j.written,(const char *)json);
  items[k]=(coax_t_PqItem){(coax_t_QrKind)q->kind,p.slots,at,q->gold,q->abstain};at+=p.slots;
 }
 coax_t_PqReport r=pq_measure((coax_v_t_PqItem){items,ITEMS},(coax_v_f64){probabilities,at});
 if(r.status){fprintf(stderr,"quality status=%u cause=%lld\n",r.status,(long long)r.cause);return 1;}
 printf("quality: items=%lld correct=%lld brier=%.6f ece=%.6f\n",(long long)r.items,(long long)r.correct,r.brier,r.ece);
 const char *names[3]={"choice","noul","score"};
 for(int k=0;k<3;k++)printf("  %s: items=%lld correct=%lld brier=%.6f\n",names[k],(long long)r.kinds.e[k].items,(long long)r.kinds.e[k].correct,r.kinds.e[k].brier);
 printf("  score mean absolute error=%.6f\n  abstention: offered=%lld abstained=%lld; answered=%lld correct=%lld\n",r.score_error,(long long)r.offered,(long long)r.abstained,(long long)r.answered,(long long)r.answered_correct);
 return 0;
}
