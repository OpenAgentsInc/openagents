// Firm reason: 1 (native SDK: owned allocations, serialized Coax calls and PCM copies).
#include "audio-host.h"
#include "NativeAudio.h"
#include <limits.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct RhAudioMemory {pthread_mutex_t mutex;coax_t_HostAudioMemory state;size_t refs;};
struct RhAudioSamples {RhAudioMemory *owner;float *data;int64_t ticket,count,channels;size_t refs;};
static coax_v_t_HostAudioMemory allocation_state(RhAudioMemory *m){return(coax_v_t_HostAudioMemory){&m->state,1};}
RhAudioMemory *rham_open(void){RhAudioMemory *m=calloc(1,sizeof*m);if(!m)return NULL;if(pthread_mutex_init(&m->mutex,NULL)){free(m);return NULL;}m->refs=1;return m;}
void rham_retain(RhAudioMemory *m){if(!m)return;pthread_mutex_lock(&m->mutex);m->refs++;pthread_mutex_unlock(&m->mutex);}
void rham_close(RhAudioMemory *m){if(!m)return;pthread_mutex_lock(&m->mutex);bool last=--m->refs==0;pthread_mutex_unlock(&m->mutex);if(last){pthread_mutex_destroy(&m->mutex);free(m);}}
int64_t rham_total(RhAudioMemory *m){if(!m)return -1;pthread_mutex_lock(&m->mutex);int64_t n=m->state.total;pthread_mutex_unlock(&m->mutex);return n;}
RhAudioSamples *rhas_open(RhAudioMemory *m,int64_t frames,int64_t channels){
 int64_t words=ham_extent(frames,channels);if(!m||!words)return NULL;
 pthread_mutex_lock(&m->mutex);int64_t ticket=ham_reserve(allocation_state(m),words);RhAudioSamples *s=NULL;
 if(ticket>=0){s=calloc(1,sizeof*s);float *data=calloc((size_t)ham_words(allocation_state(m),ticket),sizeof(float));if(!s||!data){free(s);free(data);ham_release(allocation_state(m),ticket);s=NULL;}else{*s=(RhAudioSamples){m,data,ticket,words,channels,1};m->refs++;}}
 pthread_mutex_unlock(&m->mutex);return s;
}
RhAudioSamples *rhas_copy(RhAudioMemory *m,const float *source,int64_t frames,int64_t channels){RhAudioSamples *s=rhas_open(m,frames,channels);if(s)memcpy(s->data,source,(size_t)s->count*sizeof(float));return s;}
void rhas_retain(RhAudioSamples *s){if(!s)return;pthread_mutex_lock(&s->owner->mutex);s->refs++;pthread_mutex_unlock(&s->owner->mutex);}
void rhas_close(RhAudioSamples *s){if(!s)return;RhAudioMemory *m=s->owner;pthread_mutex_lock(&m->mutex);bool last=--s->refs==0;if(last){free(s->data);ham_release(allocation_state(m),s->ticket);free(s);}pthread_mutex_unlock(&m->mutex);if(last)rham_close(m);}
float *rhas_data(RhAudioSamples *s){return s?s->data:NULL;}
int64_t rhas_count(RhAudioSamples *s){return s?s->count:0;}
int64_t rhas_channels(RhAudioSamples *s){return s?s->channels:0;}
int64_t rhas_ticket(RhAudioSamples *s){return s?s->ticket:-1;}
struct RhAudioBank {coax_t_HostAudioBank state;};
RhAudioBank *rhab_open(void){RhAudioBank *b=calloc(1,sizeof*b);if(b)hab_reset((coax_v_t_HostAudioBank){&b->state,1});return b;}
void rhab_close(RhAudioBank *b){free(b);}
int64_t rhab_count(RhAudioBank *b){return b?b->state.count:0;}
int64_t rhab_insert(RhAudioBank *b,const uint8_t *line,size_t line_bytes,const uint8_t *storage,size_t storage_bytes,RhAudioBankAsset *out){
 *out=(RhAudioBankAsset){0};if(!b||line_bytes>INT64_MAX)return -1;coax_v_t_HostAudioBank state={&b->state,1};
 int64_t id=hab_insert(state,(coax_string){(const char*)line,(int64_t)line_bytes},(uint64_t)(uintptr_t)storage,storage_bytes);
 if(id>=0){coax_t_HostAudioBankSpan p=hab_source(state,id);*out=(RhAudioBankAsset){(const uint8_t*)p.name.data,(const uint8_t*)(uintptr_t)p.address,(size_t)p.name.count,(size_t)p.bytes};}return id;
}
typedef struct {uint8_t *wav;int64_t ticket,channels;bool ready;} RhAudioAsset;
struct RhAudio {
 pthread_mutex_t mutex;
 coax_t_AudioPcmHost state;
 RhAudioBank *bank;
 RhAudioMemory *memory;
 coax_t_AudioWavFilter filter;
 double *workspace,filter_workspace[20480];int64_t workspace_words;
 RhAudioAsset assets[128];RhAudioSamples *pcm[128];float block[2048];size_t cursor;bool invalid;
};
static coax_v_t_AudioPcmHost pcm_state(RhAudio *a){return(coax_v_t_AudioPcmHost){&a->state,1};}
static coax_v_t_HostAudioBank bank_state(RhAudio *a){return(coax_v_t_HostAudioBank){&a->bank->state,1};}
static coax_v_f64 work(RhAudio *a){return(coax_v_f64){a->workspace,a->workspace_words};}
RhAudio *rha_open(void){
 RhAudio *a=calloc(1,sizeof*a);if(!a)return NULL;
 if(pthread_mutex_init(&a->mutex,NULL)){free(a);return NULL;}
 coax_default_t_AudioPcmHost(&a->state);a->bank=rhab_open();
 a->workspace_words=ac_words(1024,128);a->workspace=calloc((size_t)a->workspace_words,sizeof(double));a->cursor=1024;
 a->memory=rham_open();
 if(!a->bank||!a->memory||!a->workspace||!apw_coefficients((coax_v_t_AudioWavFilter){&a->filter,1},(coax_v_f64){a->filter_workspace,20480})){rha_close(a);return NULL;}return a;
}
static int64_t add_wav(RhAudio *a,const uint8_t *name,size_t name_bytes,const uint8_t *wav,size_t wav_bytes){
 if(a->invalid||name_bytes>INT64_MAX||wav_bytes>INT64_MAX)return -1;
 coax_string key={(const char*)name,(int64_t)name_bytes};
 if(!apv_asset_name(key)||hab_find(bank_state(a),key)>=0)return -1;
 coax_t_AudioWavPlan p=apw_plan((coax_v_u8){(uint8_t*)wav,(int64_t)wav_bytes});if(!p.valid)return -1;
 RhAudioSamples *samples=rhas_open(a->memory,p.output_frames,p.channels);if(!samples)return -1;int64_t ticket=rhas_ticket(samples);
 uint8_t *source=malloc(wav_bytes);
 if(!source){rhas_close(samples);return -1;}
 memcpy(source,wav,wav_bytes);
 char line[128];int n=snprintf(line,sizeof line,"%.*s %llu %zu",(int)name_bytes,name,(unsigned long long)(uintptr_t)source,wav_bytes);
 RhAudioBankAsset admitted;int64_t id=rhab_insert(a->bank,(const uint8_t*)line,(size_t)n,source,wav_bytes,&admitted);
 if(id<0){free(source);rhas_close(samples);return -1;}
 a->pcm[ticket]=samples;RhAudioAsset *asset=&a->assets[id];*asset=(RhAudioAsset){source,ticket,p.channels,false};
 asset->ready=apw_decode((coax_v_t_AudioWavFilter){&a->filter,1},(coax_v_u8){source,(int64_t)wav_bytes},(coax_v_f64){a->filter_workspace,20480},(coax_v_f32){rhas_data(samples),rhas_count(samples)});
 return asset->ready?id:-1;
}
int64_t rha_add_wav(RhAudio *a,const uint8_t *name,size_t name_bytes,const uint8_t *wav,size_t wav_bytes){
 if(!a)return -1;
 pthread_mutex_lock(&a->mutex);int64_t id=add_wav(a,name,name_bytes,wav,wav_bytes);pthread_mutex_unlock(&a->mutex);return id;
}
int64_t rha_bank(RhAudio *a,const uint8_t *listing,size_t listing_bytes,const uint8_t *storage,size_t storage_bytes){
 if(!a||listing_bytes>INT64_MAX)return -1;
 pthread_mutex_lock(&a->mutex);int64_t added=0;size_t start=0;
 for(size_t i=0;i<=listing_bytes;i++){if(i==listing_bytes||listing[i]=='\n'){
  if(i>start){coax_t_HostAudioBankSpan p=hab_span((coax_string){(const char*)listing+start,(int64_t)(i-start)},(uint64_t)(uintptr_t)storage,storage_bytes);
   if(!p.valid||add_wav(a,(const uint8_t*)p.name.data,(size_t)p.name.count,(const uint8_t*)(uintptr_t)p.address,(size_t)p.bytes)<0){added=-1;break;}added++;}start=i+1;
 }}pthread_mutex_unlock(&a->mutex);return added;
}
int64_t rha_command(RhAudio *a,const uint8_t *line,size_t bytes,bool started){
 if(!a||bytes>INT64_MAX)return -1;
 pthread_mutex_lock(&a->mutex);int64_t result=-1;
 if(!a->invalid){coax_string text={(const char*)line,(int64_t)bytes};coax_t_AudioHostEffect effect=audio_host_parse(text);int64_t id=hab_find(bank_state(a),effect.name);coax_v_f32 samples={NULL,0};int64_t channels=0;
  if(id>=0&&a->assets[id].ready){RhAudioAsset *asset=&a->assets[id];RhAudioSamples *owned=a->pcm[asset->ticket];samples=(coax_v_f32){rhas_data(owned),rhas_count(owned)};channels=asset->channels;}
  result=ap_context_command(pcm_state(a),work(a),text,samples,channels,started);
 }pthread_mutex_unlock(&a->mutex);return result;
}
int64_t rha_pcm_load(RhAudio *a,const float *source,size_t samples,int64_t channels){
 if(!a||samples>INT64_MAX)return -1;
 // The Coax extent helper admits channels before this SDK unit conversion.
 if(!ham_extent(1,channels)||ham_extent((int64_t)samples/channels,channels)!=(int64_t)samples)return -1;
 pthread_mutex_lock(&a->mutex);RhAudioSamples *owned=rhas_copy(a->memory,source,(int64_t)samples/channels,channels);int64_t id=rhas_ticket(owned);if(id>=0)a->pcm[id]=owned;
 pthread_mutex_unlock(&a->mutex);return id;
}
int64_t rha_command_asset(RhAudio *a,const uint8_t *line,size_t bytes,int64_t ticket,int64_t channels,bool started){
 if(!a||bytes>INT64_MAX)return -1;
 pthread_mutex_lock(&a->mutex);RhAudioSamples *owned=ticket>=0&&ticket<128?a->pcm[ticket]:NULL;int64_t result=-1;
 if(!a->invalid)result=ap_context_command(pcm_state(a),work(a),(coax_string){(const char*)line,(int64_t)bytes},(coax_v_f32){rhas_data(owned),rhas_count(owned)},channels,started);
 pthread_mutex_unlock(&a->mutex);return result;
}
static bool next_block(RhAudio *a){if(a->invalid)return false;if(!ap_block(pcm_state(a),work(a),(coax_v_f32){a->block,2048})){a->invalid=true;return false;}return true;}
bool rha_block(RhAudio *a,float output[2048]){if(!a)return false;pthread_mutex_lock(&a->mutex);bool ok=next_block(a);if(ok)memcpy(output,a->block,sizeof a->block);pthread_mutex_unlock(&a->mutex);return ok;}
bool rha_render(RhAudio *a,float *output,size_t frames,bool muted){
 if(!a)return false;
 pthread_mutex_lock(&a->mutex);size_t at=0;bool ok=true;
 while(at<frames){if(a->cursor==1024){if(!next_block(a)){ok=false;break;}a->cursor=0;}size_t n=1024-a->cursor;if(n>frames-at)n=frames-at;
  if(muted)memset(output+2*at,0,2*n*sizeof(float));else memcpy(output+2*at,a->block+2*a->cursor,2*n*sizeof(float));a->cursor+=n;at+=n;
 }if(!ok)memset(output,0,2*frames*sizeof(float));pthread_mutex_unlock(&a->mutex);return ok;
}
int64_t rha_info(RhAudio *a,int field){if(!a)return -1;pthread_mutex_lock(&a->mutex);int64_t result=-1;switch(field){
 case 0:result=a->state.frames;break;case 1:result=a->state.accepted;break;case 2:result=a->state.refused;break;case 3:result=a->state.room.e[0].parts;break;case 4:result=rham_total(a->memory);break;case 5:result=rhab_count(a->bank);break;case 6:result=a->invalid;break;case 7:result=(int64_t)a->cursor;break;
 }pthread_mutex_unlock(&a->mutex);return result;}
