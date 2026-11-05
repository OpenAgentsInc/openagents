// Firm reason: 1 (libopusfile SDK calls, owned byte/PCM allocations and copies).
#include "audio-opus.h"
#include "NativeAudio.h"
#include <stdint.h>
#include <string.h>
// The installed runtime has no development header output. These are the
// documented libopusfile 0.12 opaque C ABI declarations, not a codec/parser.
// https://opus-codec.org/docs/opusfile_api-0.12/group__stream__open__close.html
typedef struct OggOpusFile OggOpusFile;
extern OggOpusFile *op_open_memory(const unsigned char *,size_t,int *);
extern void op_free(OggOpusFile *);
extern int op_channel_count(const OggOpusFile *,int);
extern int64_t op_pcm_total(const OggOpusFile *,int);
extern int op_read_float(OggOpusFile *,float *,int,int *);

RhAudioSamples *rhao_decode(RhAudioMemory *memory,const uint8_t *name,size_t name_bytes,const uint8_t *source,size_t source_bytes,const uint8_t *companion,size_t companion_bytes,RhAudioDecodeObservation *observed){
 RhAudioDecodeObservation local={0};if(!observed)observed=&local;*observed=(RhAudioDecodeObservation){0};
 if(!memory||name_bytes>INT64_MAX||source_bytes>INT64_MAX||companion_bytes>INT64_MAX||(!name&&name_bytes)||(!source&&source_bytes)||(!companion&&companion_bytes))return NULL;
 coax_string key={(const char*)name,(int64_t)name_bytes};if(!apv_asset_name(key))return NULL;
 coax_t_HostAudioAsset asset=hpc_everglade(key);coax_v_u8 borrowed={(uint8_t*)source,(int64_t)source_bytes};
 coax_t_HostOpusPlan plan=asset.valid?hpc_source_plan(asset,borrowed):hpo_plan(borrowed);if(!plan.valid)return NULL;
 int64_t encoded_words=hae_words((int64_t)source_bytes,false);if(!encoded_words)return NULL;
 RhAudioSamples *encoded=rhas_open(memory,encoded_words,1),*apc=NULL,*decoded=NULL,*probe=NULL,*result=NULL;
 OggOpusFile *file=NULL;if(!encoded)goto done;observed->source_words=rhas_count(encoded);
 memcpy(rhas_data(encoded),source,source_bytes);coax_v_u8 owned={(uint8_t*)rhas_data(encoded),(int64_t)source_bytes};
 if(asset.canonical){
  coax_t_HostPcmAssetPlan pcm=hpc_plan(asset,owned,(coax_v_u8){(uint8_t*)companion,(int64_t)companion_bytes});if(!pcm.valid)goto done;
  int64_t apc_words=hae_words((int64_t)companion_bytes,true);if(!apc_words)goto done;
  apc=rhas_open(memory,apc_words,1);if(!apc)goto done;observed->companion_words=rhas_count(apc);memcpy(rhas_data(apc),companion,companion_bytes);
  result=rhas_open(memory,pcm.frames,pcm.channels);if(!result)goto done;
  if(!hpc_decode(asset,owned,(coax_v_u8){(uint8_t*)rhas_data(apc),(int64_t)companion_bytes},(coax_v_f32){rhas_data(result),rhas_count(result)})){rhas_close(result);result=NULL;}
 }else{
  decoded=rhas_open(memory,plan.decode_frames,plan.channels);probe=rhas_open(memory,1,plan.channels);if(!decoded||!probe)goto done;observed->coded_words=rhas_count(decoded);
  int error=0;observed->sdk_opens++;file=op_open_memory((const unsigned char*)rhas_data(encoded),source_bytes,&error);if(!file)goto done;
  if(!hpo_output(plan,op_pcm_total(file,-1),op_channel_count(file,0),48000))goto done;
  coax_t_HostAudioDecode state=had_open(plan.frames,plan.channels);coax_t_HostAudioRead offer;
  for(;;){offer=had_read(state);if(offer.kind!=1&&offer.kind!=2)break;observed->reads++;
   float *target=offer.kind==1?rhas_data(decoded)+offer.offset:rhas_data(probe);
   int count=op_read_float(file,target,(int)offer.words,NULL);
   if(!had_observe((coax_v_t_HostAudioDecode){&state,1},count))break;
  }
  if(had_read(state).kind==3&&hdo_finite((coax_v_u8){(uint8_t*)rhas_data(decoded),plan.samples*4}))result=rhas_copy(memory,rhas_data(decoded),plan.frames,plan.channels);
 }
done:
 if(file)op_free(file);rhas_close(probe);rhas_close(decoded);rhas_close(apc);rhas_close(encoded);
 observed->retained_words=rhas_count(result);return result;
}
