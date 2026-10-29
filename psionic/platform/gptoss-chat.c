/* A byte/file/device shell around Coax's admitted model, Harmony renderer,
   launch programs, greedy sampler and final-only display gate. */
#define _POSIX_C_SOURCE 200809L
#include "gptoss-device.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static void fail(const char *s){fprintf(stderr,"%s\n",s);exit(1);}
static void effect(PcStatus s){if(s){fprintf(stderr,"device effect %d: %s\n",s,pc_log());exit(1);}}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_string text(const char *s){return(coax_string){s,(int64_t)strlen(s)};}
static void *map(const char *name,size_t *n){int fd=open(name,O_RDONLY);struct stat s;if(fd<0 || fstat(fd,&s) || s.st_size<1)fail("cannot map input file");*n=(size_t)s.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)fail("mmap failed");return p;}
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static void answer(PgSession *session,coax_v_u8 table,const char *prompt){
    effect(pg_reset(session));
    uint8_t system[1024],system_work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(system_work,1024),0);if(sys.status)fail("system template refused");
    coax_t_HmMessage messages[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=text(prompt)}};
    int64_t tokens[16384],stage[16384],symbols[1024],control[8];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){messages,2},1,iv(tokens,16384),iv(stage,16384),iv(symbols,1024),0);if(r.status)fail("prompt rendering refused");
    int64_t prefix=r.written,count=prefix;double start=now();effect(pg_prefill(session,tokens,prefix,0,1,256,control));if(control[0])fail("prompt arithmetic refused");double prefill=now()-start;tokens[count++]=control[1];start=now();
    while(!control[4]){effect(pg_step(session,-1,control[2],1,256,1,control));if(control[0])fail("decode arithmetic refused");tokens[count++]=control[1];}
    fprintf(stderr,"prefix=%lld generated=%lld stop=%lld prefill=%.3f s decode=%.3f s\n",(long long)prefix,(long long)(count-prefix),(long long)control[4],prefill,now()-start);
    uint8_t bytes[65536],work[65536],visible[65536];coax_t_HmRecord records[64],draft[64];r=hm_parse_checked(table,iv(tokens,count),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(bytes,65536),bv(work,65536),0);if(r.status)fail("incomplete or invalid Harmony turn");
    for(int64_t i=0;i<r.written;i++){coax_t_GResult why=gt_display(records[i],(coax_string){(const char*)bytes,r.bytes},bv(visible,65536),0);if(!why.status){if(fwrite(visible,1,(size_t)why.written,stdout)!=(size_t)why.written)fail("stdout write failed");putchar('\n');}}
}
int main(int argc,char **argv){
    if(argc!=5){fprintf(stderr,"usage: gptoss-chat ORIGINAL_CACHE HARMONY_TABLE GENERATED_CUDA PROMPT|--interactive\n");return 2;}
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});if(profile.status)fail("profile refused");
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];for(int i=0;i<3;i++){char name[4096];int n=snprintf(name,sizeof name,"%s/%.*s",argv[1],(int)pins[i].name.count,pins[i].name.data);if(n<1 || (size_t)n>=sizeof name)fail("cache path too long");maps[i]=map(name,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    size_t nt,nc;void *table_data=map(argv[2],&nt);const char *cuda=map(argv[3],&nc);coax_v_u8 table=bv(table_data,(int64_t)nt);if(!hs_admit(table))fail("Harmony table identity refused");
    PgSession *session=NULL;coax_t_GResult why;double start=now();effect(pg_open(profile,(coax_v_t_GFile){files,3},cuda,nc,17179869184,&session,&why));if(why.status || !session){fprintf(stderr,"model admission refused: %u\n",why.status);return 1;}fprintf(stderr,"admitted/uploaded once: %.3f s\n",now()-start);
    if(!strcmp(argv[4],"--interactive")){
        char *line=NULL;size_t capacity=0;ssize_t length;
        fprintf(stderr,"Each input line is an independent request; weights stay resident. EOF exits.\n");
        while((length=getline(&line,&capacity,stdin))>=0){if(length && line[length-1]=='\n')line[--length]=0;answer(session,table,line);fflush(stdout);}free(line);
    }else answer(session,table,argv[4]);
    pg_close(session);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,nt);munmap((void*)cuda,nc);return 0;
}
