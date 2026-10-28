/* The voice engine through its C face (verse/platform/voice.h), on
 * synthetic frames and optionally a WAV. Run by verse/test_voice.sh.
 *   voice_smoke [--frames DIR] [file.wav ...]
 * --frames DIR writes one encoded Frame per kind to DIR/<kind>.bin so the
 * script can check them with protoc, an independent decoder. */
#include "voice.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

static uint32_t rng = 2463534242u;
static float noise(void) {
  rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
  return (float)rng / 2147483648.0f - 1.0f;
}
static int16_t s16(float v) { return (int16_t)(v > 32767 ? 32767 : v < -32768 ? -32768 : lrintf(v)); }

static oa_voice *fresh(uint32_t rate) {
  oa_voice *v = calloc(1, oa_voice_size());
  oa_voice_init(v, rate);
  return v;
}

typedef struct { int starts, ends, forwards, barges, decisions, too_long; uint32_t ev; int start_frame, end_frame; } Tally;

/* kind: 0 faint noise, 1 300 Hz tone at -20 dBFS, 2 speech-like noise. */
static void feed(oa_voice *v, uint32_t rate, int kind, int frames, Tally *t, long *clock_ns) {
  uint32_t n = oa_voice_input_frame_samples(v);
  int16_t pcm[OA_VOICE_MAX_INPUT_SAMPLES], out[OA_VOICE_FRAME_SAMPLES];
  static double phase, lp;
  for (int k = 0; k < frames; k++) {
    for (uint32_t i = 0; i < n; i++) {
      float x = 0;
      if (kind == 0) x = noise() * 30;
      if (kind == 1) { x = 3276.8f * (float)sin(phase); phase += 2 * M_PI * 300.0 / rate; }
      if (kind == 2) {
        lp += (noise() - lp) * 0.25 * 16000.0 / rate;
        phase += 2 * M_PI * 4.0 / rate;
        x = (float)(lp * 9000 * (0.6 + 0.4 * sin(phase)));
      }
      pcm[i] = s16(x);
    }
    oa_voice_features f;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    uint32_t ev = oa_voice_uplink(v, pcm, out, &f);
    clock_gettime(CLOCK_MONOTONIC, &b);
    if (clock_ns) *clock_ns += (b.tv_sec - a.tv_sec) * 1000000000L + (b.tv_nsec - a.tv_nsec);
    CHECK(f.stream == OA_STREAM_UPLINK && f.speech >= 0 && f.speech <= 1 && f.rms_db >= -120 && f.rms_db <= 0);
    t->ev |= ev;
    if (ev & OA_EV_TURN_START) { t->starts++; t->start_frame = (int)f.sequence; }
    if (ev & OA_EV_TURN_END) { t->ends++; t->end_frame = (int)f.sequence; }
    if (ev & OA_EV_FORWARD) t->forwards++;
    if (ev & OA_EV_BARGE_IN) t->barges++;
    if (ev & OA_EV_DECISION) t->decisions++;
    if (ev & OA_EV_TOO_LONG) t->too_long++;
  }
}

static void turns_at(uint32_t rate) {
  oa_voice *v = fresh(rate);
  Tally t = {0};
  long ns = 0;
  feed(v, rate, 0, 50, &t, &ns);
  CHECK(t.starts == 0 && t.forwards == 0);
  feed(v, rate, 1, 30, &t, &ns);
  CHECK(t.starts == 1 && t.ends == 0);
  int16_t pre[OA_VOICE_PREROLL_SAMPLES];
  uint32_t got = oa_voice_preroll(v, pre, OA_VOICE_PREROLL_SAMPLES);
  CHECK(got > 0 && got % OA_VOICE_FRAME_SAMPLES == 0);
  feed(v, rate, 0, 50, &t, &ns);
  CHECK(t.starts == 1 && t.ends == 1);
  CHECK(oa_voice_current_turn_state(v) == OA_TURN_STATE_IDLE);
  printf("  %5u Hz tone burst: turn frames %d..%d, %d forwarded, pre-roll %u samples, %.1f us/frame\n",
         rate, t.start_frame, t.end_frame, t.forwards, got, ns / 1000.0 / 130);
  Tally s = {0};
  feed(v, rate, 2, 40, &s, NULL);
  feed(v, rate, 0, 60, &s, NULL);
  CHECK(s.starts == 1 && s.ends == 1);
  printf("  %5u Hz speech-like noise: turn frames %d..%d\n", rate, s.start_frame, s.end_frame);
  free(v);
}