const float *rha_asset(RhAudio *a,int id,size_t *samples,int64_t *channels){
 *samples=0;*channels=0;if(!a)return NULL;pthread_mutex_lock(&a->mutex);const float *p=NULL;
 coax_t_HostAudioBankSpan source=hab_source(bank_state(a),id);if(source.valid&&a->assets[id].ready){RhAudioAsset *asset=&a->assets[id];RhAudioSamples *owned=a->pcm[asset->ticket];*samples=(size_t)rhas_count(owned);*channels=asset->channels;p=rhas_data(owned);}
 pthread_mutex_unlock(&a->mutex);return p;
}
void rha_reset(RhAudio *a){if(!a)return;pthread_mutex_lock(&a->mutex);coax_default_t_AudioPcmHost(&a->state);memset(a->workspace,0,(size_t)a->workspace_words*sizeof(double));a->cursor=1024;a->invalid=false;pthread_mutex_unlock(&a->mutex);}
RhAudioSamples *rha_asset_keep(RhAudio *a,int id){if(!a)return NULL;pthread_mutex_lock(&a->mutex);RhAudioSamples *owned=NULL;coax_t_HostAudioBankSpan source=hab_source(bank_state(a),id);if(source.valid&&a->assets[id].ready){owned=a->pcm[a->assets[id].ticket];rhas_retain(owned);}pthread_mutex_unlock(&a->mutex);return owned;}
void rha_close(RhAudio *a){if(!a)return;for(size_t i=0;i<sizeof a->assets/sizeof a->assets[0];i++){free(a->assets[i].wav);rhas_close(a->pcm[i]);}rhab_close(a->bank);rham_close(a->memory);free(a->workspace);pthread_mutex_destroy(&a->mutex);free(a);}
