// Firm reason: 1 (SDK fixture: calls the product PCM/PipeWire adapter and borrows measured bytes).
#include "NativeAudio.h"
#include "audio-host.h"
#include <string.h>
static RhAudio *audio;
static RhAudioDevice *device;
static float output[2048];
int64_t hva_open(void){audio=rha_open();return audio!=NULL;}
int64_t hva_wav(coax_v_u8 name,coax_v_u8 bytes){return rha_add_wav(audio,name.data,(size_t)name.count,bytes.data,(size_t)bytes.count);}
int64_t hva_raw(coax_v_u8 bytes,int64_t channels){return bytes.count%4?-1:rha_pcm_load(audio,(const float*)bytes.data,(size_t)bytes.count/4,channels);}
int64_t hva_asset(int64_t id,coax_v_u8 out){size_t n=0;int64_t ch=0;const float *p=rha_asset(audio,(int)id,&n,&ch);size_t bytes=n*sizeof(float);if(!p||bytes>(uint64_t)out.count)return -1;memcpy(out.data,p,bytes);return(int64_t)bytes;}
int64_t hva_channels(int64_t id){size_t n=0;int64_t ch=0;rha_asset(audio,(int)id,&n,&ch);return ch;}
int64_t hva_command(coax_v_u8 line,int64_t asset,int64_t channels,bool started){return asset<0?rha_command(audio,line.data,(size_t)line.count,started):rha_command_asset(audio,line.data,(size_t)line.count,asset,channels,started);}
int64_t hva_block(coax_v_u8 out){if(out.count<(int64_t)sizeof output||!rha_block(audio,output))return -1;memcpy(out.data,output,sizeof output);return sizeof output;}
void hva_reset(void){rha_reset(audio);}
int64_t hva_info(int64_t field){return rha_info(audio,(int)field);}
int64_t hva_device_open(void){device=rhap_open(audio,true);return device!=NULL;}
int64_t hva_device_start(void){return rhap_start(device);}
int64_t hva_device_pause(bool paused){return rhap_pause(device,paused);}
int64_t hva_device_info(int64_t field){return rhap_info(device,(int)field);}
void hva_device_close(void){rhap_close(device);device=NULL;}
void hva_close(void){hva_device_close();rha_close(audio);audio=NULL;}