static void barge_in(void) {
  oa_voice *v = fresh(48000);
  Tally t = {0};
  feed(v, 48000, 0, 40, &t, NULL);
  oa_voice_playing(v, true);
  CHECK(oa_voice_current_playback_state(v) == OA_PLAYBACK_STATE_PLAYING);
  uint64_t e0 = oa_voice_epoch(v);
  feed(v, 48000, 1, 30, &t, NULL);
  CHECK(t.barges == 1 && oa_voice_epoch(v) == e0 + 1 && (t.ev & OA_EV_EPOCH));
  oa_voice_decision d;
  CHECK(oa_voice_last_decision(v, &d));
  CHECK(d.action == OA_FRAME_ACTION_BARGE_IN && d.stream == OA_STREAM_DOWNLINK && d.decider.len == 10 &&
        memcmp(d.decider.ptr, "voice.coax", 10) == 0);
  CHECK(oa_voice_current_playback_state(v) == OA_PLAYBACK_STATE_SILENT);
  printf("  barge-in: epoch %llu -> %llu, %d decisions\n", (unsigned long long)e0,
         (unsigned long long)oa_voice_epoch(v), t.decisions);
  free(v);
}

static void decisions(void) {
  oa_voice *v = fresh(16000);
  int16_t buf[800];
  oa_voice_decision d = {0};
  d.stream = OA_STREAM_DOWNLINK;
  d.action = OA_FRAME_ACTION_DUCK;
  d.has = OA_DECISION_HAS_DUCK | OA_DECISION_HAS_CONFIDENCE;
  d.duck_db = 99;  /* clamped to 40 */
  d.confidence = 0.9f;
  CHECK(oa_voice_decide(v, &d));
  for (int i = 0; i < 800; i++) buf[i] = 10000;
  oa_voice_features f;
  oa_voice_downlink(v, buf, 800, 16000, &f);
  CHECK(oa_voice_gain_db(v) == -40.0f && buf[799] == 100 && f.playback_state == OA_PLAYBACK_STATE_DUCKED);
  d.has = OA_DECISION_HAS_EPOCH; d.media_epoch = 99;
  CHECK(!oa_voice_decide(v, &d));                 /* stale epoch */
  d.has = OA_DECISION_HAS_DEADLINE; d.deadline_sequence = 0;
  CHECK(!oa_voice_decide(v, &d));                 /* downlink is at 1: too late */
  d.has = OA_DECISION_HAS_CONFIDENCE; d.confidence = 0.1f;
  CHECK(!oa_voice_decide(v, &d));                 /* under the floor */
  d.has = 0; d.stream = OA_STREAM_UPLINK;
  CHECK(!oa_voice_decide(v, &d));                 /* DUCK is not an uplink action */
  d.stream = OA_STREAM_DOWNLINK; d.action = OA_FRAME_ACTION_RESUME;
  d.has = OA_DECISION_HAS_GAIN | OA_DECISION_HAS_RATE; d.gain_db = -6; d.playback_rate = 2;
  CHECK(oa_voice_decide(v, &d));
  oa_voice_downlink(v, NULL, 0, 16000, NULL);
  CHECK(oa_voice_gain_db(v) == -6.0f && oa_voice_playback_rate(v) == 1.25f);
  CHECK(!oa_voice_decide(v, NULL));
  /* Configure clamps and keeps zeros. */
  oa_voice_config c = {0};
  c.end_silence_ms = 1;
  oa_voice_configure(v, &c);
  oa_voice_get_config(v, &c);
  CHECK(c.end_silence_ms == 200 && c.min_speech_ms == 160 && c.max_turn_ms == 30000 && c.jitter_target == 2);
  printf("  decisions: clamps, epoch, deadline, floor, stream rules hold\n");
  free(v);
}

