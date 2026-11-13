// SDK effects for Coax-admitted Ogg/Opus and required APC1 companions.
#ifndef METAVERSE_NATIVE_AUDIO_OPUS_H
#define METAVERSE_NATIVE_AUDIO_OPUS_H
#include "audio-host.h"
typedef struct {int64_t sdk_opens,reads,source_words,companion_words,coded_words,retained_words;} RhAudioDecodeObservation;
// All inputs are immutable borrowed spans for this synchronous call. A null
// result publishes no PCM. Successful samples retain the shared memory owner.
RhAudioSamples *rhao_decode(RhAudioMemory *,const uint8_t *name,size_t name_bytes,const uint8_t *source,size_t source_bytes,const uint8_t *companion,size_t companion_bytes,RhAudioDecodeObservation *);
// Install bundle-file effects for ordinary lazy commands. ROOT contains
// realm URL paths; generated WAV names retain their canonical registered path.
bool rhao_resources(RhAudio *,const char *root);
#endif
