/*
 * metareal/platform/voice.c -- the plain-C face (voice.h) over the Coax voice
 * engine. Everything that decides anything lives in Coax:
 *   metareal/voice.coax        engine: resample, levels, VAD, turns, barge-in,
 *                              jitter buffer, gain/duck/rate, decisions
 *   metareal/voice-wire.coax   Frame codecs (generated from voice.pb)
 *   metareal/voice-floor.coax  consent, input floor, media epochs (#authority)
 *   metareal/voice-meter.coax  free tier and pricing (#authority)
 * This file owns storage (the opaque structs wrap the Coax state), converts
 * between the C structs in voice.h and the Coax ones, and nothing else.
 * Build each .coax with `coax/bin/coax c FILE --library --header Name.h`.
 */
#include "voice.h"

#include <string.h>

#include "VoiceEngine.h" /* metareal/voice.coax; not "Voice.h", which is voice.h on a case-insensitive disk */
#include "VoiceWire.h"
#include "VoiceFloor.h"
#include "VoiceMeter.h"

/* Enum values and bits shared with Coax must agree with voice.proto. */
_Static_assert(OA_MSG_DECISION == 11 && OA_FRAME_ACTION_DROP == 7 && OA_TURN_STATE_ENDED == 4, "voice.proto numbering");
_Static_assert(OA_PLAYBACK_STATE_PAUSED == 4 && OA_CONTROL_SPEECH_END == 8 && OA_ERROR_INTERNAL == 6, "voice.proto numbering");

struct oa_voice { coax_t_VoiceState s; };
struct oa_voice_floor { coax_t_Floor f; };
struct oa_voice_meter { coax_t_Meter m; };

static coax_v_t_VoiceState sv(const oa_voice *v) {
  return (coax_v_t_VoiceState){(coax_t_VoiceState *)&v->s, 1};
}
static coax_v_t_Floor sf(const oa_voice_floor *f) { return (coax_v_t_Floor){(coax_t_Floor *)&f->f, 1}; }
static coax_v_t_Meter sm(const oa_voice_meter *m) { return (coax_v_t_Meter){(coax_t_Meter *)&m->m, 1}; }

static const char DECIDER[] = "voice.coax";

/* ------------------------------------------------------------ the engine */

size_t oa_voice_size(void) { return sizeof(struct oa_voice); }

void oa_voice_init(oa_voice *v, uint32_t input_rate) {
  memset(v, 0, sizeof *v);
  metareal_voice_init(sv(v), input_rate);
}

void oa_voice_configure(oa_voice *v, const oa_voice_config *c) {
  if (!c) return;
  coax_t_VoiceConfig k = {
    .end_silence_ms = c->end_silence_ms, .min_speech_ms = c->min_speech_ms,
    .max_turn_ms = c->max_turn_ms, .confidence_floor = c->confidence_floor,
    .speech_threshold = c->speech_threshold, .jitter_target = c->jitter_target};
  metareal_voice_configure(sv(v), k);
}

void oa_voice_get_config(const oa_voice *v, oa_voice_config *out) {
  coax_t_VoiceConfig k = metareal_voice_config(sv(v));
  out->end_silence_ms = k.end_silence_ms;
  out->min_speech_ms = k.min_speech_ms;
  out->max_turn_ms = k.max_turn_ms;
  out->confidence_floor = k.confidence_floor;
  out->speech_threshold = k.speech_threshold;
  out->jitter_target = k.jitter_target;
}

uint32_t oa_voice_input_frame_samples(const oa_voice *v) { return metareal_voice_input_frame_samples(sv(v)); }

static void features_out(const coax_t_VoiceFeatures *k, oa_voice_features *f) {
  f->stream = k->stream;
  f->sequence = k->sequence;
  f->media_epoch = k->media_epoch;
  f->rms_db = k->rms_db;
  f->peak_db = k->peak_db;
  f->speech = k->speech;
  f->noise_db = k->noise_db;
  f->turn_state = k->turn_state;
  f->playback_state = k->playback_state;
  f->speech_ms = k->speech_ms;
  f->silence_ms = k->silence_ms;
  f->zero_crossings = k->zero_crossings;
}