static void jitter(void) {
  oa_voice *v = fresh(16000);
  int16_t p[480], o[480];
  for (int k = 0; k < 4; k++) {
    uint32_t seq = (uint32_t)(k ^ 1);  /* 1, 0, 3, 2 */
    for (int i = 0; i < 480; i++) p[i] = (int16_t)(1000 * (seq + 1));
    CHECK(oa_voice_jitter_push(v, seq, 1, p, 480));
  }
  CHECK(!oa_voice_jitter_push(v, 2, 1, p, 480));   /* duplicate */
  CHECK(!oa_voice_jitter_push(v, 9, 0 + 1, p, OA_VOICE_MAX_DOWNLINK_SAMPLES + 1));
  CHECK(oa_voice_jitter_depth(v) == 4);
  for (int k = 0; k < 4; k++) {
    CHECK(oa_voice_jitter_pop(v, o, 480) == 0);
    CHECK(o[0] == 1000 * (k + 1) && o[479] == 1000 * (k + 1));
  }
  CHECK(oa_voice_jitter_pop(v, o, 480) == OA_EV_UNDERRUN);
  CHECK(oa_voice_set_epoch(v, 5) == 5 && oa_voice_cut(v) == 6);
  CHECK(!oa_voice_jitter_push(v, 0, 5, p, 480));
  printf("  jitter buffer: reorder, duplicate, underrun, epoch fencing\n");
  free(v);
}

/* ---------------------------------------------------------------- wire */

static oa_voice_bytes B(const char *s) { return (oa_voice_bytes){(const uint8_t *)s, (uint32_t)strlen(s)}; }
static int beq(oa_voice_bytes a, const char *s) { return a.len == strlen(s) && (a.len == 0 || memcmp(a.ptr, s, a.len) == 0); }

static void sample(uint32_t kind, oa_voice_msg *m, uint8_t *audio) {
  memset(m, 0, sizeof *m);
  m->kind = kind;
  oa_voice_speaker sp = {B("npub1player"), OA_SPEAKER_ROLE_PLAYER, B("charon"), 2, B("entity:7")};
  switch (kind) {
  case OA_MSG_HELLO:
    m->u.hello = (oa_voice_hello){OA_VOICE_PROTOCOL, OA_CLIENT_IOS, true, sp, B("agent:foreman"), 48000, 24000,
                                  OA_CODEC_PCM_S16LE, B("charon"), B("session-token"), true};
    break;
  case OA_MSG_WELCOME:
    m->u.welcome = (oa_voice_welcome){OA_VOICE_PROTOCOL, B("s-1"), 16000, 24000, B("local"), true,
                                      {1200, 34, 598800, 4966, 0}, 30000};
    break;
  case OA_MSG_AUDIO:
    for (int i = 0; i < OA_VOICE_MAX_AUDIO_BYTES; i++) audio[i] = (uint8_t)(i * 7);
    m->u.audio = (oa_voice_audio){B("turn-1"), 42, 24000, OA_CODEC_PCM_S16LE, {audio, OA_VOICE_MAX_AUDIO_BYTES}, 3, B("utt-1")};
    break;
  case OA_MSG_CONTROL:
    m->u.control = (oa_voice_control){OA_CONTROL_PLAYBACK, B("turn-1"), 48000, 3, B("utt-1"), 1234567};
    break;
  case OA_MSG_TURN_EVENT:
    m->u.turn_event = (oa_voice_turn_event){B("turn-1"), B("npub1player"), OA_TURN_PHASE_FINAL, B("what's the weather"), 7, 3};
    break;
  case OA_MSG_SPEECH:
    m->u.speech = (oa_voice_speech){true, {B("utt-1"), B("agent:foreman"), B("You said: what's the weather"), 2, true, false, true},
                                    true, {B("agent:foreman"), OA_SPEAKER_ROLE_AGENT, B("charon"), 1, B("")}, 24000, 3, B("turn-1")};
    break;
  case OA_MSG_ROUTE: m->u.route = (oa_voice_route){B("turn-1"), B("agent:foreman"), B("floor")}; break;
  case OA_MSG_METER: m->u.meter = (oa_voice_meter_report){600001, 5001, 0, 0, 31}; break;
  case OA_MSG_ERROR: m->u.error = (oa_voice_error){OA_ERROR_LIMIT, B("free tier used up"), B("turn-1")}; break;
  case OA_MSG_FEATURES:
    m->u.features = (oa_voice_features){OA_STREAM_UPLINK, 77, 3, -31.5f, -12.25f, 0.75f, -64.0f, OA_TURN_STATE_TRAILING,
                                        OA_PLAYBACK_STATE_DUCKED, 820, 140, 0.125f};
    break;
  case OA_MSG_DECISION:
    m->u.decision = (oa_voice_decision){0xff, OA_STREAM_DOWNLINK, 0, 0, OA_FRAME_ACTION_DUCK, 0.0f, 12.0f, 1.1f, 0, 0.9f,
                                        B("voice.coax"), 12};
    break;
  }
}

