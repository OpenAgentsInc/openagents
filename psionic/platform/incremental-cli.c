/* String/PCM file/clock and native model-call effects for the Coax example. */
#define _POSIX_C_SOURCE 200809L
#include "incremental-cli.h"
#include "psionic-incremental.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static psionic_incremental *ptr(int64_t h){return (psionic_incremental*)(intptr_t)h;}
static int path(coax_v_u8 s,char out[4096]){if(s.count<1||s.count>=4096||memchr(s.data,0,(size_t)s.count))return 0;memcpy(out,s.data,(size_t)s.count);out[s.count]=0;return 1;}
int64_t iasi_open(coax_v_u8 directory){char p[4096],error[512]={0};if(!path(directory,p))return 0;psionic_incremental *h=psionic_incremental_open(p,error,sizeof error);if(!h)fprintf(stderr,"%s\n",error);return (int64_t)(intptr_t)h;}
void iasi_close(int64_t h){psionic_incremental_close(ptr(h));}
_Bool iasi_reset(int64_t h,uint64_t epoch){return psionic_incremental_reset(ptr(h),epoch)!=0;}
int64_t iasi_push(int64_t h,uint64_t epoch,uint64_t sequence,coax_v_f32 samples,_Bool final){return psionic_incremental_push(ptr(h),epoch,sequence,samples.data,samples.count,final);}
_Bool iasi_cancel(int64_t h,uint64_t epoch){return psionic_incremental_cancel(ptr(h),epoch)!=0;}
int64_t iasi_text(int64_t h,uint64_t epoch,coax_v_u8 out){return psionic_incremental_text(ptr(h),epoch,out.data,out.count);}
_Bool iasi_info(int64_t h,coax_v_s64 out){return out.count>=6&&psionic_incremental_info(ptr(h),out.data);}
int64_t iasi_pcm(coax_v_u8 name,coax_v_s16 out){char p[4096];if(!path(name,p))return -1;FILE *f=fopen(p,"rb");if(!f)return -1;if(fseek(f,0,SEEK_END)){fclose(f);return -1;}long bytes=ftell(f);if(bytes<0||bytes%2||bytes/2>out.count){fclose(f);return -1;}rewind(f);size_t n=fread(out.data,2,(size_t)(bytes/2),f);fclose(f);return n==(size_t)(bytes/2)?bytes/2:-1;}
int64_t iasi_now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
int main(int argc,char **argv){coax_string *args=calloc((size_t)argc,sizeof *args);if(!args)return 3;for(int i=1;i<argc;i++)args[i-1]=(coax_string){argv[i],(int64_t)strlen(argv[i])};int result=(int)iasi_main((coax_v_string){args,argc-1});free(args);return result;}