uint32_t oa_voice_uplink(oa_voice *v, const int16_t *pcm, int16_t *out16k, oa_voice_features *f) {
  if (!pcm) return 0;
  coax_t_VoiceFeatures k;
  memset(&k, 0, sizeof k);
  coax_v_s16 in = {(int16_t *)pcm, (int64_t)metareal_voice_input_frame_samples(sv(v))};
  coax_v_s16 out = {out16k, out16k ? OA_VOICE_FRAME_SAMPLES : 0};
  coax_v_t_VoiceFeatures fv = {&k, 1};
  uint32_t ev = metareal_voice_uplink(sv(v), in, out, fv);
  if (f) features_out(&k, f);
  return ev;
}

uint32_t oa_voice_preroll(const oa_voice *v, int16_t *out16k, uint32_t cap) {
  if (!out16k) return 0;
  return metareal_voice_preroll(sv(v), (coax_v_s16){out16k, cap});
}

uint32_t oa_voice_downlink(oa_voice *v, int16_t *pcm, uint32_t n, uint32_t rate, oa_voice_features *f) {
  coax_t_VoiceFeatures k;
  memset(&k, 0, sizeof k);
  coax_v_s16 io = {pcm, pcm ? (int64_t)n : 0};
  coax_v_t_VoiceFeatures fv = {&k, f ? 1 : 0};
  uint32_t ev = metareal_voice_downlink(sv(v), io, rate, fv);
  if (f) features_out(&k, f);
  return ev;
}

bool oa_voice_decide(oa_voice *v, const oa_voice_decision *d) {
  if (!d) return false;
  coax_t_VoiceDecision k = {
    .has = d->has, .stream = d->stream, .apply_from = d->apply_from_sequence,
    .epoch = d->media_epoch, .action = d->action, .gain_db = d->gain_db,
    .duck_db = d->duck_db, .rate = d->playback_rate, .hold_ms = d->hold_ms,
    .confidence = d->confidence, .deadline = d->deadline_sequence};
  return metareal_voice_decide(sv(v), k);
}

bool oa_voice_last_decision(const oa_voice *v, oa_voice_decision *out) {
  coax_t_VoiceDecision k;
  if (!out || !metareal_voice_last_decision(sv(v), (coax_v_t_VoiceDecision){&k, 1})) return false;
  memset(out, 0, sizeof *out);
  out->has = k.has;
  out->stream = k.stream;
  out->apply_from_sequence = k.apply_from;
  out->media_epoch = k.epoch;
  out->action = k.action;
  out->gain_db = k.gain_db;
  out->duck_db = k.duck_db;
  out->playback_rate = k.rate;
  out->hold_ms = k.hold_ms;
  out->confidence = k.confidence;
  out->decider = (oa_voice_bytes){(const uint8_t *)DECIDER, (uint32_t)(sizeof DECIDER - 1)};
  out->deadline_sequence = k.deadline;
  return true;
}

void oa_voice_playing(oa_voice *v, bool playing) { metareal_voice_playing(sv(v), playing); }
uint64_t oa_voice_epoch(const oa_voice *v) { return metareal_voice_epoch(sv(v)); }
uint64_t oa_voice_set_epoch(oa_voice *v, uint64_t epoch) { return metareal_voice_set_epoch(sv(v), epoch); }
uint64_t oa_voice_cut(oa_voice *v) { return metareal_voice_cut(sv(v)); }
void oa_voice_cancel_turn(oa_voice *v) { metareal_voice_cancel_turn(sv(v)); }
float oa_voice_gain_db(const oa_voice *v) { return metareal_voice_gain_db(sv(v)); }
float oa_voice_playback_rate(const oa_voice *v) { return metareal_voice_playback_rate(sv(v)); }
uint32_t oa_voice_current_turn_state(const oa_voice *v) { return metareal_voice_turn_state(sv(v)); }
uint32_t oa_voice_current_playback_state(const oa_voice *v) { return metareal_voice_playback_state(sv(v)); }
uint32_t oa_voice_uplink_sequence(const oa_voice *v) { return metareal_voice_uplink_sequence(sv(v)); }
uint32_t oa_voice_downlink_sequence(const oa_voice *v) { return metareal_voice_downlink_sequence(sv(v)); }
uint32_t oa_voice_turn_ms(const oa_voice *v) { return metareal_voice_turn_ms(sv(v)); }

