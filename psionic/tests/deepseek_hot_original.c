#define _POSIX_C_SOURCE 200809L
#include "deepseek-hot-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#define V(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define F(p,n) ((coax_v_f32){(float *)(p),(n)})
#define D(p,n) ((coax_v_f64){(double *)(p),(n)})
static void read_exact(const char *path,void *buffer,size_t bytes){FILE *f=fopen(path,"rb");assert(f&&fread(buffer,1,bytes,f)==bytes&&fgetc(f)==EOF);fclose(f);}
int main(int argc,char **argv){
    assert(argc==5);const int64_t stride=18800640;uint8_t *packet=malloc((size_t)stride),*pending=malloc((size_t)stride),*io=malloc((size_t)stride),*resident=malloc((size_t)stride);assert(packet&&pending&&io&&resident);read_exact(argv[1],packet,(size_t)stride);
    float input[5120],expected[5120],out[5121];read_exact(argv[2],input,sizeof input);read_exact(argv[3],expected,sizeof expected);float *floats=malloc(22000*sizeof(float));double *wide=malloc(5120*sizeof(double));assert(floats&&wide);coax_t_DeWork work={.floats=F(floats,22000),.wide=D(wide,5120)};
    coax_t_DcConfig c={.layers=1,.experts=1,.slots=1,.stride=stride,.budget=stride,.identity={"original-shard-3-expert-0",25}};
    int64_t profile[1]={1},heat[1],slots[1],hw[1],sw[1];coax_t_DcResult r=psionic_dc_seed(c,V(profile,1),V(heat,1),V(slots,1),V(hw,1),V(sw,1));assert(!r.status);coax_t_DcState state={.head=r.head,.heat=V(heat,1),.slots=V(slots,1)};
    int64_t proposed[1],draft[1];coax_t_DcMove moves[1],md[1];coax_t_DcPlan plan=psionic_dc_plan(c,state,V(proposed,1),(coax_v_t_DcMove){moves,1},V(draft,1),(coax_v_t_DcMove){md,1});assert(!plan.status&&plan.ticket.count==1&&moves[0].key==0);
    FILE *file=fopen(argv[1],"rb");assert(file);int64_t key[1]={0},offsets[1];uint8_t ready[1]={1};
    for(int mode=0;mode<3;mode++){
        uint8_t *bytes=packet;
        if(mode){r=psionic_dc_fetch(fileno(file),mode==2,1,stride,0,stride,V(key,1),B(pending,stride),B(io,stride),V(offsets,1));assert(!r.status&&r.written==stride&&!memcmp(pending,packet,(size_t)stride));bytes=pending;}
        if(mode==2){r=psionic_dc_install(c,state,plan.ticket,V(proposed,1),(coax_v_t_DcMove){moves,1},B(ready,1),B(bytes,stride),B(resident,stride));assert(!r.status);state.head=r.head;assert(!memcmp(resident,packet,(size_t)stride));bytes=resident;}
        out[5120]=77;coax_t_DeResult done=psionic_dc_expert(5120,2304,B(bytes,stride),F(input,5120),.137f,10,F(out,5121),work);assert(!done.status&&done.written==5120&&!memcmp(out,expected,sizeof expected)&&out[5120]==77);
        printf("Original expert: mode %d, all 5120 output words bit-identical to chapter 63's independent reference\n",mode);
    }
    FILE *result=fopen(argv[4],"wb");assert(result&&fwrite(out,1,sizeof expected,result)==sizeof expected);fclose(result);fclose(file);free(packet);free(pending);free(io);free(resident);free(floats);free(wide);return 0;
}