static void wire(const char *dir) {
  static uint8_t audio[OA_VOICE_MAX_AUDIO_BYTES], buf[OA_VOICE_MAX_FRAME_BYTES];
  oa_voice_msg m, d;
  for (uint32_t kind = 1; kind <= 11; kind++) {
    sample(kind, &m, audio);
    size_t n = oa_voice_encode(buf, sizeof buf, &m);
    CHECK(n > 0);
    CHECK(oa_voice_decode(buf, n, &d));
    CHECK(d.kind == kind);
    switch (kind) {
    case OA_MSG_HELLO: {
      oa_voice_hello *h = &d.u.hello;
      CHECK(h->protocol == 1 && h->client == OA_CLIENT_IOS && h->has_speaker && beq(h->speaker.id, "npub1player"));
      CHECK(h->speaker.role == OA_SPEAKER_ROLE_PLAYER && beq(h->speaker.profile, "charon") && h->speaker.profile_revision == 2);
      CHECK(beq(h->speaker.entity, "entity:7") && beq(h->route, "agent:foreman") && h->input_sample_rate == 48000);
      CHECK(h->output_sample_rate == 24000 && h->codec == 1 && beq(h->profile, "charon") && beq(h->token, "session-token") && h->observe);
      break;
    }
    case OA_MSG_WELCOME: {
      oa_voice_welcome *w = &d.u.welcome;
      CHECK(w->protocol == 1 && beq(w->session, "s-1") && w->input_sample_rate == 16000 && w->output_sample_rate == 24000);
      CHECK(beq(w->provider, "local") && w->has_meter && w->meter.recognized_ms == 1200 && w->meter.synthesized_chars == 34);
      CHECK(w->meter.free_ms_left == 598800 && w->meter.free_chars_left == 4966 && w->max_turn_ms == 30000);
      break;
    }
    case OA_MSG_AUDIO: {
      oa_voice_audio *a = &d.u.audio;
      CHECK(beq(a->turn, "turn-1") && a->sequence == 42 && a->sample_rate == 24000 && a->codec == 1 && a->media_epoch == 3);
      CHECK(beq(a->utterance, "utt-1") && a->data.len == OA_VOICE_MAX_AUDIO_BYTES && memcmp(a->data.ptr, audio, a->data.len) == 0);
      CHECK(a->data.ptr >= buf && a->data.ptr < buf + n);  /* borrowed from the input */
      break;
    }
    case OA_MSG_CONTROL: {
      oa_voice_control *c = &d.u.control;
      CHECK(c->kind == OA_CONTROL_PLAYBACK && beq(c->turn, "turn-1") && c->played_samples == 48000 && c->media_epoch == 3);
      CHECK(beq(c->utterance, "utt-1") && c->at_ms == 1234567);
      break;
    }
    case OA_MSG_TURN_EVENT: {
      oa_voice_turn_event *t = &d.u.turn_event;
      CHECK(beq(t->turn, "turn-1") && beq(t->speaker, "npub1player") && t->phase == OA_TURN_PHASE_FINAL);
      CHECK(beq(t->text, "what's the weather") && t->played_samples == 7 && t->media_epoch == 3);
      break;
    }
    case OA_MSG_SPEECH: {
      oa_voice_speech *s = &d.u.speech;
      CHECK(s->has_utterance && beq(s->utterance.id, "utt-1") && beq(s->utterance.speaker, "agent:foreman"));
      CHECK(beq(s->utterance.text, "You said: what's the weather") && s->utterance.priority == 2);
      CHECK(s->utterance.interruptible && !s->utterance.spatial && s->utterance.batch);
      CHECK(s->has_speaker && beq(s->speaker.id, "agent:foreman") && s->speaker.role == OA_SPEAKER_ROLE_AGENT);
      CHECK(s->speaker.profile_revision == 1 && s->speaker.entity.len == 0 && s->sample_rate == 24000);
      CHECK(s->media_epoch == 3 && beq(s->turn, "turn-1"));
      break;
    }
    case OA_MSG_ROUTE:
      CHECK(beq(d.u.route.turn, "turn-1") && beq(d.u.route.target, "agent:foreman") && beq(d.u.route.reason, "floor"));
      break;
    case OA_MSG_METER:
      CHECK(d.u.meter.recognized_ms == 600001 && d.u.meter.synthesized_chars == 5001 && d.u.meter.owed_msat == 31);
      CHECK(d.u.meter.free_ms_left == 0 && d.u.meter.free_chars_left == 0);
      break;
    case OA_MSG_ERROR:
      CHECK(d.u.error.code == OA_ERROR_LIMIT && beq(d.u.error.message, "free tier used up") && beq(d.u.error.turn, "turn-1"));
      break;
    case OA_MSG_FEATURES: {
      oa_voice_features *f = &d.u.features;
      CHECK(f->stream == 1 && f->sequence == 77 && f->media_epoch == 3 && f->rms_db == -31.5f && f->peak_db == -12.25f);
      CHECK(f->speech == 0.75f && f->noise_db == -64.0f && f->turn_state == 3 && f->playback_state == 3);
      CHECK(f->speech_ms == 820 && f->silence_ms == 140 && f->zero_crossings == 0.125f);
      break;
    }
    case OA_MSG_DECISION: {
      oa_voice_decision *x = &d.u.decision;
      CHECK(x->has == 0xff && x->stream == 2 && x->apply_from_sequence == 0 && x->media_epoch == 0 && x->action == 4);
      CHECK(x->gain_db == 0.0f && x->duck_db == 12.0f && x->playback_rate == 1.1f && x->hold_ms == 0);
      CHECK(x->confidence == 0.9f && beq(x->decider, "voice.coax") && x->deadline_sequence == 12);
      break;
    }
    }
    if (dir) {
      char path[1024];
      snprintf(path, sizeof path, "%s/%u.bin", dir, kind);
      FILE *fp = fopen(path, "wb");
      CHECK(fp != NULL);
      if (fp) { fwrite(buf, 1, n, fp); fclose(fp); }
    }
    printf("  frame kind %2u: %5zu bytes, round trip ok\n", kind, n);
  }
  /* Refusals. */
  sample(OA_MSG_AUDIO, &m, audio);
  m.u.audio.data.len = OA_VOICE_MAX_AUDIO_BYTES + 1;
  static uint8_t big[OA_VOICE_MAX_AUDIO_BYTES + 1];
  m.u.audio.data.ptr = big;
  CHECK(oa_voice_encode(buf, sizeof buf, &m) == 0);
  sample(OA_MSG_TURN_EVENT, &m, audio);
  CHECK(oa_voice_encode(buf, 10, &m) == 0);              /* no room */
  m.kind = 12;
  CHECK(oa_voice_encode(buf, sizeof buf, &m) == 0);      /* unknown kind */
  m.kind = OA_MSG_TURN_EVENT;
  m.u.turn_event.text = (oa_voice_bytes){(const uint8_t *)"\xc3(", 2};
  CHECK(oa_voice_encode(buf, sizeof buf, &m) == 0);      /* invalid UTF-8 */
  const uint8_t junk[] = {0xff, 0xff, 0xff}, two[] = {34, 2, 8, 6, 58, 0}, ping[] = {34, 2, 8, 6};
  CHECK(!oa_voice_decode(junk, sizeof junk, &d) && d.kind == 0);
  CHECK(!oa_voice_decode(two, sizeof two, &d));          /* two bodies */
  CHECK(!oa_voice_decode(ping, 0, &d));
  CHECK(oa_voice_decode(ping, sizeof ping, &d) && d.kind == OA_MSG_CONTROL && d.u.control.kind == OA_CONTROL_PING);
  /* Every byte flipped in a frame either decodes or is refused, never crashes. */
  sample(OA_MSG_SPEECH, &m, audio);
  size_t n = oa_voice_encode(buf, sizeof buf, &m);
  int refused = 0;
  for (size_t i = 0; i < n; i++) {
    uint8_t keep = buf[i];
    for (int bit = 0; bit < 8; bit++) {
      buf[i] = (uint8_t)(keep ^ (1u << bit));
      if (!oa_voice_decode(buf, n, &d)) refused++;
    }
    buf[i] = keep;
  }
  printf("  wire: refusals hold; %d of %zu one-bit corruptions refused, the rest decoded safely\n", refused, n * 8);
}