bool oa_voice_jitter_push(oa_voice *v, uint32_t sequence, uint64_t epoch, const int16_t *pcm, uint32_t n) {
  if (!pcm) return false;
  return metareal_voice_jitter_push(sv(v), sequence, epoch, (coax_v_s16){(int16_t *)pcm, n});
}
uint32_t oa_voice_jitter_pop(oa_voice *v, int16_t *out, uint32_t n) {
  if (!out) return OA_EV_UNDERRUN;
  if (n > OA_VOICE_MAX_DOWNLINK_SAMPLES) n = OA_VOICE_MAX_DOWNLINK_SAMPLES;
  return metareal_voice_jitter_pop(sv(v), (coax_v_s16){out, n});
}
uint32_t oa_voice_jitter_depth(const oa_voice *v) { return metareal_voice_jitter_depth(sv(v)); }
void oa_voice_jitter_flush(oa_voice *v) { metareal_voice_jitter_flush(sv(v)); }

/* ------------------------------------------------------------ the wire */

static coax_string cs(oa_voice_bytes b) { return (coax_string){(const char *)b.ptr, b.ptr ? (int64_t)b.len : 0}; }
static oa_voice_bytes ob(coax_string s) {
  return (oa_voice_bytes){s.count > 0 ? (const uint8_t *)s.data : NULL, (uint32_t)s.count};
}

static coax_t_WSpeaker speaker_in(const oa_voice_speaker *s) {
  return (coax_t_WSpeaker){.id = cs(s->id), .role = (int32_t)s->role, .profile = cs(s->profile),
                           .profile_revision = s->profile_revision, .entity = cs(s->entity)};
}
static oa_voice_speaker speaker_out(const coax_t_WSpeaker *s) {
  return (oa_voice_speaker){.id = ob(s->id), .role = (uint32_t)s->role, .profile = ob(s->profile),
                            .profile_revision = s->profile_revision, .entity = ob(s->entity)};
}
static coax_t_WMeter meter_in(const oa_voice_meter_report *m) {
  return (coax_t_WMeter){m->recognized_ms, m->synthesized_chars, m->free_ms_left, m->free_chars_left, m->owed_msat};
}
static oa_voice_meter_report meter_out(const coax_t_WMeter *m) {
  return (oa_voice_meter_report){m->recognized_ms, m->synthesized_chars, m->free_ms_left, m->free_chars_left, m->owed_msat};
}

