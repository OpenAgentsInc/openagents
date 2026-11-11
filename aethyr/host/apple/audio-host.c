// Firm reason: 6 (Apple audio host: scalar C wrappers over the shared Coax audio authorities).
// The wrappers audio-host.h names, included once into a translation unit that already has the
// generated Coax types of aethyr/host/native-audio.coax (MetalRealm.m's units, or the Everglade
// app's native-audio.h). Every decision is the Coax's; these only form views and copy scalars.
#include <stdlib.h>
typedef struct {coax_t_HostAudioBank state;uintptr_t base;size_t capacity;} MFAudioBank;
void *mf_audio_bank_open(const uint8_t *base,size_t capacity){MFAudioBank *p=calloc(1,sizeof*p);if(p){hab_reset((coax_v_t_HostAudioBank){&p->state,1});p->base=(uintptr_t)base;p->capacity=capacity;}return p;}
bool mf_audio_bank_register(void *bank,const uint8_t *line,size_t count,MFAudioBankAsset *out){MFAudioBank *p=bank;*out=(MFAudioBankAsset){0};if(!p||count>INT64_MAX)return false;coax_v_t_HostAudioBank s={&p->state,1};int64_t id=hab_insert(s,(coax_string){(const char*)line,(int64_t)count},p->base,p->capacity);if(id<0)return false;coax_t_HostAudioBankSpan q=hab_source(s,id);if(!q.valid)return false;*out=(MFAudioBankAsset){(const uint8_t*)q.name.data,(size_t)q.name.count,(const uint8_t*)(uintptr_t)q.address,(size_t)q.bytes};return true;}
int64_t mf_audio_bank_count(void *bank){MFAudioBank *p=bank;return p?p->state.count:0;}
void mf_audio_bank_close(void *bank){free(bank);}
int64_t mf_audio_extent(int64_t frames,int64_t channels){return ham_extent(frames,channels);}
void *mf_audio_memory_open(void){return calloc(1,sizeof(coax_t_HostAudioMemory));}
int64_t mf_audio_memory_reserve(void *memory,int64_t words){return memory?ham_reserve((coax_v_t_HostAudioMemory){memory,1},words):-1;}
int64_t mf_audio_memory_words(void *memory,int64_t ticket){return memory?ham_words((coax_v_t_HostAudioMemory){memory,1},ticket):-1;}
bool mf_audio_memory_release(void *memory,int64_t ticket){return memory&&ham_release((coax_v_t_HostAudioMemory){memory,1},ticket);}
int64_t mf_audio_memory_total(void *memory){return memory?((coax_t_HostAudioMemory*)memory)->total:0;}
void mf_audio_memory_close(void *memory){free(memory);}
bool mf_audio_decode(const uint8_t *bytes,size_t n,MFAudioEffect *out){coax_t_AudioHostEffect e=audio_host_parse((coax_string){(const char*)bytes,(int64_t)n});*out=(MFAudioEffect){e.kind,(const uint8_t*)e.name.data,(size_t)e.name.count,(const uint8_t*)e.id.data,(size_t)e.id.count,e.gain,e.pan,e.delay,e.lowpass,e.rate,e.wet};return e.kind!=0;}
bool mf_audio_asset_name(const uint8_t *bytes,size_t n){return apv_asset_name((coax_string){(const char*)bytes,(int64_t)n});}
void *mf_audio_asset_open(const uint8_t *name,size_t n,MFAudioAsset *out){
 *out=(MFAudioAsset){0};if(n>INT64_MAX)return NULL;coax_t_HostAudioAsset q=hpc_everglade((coax_string){(const char*)name,(int64_t)n});if(!q.valid)return NULL;
 *out=(MFAudioAsset){q.valid,q.canonical,(const uint8_t*)q.source_url.data,(size_t)q.source_url.count,(const uint8_t*)q.companion_url.data,(size_t)q.companion_url.count,q.source_bytes,q.companion_bytes,q.frames,q.channels};
 coax_t_HostAudioAsset *p=malloc(sizeof*p);if(p)*p=q;return p;
}
void mf_audio_asset_close(void *asset){free(asset);}
int64_t mf_audio_encoded_words(int64_t bytes,bool companion){return hae_words(bytes,companion);}
static MFOpusPlan opusOffer(coax_t_HostOpusPlan q){return(MFOpusPlan){q.valid,q.channels,q.pre_skip,q.frames,q.decode_frames,q.samples,q.decode_samples};}
MFOpusPlan mf_audio_opus_plan(void *asset,const uint8_t *source,size_t n){if(n>INT64_MAX)return(MFOpusPlan){0};coax_v_u8 bytes={(uint8_t*)source,(int64_t)n};return opusOffer(asset?hpc_source_plan(*(coax_t_HostAudioAsset*)asset,bytes):hpo_plan(bytes));}
bool mf_audio_opus_output(MFOpusPlan p,int64_t frames,int64_t channels,double rate){return hpo_output((coax_t_HostOpusPlan){p.valid,p.channels,p.pre_skip,p.frames,p.decode_frames,p.samples,p.decode_samples},frames,channels,rate);}
MFPcmAssetPlan mf_audio_companion_plan(void *asset,const uint8_t *source,size_t n,const uint8_t *companion,size_t count){if(!asset||n>INT64_MAX||count>INT64_MAX)return(MFPcmAssetPlan){0};coax_t_HostPcmAssetPlan q=hpc_plan(*(coax_t_HostAudioAsset*)asset,(coax_v_u8){(uint8_t*)source,(int64_t)n},(coax_v_u8){(uint8_t*)companion,(int64_t)count});return(MFPcmAssetPlan){q.valid,q.channels,q.frames,q.samples,q.offset};}
bool mf_audio_companion_decode(void *asset,const uint8_t *source,size_t n,const uint8_t *companion,size_t count,float *out,size_t words){return asset&&n<=INT64_MAX&&count<=INT64_MAX&&words<=INT64_MAX&&hpc_decode(*(coax_t_HostAudioAsset*)asset,(coax_v_u8){(uint8_t*)source,(int64_t)n},(coax_v_u8){(uint8_t*)companion,(int64_t)count},(coax_v_f32){out,(int64_t)words});}
typedef struct {coax_t_AudioPcmHost state;double *workspace;int64_t words;float block[2048];} MFAudioPCM;
void *mf_audio_session_open(void){coax_t_HostAudioSession *s=malloc(sizeof*s);if(s)*s=has_open();return s;}
static MFAudioSessionOffer audioOffer(coax_t_HostAudioSessionOffer q){return(MFAudioSessionOffer){q.active,q.started,q.retired};}
MFAudioSessionOffer mf_audio_session_offer(void *session){if(!session)return(MFAudioSessionOffer){0};return audioOffer(has_offer((coax_v_t_HostAudioSession){session,1}));}
MFAudioSessionOffer mf_audio_session_event(void *session,int64_t kind){if(!session)return(MFAudioSessionOffer){0};return audioOffer(has_event((coax_v_t_HostAudioSession){session,1},kind));}
void mf_audio_session_close(void *session){free(session);}
void *mf_pcm_open(void){MFAudioPCM *p=calloc(1,sizeof*p);if(!p)return NULL;coax_default_t_AudioPcmHost(&p->state);p->words=ac_words(1024,128);p->workspace=calloc((size_t)p->words,sizeof(double));if(!p->workspace){free(p);return NULL;}return p;}
int64_t mf_pcm_command(void *pcm,const uint8_t *line,size_t n,const float *samples,size_t count,int64_t channels,bool started){MFAudioPCM *p=pcm;if(!p)return -1;return ap_context_command((coax_v_t_AudioPcmHost){&p->state,1},(coax_v_f64){p->workspace,p->words},(coax_string){(const char*)line,(int64_t)n},(coax_v_f32){(float*)samples,(int64_t)count},channels,started);}
const float *mf_pcm_block(void *pcm,size_t *samples){MFAudioPCM *p=pcm;*samples=0;if(!p||!ap_block((coax_v_t_AudioPcmHost){&p->state,1},(coax_v_f64){p->workspace,p->words},(coax_v_f32){p->block,2048}))return NULL;*samples=2048;return p->block;}
int64_t mf_pcm_info(void *pcm,int32_t field){MFAudioPCM *p=pcm;if(!p)return 0;switch(field){case 0:return p->state.frames;case 1:return p->state.accepted;case 2:return p->state.refused;case 3:return p->state.room.e[0].parts;case 4:{int64_t n=0;for(int i=0;i<128;i++)if(p->state.voices.e[i].active)n++;return n;}default:return 0;}}
void mf_pcm_close(void *pcm){MFAudioPCM *p=pcm;if(p){free(p->workspace);free(p);}}
typedef struct {coax_t_AudioWavFilter state;double workspace[20480];} MFAudioWav;
void *mf_wav_open(void){MFAudioWav *p=calloc(1,sizeof*p);if(p&&!apw_coefficients((coax_v_t_AudioWavFilter){&p->state,1},(coax_v_f64){p->workspace,20480})){free(p);return NULL;}return p;}
int64_t mf_wav_info(const uint8_t *bytes,size_t count,int32_t field){coax_t_AudioWavPlan p=apw_plan((coax_v_u8){(uint8_t*)bytes,(int64_t)count});if(!p.valid)return -1;return field==0?p.samples:field==1?p.channels:field==2?p.output_frames:p.rate;}
bool mf_wav_decode(void *decoder,const uint8_t *bytes,size_t count,float *samples,size_t sample_count){MFAudioWav *p=decoder;return p&&apw_decode((coax_v_t_AudioWavFilter){&p->state,1},(coax_v_u8){(uint8_t*)bytes,(int64_t)count},(coax_v_f64){p->workspace,20480},(coax_v_f32){samples,(int64_t)sample_count});}
void mf_wav_close(void *decoder){free(decoder);}
