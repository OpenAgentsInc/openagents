// Firm reason: 6 (C host memory owner bridging Coax generated music_studio.h to Swift).
#include "music-bridge.h"
#include "music_studio.h"
#include <stdlib.h>
#include <string.h>

#define MS_SMF_SCRATCH 65536
#define MS_STEP_BUF 512

struct OaMusicHandle {
  coax_t_MusicProject proj;
  coax_t_SmfOutEv smf_scratch[MS_SMF_SCRATCH];
  coax_t_MusicSchedEvent step_buf[MS_STEP_BUF];
};

static coax_v_t_MusicProject pv(OaMusicHandle *h) {
  coax_v_t_MusicProject v = { &h->proj, 1 };
  return v;
}

static coax_string cstr(const char *s) {
  if (!s) {
    coax_string empty = { (const uint8_t *)"", 0 };
    return empty;
  }
  coax_string cs = { (const uint8_t *)s, (int64_t)strlen(s) };
  return cs;
}

OaMusicHandle *ms_create(void) {
  OaMusicHandle *h = (OaMusicHandle *)calloc(1, sizeof(OaMusicHandle));
  if (!h) return NULL;
  coax_default_t_MusicProject(&h->proj);
  oa_music_load_starter(pv(h));
  return h;
}

void ms_destroy(OaMusicHandle *h) {
  free(h);
}

void ms_load_starter(OaMusicHandle *h) {
  if (!h) return;
  oa_music_load_starter(pv(h));
}

void ms_init_empty(OaMusicHandle *h, const char *title) {
  if (!h) return;
  oa_music_init(pv(h), cstr(title));
}

int32_t ms_sample_rate(const OaMusicHandle *h) { return h ? h->proj.sample_rate : 48000; }
int32_t ms_ppq(const OaMusicHandle *h) { return h ? h->proj.ppq : 960; }
uint64_t ms_revision(const OaMusicHandle *h) { return h ? h->proj.revision : 0; }
int64_t ms_total_ticks(OaMusicHandle *h) { return h ? oa_music_total_ticks(pv(h)) : 0; }

int32_t ms_track_count(const OaMusicHandle *h) { return h ? h->proj.track_count : 0; }

bool ms_get_track(const OaMusicHandle *h, int32_t index, MsTrackInfo *out) {
  if (!h || !out || index < 0 || index >= h->proj.track_count) return false;
  const coax_t_TrackState *t = &h->proj.tracks.e[index];
  out->active = t->active ? 1 : 0;
  out->section = t->section;
  out->bank_kind = t->bank_kind;
  out->midi_channel = t->midi_channel;
  out->gm_program = t->gm_program;
  out->transpose_preview = t->transpose_preview;
  out->mute = t->mute ? 1 : 0;
  out->solo = t->solo ? 1 : 0;
  out->ghost_visible = t->ghost_visible ? 1 : 0;
  out->gain = t->gain;
  out->pan = t->pan;
  out->reverb_send = t->reverb_send;
  uint8_t n = t->name_n < 48 ? t->name_n : 48;
  memcpy(out->name, t->name.e, n);
  out->name[n] = '\0';
  uint8_t an = t->au_id_n < 64 ? t->au_id_n : 64;
  memcpy(out->au_id, t->au_id.e, an);
  out->au_id[an] = '\0';
  return true;
}

int32_t ms_add_track(OaMusicHandle *h, const char *name, uint8_t section, uint8_t gm_program, uint8_t midi_channel, float pan, float reverb_send) {
  if (!h) return -1;
  return oa_music_add_track(pv(h), cstr(name), section, gm_program, midi_channel, pan, reverb_send);
}

void ms_set_track_program(OaMusicHandle *h, int32_t index, uint8_t gm_program) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].gm_program = gm_program & 127;
  h->proj.revision += 1;
}
void ms_set_track_bank(OaMusicHandle *h, int32_t index, uint8_t bank_kind, const char *au_id) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].bank_kind = bank_kind;
  if (au_id) {
    size_t len = strlen(au_id);
    if (len > 64) len = 64;
    memcpy(h->proj.tracks.e[index].au_id.e, au_id, len);
    h->proj.tracks.e[index].au_id_n = (uint8_t)len;
  } else {
    h->proj.tracks.e[index].au_id_n = 0;
  }
  h->proj.revision += 1;
}
void ms_set_track_mute(OaMusicHandle *h, int32_t index, bool mute) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].mute = mute;
  h->proj.revision += 1;
}
void ms_set_track_solo(OaMusicHandle *h, int32_t index, bool solo) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].solo = solo;
  h->proj.revision += 1;
}
void ms_set_track_ghost(OaMusicHandle *h, int32_t index, bool ghost) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].ghost_visible = ghost;
  h->proj.revision += 1;
}
void ms_set_track_gain(OaMusicHandle *h, int32_t index, float gain) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].gain = gain;
  h->proj.revision += 1;
}
void ms_set_track_pan(OaMusicHandle *h, int32_t index, float pan) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].pan = pan;
  h->proj.revision += 1;
}
void ms_set_track_reverb(OaMusicHandle *h, int32_t index, float reverb) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  h->proj.tracks.e[index].reverb_send = reverb;
  h->proj.revision += 1;
}
void ms_set_track_transpose(OaMusicHandle *h, int32_t index, int8_t semitones) {
  if (!h || index < 0 || index >= h->proj.track_count) return;
  if (semitones < -24) semitones = -24;
  if (semitones > 24) semitones = 24;
  h->proj.tracks.e[index].transpose_preview = semitones;
  h->proj.revision += 1;
}