/* WMsg holds every body side by side (Coax has no unions yet): ~1 KiB. */
static void msg_in(const oa_voice_msg *m, coax_t_WMsg *w) {
  memset(w, 0, sizeof *w);
  w->kind = m->kind;
  switch (m->kind) {
  case OA_MSG_HELLO: {
    const oa_voice_hello *h = &m->u.hello;
    w->hello = (coax_t_WHello){.protocol = h->protocol, .client = (int32_t)h->client, .has_speaker = h->has_speaker,
      .speaker = speaker_in(&h->speaker), .route = cs(h->route), .input_sample_rate = h->input_sample_rate,
      .output_sample_rate = h->output_sample_rate, .codec = (int32_t)h->codec, .profile = cs(h->profile),
      .token = cs(h->token), .observe = h->observe};
    break;
  }
  case OA_MSG_WELCOME: {
    const oa_voice_welcome *x = &m->u.welcome;
    w->welcome = (coax_t_WWelcome){.protocol = x->protocol, .session = cs(x->session),
      .input_sample_rate = x->input_sample_rate, .output_sample_rate = x->output_sample_rate,
      .provider = cs(x->provider), .has_meter = x->has_meter, .meter = meter_in(&x->meter),
      .max_turn_ms = x->max_turn_ms};
    break;
  }
  case OA_MSG_AUDIO: {
    const oa_voice_audio *a = &m->u.audio;
    w->audio = (coax_t_WAudio){.turn = cs(a->turn), .sequence = a->sequence, .sample_rate = a->sample_rate,
      .codec = (int32_t)a->codec, .data = {(uint8_t *)a->data.ptr, a->data.ptr ? (int64_t)a->data.len : 0},
      .media_epoch = a->media_epoch, .utterance = cs(a->utterance)};
    break;
  }
  case OA_MSG_CONTROL: {
    const oa_voice_control *c = &m->u.control;
    w->control = (coax_t_WControl){.kind = (int32_t)c->kind, .turn = cs(c->turn), .played_samples = c->played_samples,
      .media_epoch = c->media_epoch, .utterance = cs(c->utterance), .at_ms = c->at_ms};
    break;
  }
  case OA_MSG_TURN_EVENT: {
    const oa_voice_turn_event *t = &m->u.turn_event;
    w->turn_event = (coax_t_WTurnEvent){.turn = cs(t->turn), .speaker = cs(t->speaker), .phase = (int32_t)t->phase,
      .text = cs(t->text), .played_samples = t->played_samples, .media_epoch = t->media_epoch};
    break;
  }
  case OA_MSG_SPEECH: {
    const oa_voice_speech *s = &m->u.speech;
    const oa_voice_utterance *u = &s->utterance;
    w->speech = (coax_t_WSpeech){.has_utterance = s->has_utterance,
      .utterance = {.id = cs(u->id), .speaker = cs(u->speaker), .text = cs(u->text), .priority = u->priority,
                    .interruptible = u->interruptible, .spatial = u->spatial, .batch = u->batch},
      .has_speaker = s->has_speaker, .speaker = speaker_in(&s->speaker), .sample_rate = s->sample_rate,
      .media_epoch = s->media_epoch, .turn = cs(s->turn)};
    break;
  }
  case OA_MSG_ROUTE:
    w->route = (coax_t_WRoute){cs(m->u.route.turn), cs(m->u.route.target), cs(m->u.route.reason)};
    break;
  case OA_MSG_METER: w->meter = meter_in(&m->u.meter); break;
  case OA_MSG_ERROR:
    w->error = (coax_t_WError){(int32_t)m->u.error.code, cs(m->u.error.message), cs(m->u.error.turn)};
    break;
  case OA_MSG_FEATURES: {
    const oa_voice_features *f = &m->u.features;
    w->features = (coax_t_WFeatures){.stream = (int32_t)f->stream, .sequence = f->sequence, .media_epoch = f->media_epoch,
      .rms_db = f->rms_db, .peak_db = f->peak_db, .speech = f->speech, .noise_db = f->noise_db,
      .turn_state = (int32_t)f->turn_state, .playback_state = (int32_t)f->playback_state,
      .speech_ms = f->speech_ms, .silence_ms = f->silence_ms, .zero_crossings = f->zero_crossings};
    break;
  }
  case OA_MSG_DECISION: {
    const oa_voice_decision *d = &m->u.decision;
    w->decision = (coax_t_WDecision){.has = d->has, .stream = (int32_t)d->stream, .apply_from = d->apply_from_sequence,
      .epoch = d->media_epoch, .action = (int32_t)d->action, .gain_db = d->gain_db, .duck_db = d->duck_db,
      .rate = d->playback_rate, .hold_ms = d->hold_ms, .confidence = d->confidence, .decider = cs(d->decider),
      .deadline = d->deadline_sequence};
    break;
  }
  default: break;
  }
}

