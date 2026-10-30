#include "byte-lm-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "byte-lm-goldens.h"
static unsigned char *read_file(const char *path,size_t *n){FILE *f=fopen(path,"rb");assert(f && !fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0 && !fseek(f,0,SEEK_SET));unsigned char *p=malloc((size_t)size);assert(p && fread(p,1,(size_t)size,f)==(size_t)size && !fclose(f));*n=(size_t)size;return p;}
int main(int argc,char **argv){assert(argc==5);size_t nt,nv,ns,ne;unsigned char *train=read_file(argv[1],&nt),*validation=read_file(argv[2],&nv),*test=read_file(argv[3],&ns),*expected=read_file(argv[4],&ne);assert(ne==65536*8);
 double *w=malloc(ne),*stage=malloc(ne),trace[66];assert(w && stage);memset(stage,0xa5,ne);trace[65]=-7;
 coax_t_BgFit fit=psionic_bg_fit((coax_v_u8){train,(int64_t)nt},64,32,(coax_v_f64){w,65536},(coax_v_f64){stage,65536},(coax_v_f64){trace,66});assert(!fit.status && fit.updates==64 && fit.transitions==(int64_t)nt-1 && trace[65]==-7);
 double error=0;for(size_t i=0;i<65536;i++){double value;memcpy(&value,expected+i*8,8);double delta=fabs(w[i]-value);if(delta>error)error=delta;assert(delta<1e-12);}for(int i=0;i<65;i++)assert(fabs(trace[i]-bg_trace[i])<1e-11);
 coax_t_BgMetric a=bg_evaluate((coax_v_f64){w,65536},(coax_v_u8){train,(int64_t)nt}),b=bg_evaluate((coax_v_f64){w,65536},(coax_v_u8){validation,(int64_t)nv}),c=bg_evaluate((coax_v_f64){w,65536},(coax_v_u8){test,(int64_t)ns});assert(!a.status && !b.status && !c.status && fabs(a.nats-bg_train_loss)<1e-11 && fabs(b.nats-bg_validation_loss)<1e-11 && fabs(c.nats-bg_test_loss)<1e-11);
 unsigned char generated[129];memset(generated,77,sizeof generated);coax_t_BgGeneration g=psionic_bg_generate((coax_v_f64){w,65536},(coax_t_CrSeed){7,11},32,128,(coax_v_u8){generated,129});assert(!g.status && g.written==128 && g.next.seed==7 && g.next.counter==139 && !memcmp(generated,bg_generated,128) && generated[128]==77);
 assert(psionic_bg_fit((coax_v_u8){train,(int64_t)nt},64,32,(coax_v_f64){w,65536},(coax_v_f64){w,65536},(coax_v_f64){trace,66}).status==COAX_BgStatus_alias);
 assert(psionic_bg_generate((coax_v_f64){w,65536},(coax_t_CrSeed){0,0},0,1,(coax_v_u8){(unsigned char *)w,1}).status==COAX_BgStatus_alias);
 double saved=w[65535];uint64_t bits=UINT64_C(0x7ff8000000000000);memcpy(w+65535,&bits,8);assert(bg_evaluate((coax_v_f64){w,65536},(coax_v_u8){test,(int64_t)ns}).status==COAX_BgStatus_number);assert(psionic_bg_generate((coax_v_f64){w,65536},(coax_t_CrSeed){0,0},0,1,(coax_v_u8){generated,129}).status==COAX_BgStatus_number && !memcmp(generated,bg_generated,128));w[65535]=saved;
 printf("byte LM: all 65536 weights, 65 losses, held-out metrics and 128 sampled bytes agree; max parameter error %.3g\n",error);
 free(train);free(validation);free(test);free(expected);free(w);free(stage);return 0;}
