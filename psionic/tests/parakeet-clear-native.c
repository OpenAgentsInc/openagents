/* Firm reason: 2 (independent resident-memory ownership/effect oracle).
 * A synthetic admitted owner avoids loading or computing a model. Private
 * arenas are observed because public snapshot refusal alone cannot prove wipe. */
#define PSIONIC_PARAKEET_LIBRARY 1
#define PSIONIC_PARAKEET_CPU_ONLY 1
#include "../platform/parakeet-face.c"
#include <assert.h>
static void zero(const void *p,size_t n){const unsigned char*b=p;for(size_t i=0;i<n;i++)assert(b[i]==0);}
static void filled(const void *p,size_t n,unsigned char v){const unsigned char*b=p;for(size_t i=0;i<n;i++)assert(b[i]==v);}
static void *arena(size_t n,unsigned char v){void*p=malloc(n);assert(p);memset(p,v,n);return p;}
int main(void){
 assert(!psionic_parakeet_clear(NULL));
 psionic_parakeet other={0};assert(!psionic_parakeet_clear(&other));
 psionic_parakeet*s=calloc(1,sizeof*s);assert(s);s->n32=37;s->n64=29;s->ne=17;s->nd=13;
 s->ew=arena((size_t)s->ne*4,0x21);s->dw=arena((size_t)s->nd*4,0x43);
 s->bytes=arena(BYTES,0x65);s->pieces=arena(VOCAB*sizeof*s->pieces,0x87);
 s->w32=arena((size_t)s->n32*4,0xa5);s->w64=arena((size_t)s->n64*8,0xa5);s->wave=arena(MAX_SAMPLES*8,0xa5);
 s->features=arena(128*2048*4,0xa5);s->encoded=arena(256*1024*4,0xa5);
 s->last_features=arena(128*2048*4,0xa5);s->last_encoded=arena(256*1024*4,0xa5);
 memset(s->tokens,0xa5,sizeof s->tokens);memset(s->frames,0xa5,sizeof s->frames);
 memset(s->draft_tokens,0xa5,sizeof s->draft_tokens);memset(s->draft_frames,0xa5,sizeof s->draft_frames);
 memset(s->stats,0xa5,sizeof s->stats);memset(s->text,0xa5,sizeof s->text);memset(&s->info,0xa5,sizeof s->info);
 s->gpu=0;s->has_result=1;resident=s;char external[32]="caller text survives";
 assert(psionic_parakeet_clear(s));assert(!strcmp(external,"caller text survives"));
 zero(s->w32,(size_t)s->n32*4);zero(s->w64,(size_t)s->n64*8);zero(s->wave,MAX_SAMPLES*8);
 zero(s->features,128*2048*4);zero(s->encoded,256*1024*4);zero(s->last_features,128*2048*4);zero(s->last_encoded,256*1024*4);
 zero(s->tokens,sizeof s->tokens);zero(s->frames,sizeof s->frames);zero(s->draft_tokens,sizeof s->draft_tokens);zero(s->draft_frames,sizeof s->draft_frames);
 zero(s->stats,sizeof s->stats);zero(s->text,sizeof s->text);zero(&s->info,sizeof s->info);assert(s->has_result==0);
 filled(s->ew,(size_t)s->ne*4,0x21);filled(s->dw,(size_t)s->nd*4,0x43);filled(s->bytes,BYTES,0x65);filled(s->pieces,VOCAB*sizeof*s->pieces,0x87);
 assert(s->ne==17&&s->nd==13&&s->n32==37&&s->n64==29&&s->gpu==0&&resident==s);
 psionic_parakeet_info info;unsigned char saved[sizeof info];memset(&info,0x17,sizeof info);memcpy(saved,&info,sizeof info);
 assert(!psionic_parakeet_last_info(s,&info)&&!memcmp(&info,saved,sizeof info));
 int64_t token= -71,frame= -72;assert(psionic_parakeet_tokens(s,&token,&frame,1)==-1&&token== -71&&frame== -72);
 float stage= -73;assert(psionic_parakeet_stage(s,0,&stage,1)==-1&&stage== -73);
 assert(psionic_parakeet_stage(s,1,&stage,1)==-1&&stage== -73);
 assert(!psionic_parakeet_json(s,external,sizeof external)&&!strcmp(external,"caller text survives"));
 assert(psionic_parakeet_clear(s));psionic_parakeet_close(s);assert(!psionic_parakeet_clear(NULL));
 puts("Parakeet privacy clear: every owned CPU arena/result zero, immutable model retained, snapshots refused, caller bytes intact");return 0;
}
