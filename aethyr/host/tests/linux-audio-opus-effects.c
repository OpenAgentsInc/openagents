// Firm reason: 1 (SDK fixture: actual decoder, owned samples and measured copies).
#include "NativeAudio.h"
#include "audio-opus.h"
#include <string.h>
static RhAudioMemory *memory;
static RhAudioSamples *samples;
static RhAudioSamples *budget;
static RhAudioDecodeObservation observed;
int64_t hao_open(void){memory=rham_open();return memory!=NULL;}
int64_t hao_decode(coax_v_u8 name,coax_v_u8 source,coax_v_u8 companion){rhas_close(samples);samples=rhao_decode(memory,name.data,(size_t)name.count,source.data,(size_t)source.count,companion.data,(size_t)companion.count,&observed);return rhas_count(samples);}
int64_t hao_info(int64_t field){switch(field){case 0:return rhas_channels(samples);case 1:return observed.sdk_opens;case 2:return observed.reads;case 3:return observed.source_words;case 4:return observed.companion_words;case 5:return observed.coded_words;case 6:return observed.retained_words;case 7:return rham_total(memory);}return -1;}
int64_t hao_copy(coax_v_u8 out){size_t bytes=(size_t)rhas_count(samples)*sizeof(float);if(!samples||bytes>(uint64_t)out.count)return -1;memcpy(out.data,rhas_data(samples),bytes);return(int64_t)bytes;}
void hao_release(void){rhas_close(samples);samples=NULL;}
void hao_budget(bool fill){rhas_close(budget);budget=fill?rhas_open(memory,8388608,1):NULL;}
void hao_close(void){hao_release();hao_budget(false);rham_close(memory);memory=NULL;}
int64_t hao_asset(int64_t id,int64_t field){coax_t_HostAudioAsset a=hpc_everglade_at(id);switch(field){case 0:return a.frames;case 1:return a.channels;case 2:return a.canonical;}return -1;}
int64_t hao_text(int64_t id,int64_t field,coax_v_u8 out){coax_t_HostAudioAsset a=hpc_everglade_at(id);coax_string text=field==0?a.name:field==1?a.source_url:a.companion_url;if(text.count>out.count)return -1;memcpy(out.data,text.data,(size_t)text.count);return text.count;}
