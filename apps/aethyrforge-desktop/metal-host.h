// Scalar SDK bridge. Product decisions and all game/HUD behavior stay in Coax.
#ifndef FORGE_METAL_HOST_H
#define FORGE_METAL_HOST_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
void *mf_open(const char *resources,const char *producer,bool golden,bool fixture);
const char *mf_error(void *host);
void mf_layer(void *host,void *layer);
bool mf_offscreen_bgra8(void *host);
typedef struct {int64_t width,height;float scale;} MFSurface;
bool mf_surface(void *host,int64_t css_width,int64_t css_height,int64_t device_width,int64_t device_height,const char *quality,MFSurface *out);
bool mf_frame(void *host,float width,float height,float scale,float dt);
bool mf_frame_at(void *host,float width,float height,float scale,int64_t monotonic_us);
void mf_shown(void *host,bool shown);
int64_t mf_lifecycle_info(void *host,int32_t field);
bool mf_replay(void *host,const char *frame_config);
bool mf_recover(void *host);
// Observed last-frame producer bytes: 0 uniforms,1 HUD,2 draws,3..6 stores,
// 7 particles,8 complete allocated producer state,9 exact media packet
// (diagnostics only).
const uint8_t *mf_cpu_bytes(void *host,int32_t kind,size_t *count);
// Measured producer allocation, independently of its current upload prefix.
size_t mf_cpu_capacity(void *host,int32_t kind);
void mf_still(void *host,bool still);
bool mf_asset(void *host,int32_t index,const uint8_t *bytes,size_t count);
void mf_message(void *host,int32_t kind,const uint8_t *bytes,size_t count);
bool mf_key(void *host,int32_t key,bool down,int32_t mods);
bool mf_pointer(void *host,float x,float y,int32_t phase,int32_t kind,int32_t id);
void mf_wheel(void *host,float dy);
// Coax projection of observed AppKit pixel/Cocoa-tick travel to CSS units.
float hmi_appkit_wheel(float delta,bool precise);
void mf_focus(void *host,bool focused);
int32_t mf_action(void *host);
const uint8_t *mf_outbox(void *host,size_t *count);
void mf_action_done(void *host);
int32_t mf_probe(void *host,int32_t field);
int64_t mf_observe(void *host,int64_t event,int64_t field);
void mf_observe_reset(void *host);
int64_t mf_info(void *host,int32_t field);
typedef struct {uint64_t frame,start_ns,submit_ns,complete_ns,gpu_ns,input_sequence,input_ns;int64_t x_mm,z_mm,width,height;float scale;} MFFrameObservation;
void mf_measure_reset(void *host);
const MFFrameObservation *mf_measure_rows(void *host,size_t *count,size_t *dropped);
const char *mf_take_effect(void *host);
bool mf_http_chunk(void *host,int64_t id,int64_t generation,int64_t engine,int64_t count);
void mf_http_finish(void *host,int64_t id,int64_t generation,int64_t engine,const uint8_t *bytes,size_t count,bool ok,bool utf8);
void mf_http_timeout(void *host,int64_t id,int64_t generation,int64_t engine);
void mf_socket_event(void *host,int64_t id,int64_t generation,int64_t engine,int32_t event,const uint8_t *bytes,size_t count);
bool mf_snapshot(void *host,const char *path);
// Disabled-by-default full-graph FP16 to actual UNORM display experiment.
bool mf_presentation_experiment(void *host);
bool mf_presentation_experiment_v1(void *host);
bool mf_snapshot_presented(void *host,const char *path);
bool mf_snapshot_presented_raw(void *host,const char *path);
// Diagnostic readback of an actually uploaded ready media texture.
bool mf_media_snapshot(void *host,const char *url,const char *path);
const uint8_t *mf_media_notice(void *host,size_t *count);
// 0 cached,1 ready,2 failed,3 width,4 height (diagnostic observation only).
int64_t mf_media_info(void *host,const char *url,int32_t field);
// Test-only source upload and one presentation pass; never calls the producer.
// Quantized requires the separately linked private Coax v1 descriptor.
bool mf_present_probe(void *host,const uint8_t *bytes,size_t count,int64_t width,int64_t height,int64_t format,bool quantized);
const uint8_t *mf_frame_jpeg(void *host,size_t *count);
int64_t mf_http_image(const uint8_t *body,size_t body_count,const uint8_t *image,size_t image_count,uint8_t *out,size_t out_count);
void mf_network_config(void *host,const char *page_origin,const char *local_origin);
void mf_network_cookie_storage(void *host,void *storage);
void mf_network_reset(void *host);
// Nonblocking raw SDK delivery, independent of world render offers.
int64_t mf_network_poll(void *host);
int64_t mf_network_info(void *host,int32_t field);
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
void *mf_pcm_open(void);
int64_t mf_pcm_command(void *pcm,const uint8_t *line,size_t count,const float *samples,size_t sample_count,int64_t channels,bool started);
const float *mf_pcm_block(void *pcm,size_t *samples);
int64_t mf_pcm_info(void *pcm,int32_t field);
void mf_pcm_close(void *pcm);
void *mf_wav_open(void);
int64_t mf_wav_info(const uint8_t *bytes,size_t count,int32_t field);
bool mf_wav_decode(void *decoder,const uint8_t *bytes,size_t count,float *samples,size_t sample_count);
void mf_wav_close(void *decoder);
void mf_close(void *host);
int64_t mf_closed_info(int32_t field);
#endif
