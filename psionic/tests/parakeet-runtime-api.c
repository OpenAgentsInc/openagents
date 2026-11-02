/* Firm reason: 2 (independent native C API lifecycle/refusal/atomicity oracle). */
#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#include "psionic-parakeet.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(void){
 char err[512],text[32]="unchanged";float sample=0;
 assert(psionic_parakeet_decoder_threads(NULL)==0);
 assert(!psionic_parakeet_text(NULL,&sample,1,text,sizeof text,err,sizeof err));assert(!strcmp(text,"unchanged"));
 assert(!psionic_parakeet_decode(NULL,&sample,1,text,sizeof text,err,sizeof err));assert(!strcmp(text,"unchanged"));
 assert(!psionic_parakeet_json(NULL,text,sizeof text)&&!strcmp(text,"unchanged"));
 psionic_parakeet_info info={0};info.samples= -7;assert(!psionic_parakeet_last_info(NULL,&info)&&info.samples== -7);
 float stage= -7;assert(psionic_parakeet_stage(NULL,0,&stage,1)== -1&&stage== -7);
 assert(!psionic_parakeet_open("/this/parakeet/directory/does/not/exist",0,err,sizeof err));
 char dir[]="/tmp/psionic-parakeet-api-XXXXXX";assert(mkdtemp(dir));char path[4096];
 snprintf(path,sizeof path,"%s/model.safetensors",dir);FILE*f=fopen(path,"wb");assert(f);assert(fwrite("bad model",1,9,f)==9);assert(fclose(f)==0);
 snprintf(path,sizeof path,"%s/tokenizer.json",dir);f=fopen(path,"wb");assert(f);assert(fwrite("{}",1,2,f)==2);assert(fclose(f)==0);
 for(int i=0;i<2;i++){assert(!psionic_parakeet_open(dir,0,err,sizeof err));assert(strstr(err,"status 1"));}
 psionic_parakeet_close(NULL);assert(unlink(path)==0);snprintf(path,sizeof path,"%s/model.safetensors",dir);assert(unlink(path)==0);assert(rmdir(dir)==0);
 puts("Parakeet native API: invalid handles, failed-open retry, wrong model, output atomicity and null close pass");return 0;
}
