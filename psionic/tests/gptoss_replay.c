/* Focused graph/stream parity on owned tiny weights. Private benchmark knobs
   never become serving configuration. No original CPU inference. */
#define _POSIX_C_SOURCE 200809L
#define PSIONIC_DEVICE_TEST 1
#include "gptoss-device.c"
#include "gptoss-tiny.h"
#include <assert.h>
#define OK(x) do{PcStatus z=(x);if(z){fprintf(stderr,"%s: %d %s\n",#x,z,pc_log());exit(2);}}while(0)
static char *read_file(const char *path,size_t *bytes){FILE *f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0);rewind(f);char *p=malloc((size_t)n);assert(p && fread(p,1,(size_t)n,f)==(size_t)n);fclose(f);*bytes=(size_t)n;return p;}
int main(int argc,char **argv){
    assert(argc==3);size_t ns,nw;char *source=read_file(argv[1],&ns),*weights=read_file(argv[2],&nw);
    coax_t_GFile file={{"tiny.safetensors",16},{(uint8_t*)weights,(int64_t)nw}};
    coax_t_GProfile profile={.shape=tiny_shape,.pins={tiny_artifacts,1},.specs={tiny_specs,33}};
    PgSession *s[2];coax_t_GResult why;
    for(int v=0;v<2;v++){OK(pg_open(profile,(coax_v_t_GFile){&file,1},source,ns,1073741824,&s[v],&why));assert(!why.status);}
    s[0]->replay_disabled=1;int slots[]={16,18,19,20,21,24};
    for(int round=0;round<3;round++){
        for(int v=0;v<2;v++)OK(pg_reset(s[v]));
        for(int step=0;step<8;step++){
            int64_t control[2][8];int choose=round==0?0:round==1?1:step%2;int wait=!(round==2 && step==2);
            for(int v=0;v<2;v++){OK(pg_step(s[v],1+step%4,step,choose,8,wait,control[v]));OK(pg_read(s[v],24,control[v],64));assert(!control[v][0]);}
            assert(!memcmp(control[0],control[1],64));
            for(size_t i=0;i<sizeof slots/sizeof *slots;i++){int slot=slots[i];size_t bytes=(size_t)(s[0]->plan.counts.e[slot]*s[0]->plan.width.e[slot]);void *a=malloc(bytes),*b=malloc(bytes);assert(a && b);OK(pg_read(s[0],slot,a,bytes));OK(pg_read(s[1],slot,b,bytes));assert(!memcmp(a,b,bytes));free(a);free(b);}
        }
        assert(s[1]->replay && !s[0]->replay);
    }
    puts("graph/stream: bitwise equal complete public state across reset, window wrap, choose topology changes and asynchronous interleave");
    for(int v=0;v<2;v++)pg_close(s[v]);free(source);free(weights);return 0;
}