static void floor_and_meter(void) {
  oa_voice_floor *fl = calloc(1, oa_voice_floor_size());
  oa_voice_floor_init(fl);
  CHECK(!oa_voice_floor_may_hear(fl, 0) && !oa_voice_floor_bind(fl, 0, 3) && !oa_voice_floor_grant(fl, 0));
  CHECK(oa_voice_floor_consent(fl, 0, true) && oa_voice_floor_bind(fl, 0, 3) && oa_voice_floor_grant(fl, 0));
  CHECK(oa_voice_floor_may_hear(fl, 0) && oa_voice_floor_route(fl, 0) == 3 && !oa_voice_floor_may_hear(fl, 1));
  CHECK(oa_voice_floor_epoch(fl) == 1 && oa_voice_floor_bump(fl) == 2 && oa_voice_floor_epoch_ok(fl, 2) && !oa_voice_floor_epoch_ok(fl, 1));
  CHECK(oa_voice_floor_consent(fl, 0, false) && !oa_voice_floor_may_hear(fl, 0) && oa_voice_floor_route(fl, 0) == 0);
  CHECK(!oa_voice_floor_consent(fl, OA_VOICE_FLOOR_SLOTS, true));
  free(fl);
  oa_voice_meter *m = calloc(1, oa_voice_meter_size());
  oa_voice_meter_init(m, 20000, 0);
  CHECK(oa_voice_meter_may_run(m, OA_VOICE_FREE_MS_PER_DAY, OA_VOICE_FREE_CHARS_PER_DAY));
  CHECK(!oa_voice_meter_may_run(m, OA_VOICE_FREE_MS_PER_DAY + 1, 0));
  CHECK(oa_voice_meter_charge(m, OA_VOICE_FREE_MS_PER_DAY, 0));
  CHECK(!oa_voice_meter_charge(m, 10, 1 + OA_VOICE_FREE_CHARS_PER_DAY));
  oa_voice_meter_report r;
  oa_voice_meter_report_get(m, &r);
  CHECK(r.owed_msat == 10 * OA_VOICE_MSAT_PER_MS + 1 * OA_VOICE_MSAT_PER_CHAR && r.free_ms_left == 0);
  oa_voice_meter_budget(m, r.owed_msat);
  CHECK(oa_voice_meter_may_run(m, 0, 0) && !oa_voice_meter_may_run(m, 1, 0));
  oa_voice_meter_day(m, 20001);
  oa_voice_meter_report_get(m, &r);
  CHECK(r.recognized_ms == 0 && r.free_ms_left == OA_VOICE_FREE_MS_PER_DAY && r.free_chars_left == OA_VOICE_FREE_CHARS_PER_DAY);
  free(m);
  printf("  floor and meter: consent, binding, epochs, free tier, prices\n");
}

