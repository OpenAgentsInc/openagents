// Firm reason: 6 (C ABI bridge between Coax music/studio.coax and Swift AppKit/AVFoundation host).
#ifndef OA_MUSIC_STUDIO_BRIDGE_H
#define OA_MUSIC_STUDIO_BRIDGE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OaMusicHandle OaMusicHandle;

typedef struct {
  uint8_t active;
  uint8_t section;
  uint8_t bank_kind;         // 0=Orchestra SF2, 1=Concert Grand SF2, 2=macOS Audio Unit
  uint8_t midi_channel;
  uint8_t gm_program;
  int8_t transpose_preview;
  uint8_t mute;
  uint8_t solo;
  uint8_t ghost_visible;
  float gain;
  float pan;
  float reverb_send;
  char name[49];
  char au_id[65];
} MsTrackInfo;

typedef struct {
  uint32_t id;
  int32_t slot;
  uint8_t track;
  uint8_t pitch;
  uint8_t velocity;
  uint8_t channel;
  uint8_t flags;
  uint8_t active;
  int64_t start_tick;
  int64_t dur_tick;
} MsNoteInfo;

typedef struct {
  int64_t tick;
  uint8_t tonic;
  uint8_t minor;
  uint8_t num;
  uint8_t den;
  uint8_t section_slot;
  char label[49];
} MsMarkerInfo;

typedef struct {
  int32_t sample_offset;
  uint8_t track;
  uint8_t channel;
  uint8_t kind;      // 1 = NOTE_ON, 2 = NOTE_OFF, 3 = ALL_NOTES_OFF
  uint8_t pitch;
  uint8_t velocity;
} MsSchedEvent;

typedef struct {
  int64_t next_sample;
  int32_t event_count;
  uint8_t wrapped;
} MsStepOut;

OaMusicHandle *ms_create(void);
void ms_destroy(OaMusicHandle *h);
void ms_load_starter(OaMusicHandle *h);
void ms_init_empty(OaMusicHandle *h, const char *title);

int32_t ms_sample_rate(const OaMusicHandle *h);
int32_t ms_ppq(const OaMusicHandle *h);
uint64_t ms_revision(const OaMusicHandle *h);
int64_t ms_total_ticks(OaMusicHandle *h);

int32_t ms_track_count(const OaMusicHandle *h);
bool ms_get_track(const OaMusicHandle *h, int32_t index, MsTrackInfo *out);
int32_t ms_add_track(OaMusicHandle *h, const char *name, uint8_t section, uint8_t gm_program, uint8_t midi_channel, float pan, float reverb_send);
void ms_set_track_program(OaMusicHandle *h, int32_t index, uint8_t gm_program);
void ms_set_track_bank(OaMusicHandle *h, int32_t index, uint8_t bank_kind, const char *au_id);
void ms_set_track_mute(OaMusicHandle *h, int32_t index, bool mute);
void ms_set_track_solo(OaMusicHandle *h, int32_t index, bool solo);
void ms_set_track_ghost(OaMusicHandle *h, int32_t index, bool ghost);
void ms_set_track_gain(OaMusicHandle *h, int32_t index, float gain);
void ms_set_track_pan(OaMusicHandle *h, int32_t index, float pan);
void ms_set_track_reverb(OaMusicHandle *h, int32_t index, float reverb);
void ms_set_track_transpose(OaMusicHandle *h, int32_t index, int8_t semitones);

int32_t ms_note_capacity(const OaMusicHandle *h);
bool ms_get_note(const OaMusicHandle *h, int32_t slot, MsNoteInfo *out);
int32_t ms_add_note(OaMusicHandle *h, uint8_t track, uint8_t pitch, uint8_t velocity, int64_t start_tick, int64_t dur_tick);
bool ms_update_note(OaMusicHandle *h, int32_t slot, uint8_t pitch, uint8_t velocity, int64_t start_tick, int64_t dur_tick);
bool ms_remove_note(OaMusicHandle *h, int32_t slot);

int32_t ms_marker_count(const OaMusicHandle *h);
bool ms_get_marker(const OaMusicHandle *h, int32_t index, MsMarkerInfo *out);
int32_t ms_marker_at_tick(OaMusicHandle *h, int64_t tick);
bool ms_pitch_in_scale(int32_t pitch, int32_t tonic, bool minor);

int64_t ms_tick_to_sample(OaMusicHandle *h, int64_t tick);
int64_t ms_sample_to_tick(OaMusicHandle *h, int64_t sample);
int32_t ms_bpm_at_tick(OaMusicHandle *h, int64_t tick);
bool ms_set_tempo(OaMusicHandle *h, int64_t tick, int32_t bpm_milli, bool ramp);

bool ms_undo(OaMusicHandle *h);
bool ms_redo(OaMusicHandle *h);
int32_t ms_undo_cursor(const OaMusicHandle *h);
int32_t ms_undo_count(const OaMusicHandle *h);

int32_t ms_smf_import(OaMusicHandle *h, const uint8_t *bytes, int32_t len, int64_t merge_offset_tick, const char *title);
int32_t ms_smf_export(OaMusicHandle *h, uint8_t *out, int32_t out_cap);

int32_t ms_remix_transpose(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t semitones);
int32_t ms_remix_quantize(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int64_t grid_ticks, int32_t strength_pct);
int32_t ms_remix_humanize(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t max_tick, int32_t max_vel, uint32_t seed);
int32_t ms_remix_copy_layer(OaMusicHandle *h, int32_t src_track, int32_t dst_track, int64_t start_tick, int64_t end_tick, int64_t tick_delta, int32_t semitones, int32_t vel_pct);
int32_t ms_remix_explode(OaMusicHandle *h, int32_t src_track, int64_t start_tick, int64_t end_tick, const uint8_t *dst_tracks, int32_t dst_count);
int32_t ms_remix_swell(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t v0, int32_t vpeak, int32_t v1);
int32_t ms_remix_pianist_touch(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, uint32_t seed);
int32_t ms_add_cc(OaMusicHandle *h, uint8_t track, uint8_t cc_num, uint8_t value, int64_t tick);
void ms_remix_pit_seating(OaMusicHandle *h);

MsStepOut ms_transport_step(OaMusicHandle *h, int64_t from_sample, int32_t frame_count, bool loop_on, int64_t loop_start_tick, int64_t loop_end_tick, MsSchedEvent *out_events, int32_t max_events);
void ms_update_active(OaMusicHandle *h, int64_t tick);
void ms_clear_active(OaMusicHandle *h);
uint64_t ms_active_lo(const OaMusicHandle *h, int32_t track);
uint64_t ms_active_hi(const OaMusicHandle *h, int32_t track);

#ifdef __cplusplus
}
#endif

#endif