int32_t ms_note_capacity(const OaMusicHandle *h) { return h ? h->proj.note_count : 0; }

bool ms_get_note(const OaMusicHandle *h, int32_t slot, MsNoteInfo *out) {
  if (!h || !out || slot < 0 || slot >= h->proj.note_count) return false;
  const coax_t_MidiNote *n = &h->proj.notes.e[slot];
  out->id = n->id;
  out->slot = slot;
  out->track = n->track;
  out->pitch = n->pitch;
  out->velocity = n->velocity;
  out->channel = n->channel;
  out->flags = n->flags;
  out->active = n->active ? 1 : 0;
  out->start_tick = n->start_tick;
  out->dur_tick = n->dur_tick;
  return true;
}

int32_t ms_add_note(OaMusicHandle *h, uint8_t track, uint8_t pitch, uint8_t velocity, int64_t start_tick, int64_t dur_tick) {
  if (!h) return -1;
  return oa_music_add_note(pv(h), track, pitch, velocity, start_tick, dur_tick);
}
bool ms_update_note(OaMusicHandle *h, int32_t slot, uint8_t pitch, uint8_t velocity, int64_t start_tick, int64_t dur_tick) {
  if (!h) return false;
  return oa_music_update_note(pv(h), slot, pitch, velocity, start_tick, dur_tick);
}
bool ms_remove_note(OaMusicHandle *h, int32_t slot) {
  if (!h) return false;
  return oa_music_remove_note(pv(h), slot);
}

int32_t ms_marker_count(const OaMusicHandle *h) { return h ? h->proj.marker_count : 0; }
bool ms_get_marker(const OaMusicHandle *h, int32_t index, MsMarkerInfo *out) {
  if (!h || !out || index < 0 || index >= h->proj.marker_count) return false;
  const coax_t_KeyMarker *m = &h->proj.markers.e[index];
  out->tick = m->tick;
  out->tonic = m->tonic;
  out->minor = m->minor ? 1 : 0;
  out->num = m->num;
  out->den = m->den;
  out->section_slot = m->section_slot;
  uint8_t n = m->label_n < 48 ? m->label_n : 48;
  memcpy(out->label, m->label.e, n);
  out->label[n] = '\0';
  return true;
}
int32_t ms_marker_at_tick(OaMusicHandle *h, int64_t tick) {
  if (!h) return -1;
  return oa_music_marker_at_tick(pv(h), tick);
}
bool ms_pitch_in_scale(int32_t pitch, int32_t tonic, bool minor) {
  return oa_music_pitch_in_scale(pitch, tonic, minor);
}

int64_t ms_tick_to_sample(OaMusicHandle *h, int64_t tick) { return h ? oa_music_tick_to_sample(pv(h), tick) : 0; }
int64_t ms_sample_to_tick(OaMusicHandle *h, int64_t sample) { return h ? oa_music_sample_to_tick(pv(h), sample) : 0; }
int32_t ms_bpm_at_tick(OaMusicHandle *h, int64_t tick) { return h ? oa_music_bpm_at_tick(pv(h), tick) : 120000; }
bool ms_set_tempo(OaMusicHandle *h, int64_t tick, int32_t bpm_milli, bool ramp) {
  return h ? oa_music_set_tempo(pv(h), tick, bpm_milli, ramp) : false;
}

bool ms_undo(OaMusicHandle *h) { return h ? oa_music_undo(pv(h)) : false; }
bool ms_redo(OaMusicHandle *h) { return h ? oa_music_redo(pv(h)) : false; }
int32_t ms_undo_cursor(const OaMusicHandle *h) { return h ? h->proj.undo_cursor : 0; }
int32_t ms_undo_count(const OaMusicHandle *h) { return h ? h->proj.undo_count : 0; }

int32_t ms_smf_import(OaMusicHandle *h, const uint8_t *bytes, int32_t len, int64_t merge_offset_tick, const char *title) {
  if (!h || !bytes || len <= 0) return -1;
  coax_v_u8 data = { (uint8_t *)bytes, len };
  return oa_music_smf_import(pv(h), data, merge_offset_tick, cstr(title));
}

