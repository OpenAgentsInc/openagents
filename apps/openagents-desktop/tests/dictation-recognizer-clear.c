/* Firm reason: 2 (independent C ABI effect witness for Swift defer order). */
#include "dictation-recognizer-clear.h"
#include <assert.h>
#include <string.h>
struct psionic_parakeet {int live;};static struct psionic_parakeet owner;
static unsigned char private_audio[37];static int fail_next,clears,closed;
void clear_oracle_fail(int fail){fail_next=fail;}
int clear_oracle_calls(void){return clears;}
int clear_oracle_closed(void){return closed;}
int clear_oracle_dirty(void){for(unsigned i=0;i<sizeof private_audio;i++)if(private_audio[i])return 1;return 0;}
psionic_parakeet *psionic_parakeet_open(const char*d,int gpu,char*err,size_t n){(void)d;(void)err;(void)n;assert(gpu==1&&!owner.live);owner.live=1;return &owner;}
int psionic_parakeet_text(psionic_parakeet*s,const float*x,int64_t n,char*out,size_t cap,char*err,size_t ec){(void)x;(void)n;(void)err;(void)ec;assert(s==&owner&&s->live);memset(private_audio,0x51,sizeof private_audio);if(fail_next)return 0;assert(cap>=10);strcpy(out,"kept text");return 1;}
int psionic_parakeet_clear(psionic_parakeet*s){assert(s==&owner&&s->live);memset(private_audio,0,sizeof private_audio);clears++;return 1;}
void psionic_parakeet_close(psionic_parakeet*s){assert(s==&owner&&s->live);s->live=0;closed++;}
