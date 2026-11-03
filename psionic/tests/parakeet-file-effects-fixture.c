/* Independent native-effect oracle. No model is loaded or speech recognized. */
#define _POSIX_C_SOURCE 200809L
#include "../platform/psionic-parakeet.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct psionic_parakeet {int active;};
static struct psionic_parakeet fixture;
static int calls,gpu,silent;
static int64_t samples;
static int setting(const char *name){const char *s=getenv(name);return s?atoi(s):0;}
psionic_parakeet *psionic_parakeet_open(const char *dir,int device,char *err,size_t cap){(void)dir;(void)err;(void)cap;fixture.active=1;gpu=device;return &fixture;}
int psionic_parakeet_text(psionic_parakeet *s,const float *in,int64_t n,char *out,size_t cap,char *err,size_t errcap){
  if(s!=&fixture||!s->active||!in||n<320||n>327520)return 0;
  calls++;samples=n;silent=setting("PKFILE_FIXTURE_SILENT");
  if(setting("PKFILE_FIXTURE_FAIL_AT")==calls){snprintf(err,errcap,"injected recognition failure");return 0;}
  if(setting("PKFILE_FIXTURE_CANCEL_AT")==calls)raise(SIGINT);
  if(setting("PKFILE_FIXTURE_LARGE")){if(cap<8193)return 0;memset(out,'x',8192);out[8192]=0;}
  else {const char *text=silent?"":"fixture";if(strlen(text)>=cap)return 0;strcpy(out,text);}
  return 1;
}
int psionic_parakeet_last_info(psionic_parakeet *s,psionic_parakeet_info *out){if(s!=&fixture||!out)return 0;*out=(psionic_parakeet_info){.samples=samples,.feature_frames=samples/160,.encoded_frames=(samples/160+7)/8,.tokens=silent?0:1,.text_bytes=silent?0:7,.gpu=gpu};return 1;}
int64_t psionic_parakeet_tokens(psionic_parakeet *s,int64_t *tokens,int64_t *frames,size_t cap){if(s!=&fixture||!tokens||!frames||cap<1)return -1;if(silent)return 0;tokens[0]=7;frames[0]=0;return 1;}
int psionic_parakeet_clear(psionic_parakeet *s){return s==&fixture&&s->active;}
int psionic_parakeet_json(psionic_parakeet *s,char *out,size_t cap){(void)s;(void)out;(void)cap;return 0;}
int64_t psionic_parakeet_stage(psionic_parakeet *s,int stage,float *out,size_t cap){(void)s;(void)stage;(void)out;(void)cap;return -1;}
void psionic_parakeet_close(psionic_parakeet *s){if(s==&fixture)s->active=0;}
int pkfile_main(int argc,char **argv);
int main(int argc,char **argv){return pkfile_main(argc,argv);}