static void msg_out(const coax_t_WMsg *w, oa_voice_msg *m) {
  memset(m, 0, sizeof *m);
  m->kind = w->kind;
  switch (w->kind) {
  case OA_MSG_HELLO: {
    const coax_t_WHello *h = &w->hello;
    m->u.hello = (oa_voice_hello){.protocol = h->protocol, .client = (uint32_t)h->client, .has_speaker = h->has_speaker,
      .speaker = speaker_out(&h->speaker), .route = ob(h->route), .input_sample_rate = h->input_sample_rate,
      .output_sample_rate = h->output_sample_rate, .codec = (uint32_t)h->codec, .profile = ob(h->profile),
      .token = ob(h->token), .observe = h->observe};
    break;
  }
  case OA_MSG_WELCOME: {
    const coax_t_WWelcome *x = &w->welcome;
    m->u.welcome = (oa_voice_welcome){.protocol = x->protocol, .session = ob(x->session),
      .input_sample_rate = x->input_sample_rate, .output_sample_rate = x->output_sample_rate,
      .provider = ob(x->provider), .has_meter = x->has_meter, .meter = meter_out(&x->meter),
      .max_turn_ms = x->max_turn_ms};
    break;
  }
  case OA_MSG_AUDIO: {
    const coax_t_WAudio *a = &w->audio;
    m->u.audio = (oa_voice_audio){.turn = ob(a->turn), .sequence = a->sequence, .sample_rate = a->sample_rate,
      .codec = (uint32_t)a->codec, .data = {a->data.count > 0 ? a->data.data : NULL, (uint32_t)a->data.count},
      .media_epoch = a->media_epoch, .utterance = ob(a->utterance)};
    break;
  }
  case OA_MSG_CONTROL: {
    const coax_t_WControl *c = &w->control;
    m->u.control = (oa_voice_control){.kind = (uint32_t)c->kind, .turn = ob(c->turn), .played_samples = c->played_samples,
      .media_epoch = c->media_epoch, .utterance = ob(c->utterance), .at_ms = c->at_ms};
    break;
  }
  case OA_MSG_TURN_EVENT: {
    const coax_t_WTurnEvent *t = &w->turn_event;
    m->u.turn_event = (oa_voice_turn_event){.turn = ob(t->turn), .speaker = ob(t->speaker), .phase = (uint32_t)t->phase,
      .text = ob(t->text), .played_samples = t->played_samples, .media_epoch = t->media_epoch};
    break;
  }
  case OA_MSG_SPEECH: {
    const coax_t_WSpeech *s = &w->speech;
    const coax_t_WUtterance *u = &s->utterance;
    m->u.speech = (oa_voice_speech){.has_utterance = s->has_utterance,
      .utterance = {.id = ob(u->id), .speaker = ob(u->speaker), .text = ob(u->text), .priority = u->priority,
                    .interruptible = u->interruptible, .spatial = u->spatial, .batch = u->batch},
      .has_speaker = s->has_speaker, .speaker = speaker_out(&s->speaker), .sample_rate = s->sample_rate,
      .media_epoch = s->media_epoch, .turn = ob(s->turn)};
    break;
  }
  case OA_MSG_ROUTE:
    m->u.route = (oa_voice_route){ob(w->route.turn), ob(w->route.target), ob(w->route.reason)};
    break;
  case OA_MSG_METER: m->u.meter = meter_out(&w->meter); break;
  case OA_MSG_ERROR:
    m->u.error = (oa_voice_error){(uint32_t)w->error.code, ob(w->error.message), ob(w->error.turn)};
    break;
  case OA_MSG_FEATURES: {
    const coax_t_WFeatures *f = &w->features;
    m->u.features = (oa_voice_features){.stream = (uint32_t)f->stream, .sequence = f->sequence,
      .media_epoch = f->media_epoch, .rms_db = f->rms_db, .peak_db = f->peak_db, .speech = f->speech,
      .noise_db = f->noise_db, .turn_state = (uint32_t)f->turn_state, .playback_state = (uint32_t)f->playback_state,
      .speech_ms = f->speech_ms, .silence_ms = f->silence_ms, .zero_crossings = f->zero_crossings};
    break;
  }
  case OA_MSG_DECISION: {
    const coax_t_WDecision *d = &w->decision;
    m->u.decision = (oa_voice_decision){.has = d->has, .stream = (uint32_t)d->stream,
      .apply_from_sequence = d->apply_from, .media_epoch = d->epoch, .action = (uint32_t)d->action,
      .gain_db = d->gain_db, .duck_db = d->duck_db, .playback_rate = d->rate, .hold_ms = d->hold_ms,
      .confidence = d->confidence, .decider = ob(d->decider), .deadline_sequence = d->deadline};
    break;
  }
  default: break;
  }
}

