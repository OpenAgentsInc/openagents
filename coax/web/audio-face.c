// Firm reason: 1 (browser glue: measured Wasm allocations and scalar projections of shared Coax audio).
#include "audio-carrier.h"
#include <stdlib.h>
#include <string.h>
#define AUDIO_EXPORT(name) __attribute__((export_name(name)))
static coax_t_AudioPcmHost audio_state;
static coax_t_HostAudioMemory audio_memory;
static double *audio_workspace;
static size_t audio_workspace_words;
static float *audio_assets[128],audio_output[2048];
static uint8_t audio_line[1024];
static coax_t_AudioHostEffect audio_effect;
static coax_t_AudioWavFilter audio_filter;
static double audio_filter_words[20480];
static coax_t_AudioWavPlan audio_wav;
static coax_v_t_AudioPcmHost audio_state_view(void){return (coax_v_t_AudioPcmHost){&audio_state,1};}
static coax_v_t_HostAudioMemory audio_memory_view(void){return (coax_v_t_HostAudioMemory){&audio_memory,1};}
static coax_v_f64 audio_workspace_view(void){return (coax_v_f64){audio_workspace,(int64_t)audio_workspace_words};}
AUDIO_EXPORT("audio_init") uint32_t audio_init(void){
  if(audio_workspace)return 1;
  coax_default_t_AudioPcmHost(&audio_state);
  audio_workspace_words=(size_t)ac_words(1024,128);audio_workspace=calloc(audio_workspace_words,sizeof(double));return audio_workspace!=NULL;
}
AUDIO_EXPORT("audio_line_ptr") uint32_t audio_line_ptr(void){return (uint32_t)(uintptr_t)audio_line;}
AUDIO_EXPORT("audio_line_capacity") uint32_t audio_line_capacity(void){return sizeof audio_line;}
AUDIO_EXPORT("audio_parse") int32_t audio_parse(uint32_t n){if(n>sizeof audio_line)return 0;audio_effect=audio_host_parse((coax_string){(const char*)audio_line,n});return (int32_t)audio_effect.kind;}
AUDIO_EXPORT("audio_name_ptr") uint32_t audio_name_ptr(void){return (uint32_t)(uintptr_t)audio_effect.name.data;}
AUDIO_EXPORT("audio_name_bytes") uint32_t audio_name_bytes(void){return (uint32_t)audio_effect.name.count;}
AUDIO_EXPORT("audio_alloc") int32_t audio_alloc(int64_t words){
  int64_t id=ham_reserve(audio_memory_view(),words);if(id<0)return -1;
  audio_assets[id]=calloc((size_t)ham_words(audio_memory_view(),id),sizeof(float));
  if(!audio_assets[id]){ham_release(audio_memory_view(),id);return -1;}return (int32_t)id;
}
AUDIO_EXPORT("audio_asset_ptr") uint32_t audio_asset_ptr(int32_t id){return ham_words(audio_memory_view(),id)>0?(uint32_t)(uintptr_t)audio_assets[id]:0;}
AUDIO_EXPORT("audio_release") uint32_t audio_release(int32_t id){if(!ham_release(audio_memory_view(),id))return 0;free(audio_assets[id]);audio_assets[id]=NULL;return 1;}
AUDIO_EXPORT("audio_decode_wav") int32_t audio_decode_wav(int32_t id,uint32_t bytes){
  int64_t words=ham_words(audio_memory_view(),id);if(words<1 || bytes>(uint64_t)words*sizeof(float))return -1;
  coax_v_u8 source={(uint8_t*)audio_assets[id],bytes};audio_wav=apw_plan(source);if(!audio_wav.valid)return -1;
  int32_t out=audio_alloc(audio_wav.samples);if(out<0)return -1;
  coax_v_t_AudioWavFilter filter={&audio_filter,1};coax_v_f64 work={audio_filter_words,20480};
  if((!audio_filter.ready&&!apw_coefficients(filter,work)) || !apw_decode(filter,source,work,(coax_v_f32){audio_assets[out],audio_wav.samples})){audio_release(out);return -1;}return out;
}
AUDIO_EXPORT("audio_wav_channels") uint32_t audio_wav_channels(void){return (uint32_t)audio_wav.channels;}
AUDIO_EXPORT("audio_command") int32_t audio_command(uint32_t n,int32_t asset,int32_t channels,uint32_t started){
  if(!audio_workspace || n>sizeof audio_line)return -1;int64_t words=ham_words(audio_memory_view(),asset);
  coax_v_f32 samples=words>0?(coax_v_f32){audio_assets[asset],words}:(coax_v_f32){NULL,0};
  return (int32_t)ap_context_command(audio_state_view(),audio_workspace_view(),(coax_string){(const char*)audio_line,n},samples,channels,started!=0);
}
AUDIO_EXPORT("audio_block") uint32_t audio_block(void){return audio_workspace&&ap_block(audio_state_view(),audio_workspace_view(),(coax_v_f32){audio_output,2048});}
AUDIO_EXPORT("audio_output_ptr") uint32_t audio_output_ptr(void){return (uint32_t)(uintptr_t)audio_output;}
AUDIO_EXPORT("audio_reset") uint32_t audio_reset(void){if(!audio_workspace)return 0;coax_default_t_AudioPcmHost(&audio_state);memset(audio_workspace,0,audio_workspace_words*sizeof(double));return 1;}
AUDIO_EXPORT("audio_info") int64_t audio_info(int32_t field){
  switch(field){case 0:return audio_state.frames;case 1:return audio_state.accepted;case 2:return audio_state.refused;case 3:return audio_state.room.e[0].parts;case 4:{int64_t n=0;for(int i=0;i<128;i++)n+=audio_state.voices.e[i].active;return n;}case 5:return audio_memory.total;case 6:return audio_wav.samples;default:return -1;}
}
