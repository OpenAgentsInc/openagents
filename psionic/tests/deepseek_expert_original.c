/* Explicit original-component qualification. I/O, allocation and clocks only.
 * Coax authenticates the whole shard, binds reviewed views and runs the FFNs. */
#define _POSIX_C_SOURCE 200809L
#include "deepseek-expert-memory.h"
#include <assert.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static double now(void) {struct timespec t;assert(clock_gettime(CLOCK_MONOTONIC,&t)==0);return (double)t.tv_sec+1e-9*(double)t.tv_nsec;}
static void *allocate(int64_t n,size_t width) {void *p=calloc((size_t)n,width);assert(p);return p;}
static FILE *output(const char *dir,const char *name,const char *mode) {char path[4096];assert(snprintf(path,sizeof(path),"%s/%s",dir,name)>0);FILE *f=fopen(path,mode);assert(f);return f;}
static void write_array(const char *dir,const char *tag,const char *kind,const void *data,size_t width,int64_t count) {char name[128];assert(snprintf(name,sizeof(name),"%s-%s",tag,kind)>0);FILE *f=output(dir,name,"wb");assert(fwrite(data,width,(size_t)count,f)==(size_t)count);fclose(f);}
static void mixture(coax_v_u8 data,const char *dir,double admission) {
    coax_t_DeExpert *experts=allocate(384,sizeof(*experts));assert(de_test_original_all(data,(coax_v_t_DeExpert){experts,384})==384);coax_t_DeExpert shared=de_test_original(data,1);coax_t_DePlan p=de_plan(shared);assert(p.status==COAX_DsStatus_ok);
    float *x=allocate(p.width,sizeof(float)),*out=allocate(p.width,sizeof(float));assert(de_test_original_input((coax_v_f32){x,p.width})==p.width);
    float logits[384],temporary[384],weights[8];int64_t ids[8];double raw[384],ranked[384];
    double begin=now();coax_t_DrRoute route=de_test_original_route(data,(coax_v_f32){x,p.width},(coax_v_f32){logits,384},(coax_v_f32){temporary,384},(coax_v_f64){raw,384},(coax_v_f64){ranked,384});assert(route.status==COAX_DrStatus_ok && route.count==6);assert(de_test_route_values(route,(coax_v_s64){ids,8},(coax_v_f32){weights,8})==6);
    coax_t_DeWork work={.floats={allocate(p.floats+2*p.width,sizeof(float)),p.floats+2*p.width},.wide={allocate(p.wide,sizeof(double)),p.wide}};coax_t_PsionicAccumulator *acc=allocate(p.width,sizeof(*acc));
    coax_t_DeResult r=psionic_de_mixture((coax_v_t_DeExpert){experts,384},shared,(coax_v_f32){x,p.width},(coax_v_s64){ids,6},(coax_v_f32){weights,6},10,(coax_v_f32){out,p.width},work,(coax_v_t_PsionicAccumulator){acc,p.width});assert(r.status==COAX_DsStatus_ok && r.written==p.width);double elapsed=now()-begin;
    write_array(dir,"mixture","input.f32",x,sizeof(float),p.width);write_array(dir,"mixture","output.f32",out,sizeof(float),p.width);write_array(dir,"mixture","gate-logits.f32",logits,sizeof(float),384);write_array(dir,"mixture","scores.f64",raw,sizeof(double),384);write_array(dir,"mixture","ranked.f64",ranked,sizeof(double),384);write_array(dir,"mixture","ids.i64",ids,sizeof(int64_t),6);write_array(dir,"mixture","weights.f32",weights,sizeof(float),6);
    FILE *f=output(dir,"mixture.json","w");fprintf(f,"{\"experts\":384,\"selected\":6,\"width\":%" PRId64 ",\"hidden\":%" PRId64 ",\"gate_clamped\":%" PRId64 ",\"up_clamped\":%" PRId64 ",\"margin\":%.17g,\"excluded\":%" PRId64 ",\"seconds\":%.9f,\"admission_seconds\":%.9f}\n",p.width,p.hidden,r.gate_clamped,r.up_clamped,route.margin,route.excluded,elapsed,admission);fclose(f);
    fprintf(stderr,"original 384/6 MoE plus shared FP8 expert in %.6fs; route margin %.9g\n",elapsed,route.margin);free(x);free(out);free(experts);free(acc);free(work.floats.data);free(work.wide.data);
}
int main(int argc,char **argv) {
    assert(argc==3 || (argc==4 && strcmp(argv[3],"--mixture")==0));int fd=open(argv[1],O_RDONLY);assert(fd>=0);struct stat st;assert(fstat(fd,&st)==0 && st.st_size>0);
    uint8_t *data=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);assert(data!=MAP_FAILED);close(fd);
    coax_t_DsArtifact pin=de_test_original_pin();double begin=now();assert(pw_digest((coax_v_u8){data,st.st_size},pin.sha));double admission=now()-begin;
    fprintf(stderr,"all original shard bytes authenticated in %.6fs\n",admission);
    if (argc==4) {mixture((coax_v_u8){data,st.st_size},argv[2],admission);munmap(data,(size_t)st.st_size);return 0;}
    for (int shared=0;shared<2;shared++) {
        const char *tag=shared?"shared":"routed";coax_t_DeExpert e=de_test_original((coax_v_u8){data,st.st_size},shared);coax_t_DePlan p=de_plan(e);assert(p.status==COAX_DsStatus_ok);
        float *x=allocate(p.width,sizeof(float)),*out=allocate(p.width,sizeof(float));assert(de_test_original_input((coax_v_f32){x,p.width})==p.width);
        coax_t_DeWork work={.floats={allocate(p.floats,sizeof(float)),p.floats},.wide={allocate(p.wide,sizeof(double)),p.wide}};
        float route=shared?1:.137f;begin=now();coax_t_DeResult r=psionic_de_expert(e,(coax_v_f32){x,p.width},route,10,(coax_v_f32){out,p.width},work);assert(r.status==COAX_DsStatus_ok);double elapsed=now()-begin;
        write_array(argv[2],tag,"input.f32",x,sizeof(float),p.width);write_array(argv[2],tag,"output.f32",out,sizeof(float),p.width);
        write_array(argv[2],tag,"gate.f32",work.floats.data+p.begin.e[0],sizeof(float),p.hidden);write_array(argv[2],tag,"up.f32",work.floats.data+p.begin.e[1],sizeof(float),p.hidden);write_array(argv[2],tag,"activation.f32",work.floats.data+p.begin.e[2],sizeof(float),p.hidden);write_array(argv[2],tag,"down-grid.f64",work.wide.data,sizeof(double),p.hidden);
        char name[128];assert(snprintf(name,sizeof(name),"%s.json",tag)>0);FILE *f=output(argv[2],name,"w");fprintf(f,"{\"mode\":\"%s\",\"width\":%" PRId64 ",\"hidden\":%" PRId64 ",\"route\":%.9g,\"gate_clamped\":%" PRId64 ",\"up_clamped\":%" PRId64 ",\"seconds\":%.9f,\"admission_seconds\":%.9f}\n",tag,p.width,p.hidden,(double)route,r.gate_clamped,r.up_clamped,elapsed,admission);fclose(f);
        fprintf(stderr,"%s original %" PRId64 "x%" PRId64 " complete expert in %.6fs\n",tag,p.width,p.hidden,elapsed);free(x);free(out);free(work.floats.data);free(work.wide.data);
    }
    munmap(data,(size_t)st.st_size);return 0;
}
