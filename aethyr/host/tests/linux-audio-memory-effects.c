// Firm reason: 1 (SDK fixture: actual owned allocations and admitted pointer readback).
#include "HamCheck.h"
#include "audio-host.h"
#include <string.h>
static RhAudioMemory *memory;
static RhAudioBank *bank;
static RhAudioSamples *samples[128];
static size_t refs[128];
static bool dropped;
static uint8_t storage[44];
static RhAudioBankAsset last;
int64_t hma_open(coax_v_u8 source){if(source.count!=sizeof storage)return 0;memcpy(storage,source.data,sizeof storage);memory=rham_open();bank=rhab_open();if(!memory||!bank){rham_close(memory);rhab_close(bank);return 0;}rham_retain(memory);return 1;}
int64_t hma_allocate(int64_t frames,int64_t channels){RhAudioSamples *s=rhas_open(memory,frames,channels);int64_t ticket=rhas_ticket(s);if(ticket>=0){samples[ticket]=s;refs[ticket]=1;}return ticket;}
int64_t hma_count(int64_t id){return id>=0&&id<128?rhas_count(samples[id]):-1;}
int64_t hma_total(void){return rham_total(memory);}
void hma_release(int64_t id){if(id<0||id>=128||!samples[id])return;rhas_close(samples[id]);if(--refs[id]==0)samples[id]=NULL;}
int64_t hma_copy(coax_v_u8 source){if(source.count!=24)return -1;float input[6];memcpy(input,source.data,sizeof input);RhAudioSamples *s=rhas_copy(memory,input,3,2);int64_t ticket=rhas_ticket(s);if(ticket>=0){samples[ticket]=s;refs[ticket]=1;}return ticket;}
int64_t hma_last(int64_t id){return id>=0&&id<128&&rhas_count(samples[id])==6&&rhas_data(samples[id])[5]==0.625f;}
int64_t hma_address(void){return(int64_t)(uintptr_t)storage;}
int64_t hma_insert(coax_v_u8 line){return rhab_insert(bank,line.data,(size_t)line.count,storage,sizeof storage,&last)>=0;}
int64_t hma_bank(int64_t field){switch(field){case 0:return rhab_count(bank);case 1:return(int64_t)last.byte_count;case 2:return last.name?*last.name:0;case 3:return last.bytes?*last.bytes:0;}return -1;}
void hma_keep(int64_t id){if(id>=0&&id<128&&samples[id]){rhas_retain(samples[id]);refs[id]++;}}
void hma_owner_drop(void){if(!dropped){rham_close(memory);dropped=true;}}
void hma_close(void){for(size_t i=0;i<128;i++)while(refs[i])hma_release((int64_t)i);rhab_close(bank);if(!dropped)rham_close(memory);rham_close(memory);memory=NULL;bank=NULL;}