/* ---------------------------------------------------------------- WAV */

static void wav(const char *path) {
  FILE *fp = fopen(path, "rb");
  if (!fp) { fprintf(stderr, "cannot open %s\n", path); failures++; return; }
  fseek(fp, 0, SEEK_END);
  long size = ftell(fp);
  fseek(fp, 0, SEEK_SET);
  uint8_t *b = malloc((size_t)size);
  if (fread(b, 1, (size_t)size, fp) != (size_t)size) size = 0;
  fclose(fp);
  uint32_t rate = 0, channels = 0, bits = 0;
  const uint8_t *data = NULL;
  uint32_t data_len = 0;
  for (long p = 12; size >= 44 && p + 8 <= size;) {
    uint32_t len = (uint32_t)(b[p + 4] | b[p + 5] << 8 | b[p + 6] << 16 | (uint32_t)b[p + 7] << 24);
    if (!memcmp(b + p, "fmt ", 4) && p + 24 <= size) {
      channels = (uint32_t)(b[p + 10] | b[p + 11] << 8);
      rate = (uint32_t)(b[p + 12] | b[p + 13] << 8 | b[p + 14] << 16 | (uint32_t)b[p + 15] << 24);
      bits = (uint32_t)(b[p + 22] | b[p + 23] << 8);
    }
    if (!memcmp(b + p, "data", 4)) { data = b + p + 8; data_len = (uint32_t)(len <= size - p - 8 ? len : size - p - 8); break; }
    p += 8 + len + (len & 1);
  }
  if (!data || channels != 1 || bits != 16 || (rate != 16000 && rate != 24000 && rate != 44100 && rate != 48000)) {
    fprintf(stderr, "%s: need 16-bit mono WAV at 16/24/44.1/48 kHz\n", path);
    failures++;
    free(b);
    return;
  }
  oa_voice *v = fresh(rate);
  uint32_t n = oa_voice_input_frame_samples(v), frames = data_len / 2 / n, turns = 0;
  int16_t pcm[OA_VOICE_MAX_INPUT_SAMPLES];
  printf("  %s: %u Hz, %.2f s\n", path, rate, frames * 0.02);
  for (uint32_t k = 0; k < frames; k++) {
    memcpy(pcm, data + (size_t)k * n * 2, n * 2);
    oa_voice_features f;
    uint32_t ev = oa_voice_uplink(v, pcm, NULL, &f);
    if (ev & OA_EV_TURN_START) {
      int16_t pre[OA_VOICE_PREROLL_SAMPLES];
      uint32_t got = oa_voice_preroll(v, pre, OA_VOICE_PREROLL_SAMPLES);
      printf("    %6.2f s  turn start (speech %.2f, %.1f dBFS over %.1f)\n", k * 0.02, f.speech, f.rms_db, f.noise_db);
      printf("    onset %u ms: audio sent from %u ms (%u ms pre-roll)\n", k * 20, k * 20 - got / 16, got / 16);
    }
    if (ev & OA_EV_DECISION) {
      oa_voice_decision d;
      if (oa_voice_last_decision(v, &d)) printf("    %6.2f s  decision action %u\n", k * 0.02, d.action);
    }
    if (ev & OA_EV_TURN_END) { turns++; printf("    %6.2f s  turn end, %u ms%s\n", k * 0.02, oa_voice_turn_ms(v), ev & OA_EV_TOO_LONG ? " (max length)" : ""); }
  }
  printf("    %u turn(s)\n", turns);
  free(v);
  free(b);
}

int main(int argc, char **argv) {
  const char *dir = NULL;
  int first = 1;
  if (argc > 2 && !strcmp(argv[1], "--frames")) { dir = argv[2]; first = 3; }
  printf("voice engine (oa_voice %zu bytes)\n", oa_voice_size());
  turns_at(16000);
  turns_at(24000);
  turns_at(44100);
  turns_at(48000);
  barge_in();
  decisions();
  jitter();
  printf("voice wire\n");
  wire(dir);
  printf("voice floor and meter\n");
  floor_and_meter();
  for (int i = first; i < argc; i++) wav(argv[i]);
  if (failures) { fprintf(stderr, "%d check(s) failed\n", failures); return 1; }
  printf("all voice checks passed\n");
  return 0;
}
