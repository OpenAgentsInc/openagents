// The Apple audio host's C side (metaverse-engine/host/README.md, Audio): scalar wrappers over the shared
// Coax audio authorities (metaverse-engine/host/native-audio.coax: the PCM interpreter, the session, the bank,
// the memory tickets, the WAV and Opus plans and the Everglade PCM assets), and the four calls a
// realm host answers for its page: its sound bank and its sound packets. The Metaverse Engine desktop app's realm host
// (apps/metaverse-engine-desktop/MetalRealm.m) and the Metaverse Engine's page host (metaverse-engine/host/apple/PageHost.m)
// each answer those four; the wrappers are audio-host.c, built into the TU that has the Coax
// types. The Swift side is apps/metaverse-engine-desktop/Sources/MetalAudio.swift.
#ifndef AETHYR_APPLE_AUDIO_HOST_H
#define AETHYR_APPLE_AUDIO_HOST_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
const uint8_t *mf_sound_bank(void *host,size_t *count);
const uint8_t *mf_sound_bank_storage(void *host,size_t *capacity);
typedef struct {const uint8_t *name;size_t name_count;const uint8_t *bytes;size_t byte_count;} MFAudioBankAsset;
void *mf_audio_bank_open(const uint8_t *base,size_t capacity);
bool mf_audio_bank_register(void *bank,const uint8_t *line,size_t count,MFAudioBankAsset *out);
int64_t mf_audio_bank_count(void *bank);
void mf_audio_bank_close(void *bank);
// The shared authority admits exact extents before every PCM allocation.
int64_t mf_audio_extent(int64_t frames,int64_t channels);
void *mf_audio_memory_open(void);
int64_t mf_audio_memory_reserve(void *memory,int64_t words);
int64_t mf_audio_memory_words(void *memory,int64_t ticket);
bool mf_audio_memory_release(void *memory,int64_t ticket);
int64_t mf_audio_memory_total(void *memory);
void mf_audio_memory_close(void *memory);
const uint8_t *mf_sound_commands(void *host,size_t *count);
int32_t mf_sound_probe(void *host,int32_t slot,float right,float ahead,float up);
typedef struct {int64_t kind;const uint8_t *name;size_t name_count;const uint8_t *voice_id;size_t id_count;float gain,pan,delay,lowpass,rate,wet;} MFAudioEffect;
bool mf_audio_decode(const uint8_t *bytes,size_t count,MFAudioEffect *out);
bool mf_audio_asset_name(const uint8_t *bytes,size_t count);
// Borrowed catalogue strings live independently of the opaque descriptor.
typedef struct {bool valid,canonical;const uint8_t *source_url;size_t source_url_count;const uint8_t *companion_url;size_t companion_url_count;int64_t source_bytes,companion_bytes,frames,channels;} MFAudioAsset;
typedef struct {bool valid;int64_t channels,pre_skip,frames,decode_frames,samples,decode_samples;} MFOpusPlan;
typedef struct {bool valid;int64_t channels,frames,samples,offset;} MFPcmAssetPlan;
void *mf_audio_asset_open(const uint8_t *name,size_t count,MFAudioAsset *out);
void mf_audio_asset_close(void *asset);
int64_t mf_audio_encoded_words(int64_t bytes,bool companion);
MFOpusPlan mf_audio_opus_plan(void *asset,const uint8_t *source,size_t count);
bool mf_audio_opus_output(MFOpusPlan plan,int64_t frames,int64_t channels,double rate);
MFPcmAssetPlan mf_audio_companion_plan(void *asset,const uint8_t *source,size_t source_count,const uint8_t *companion,size_t companion_count);
bool mf_audio_companion_decode(void *asset,const uint8_t *source,size_t source_count,const uint8_t *companion,size_t companion_count,float *out,size_t count);
typedef struct {bool active,started,retired;} MFAudioSessionOffer;
void *mf_audio_session_open(void);
MFAudioSessionOffer mf_audio_session_offer(void *session);
MFAudioSessionOffer mf_audio_session_event(void *session,int64_t kind);
void mf_audio_session_close(void *session);
void *mf_pcm_open(void);
int64_t mf_pcm_command(void *pcm,const uint8_t *line,size_t count,const float *samples,size_t sample_count,int64_t channels,bool started);
const float *mf_pcm_block(void *pcm,size_t *samples);
int64_t mf_pcm_info(void *pcm,int32_t field);
void mf_pcm_close(void *pcm);
void *mf_wav_open(void);
int64_t mf_wav_info(const uint8_t *bytes,size_t count,int32_t field);
bool mf_wav_decode(void *decoder,const uint8_t *bytes,size_t count,float *samples,size_t sample_count);
void mf_wav_close(void *decoder);
#endif