int32_t ms_smf_export(OaMusicHandle *h, uint8_t *out, int32_t out_cap) {
  if (!h || !out || out_cap <= 0) return -1;
  coax_v_u8 ov = { out, out_cap };
  coax_v_t_SmfOutEv sv = { h->smf_scratch, MS_SMF_SCRATCH };
  return oa_music_smf_export(pv(h), ov, sv);
}

int32_t ms_remix_transpose(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t semitones) {
  return h ? oa_music_remix_transpose(pv(h), track, start_tick, end_tick, semitones) : 0;
}
int32_t ms_remix_quantize(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int64_t grid_ticks, int32_t strength_pct) {
  return h ? oa_music_remix_quantize(pv(h), track, start_tick, end_tick, grid_ticks, strength_pct) : 0;
}
int32_t ms_remix_humanize(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t max_tick, int32_t max_vel, uint32_t seed) {
  return h ? oa_music_remix_humanize(pv(h), track, start_tick, end_tick, max_tick, max_vel, seed) : 0;
}
int32_t ms_remix_copy_layer(OaMusicHandle *h, int32_t src_track, int32_t dst_track, int64_t start_tick, int64_t end_tick, int64_t tick_delta, int32_t semitones, int32_t vel_pct) {
  return h ? oa_music_remix_copy_layer(pv(h), src_track, dst_track, start_tick, end_tick, tick_delta, semitones, vel_pct) : 0;
}
int32_t ms_remix_explode(OaMusicHandle *h, int32_t src_track, int64_t start_tick, int64_t end_tick, const uint8_t *dst_tracks, int32_t dst_count) {
  if (!h || !dst_tracks || dst_count <= 0) return 0;
  coax_v_u8 dv = { (uint8_t *)dst_tracks, dst_count };
  return oa_music_remix_explode(pv(h), src_track, start_tick, end_tick, dv);
}
int32_t ms_remix_swell(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, int32_t v0, int32_t vpeak, int32_t v1) {
  return h ? oa_music_remix_swell(pv(h), track, start_tick, end_tick, v0, vpeak, v1) : 0;
}
int32_t ms_remix_pianist_touch(OaMusicHandle *h, int32_t track, int64_t start_tick, int64_t end_tick, uint32_t seed) {
  return h ? oa_music_remix_pianist_touch(pv(h), track, start_tick, end_tick, seed) : 0;
}
int32_t ms_add_cc(OaMusicHandle *h, uint8_t track, uint8_t cc_num, uint8_t value, int64_t tick) {
  return h ? oa_music_add_cc(pv(h), track, cc_num, value, tick) : -1;
}
void ms_remix_pit_seating(OaMusicHandle *h) {
  if (h) oa_music_remix_pit_seating(pv(h));
}

MsStepOut ms_transport_step(OaMusicHandle *h, int64_t from_sample, int32_t frame_count, bool loop_on, int64_t loop_start_tick, int64_t loop_end_tick, MsSchedEvent *out_events, int32_t max_events) {
  MsStepOut r = { from_sample, 0, 0 };
  if (!h) return r;
  int32_t cap = max_events < MS_STEP_BUF ? max_events : MS_STEP_BUF;
  coax_v_t_MusicSchedEvent ev = { h->step_buf, cap };
  coax_t_MusicStepResult cr = oa_music_transport_step(pv(h), from_sample, frame_count, loop_on, loop_start_tick, loop_end_tick, ev);
  r.next_sample = cr.next_sample;
  r.event_count = cr.event_count;
  r.wrapped = cr.wrapped ? 1 : 0;
  if (out_events && cr.event_count > 0) {
    int32_t n = cr.event_count < max_events ? cr.event_count : max_events;
    for (int32_t i = 0; i < n; i++) {
      out_events[i].sample_offset = h->step_buf[i].sample_offset;
      out_events[i].track = h->step_buf[i].track;
      out_events[i].channel = h->step_buf[i].channel;
      out_events[i].kind = h->step_buf[i].kind;
      out_events[i].pitch = h->step_buf[i].pitch;
      out_events[i].velocity = h->step_buf[i].velocity;
    }
  }
  return r;
}

void ms_update_active(OaMusicHandle *h, int64_t tick) { if (h) oa_music_update_active(pv(h), tick); }
void ms_clear_active(OaMusicHandle *h) { if (h) oa_music_clear_active(pv(h)); }
uint64_t ms_active_lo(const OaMusicHandle *h, int32_t track) {
  if (!h || track < 0 || track >= 32) return 0;
  return h->proj.active_lo.e[track];
}
uint64_t ms_active_hi(const OaMusicHandle *h, int32_t track) {
  if (!h || track < 0 || track >= 32) return 0;
  return h->proj.active_hi.e[track];
}