size_t oa_voice_encode(uint8_t *out, size_t cap, const oa_voice_msg *m) {
  if (!out || !m) return 0;
  if (cap > OA_VOICE_MAX_FRAME_BYTES) cap = OA_VOICE_MAX_FRAME_BYTES;
  coax_t_WMsg w;
  msg_in(m, &w);
  int64_t n = metareal_voice_wire_encode((coax_v_t_WMsg){&w, 1}, (coax_v_u8){out, (int64_t)cap});
  return n > 0 ? (size_t)n : 0;
}

bool oa_voice_decode(const uint8_t *in, size_t n, oa_voice_msg *m) {
  if (!m) return false;
  memset(m, 0, sizeof *m);
  if (!in || n == 0 || n > OA_VOICE_MAX_FRAME_BYTES) return false;
  coax_t_WMsg w;
  if (!metareal_voice_wire_decode((coax_v_u8){(uint8_t *)in, (int64_t)n}, (coax_v_t_WMsg){&w, 1})) return false;
  msg_out(&w, m);
  return true;
}

/* ------------------------------------------------------------ floor */

size_t oa_voice_floor_size(void) { return sizeof(struct oa_voice_floor); }
void oa_voice_floor_init(oa_voice_floor *fl) { memset(fl, 0, sizeof *fl); metareal_voice_floor_init(sf(fl)); }
bool oa_voice_floor_consent(oa_voice_floor *fl, uint32_t slot, bool consent) { return metareal_voice_floor_consent(sf(fl), slot, consent); }
bool oa_voice_floor_bind(oa_voice_floor *fl, uint32_t slot, uint32_t target) { return metareal_voice_floor_bind(sf(fl), slot, target); }
bool oa_voice_floor_grant(oa_voice_floor *fl, int32_t slot) { return metareal_voice_floor_grant(sf(fl), slot); }
bool oa_voice_floor_may_hear(const oa_voice_floor *fl, uint32_t slot) { return metareal_voice_floor_may_hear(sf(fl), slot); }
uint32_t oa_voice_floor_route(const oa_voice_floor *fl, uint32_t slot) { return metareal_voice_floor_route(sf(fl), slot); }
uint64_t oa_voice_floor_epoch(const oa_voice_floor *fl) { return metareal_voice_floor_epoch(sf(fl)); }
uint64_t oa_voice_floor_bump(oa_voice_floor *fl) { return metareal_voice_floor_bump(sf(fl)); }
bool oa_voice_floor_epoch_ok(const oa_voice_floor *fl, uint64_t epoch) { return metareal_voice_floor_epoch_ok(sf(fl), epoch); }

/* ------------------------------------------------------------ meter */

size_t oa_voice_meter_size(void) { return sizeof(struct oa_voice_meter); }
void oa_voice_meter_init(oa_voice_meter *m, uint32_t day, uint64_t budget_msat) {
  memset(m, 0, sizeof *m);
  metareal_voice_meter_init(sm(m), day, budget_msat);
}
void oa_voice_meter_day(oa_voice_meter *m, uint32_t day) { metareal_voice_meter_day(sm(m), day); }
void oa_voice_meter_budget(oa_voice_meter *m, uint64_t budget_msat) { metareal_voice_meter_budget(sm(m), budget_msat); }
bool oa_voice_meter_may_run(const oa_voice_meter *m, uint64_t max_ms, uint64_t max_chars) {
  return metareal_voice_meter_may_run(sm(m), max_ms, max_chars);
}
bool oa_voice_meter_charge(oa_voice_meter *m, uint64_t recognized_ms, uint64_t synthesized_chars) {
  return metareal_voice_meter_charge(sm(m), recognized_ms, synthesized_chars);
}
void oa_voice_meter_report_get(const oa_voice_meter *m, oa_voice_meter_report *out) {
  coax_t_MeterReport r = metareal_voice_meter_report(sm(m));
  *out = (oa_voice_meter_report){r.recognized_ms, r.synthesized_chars, r.free_ms_left, r.free_chars_left, r.owed_msat};
}
