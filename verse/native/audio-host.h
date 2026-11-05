// Native audio SDK boundary. PCM/WAV, bank spans and memory admission are Coax.
#ifndef VERSE_NATIVE_AUDIO_HOST_H
#define VERSE_NATIVE_AUDIO_HOST_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct RhAudio RhAudio;
typedef struct RhAudioDevice RhAudioDevice;
typedef struct RhAudioMemory RhAudioMemory;
typedef struct RhAudioSamples RhAudioSamples;
typedef struct RhAudioBank RhAudioBank;
typedef struct {const uint8_t *name,*bytes;size_t name_bytes,byte_count;} RhAudioBankAsset;
RhAudioBank *rhab_open(void);
void rhab_close(RhAudioBank *);
int64_t rhab_insert(RhAudioBank *,const uint8_t *line,size_t line_bytes,const uint8_t *storage,size_t storage_bytes,RhAudioBankAsset *);
int64_t rhab_count(RhAudioBank *);
// An allocation retains its measured budget owner until its last reference.
// Extent admission and ticket arithmetic are the shared Coax ham_* API.
RhAudioMemory *rham_open(void);
void rham_retain(RhAudioMemory *);
void rham_close(RhAudioMemory *);
int64_t rham_total(RhAudioMemory *);
RhAudioSamples *rhas_open(RhAudioMemory *,int64_t frames,int64_t channels);
RhAudioSamples *rhas_copy(RhAudioMemory *,const float *,int64_t frames,int64_t channels);
void rhas_retain(RhAudioSamples *);
void rhas_close(RhAudioSamples *);
float *rhas_data(RhAudioSamples *);
int64_t rhas_count(RhAudioSamples *);
int64_t rhas_channels(RhAudioSamples *);
int64_t rhas_ticket(RhAudioSamples *);
RhAudio *rha_open(void);
int64_t rha_add_wav(RhAudio *,const uint8_t *name,size_t name_bytes,const uint8_t *wav,size_t wav_bytes);
int64_t rha_bank(RhAudio *,const uint8_t *listing,size_t listing_bytes,const uint8_t *storage,size_t storage_bytes);
int64_t rha_command(RhAudio *,const uint8_t *line,size_t bytes,bool started);
// Product calls capture this owner's actual gesture/cover/retirement facts.
bool rha_session_event(RhAudio *,int event);
int64_t rha_command_current(RhAudio *,const uint8_t *line,size_t bytes);
// Already decoded immutable PCM uses the same bounded allocation tickets.
int64_t rha_pcm_load(RhAudio *,const float *source,size_t samples,int64_t channels);
int64_t rha_command_asset(RhAudio *,const uint8_t *line,size_t bytes,int64_t ticket,int64_t channels,bool started);
bool rha_block(RhAudio *,float output[2048]);
bool rha_render(RhAudio *,float *output,size_t frames,bool muted);
int64_t rha_info(RhAudio *,int field);
const float *rha_asset(RhAudio *,int id,size_t *samples,int64_t *channels);
RhAudioSamples *rha_asset_keep(RhAudio *,int id);
void rha_reset(RhAudio *);
void rha_close(RhAudio *);
// Close the SDK device before freeing the core or any borrowed PCM. Open is
// inactive until start; pause preserves the Coax sample/block cursor.
RhAudioDevice *rhap_open(RhAudio *,bool muted);
bool rhap_start(RhAudioDevice *);
bool rhap_pause(RhAudioDevice *,bool paused);
int64_t rhap_info(RhAudioDevice *,int field);
void rhap_close(RhAudioDevice *);
#endif
