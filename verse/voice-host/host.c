// The voice host: one process, one thread for sockets and the audio path,
// worker threads for provider calls. See docs/voice/dev.md.
//
// Wire: RFC 6455 WebSocket on --listen (default 127.0.0.1:8787); each binary
// message is one openagents.voice.v1alpha1.Frame (oa_voice_encode/decode from
// verse/platform/voice.h). GET /health answers "ok" over plain HTTP.
//
// The voice engine (voice.coax via voice.h) is confined to the main thread:
// every oa_voice*, oa_voice_floor* and oa_voice_meter* call happens here.
// Workers only do HTTP (speech.c) and hand results back through a pipe.
#include "../platform/voice.h"
#include "speech.h"
#include "ws.h"
#include "VoiceReply.h"
#include <arpa/inet.h>
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define MAX_SESSIONS 256
#define MAX_TEXT MAX_REPLY      // Utterance.text / TurnEvent.text max_bytes
#define WORKERS 8
#define LEAD_MS 400             // how far downlink audio runs ahead of real time
#define PARTIAL_EVERY 16000     // re-recognize for a PARTIAL every ~1 s of new 16 kHz audio
#define HISTORY 10             // exchanges of conversation the agent sees
#define OUT_LIMIT (2u << 20)    // a client this far behind is dropped
#define TARGET_ROUTE 1          // the floor's id for the session's Hello.route

static speech_provider g_provider = PROVIDER_GOOGLE;
static speech_agent g_agent = AGENT_GEMINI;
static uint64_t g_budget_msat = 0;
static bool g_verbose = false;

static uint64_t now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (uint64_t)t.tv_sec * 1000u + (uint64_t)t.tv_nsec / 1000000u;
}

static void logf_(const char *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  fprintf(stderr, "voice-host: ");
  vfprintf(stderr, fmt, ap);
  fputc('\n', stderr);
  va_end(ap);
}

// ---------------------------------------------------------------- byte buffers

typedef struct { uint8_t *p; size_t n, cap; } bytes;

static bool bytes_put(bytes *b, const void *d, size_t n) {
  if (b->n + n > b->cap) {
    size_t c = b->cap ? b->cap : 8192;
    while (c < b->n + n) c *= 2;
    uint8_t *q = realloc(b->p, c);
    if (!q) return false;
    b->p = q;
    b->cap = c;
  }
  memcpy(b->p + b->n, d, n);
  b->n += n;
  return true;
}
static void bytes_drop(bytes *b, size_t n) {
  memmove(b->p, b->p + n, b->n - n);
  b->n -= n;
}

static oa_voice_bytes B(const char *s) {
  oa_voice_bytes r = {(const uint8_t *)s, s ? (uint32_t)strlen(s) : 0};
  return r;
}
static void copy_bytes(char *dst, size_t cap, oa_voice_bytes b) {
  size_t n = b.len < cap - 1 ? b.len : cap - 1;
  if (n) memcpy(dst, b.ptr, n);
  dst[n] = 0;
}
static bool has_nul(oa_voice_bytes b) { return b.len && memchr(b.ptr, 0, b.len) != NULL; }

// Truncates s to at most max bytes without splitting a UTF-8 sequence.
static void clip_utf8(char *s, size_t max) {
  size_t n = strlen(s);
  if (n <= max) return;
  n = max;
  while (n && ((unsigned char)s[n] & 0xC0) == 0x80) n--;
  s[n] = 0;
}

// ---------------------------------------------------------------- accounts (meter)

typedef struct {
  char id[129];
  oa_voice_meter *m;
} account;
static account accounts[64];
static size_t naccounts;
// Speakers metered at once (--max-speakers): with the free tier each, this caps a public host's
// spend for the day, however many speaker ids a client invents.
static size_t g_max_speakers = sizeof accounts / sizeof accounts[0];

static uint32_t today(void) { return (uint32_t)(time(NULL) / 86400); }

static account *account_for(const char *id) {
  for (size_t i = 0; i < naccounts; i++)
    if (strcmp(accounts[i].id, id) == 0) {
      oa_voice_meter_day(accounts[i].m, today());
      return &accounts[i];
    }
  if (naccounts >= g_max_speakers) return NULL;
  account *a = &accounts[naccounts];
  a->m = calloc(1, oa_voice_meter_size());
  if (!a->m) return NULL;
  oa_voice_meter_init(a->m, today(), g_budget_msat);
  snprintf(a->id, sizeof a->id, "%s", id);
  naccounts++;
  return a;
}

// ---------------------------------------------------------------- jobs (worker threads)

typedef enum {
  JOB_RECOGNIZE,  // a partial (every ~1 s) or an early recognition (trailing silence)
  JOB_FINAL,      // the whole turn
  JOB_TAIL,       // the audio after the last partial; its text is appended
  JOB_AGENT,      // the streaming agent; posts JOB_SENTENCE events while it runs
  JOB_SENTENCE,   // an event, not work: one complete reply sentence
  JOB_TTS,        // one sentence of the reply
  JOB_WARM,       // token refresh and connection warm-up
} job_kind;

typedef struct job {
  job_kind kind;
  uint64_t session;          // session serial; results for a closed session are dropped
  uint32_t turn_no;
  char turn[65];
  int16_t *pcm;              // in: audio to recognize; out (TTS): rendered samples
  size_t n;
  size_t cover;              // RECOGNIZE: samples of the turn this covers
  uint32_t cut_silence_ms;   // RECOGNIZE: silence at its cut point
  uint32_t rate;
  const speech_profile *prof;
  uint32_t utter_no, seg;    // TTS: which utterance and sentence
  speech_exchange hist[HISTORY];
  size_t nhist;
  char route[129];           // AGENT: whose instruction to use
  char prefix[MAX_TEXT + 1]; // TAIL: the partial's text the tail is appended to
  char text[MAX_TEXT + 1];   // in: transcript (AGENT) or sentence (TTS); out: transcript (STT)
  char reply[MAX_TEXT + 1];  // out (AGENT)
  volatile int cancel;       // AGENT: set by the main thread to stop the stream
  bool ok, cached;
  uint64_t started;
  char err[300];
  struct job *next;
} job;

static pthread_mutex_t q_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t q_cond = PTHREAD_COND_INITIALIZER;
static job *todo_head, *todo_tail, *done_head, *done_tail;
static int wake_pipe[2];

static void post_done(job *j) {
  pthread_mutex_lock(&q_lock);
  if (done_tail) done_tail->next = j;
  else done_head = j;
  done_tail = j;
  pthread_mutex_unlock(&q_lock);
  char c = 1;
  (void)!write(wake_pipe[1], &c, 1);
}

// Worker thread: one finished sentence of a streaming reply goes straight to
// the main thread, which starts its synthesis while the rest still arrives.
static void on_sentence(void *ctx, const char *sentence) {
  job *a = ctx;
  job *e = calloc(1, sizeof *e);
  if (!e) return;
  e->kind = JOB_SENTENCE;
  e->session = a->session;
  e->turn_no = a->turn_no;
  snprintf(e->turn, sizeof e->turn, "%s", a->turn);
  snprintf(e->text, sizeof e->text, "%s", sentence);
  e->ok = true;
  e->started = a->started;
  post_done(e);
}

static void job_run(job *j) {
  switch (j->kind) {
  case JOB_RECOGNIZE:
  case JOB_FINAL:
  case JOB_TAIL:
    j->ok = speech_recognize(g_provider, j->pcm, j->n, j->text, sizeof j->text, j->err, sizeof j->err);
    clip_utf8(j->text, MAX_TEXT);
    break;
  case JOB_AGENT:
    j->ok = speech_agent_stream(g_agent, j->route, j->hist, j->nhist, j->text, on_sentence, j, &j->cancel, j->reply,
                                sizeof j->reply, j->err, sizeof j->err);
    clip_utf8(j->reply, MAX_TEXT);
    break;
  case JOB_TTS:
    free(j->pcm);
    j->pcm = speech_synthesize(g_provider, j->prof, j->text, j->rate, &j->n, &j->cached, j->err, sizeof j->err);
    j->ok = j->pcm != NULL;
    break;
  case JOB_WARM:
    speech_warm(g_provider == PROVIDER_GOOGLE, g_agent == AGENT_GEMINI);
    break;
  case JOB_SENTENCE: break;
  }
}

static void *worker(void *arg) {
  (void)arg;
  for (;;) {
    pthread_mutex_lock(&q_lock);
    while (!todo_head) pthread_cond_wait(&q_cond, &q_lock);
    job *j = todo_head;
    todo_head = j->next;
    if (!todo_head) todo_tail = NULL;
    pthread_mutex_unlock(&q_lock);
    j->next = NULL;
    job_run(j);
    post_done(j);
  }
  return NULL;
}

static void submit(job *j) {
  j->started = now_ms();
  pthread_mutex_lock(&q_lock);
  if (todo_tail) todo_tail->next = j;
  else todo_head = j;
  todo_tail = j;
  pthread_cond_signal(&q_cond);
  pthread_mutex_unlock(&q_lock);
}

static void job_free(job *j) {
  free(j->pcm);
  free(j);
}

// Warm-up is shared by every session (one connection cache), so at most
// one every 20 s.
static uint64_t last_warm;
static void warm(void) {
  if (g_provider != PROVIDER_GOOGLE && g_agent != AGENT_GEMINI) return;
  uint64_t now = now_ms();
  if (last_warm && now - last_warm < 20000) return;
  last_warm = now;
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->kind = JOB_WARM;
  submit(j);
}

// ---------------------------------------------------------------- sessions

#define MAX_SEGS 24
#define MAX_FLIGHT 3  // recognitions of one turn in flight at once

// One reply: one Utterance whose audio arrives sentence by sentence and is
// sent as one continuous stream.
typedef struct utter {
  uint32_t no;
  char id[16];
  char turn[65];
  uint32_t turn_no;
  char text[MAX_TEXT + 1];   // the sentences known so far
  int16_t *pcm;              // contiguous rendered audio, in sentence order
  size_t n, cap, sent;
  struct { int16_t *pcm; size_t n; bool ready; } seg[MAX_SEGS];
  int nseg, appended;
  char overflow[MAX_TEXT + 1];  // sentences past MAX_SEGS - 1, rendered as one
  bool text_done;            // the agent has finished (or failed)
  bool started;              // Speech sent
  // Latency, ms since the monotonic epoch.
  uint64_t t_end, t_final, t_sentence, t_audio;
  const char *plan;
  uint64_t gap_us;           // playback gaps: audio that wasn't ready in time
  struct utter *next;
} utter;

enum { S_HTTP, S_HELLO, S_READY, S_CLOSING };

typedef struct {
  int fd;
  int state;
  uint64_t serial;
  char sid[48];
  bytes in, out, msg;      // raw input, pending output, fragmented message assembly
  int msg_op;
  // From Hello.
  uint32_t in_rate, out_rate;
  bool observe;
  char route[129], speaker[129];
  const speech_profile *prof;
  account *acct;
  oa_voice *v;             // the host's own engine for this session
  oa_voice_floor *fl;
  // The open uplink turn.
  bool turn_open, turn_refused;
  char turn[65];
  uint32_t turn_no;        // increases per TURN_START
  int16_t *tb;             // 16 kHz audio for recognition
  size_t tn, tcap;
  int16_t pend[OA_VOICE_MAX_INPUT_SAMPLES];
  uint32_t pend_n;         // uplink samples waiting to fill a 20 ms frame
  size_t voiced_end;       // tn just after the last frame the engine heard speech in
  size_t partial_at;       // tn when the last recognition was requested
  size_t flight[MAX_FLIGHT]; // samples each in-flight recognition covers
  int nflight;
  size_t done_cover;       // samples the last finished recognition covered
  uint32_t done_cut_ms;
  char done_text[MAX_TEXT + 1];
  // The ended turn whose final transcript is still being decided.
  bool final_pending;
  int16_t *fin;            // that turn's audio
  size_t fin_n, fin_voiced;
  uint64_t t_end, t_final;
  const char *plan;
  uint32_t dropped_upto;   // replies for turns <= this are discarded (barge-in, cancel)
  job *agent;              // the streaming agent call in flight, if any
  // Downlink.
  utter *speech;           // head is speaking; the rest wait or are being made
  bool speaking;
  uint64_t play_clock_us;  // when the audio sent so far finishes playing
  uint64_t speak_floor_epoch;
  uint32_t utt_no;
  uint64_t played_samples; // from the client's PLAYBACK / BARGE_IN
  speech_exchange hist[HISTORY];
  size_t nhist;
  // The reply a barge-in last cut, kept until that turn's transcript says
  // whether it was someone talking or the reply's own echo (resume_if_echo).
  char cut_text[MAX_TEXT + 1];
  int16_t *cut_pcm;           // its rendered audio
  size_t cut_n, cut_played;   // samples rendered and played when it was cut
  int cut_rendered;           // sentences rendered; the rest are still text
  uint64_t cut_at;            // now_ms() of the cut; 0: nothing kept
  int cut_resumes;            // resumes in a row, so an echo can't cut it forever
} session;

static session *sessions[MAX_SESSIONS];
static uint64_t next_serial = 1;

static uint64_t now_us(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (uint64_t)t.tv_sec * 1000000u + (uint64_t)t.tv_nsec / 1000u;
}

static session *session_by_serial(uint64_t serial) {
  for (int i = 0; i < MAX_SESSIONS; i++)
    if (sessions[i] && sessions[i]->serial == serial && sessions[i]->state != S_CLOSING) return sessions[i];
  return NULL;
}

static void raw_out(session *s, const void *d, size_t n) {
  if (s->state == S_CLOSING) return;
  if (s->out.n + n > OUT_LIMIT || !bytes_put(&s->out, d, n)) {
    logf_("%s: client too slow, closing", s->sid);
    s->state = S_CLOSING;
  }
}

static void send_msg(session *s, const oa_voice_msg *m) {
  uint8_t enc[OA_VOICE_MAX_FRAME_BYTES], frame[OA_VOICE_MAX_FRAME_BYTES + 16];
  size_t n = oa_voice_encode(enc, sizeof enc, m);
  if (!n) {
    logf_("%s: encode failed for frame kind %u", s->sid, m->kind);
    return;
  }
  size_t w = ws_build(frame, sizeof frame, WS_BINARY, enc, n, false);
  raw_out(s, frame, w);
}

static void send_error(session *s, uint32_t code, const char *message, const char *turn) {
  oa_voice_msg m = {.kind = OA_MSG_ERROR};
  m.u.error.code = code;
  m.u.error.message = B(message);
  m.u.error.turn = B(turn);
  char clipped[257];
  if (m.u.error.message.len > 256) {
    snprintf(clipped, sizeof clipped, "%s", message);
    clip_utf8(clipped, 256);
    m.u.error.message = B(clipped);
  }
  send_msg(s, &m);
  logf_("%s: error %u %s", s->sid, code, message);
}

static void send_turn_event(session *s, const char *turn, uint32_t phase, const char *text, uint64_t played) {
  oa_voice_msg m = {.kind = OA_MSG_TURN_EVENT};
  m.u.turn_event.turn = B(turn);
  m.u.turn_event.speaker = B(s->speaker);
  m.u.turn_event.phase = phase;
  m.u.turn_event.text = B(text);
  m.u.turn_event.played_samples = played;
  m.u.turn_event.media_epoch = oa_voice_epoch(s->v);
  send_msg(s, &m);
}

static void send_meter(session *s) {
  oa_voice_msg m = {.kind = OA_MSG_METER};
  oa_voice_meter_report_get(s->acct->m, &m.u.meter);
  send_msg(s, &m);
}

static void observe_features(session *s, const oa_voice_features *f) {
  if (!s->observe) return;
  oa_voice_msg m = {.kind = OA_MSG_FEATURES};
  m.u.features = *f;
  send_msg(s, &m);
}

static void observe_decision(session *s, uint32_t ev) {
  if (!s->observe || !(ev & OA_EV_DECISION)) return;
  oa_voice_msg m = {.kind = OA_MSG_DECISION};
  if (oa_voice_last_decision(s->v, &m.u.decision)) send_msg(s, &m);
}

static void utter_free(utter *u) {
  free(u->pcm);
  for (int i = 0; i < u->nseg; i++) free(u->seg[i].pcm);
  free(u);
}

static void utter_free_all(session *s) {
  while (s->speech) {
    utter *u = s->speech;
    s->speech = u->next;
    utter_free(u);
  }
  s->speaking = false;
  oa_voice_playing(s->v, false);
}

static void stop_agent(session *s) {
  if (s->agent) s->agent->cancel = 1;  // the worker aborts the stream; its result is dropped
  s->agent = NULL;
}

static void drop_final(session *s) {
  s->final_pending = false;
  free(s->fin);
  s->fin = NULL;
  s->fin_n = 0;
}

static void session_close(int idx) {
  session *s = sessions[idx];
  if (s->fd >= 0) close(s->fd);
  if (s->v) utter_free_all(s);
  stop_agent(s);
  drop_final(s);
  free(s->cut_pcm);
  free(s->in.p);
  free(s->out.p);
  free(s->msg.p);
  free(s->tb);
  free(s->v);
  free(s->fl);
  if (s->state != S_HTTP) logf_("%s: closed", s->sid);
  free(s);
  sessions[idx] = NULL;
}

static uint32_t ms_of(size_t samples) { return (uint32_t)(samples * 1000 / OA_VOICE_RATE); }

// ---------------------------------------------------------------- the turn path

static void deliver_final(session *s, uint32_t turn_no, const char *turn, const char *text);
static void plan_final(session *s);

static void start_turn(session *s, const oa_voice_control *c) {
  char turn[65];
  copy_bytes(turn, sizeof turn, c->turn);
  if (!turn[0] || has_nul(c->turn)) { send_error(s, OA_ERROR_PROTOCOL, "TURN_START needs a turn id", ""); return; }
  if (s->turn_open) { send_error(s, OA_ERROR_PROTOCOL, "TURN_START while a turn is open", turn); return; }
  // A previous turn still waiting on its in-flight recognition is decided
  // now, without it: the whole turn is recognized.
  if (s->final_pending) {
    s->nflight = 0;
    s->done_cover = 0;
    s->done_text[0] = 0;
    plan_final(s);
  }
  warm();
  s->turn_no++;
  snprintf(s->turn, sizeof s->turn, "%s", turn);
  s->turn_open = true;
  s->tn = 0;
  s->pend_n = 0;
  s->partial_at = 0;
  s->voiced_end = 0;
  s->nflight = 0;
  s->done_cover = 0;
  s->done_cut_ms = 0;
  s->done_text[0] = 0;
  // voice-meter.coax decides whether this turn may run at all (worst case:
  // the longest turn the host accepts plus the longest reply).
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (!oa_voice_meter_may_run(s->acct->m, cfg.max_turn_ms, MAX_TEXT)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    return;
  }
  // voice-floor.coax: the speaker must hold consent and the floor to be heard.
  if (!oa_voice_floor_may_hear(s->fl, 0)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_UNAUTHORIZED, "this speaker may not be heard now", s->turn);
    return;
  }
  s->turn_refused = false;
  send_turn_event(s, s->turn, OA_TURN_PHASE_STARTED, NULL, 0);
  oa_voice_msg r = {.kind = OA_MSG_ROUTE};
  r.u.route.turn = B(s->turn);
  r.u.route.target = B(oa_voice_floor_route(s->fl, 0) == TARGET_ROUTE ? s->route : "");
  r.u.route.reason = B("default");
  send_msg(s, &r);
}

// Recognize the turn so far: every ~1 s of new audio (a PARTIAL), and early,
// as soon as voice-reply.coax sees trailing silence after speech, so that a
// finished recognition already covers every voiced frame when TURN_END comes.
static size_t flight_max(const session *s) {
  size_t m = 0;
  for (int i = 0; i < s->nflight; i++) if (s->flight[i] > m) m = s->flight[i];
  return m;
}

static void maybe_recognize(session *s) {
  if (s->turn_refused || s->tn < OA_VOICE_FRAME_SAMPLES || s->nflight == MAX_FLIGHT) return;
  uint32_t silence = ms_of(s->tn - s->voiced_end);
  size_t cover = s->done_cover > flight_max(s) ? s->done_cover : flight_max(s);
  // Early recognition may overlap one still in flight (a mid-sentence pause
  // started it); the regular cadence waits for the last one to finish.
  bool early = verse_voice_reply_recognize_early(silence, ms_of(s->voiced_end), ms_of(cover));
  bool cadence = s->nflight == 0 && s->tn - s->partial_at >= PARTIAL_EVERY;
  if (!early && !cadence) return;
  // voice-meter.coax decides whether re-sending the turn so far fits; the
  // usage is recorded like any other recognized audio.
  uint64_t ms = ms_of(s->tn);
  if (!oa_voice_meter_may_run(s->acct->m, ms, 0)) return;
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->pcm = malloc(s->tn * 2);
  if (!j->pcm) { free(j); return; }
  oa_voice_meter_charge(s->acct->m, ms, 0);  // records usage; false = now past budget
  memcpy(j->pcm, s->tb, s->tn * 2);
  j->n = s->tn;
  j->cover = s->tn;
  j->cut_silence_ms = silence;
  j->kind = JOB_RECOGNIZE;
  j->session = s->serial;
  j->turn_no = s->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", s->turn);
  s->partial_at = s->tn;
  s->flight[s->nflight++] = s->tn;
  submit(j);
}

static void submit_recognition(session *s, job_kind kind, const int16_t *pcm, size_t n, const char *prefix) {
  uint64_t ms = ms_of(n);
  // Decide before the audio goes to the provider; then record it.
  if (!oa_voice_meter_may_run(s->acct->m, ms, 0)) {
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  oa_voice_meter_charge(s->acct->m, ms, 0);  // records usage; false = now past budget
  job *j = calloc(1, sizeof *j);
  if (j) j->pcm = malloc(n * 2 + 2);
  if (!j || !j->pcm) { free(j); send_error(s, OA_ERROR_INTERNAL, "out of memory", s->turn); return; }
  memcpy(j->pcm, pcm, n * 2);
  j->n = n;
  j->kind = kind;
  j->session = s->serial;
  j->turn_no = s->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", s->turn);
  if (prefix) snprintf(j->prefix, sizeof j->prefix, "%s", prefix);
  submit(j);
}

// Decides the ended turn's final transcript with voice-reply.coax.
static void plan_final(session *s) {
  size_t voiced = s->fin_voiced ? s->fin_voiced : s->fin_n;  // no voiced frame seen: all of it must be covered
  int64_t p = verse_voice_reply_final_plan(ms_of(voiced), ms_of(s->done_cover), s->done_text[0] != 0,
                                              s->done_cut_ms, ms_of(flight_max(s)));
  if (p == 3) {  // wait for the in-flight recognition, then decide again
    s->final_pending = true;
    s->plan = "wait";
    return;
  }
  s->final_pending = false;
  if (p == 1) {
    s->plan = "partial";
    char text[MAX_TEXT + 1];
    snprintf(text, sizeof text, "%s", s->done_text);
    drop_final(s);
    deliver_final(s, s->turn_no, s->turn, text);
    return;
  }
  if (p == 2 && s->done_cover < s->fin_n) {
    s->plan = "tail";
    submit_recognition(s, JOB_TAIL, s->fin + s->done_cover, s->fin_n - s->done_cover, s->done_text);
  } else {
    s->plan = "whole";
    submit_recognition(s, JOB_FINAL, s->fin, s->fin_n, NULL);
  }
  drop_final(s);
}

static void end_turn(session *s, bool too_long) {
  if (!s->turn_open) return;
  s->turn_open = false;
  if (s->turn_refused) return;
  if (too_long) send_error(s, OA_ERROR_TOO_LONG, "the turn exceeded max_turn_ms", s->turn);
  if (s->tn < OA_VOICE_FRAME_SAMPLES) {
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  s->t_end = now_ms();
  drop_final(s);
  s->fin = s->tb;  // the turn's audio, kept until its final transcript is decided
  s->fin_n = s->tn;
  s->fin_voiced = s->voiced_end;
  s->tb = NULL;
  s->tn = s->tcap = 0;
  plan_final(s);
}

static void uplink_frame(session *s, const int16_t *frame) {
  int16_t out16k[OA_VOICE_FRAME_SAMPLES];
  oa_voice_features f;
  uint32_t ev = oa_voice_uplink(s->v, frame, out16k, &f);
  observe_features(s, &f);
  observe_decision(s, ev);
  // A DROP decision (from the policy or an observer) keeps this frame from
  // recognition. Turn boundaries stay the client's (its engine decided them).
  if (ev & OA_EV_DROPPED) return;
  if (s->tn + OA_VOICE_FRAME_SAMPLES > s->tcap) {
    size_t c = s->tcap ? s->tcap * 2 : OA_VOICE_RATE * 4;
    int16_t *q = realloc(s->tb, c * 2);
    if (!q) return;
    s->tb = q;
    s->tcap = c;
  }
  memcpy(s->tb + s->tn, out16k, sizeof out16k);
  s->tn += OA_VOICE_FRAME_SAMPLES;
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (f.speech >= cfg.speech_threshold) s->voiced_end = s->tn;
}

static void on_uplink_audio(session *s, const oa_voice_audio *a) {
  if (!s->turn_open) { send_error(s, OA_ERROR_PROTOCOL, "uplink audio outside a turn", ""); return; }
  if (s->turn_refused) return;
  if (a->turn.len != strlen(s->turn) || memcmp(a->turn.ptr, s->turn, a->turn.len)) {
    send_error(s, OA_ERROR_PROTOCOL, "uplink audio for a turn that is not open", s->turn);
    return;
  }
  if ((a->codec != OA_CODEC_PCM_S16LE && a->codec != OA_CODEC_UNSPECIFIED) || (a->sample_rate && a->sample_rate != s->in_rate) ||
      (a->data.len & 1)) {
    send_error(s, OA_ERROR_PROTOCOL, "uplink audio must be PCM s16le at the Hello's input rate", s->turn);
    return;
  }
  uint32_t fs = oa_voice_input_frame_samples(s->v);
  const uint8_t *d = a->data.ptr;
  size_t n = a->data.len / 2;
  for (size_t i = 0; i < n; i++) {
    s->pend[s->pend_n++] = (int16_t)(d[2 * i] | d[2 * i + 1] << 8);
    if (s->pend_n == fs) {
      uplink_frame(s, s->pend);
      s->pend_n = 0;
    }
  }
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (ms_of(s->tn) > cfg.max_turn_ms) { end_turn(s, true); return; }
  maybe_recognize(s);
}

static void clear_cut(session *s) {
  free(s->cut_pcm);
  s->cut_pcm = NULL; s->cut_n = 0; s->cut_at = 0; s->cut_resumes = 0;
}

// Barge-in or cancel: cut playback, move the epoch forward, stop the agent,
// and discard every reply still being made for turns up to now.
static void cut_speech(session *s, uint64_t client_epoch, bool barge) {
  utter *u = s->speech;
  bool was = s->speaking && u;
  char turn[65] = "";
  if (u) snprintf(turn, sizeof turn, "%s", u->turn);
  if (barge && was && u->text[0]) {
    free(s->cut_pcm);
    snprintf(s->cut_text, sizeof s->cut_text, "%s", u->text);
    s->cut_pcm = u->pcm; u->pcm = NULL;   // kept for resume_cut; utter_free_all frees the rest
    s->cut_n = u->n; s->cut_played = (size_t)s->played_samples; s->cut_rendered = u->appended; s->cut_at = now_ms();
  } else if (!barge) {
    clear_cut(s);
  }
  uint64_t e = oa_voice_cut(s->v);
  if (client_epoch > e) e = oa_voice_set_epoch(s->v, client_epoch);
  oa_voice_floor_bump(s->fl);
  s->dropped_upto = s->turn_no - (s->turn_open ? 1 : 0);
  if (!s->turn_open) drop_final(s);
  stop_agent(s);
  utter_free_all(s);
  if (was) send_turn_event(s, turn, OA_TURN_PHASE_INTERRUPTED, NULL, s->played_samples);
  logf_("%s: %s, epoch now %llu", s->sid, barge ? "barge-in" : "cancel", (unsigned long long)e);
}

// ---------------------------------------------------------------- downlink

static void log_latency(session *s, const utter *u, uint64_t first_sent) {
  logf_("%s: latency %s plan=%s recognition=%llu agent_first_sentence=%llu tts_first=%llu total=%llu ms",
        s->sid, u->turn, u->plan ? u->plan : "?", (unsigned long long)(u->t_final - u->t_end),
        (unsigned long long)(u->t_sentence - u->t_final), (unsigned long long)(u->t_audio - u->t_sentence),
        (unsigned long long)(first_sent - u->t_end));
}

static bool utter_done(const utter *u) { return u->text_done && u->appended == u->nseg; }
static void send_speech(session *s, const utter *u);

static void start_next_utterance(session *s) {
  utter *u = s->speech;
  if (!u || s->speaking || !u->n) return;
  s->speaking = true;
  s->play_clock_us = now_us();
  s->speak_floor_epoch = oa_voice_floor_epoch(s->fl);
  s->played_samples = 0;
  oa_voice_playing(s->v, true);
  u->started = true;
  send_speech(s, u);
}

// Speech for an utterance: first when its audio starts, then again (same
// utterance id, same epoch) each time the streamed reply grows a sentence,
// carrying the full text known so far, so a caption shows the whole reply.
// A client treats a repeated id as a text update, not a new utterance.
static void send_speech(session *s, const utter *u) {
  oa_voice_msg m = {.kind = OA_MSG_SPEECH};
  m.u.speech.has_utterance = true;
  m.u.speech.utterance.id = B(u->id);
  m.u.speech.utterance.speaker = B(s->route);
  m.u.speech.utterance.text = B(u->text);
  m.u.speech.utterance.interruptible = true;
  m.u.speech.has_speaker = true;
  m.u.speech.speaker.id = B(s->route);
  m.u.speech.speaker.role = OA_SPEAKER_ROLE_AGENT;
  m.u.speech.speaker.profile = B(s->prof->id);
  m.u.speech.speaker.profile_revision = s->prof->revision;
  m.u.speech.sample_rate = s->out_rate;
  m.u.speech.media_epoch = oa_voice_epoch(s->v);
  m.u.speech.turn = B(u->turn);
  send_msg(s, &m);
}

static void end_utterance(session *s, utter *u) {
  if (u->started)
    logf_("%s: %s %s spoken: %zu samples in %d sentence(s), gaps %llu ms", s->sid, u->turn, u->id, u->n, u->nseg,
          (unsigned long long)(u->gap_us / 1000));
  if (u->started) {
    oa_voice_msg c = {.kind = OA_MSG_CONTROL};
    c.u.control.kind = OA_CONTROL_SPEECH_END;
    c.u.control.turn = B(u->turn);
    c.u.control.utterance = B(u->id);
    c.u.control.media_epoch = oa_voice_epoch(s->v);
    c.u.control.played_samples = u->n;
    send_msg(s, &c);
  }
  send_turn_event(s, u->turn, OA_TURN_PHASE_ENDED, NULL, u->n);
  s->speech = u->next;
  utter_free(u);
  s->speaking = false;
  oa_voice_playing(s->v, false);
}

// Sends reply audio up to LEAD_MS ahead of the client's playback clock, in
// chunks of at most 200 ms (and at most Audio.data's 9600 bytes). Audio from
// later sentences continues the same Utterance with no gap when it is ready
// in time; the playback clock carries across sentences.
static void pump_speech(session *s, uint64_t now) {
  (void)now;
  for (;;) {
    utter *u = s->speech;
    if (!u) return;
    if (!s->speaking) {
      // A reply that ended with no audio at all (every sentence failed).
      if (utter_done(u) && !u->n) { end_utterance(s, u); continue; }
      start_next_utterance(s);
      if (!s->speaking) return;
    }
    if (!oa_voice_floor_epoch_ok(s->fl, s->speak_floor_epoch)) { utter_free_all(s); return; }
    uint32_t chunk = s->out_rate / 5;
    if (chunk > OA_VOICE_MAX_AUDIO_BYTES / 2) chunk = OA_VOICE_MAX_AUDIO_BYTES / 2;
    uint64_t t = now_us();
    while (u->sent < u->n && s->play_clock_us < t + LEAD_MS * 1000u) {
      uint32_t k = (uint32_t)(u->n - u->sent < chunk ? u->n - u->sent : chunk);
      int16_t pcm[OA_VOICE_MAX_AUDIO_BYTES / 2];
      memcpy(pcm, u->pcm + u->sent, k * 2);
      oa_voice_features f;
      uint32_t seq = oa_voice_downlink_sequence(s->v);
      uint32_t ev = oa_voice_downlink(s->v, pcm, k, s->out_rate, &f);  // gain/duck/pause from decisions
      observe_features(s, &f);
      observe_decision(s, ev);
      oa_voice_msg m = {.kind = OA_MSG_AUDIO};
      m.u.audio.turn = B(u->turn);
      m.u.audio.sequence = seq;
      m.u.audio.sample_rate = s->out_rate;
      m.u.audio.codec = OA_CODEC_PCM_S16LE;
      m.u.audio.data.ptr = (const uint8_t *)pcm;
      m.u.audio.data.len = k * 2;
      m.u.audio.media_epoch = oa_voice_epoch(s->v);
      m.u.audio.utterance = B(u->id);
      send_msg(s, &m);
      if (u->sent == 0) log_latency(s, u, now_ms());
      u->sent += k;
      if (s->play_clock_us < t) {  // an underrun: playback restarts now
        if (u->sent) u->gap_us += t - s->play_clock_us;
        s->play_clock_us = t;
      }
      s->play_clock_us += (uint64_t)k * 1000000u / s->out_rate;
    }
    if (u->sent < u->n || !utter_done(u)) return;
    // Everything is sent; the utterance ends when its last sample has played.
    if (t < s->play_clock_us) return;
    end_utterance(s, u);
  }
}

static utter *utter_by_no(session *s, uint32_t no) {
  for (utter *u = s->speech; u; u = u->next)
    if (u->no == no) return u;
  return NULL;
}
static utter *utter_by_turn(session *s, uint32_t turn_no) {
  for (utter *u = s->speech; u; u = u->next)
    if (u->turn_no == turn_no) return u;
  return NULL;
}

static void synthesize(session *s, utter *u, int seg, const char *text) {
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->kind = JOB_TTS;
  j->session = s->serial;
  j->turn_no = u->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", u->turn);
  snprintf(j->text, sizeof j->text, "%s", text);
  j->rate = s->out_rate;
  j->prof = s->prof;
  j->utter_no = u->no;
  j->seg = (uint32_t)seg;
  u->nseg = seg + 1;
  submit(j);
}

// A complete sentence of the reply: charge it, start rendering it now.
static void on_reply_sentence(session *s, const job *e) {
  utter *u = utter_by_turn(s, e->turn_no);
  if (!u) {
    u = calloc(1, sizeof *u);
    if (!u) return;
    u->no = ++s->utt_no;
    snprintf(u->id, sizeof u->id, "u%u", u->no);
    snprintf(u->turn, sizeof u->turn, "%s", e->turn);
    u->turn_no = e->turn_no;
    u->t_end = s->t_end;
    u->t_final = s->t_final;
    u->t_sentence = now_ms();
    u->plan = s->plan;
    utter **t = &s->speech;
    while (*t) t = &(*t)->next;
    *t = u;
  }
  if (u->text_done) return;
  size_t len = strlen(e->text);
  // Decide before synthesis; then record the characters.
  if (!oa_voice_meter_may_run(s->acct->m, 0, len)) {
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", u->turn);
    send_meter(s);
    u->text_done = true;
    stop_agent(s);
    return;
  }
  oa_voice_meter_charge(s->acct->m, 0, len);
  size_t have = strlen(u->text);
  snprintf(u->text + have, sizeof u->text - have, "%s%s", have ? " " : "", e->text);
  clip_utf8(u->text, MAX_TEXT);
  if (u->started) send_speech(s, u);
  if (u->nseg < MAX_SEGS - 1) synthesize(s, u, u->nseg, e->text);
  else {
    size_t o = strlen(u->overflow);
    snprintf(u->overflow + o, sizeof u->overflow - o, "%s%s", o ? " " : "", e->text);
  }
}

static void on_reply_audio(session *s, job *j) {
  utter *u = utter_by_no(s, j->utter_no);
  if (!u || (int)j->seg >= u->nseg) return;  // cut by barge-in or cancel
  if (!j->ok) send_error(s, OA_ERROR_PROVIDER, j->err, u->turn);
  u->seg[j->seg].ready = true;
  u->seg[j->seg].pcm = j->ok ? j->pcm : NULL;
  u->seg[j->seg].n = j->ok ? j->n : 0;
  if (j->ok) j->pcm = NULL;
  if (!u->t_audio && j->ok) u->t_audio = now_ms();
  while (u->appended < u->nseg && u->seg[u->appended].ready) {
    size_t k = u->seg[u->appended].n;
    if (u->n + k > u->cap) {
      size_t c = u->cap ? u->cap : (size_t)s->out_rate * 4;
      while (c < u->n + k) c *= 2;
      int16_t *q = realloc(u->pcm, c * 2);
      if (!q) break;
      u->pcm = q;
      u->cap = c;
    }
    if (k) memcpy(u->pcm + u->n, u->seg[u->appended].pcm, k * 2);
    u->n += k;
    free(u->seg[u->appended].pcm);
    u->seg[u->appended].pcm = NULL;
    u->appended++;
  }
  pump_speech(s, now_ms());
}

// ---------------------------------------------------------------- job results

// The words of text, lowercased letters and digits, into w (at most n words
// of 31 bytes). Returns how many.
static int words_of(const char *text, char w[][32], int n) {
  int k = 0, len = 0;
  for (const unsigned char *c = (const unsigned char *)text;; c++) {
    bool in = *c && (isalnum(*c) || *c >= 0x80 || *c == '\'');
    if (in && *c != '\'') { if (len < 31) w[k][len++] = (char)tolower(*c); }
    else if (!in && len) { w[k][len] = 0; len = 0; if (++k == n) return k; }
    if (!*c) return k;
  }
}

// A turn heard over the reply is its echo when most of its words are the
// reply's: the phone's mic caught the speaker and barged in on itself.
// Only real recognition: the offline provider's stand-in transcript ("I
// heard N milliseconds of speech") is what its echo agent says back.
// A barge-in with no words at all (the mic caught the reply, or a noise) is
// never someone talking either.
static bool is_echo(const session *s, const char *text) {
  if (g_provider != PROVIDER_GOOGLE || !s->cut_at || now_ms() - s->cut_at > 10000) return false;
  if (!text[0]) return true;
  static char t[64][32], r[256][32];
  int nt = words_of(text, t, 64), nr = words_of(s->cut_text, r, 256), hit = 0;
  if (!nt) return false;
  // The last word is often cut off where the barge-in cut the reply ("No
  // form" of "No foreman"): its first three letters are enough.
  for (int i = 0; i < nt; i++)
    for (int j = 0; j < nr; j++)
      if (!strcmp(t[i], r[j]) || (i == nt - 1 && strlen(t[i]) >= 3 && !strncmp(t[i], r[j], 3))) { hit++; break; }
  return hit * 4 >= nt * 3;
}

static void on_reply_sentence(session *s, const job *e);

// The cut reply again, for this turn, from the sample the phone had played
// up to: its rendered audio from there, then the sentences not rendered yet.
// Three echoes in a row and the reply stops.
static void resume_cut(session *s, uint32_t turn_no, const char *turn) {
  if (++s->cut_resumes > 3 || !s->cut_pcm) { clear_cut(s); send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0); return; }
  size_t from = s->cut_played < s->cut_n ? s->cut_played : s->cut_n;
  // The sentences of the reply, and where the unrendered ones start.
  const char *t = s->cut_text;
  size_t len = strlen(t), at = 0, rest_at = len;
  for (int k = 0; k < s->cut_rendered && at < len; k++) {
    while (at < len && !(strchr(".?!", t[at]) && (t[at + 1] == ' ' || !t[at + 1]))) at++;
    if (at < len) at++;
    while (t[at] == ' ') at++;
  }
  rest_at = at;
  utter *u = calloc(1, sizeof *u);
  size_t n = s->cut_n - from;
  if (!u || (n && !(u->pcm = malloc(n * 2)))) { free(u); clear_cut(s); send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0); return; }
  if (n) memcpy(u->pcm, s->cut_pcm + from, n * 2);
  u->n = u->cap = n;
  u->no = ++s->utt_no;
  snprintf(u->id, sizeof u->id, "u%u", u->no);
  snprintf(u->turn, sizeof u->turn, "%s", turn);
  u->turn_no = turn_no;
  u->t_end = u->t_final = u->t_sentence = u->t_audio = now_ms();
  u->plan = "resume";
  // The caption: the words from about where it was cut.
  size_t c = s->cut_n ? (size_t)((double)rest_at * (double)from / (double)s->cut_n) : 0;
  while (c > 0 && t[c - 1] != ' ') c--;
  snprintf(u->text, sizeof u->text, "%s", t + c);
  for (const char *a = t + rest_at; *a && u->nseg < MAX_SEGS - 1;) {
    const char *b = a;
    while (*b && !(strchr(".?!", *b) && (b[1] == ' ' || !b[1]))) b++;
    if (*b) b++;
    char sentence[MAX_TEXT + 1];
    snprintf(sentence, sizeof sentence, "%.*s", (int)(b - a), a);
    synthesize(s, u, u->nseg, sentence);
    a = b;
    while (*a == ' ') a++;
  }
  u->text_done = true;
  free(s->cut_pcm); s->cut_pcm = NULL;
  logf_("%s: %s is the reply's echo; resuming at %zu of %zu samples (%d sentence(s) to render): %s", s->sid, turn, from, s->cut_n,
        u->nseg, u->text);
  s->cut_at = 0;
  utter **q = &s->speech;
  while (*q) q = &(*q)->next;
  *q = u;
  pump_speech(s, now_ms());
}

static void deliver_final(session *s, uint32_t turn_no, const char *turn, const char *text) {
  s->t_final = now_ms();
  if (is_echo(s, text)) {
    logf_("%s: %s heard %s", s->sid, turn, text);
    resume_cut(s, turn_no, turn);
    return;
  }
  clear_cut(s);
  if (g_verbose) logf_("%s: %s final (%s) in %llu ms: %s", s->sid, turn, s->plan, (unsigned long long)(s->t_final - s->t_end), text);
  else logf_("%s: %s final (%s) in %llu ms (%zu bytes)", s->sid, turn, s->plan, (unsigned long long)(s->t_final - s->t_end), strlen(text));
  send_turn_event(s, turn, OA_TURN_PHASE_FINAL, text, 0);
  send_meter(s);
  // The transcript goes where the floor binds this speaker; never chosen from the words.
  if (!text[0] || oa_voice_floor_route(s->fl, 0) != TARGET_ROUTE) {
    send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  job *j = calloc(1, sizeof *j);
  if (!j) { send_error(s, OA_ERROR_INTERNAL, "out of memory", turn); return; }
  j->kind = JOB_AGENT;
  j->session = s->serial;
  j->turn_no = turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", turn);
  snprintf(j->text, sizeof j->text, "%s", text);
  memcpy(j->hist, s->hist, sizeof s->hist);
  j->nhist = s->nhist;
  snprintf(j->route, sizeof j->route, "%s", s->route);
  stop_agent(s);
  s->agent = j;
  submit(j);
}

static void on_job_done(job *j) {
  if (j->kind == JOB_WARM) { job_free(j); return; }
  session *s = session_by_serial(j->session);
  if (!s) { job_free(j); return; }
  if (j->kind == JOB_RECOGNIZE) {
    if (g_verbose) logf_("%s: %s recognized %u ms (cut in %u ms of silence) in %llu ms: %s", s->sid, j->turn, ms_of(j->cover),
                         j->cut_silence_ms, (unsigned long long)(now_ms() - j->started), j->ok ? j->text : j->err);
    if (j->turn_no == s->turn_no) {
      for (int i = 0; i < s->nflight; i++)
        if (s->flight[i] == j->cover) { s->flight[i] = s->flight[--s->nflight]; break; }
      if (j->ok && j->cover > s->done_cover) {
        s->done_cover = j->cover;
        s->done_cut_ms = j->cut_silence_ms;
        snprintf(s->done_text, sizeof s->done_text, "%s", j->text);
      }
      if (j->ok && s->turn_open && j->text[0]) send_turn_event(s, j->turn, OA_TURN_PHASE_PARTIAL, j->text, 0);
      if (s->final_pending) plan_final(s);
      else if (s->turn_open) maybe_recognize(s);
    }
    job_free(j);
    return;
  }
  if (j->turn_no <= s->dropped_upto) {  // barged or cancelled
    if (s->agent == j) s->agent = NULL;
    job_free(j);
    return;
  }
  switch (j->kind) {
  case JOB_FINAL:
  case JOB_TAIL:
    if (!j->ok) {
      send_error(s, OA_ERROR_PROVIDER, j->err, j->turn);
      send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0);
      break;
    }
    if (j->kind == JOB_TAIL && j->prefix[0]) {
      char text[2 * MAX_TEXT + 2];
      snprintf(text, sizeof text, "%s%s%s", j->prefix, j->text[0] ? " " : "", j->text);
      clip_utf8(text, MAX_TEXT);
      deliver_final(s, j->turn_no, j->turn, text);
    } else {
      deliver_final(s, j->turn_no, j->turn, j->text);
    }
    break;
  case JOB_SENTENCE:
    on_reply_sentence(s, j);
    break;
  case JOB_AGENT: {
    if (s->agent == j) s->agent = NULL;
    utter *u = utter_by_turn(s, j->turn_no);
    logf_("%s: %s agent reply in %llu ms (%zu bytes)", s->sid, j->turn, (unsigned long long)(now_ms() - j->started), strlen(j->reply));
    if (g_verbose && j->ok) logf_("%s: reply: %s", s->sid, j->reply);
    if (!j->ok) send_error(s, OA_ERROR_PROVIDER, j->err, j->turn);
    else {
      if (s->nhist == HISTORY) { memmove(s->hist, s->hist + 1, sizeof s->hist[0] * (HISTORY - 1)); s->nhist--; }
      snprintf(s->hist[s->nhist].user, sizeof s->hist[0].user, "%s", j->text);
      snprintf(s->hist[s->nhist].agent, sizeof s->hist[0].agent, "%s", j->reply);
      s->nhist++;
    }
    send_meter(s);
    if (!u) { send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0); break; }
    if (!u->text_done && u->overflow[0]) synthesize(s, u, u->nseg, u->overflow);
    u->text_done = true;
    pump_speech(s, now_ms());
    break;
  }
  case JOB_TTS:
    if (j->ok) logf_("%s: %s sentence %u: %zu samples in %llu ms%s", s->sid, j->turn, j->seg, j->n,
                     (unsigned long long)(now_ms() - j->started), j->cached ? " (cached)" : "");
    on_reply_audio(s, j);
    break;
  default: break;
  }
  job_free(j);
}

// ---------------------------------------------------------------- frames in

static void on_hello(session *s, const oa_voice_hello *h) {
  if (h->protocol != OA_VOICE_PROTOCOL) { send_error(s, OA_ERROR_PROTOCOL, "unsupported protocol", ""); s->state = S_CLOSING; return; }
  uint32_t in = h->input_sample_rate ? h->input_sample_rate : OA_VOICE_RATE;
  uint32_t out = h->output_sample_rate ? h->output_sample_rate : 24000;
  if (in != 16000 && in != 24000 && in != 44100 && in != 48000) { send_error(s, OA_ERROR_PROTOCOL, "unsupported input_sample_rate", ""); s->state = S_CLOSING; return; }
  if (out != 16000 && out != 24000 && out != 48000) { send_error(s, OA_ERROR_PROTOCOL, "unsupported output_sample_rate", ""); s->state = S_CLOSING; return; }
  if (h->codec != OA_CODEC_UNSPECIFIED && h->codec != OA_CODEC_PCM_S16LE) { send_error(s, OA_ERROR_PROTOCOL, "only PCM s16le is supported", ""); s->state = S_CLOSING; return; }
  char profile[65];
  copy_bytes(profile, sizeof profile, h->profile);
  s->prof = speech_find_profile(profile);
  if (!s->prof) { send_error(s, OA_ERROR_PROTOCOL, "unknown voice profile", ""); s->state = S_CLOSING; return; }
  copy_bytes(s->route, sizeof s->route, h->route);
  if (!s->route[0]) snprintf(s->route, sizeof s->route, "foreman");
  if (!speech_route_known(s->route)) { send_error(s, OA_ERROR_PROTOCOL, "unknown route", ""); s->state = S_CLOSING; return; }
  if (h->has_speaker) copy_bytes(s->speaker, sizeof s->speaker, h->speaker.id);
  if (!s->speaker[0]) snprintf(s->speaker, sizeof s->speaker, "dev");
  // Dev host: Hello.token is not checked yet (never logged either).
  s->acct = account_for(s->speaker);
  if (!s->acct) { send_error(s, OA_ERROR_LIMIT, "voice is busy: too many speakers today", ""); s->state = S_CLOSING; return; }
  s->v = calloc(1, oa_voice_size());
  s->fl = calloc(1, oa_voice_floor_size());
  if (!s->acct || !s->v || !s->fl) { send_error(s, OA_ERROR_INTERNAL, "out of memory", ""); s->state = S_CLOSING; return; }
  s->in_rate = in;
  s->out_rate = out;
  s->observe = h->observe;
  oa_voice_init(s->v, in);
  oa_voice_floor_init(s->fl);
  // Opening a session is this speaker's consent to be heard by its route; the
  // floor records it and binds the speaker's turns to Hello.route.
  oa_voice_floor_consent(s->fl, 0, true);
  oa_voice_floor_bind(s->fl, 0, TARGET_ROUTE);
  oa_voice_floor_grant(s->fl, 0);
  oa_voice_msg m = {.kind = OA_MSG_WELCOME};
  m.u.welcome.protocol = OA_VOICE_PROTOCOL;
  m.u.welcome.session = B(s->sid);
  m.u.welcome.input_sample_rate = in;
  m.u.welcome.output_sample_rate = out;
  m.u.welcome.provider = B(g_provider == PROVIDER_GOOGLE ? "google" : "local");
  m.u.welcome.has_meter = true;
  oa_voice_meter_report_get(s->acct->m, &m.u.welcome.meter);
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  m.u.welcome.max_turn_ms = cfg.max_turn_ms;
  send_msg(s, &m);
  s->state = S_READY;
  logf_("%s: hello speaker=%s route=%s in=%u out=%u profile=%s%s", s->sid, s->speaker, s->route, in, out, s->prof->id,
        s->observe ? " observe" : "");
}

// The client's media epoch outlives a connection: a phone that cut a reply
// and reconnected is ahead of this new session, and drops every reply
// stamped with an older epoch as stale. Any frame the client sends that
// carries a later epoch moves this session up to it.
static void adopt_epoch(session *s, uint64_t client_epoch) {
  if (client_epoch <= oa_voice_epoch(s->v)) return;
  uint64_t e = oa_voice_set_epoch(s->v, client_epoch);
  logf_("%s: client epoch %llu, epoch now %llu", s->sid, (unsigned long long)client_epoch, (unsigned long long)e);
}

static void on_control(session *s, const oa_voice_control *c) {
  if (c->kind != OA_CONTROL_BARGE_IN && c->kind != OA_CONTROL_CANCEL) adopt_epoch(s, c->media_epoch);
  switch (c->kind) {
  case OA_CONTROL_TURN_START: start_turn(s, c); break;
  case OA_CONTROL_TURN_END: {
    if (!s->turn_open || c->turn.len != strlen(s->turn) || memcmp(c->turn.ptr, s->turn, c->turn.len)) {
      send_error(s, OA_ERROR_PROTOCOL, "TURN_END for a turn that is not open", "");
      break;
    }
    // Flush a partial frame by padding it with silence.
    uint32_t fs = oa_voice_input_frame_samples(s->v);
    if (s->pend_n && !s->turn_refused) {
      memset(s->pend + s->pend_n, 0, (fs - s->pend_n) * 2);
      uplink_frame(s, s->pend);
      s->pend_n = 0;
    }
    end_turn(s, false);
    break;
  }
  case OA_CONTROL_BARGE_IN:
    if (c->played_samples) s->played_samples = c->played_samples;
    cut_speech(s, c->media_epoch, true);
    break;
  case OA_CONTROL_CANCEL:
    if (s->turn_open) {
      s->turn_open = false;
      oa_voice_cancel_turn(s->v);
      send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    }
    cut_speech(s, c->media_epoch, false);
    s->dropped_upto = s->turn_no;
    drop_final(s);
    break;
  case OA_CONTROL_PLAYBACK:
    // What the phone says it has played of the current reply: in a verbose
    // run, every report (about four a second while a reply plays), so a
    // silent client shows up as a stuck count or as no reports at all.
    if (g_verbose)
      logf_("%s: playback %.*s played %llu samples", s->sid, (int)c->utterance.len, (const char *)c->utterance.ptr,
            (unsigned long long)c->played_samples);
    s->played_samples = c->played_samples;
    break;
  case OA_CONTROL_PING: {
    oa_voice_msg m = {.kind = OA_MSG_CONTROL};
    m.u.control.kind = OA_CONTROL_PONG;
    m.u.control.at_ms = c->at_ms;
    send_msg(s, &m);
    break;
  }
  case OA_CONTROL_PONG: break;
  default: send_error(s, OA_ERROR_PROTOCOL, "unexpected control kind", "");
  }
}

static void on_frame(session *s, const uint8_t *p, size_t n) {
  oa_voice_msg m;
  memset(&m, 0, sizeof m);
  if (n > OA_VOICE_MAX_FRAME_BYTES || !oa_voice_decode(p, n, &m)) {
    send_error(s, OA_ERROR_PROTOCOL, "malformed frame", "");
    if (s->state == S_HELLO) s->state = S_CLOSING;
    return;
  }
  if (s->state == S_HELLO) {
    if (m.kind != OA_MSG_HELLO) { send_error(s, OA_ERROR_PROTOCOL, "the first frame must be Hello", ""); s->state = S_CLOSING; return; }
    on_hello(s, &m.u.hello);
    return;
  }
  switch (m.kind) {
  case OA_MSG_AUDIO: adopt_epoch(s, m.u.audio.media_epoch); on_uplink_audio(s, &m.u.audio); break;
  case OA_MSG_CONTROL: on_control(s, &m.u.control); break;
  case OA_MSG_DECISION: {
    // The seam for an external decider: applied to this session's engine.
    // Dropped (late, stale epoch, low confidence) decisions change nothing.
    bool kept = oa_voice_decide(s->v, &m.u.decision);
    if (g_verbose) logf_("%s: decision stream=%u action=%u %s", s->sid, m.u.decision.stream, m.u.decision.action, kept ? "applied" : "dropped");
    break;
  }
  case OA_MSG_FEATURES: break;  // client-side features: accepted, not used yet
  case OA_MSG_HELLO: send_error(s, OA_ERROR_PROTOCOL, "Hello was already received", ""); break;
  default: send_error(s, OA_ERROR_PROTOCOL, "a client may not send this frame", ""); break;
  }
}

// ---------------------------------------------------------------- socket I/O

static void handle_http(session *s) {
  char *req = (char *)s->in.p;
  void *end = memmem(s->in.p, s->in.n, "\r\n\r\n", 4);
  if (!end) {
    if (s->in.n > 8192) s->state = S_CLOSING;
    return;
  }
  size_t n = (size_t)((uint8_t *)end - s->in.p) + 4;
  char resp[512];
  size_t w = ws_server_handshake(req, n, resp, sizeof resp);
  if (w) {
    raw_out(s, resp, w);
    s->state = S_HELLO;
    bytes_drop(&s->in, n);
    return;
  }
  const char *body = strncmp(req, "GET /health", 11) == 0 ? "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n"
                                                        : "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
  raw_out(s, body, strlen(body));
  s->state = S_CLOSING;
}

static void handle_ws(session *s) {
  while (s->state == S_HELLO || s->state == S_READY) {
    ws_frame f;
    int r = ws_parse(s->in.p, s->in.n, OA_VOICE_MAX_FRAME_BYTES, &f);
    if (r == 0) return;
    if (r < 0) { s->state = S_CLOSING; return; }
    if (f.opcode == WS_CLOSE) {
      uint8_t fr[8];
      raw_out(s, fr, ws_build(fr, sizeof fr, WS_CLOSE, NULL, 0, false));
      s->state = S_CLOSING;
      return;
    }
    if (f.opcode == WS_PING) {
      uint8_t fr[140];
      raw_out(s, fr, ws_build(fr, sizeof fr, WS_PONG, f.payload, f.length, false));
    } else if (f.opcode == WS_BINARY || f.opcode == WS_TEXT || f.opcode == WS_CONT) {
      if (f.opcode != WS_CONT) { s->msg.n = 0; s->msg_op = f.opcode; }
      if (s->msg.n + f.length > OA_VOICE_MAX_FRAME_BYTES) { s->state = S_CLOSING; return; }
      bytes_put(&s->msg, f.payload, f.length);
      if (f.fin) {
        if (s->msg_op == WS_BINARY) on_frame(s, s->msg.p, s->msg.n);
        else send_error(s, OA_ERROR_PROTOCOL, "frames are binary messages", "");
        s->msg.n = 0;
      }
    }
    bytes_drop(&s->in, f.consumed);
  }
}

static void on_readable(session *s) {
  uint8_t tmp[65536];
  for (;;) {
    ssize_t r = recv(s->fd, tmp, sizeof tmp, 0);
    if (r > 0) {
      if (!bytes_put(&s->in, tmp, (size_t)r)) { s->state = S_CLOSING; return; }
      if (s->state == S_HTTP) handle_http(s);
      if (s->state == S_HELLO || s->state == S_READY) handle_ws(s);
      if (s->state == S_CLOSING) return;
      continue;
    }
    if (r == 0) { s->state = S_CLOSING; s->out.n = 0; return; }
    if (errno == EAGAIN || errno == EWOULDBLOCK) return;
    if (errno == EINTR) continue;
    s->state = S_CLOSING;
    s->out.n = 0;
    return;
  }
}

static void flush_out(session *s) {
  while (s->out.n) {
    ssize_t w = send(s->fd, s->out.p, s->out.n, 0);
    if (w > 0) { bytes_drop(&s->out, (size_t)w); continue; }
    if (w < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) return;
    if (w < 0 && errno == EINTR) continue;
    s->out.n = 0;
    s->state = S_CLOSING;
    return;
  }
}

static int listen_on(const char *spec, int *port_out) {
  char host[64] = "127.0.0.1";
  int port = 8787;
  const char *colon = strrchr(spec, ':');
  if (colon) {
    size_t hl = (size_t)(colon - spec);
    if (hl && hl < sizeof host) { memcpy(host, spec, hl); host[hl] = 0; }
    port = atoi(colon + 1);
  } else if (*spec) port = atoi(spec);
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  int one = 1;
  setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons((uint16_t)port)};
  if (inet_pton(AF_INET, host, &a.sin_addr) != 1) { logf_("bad listen address %s", spec); return -1; }
  if (bind(fd, (struct sockaddr *)&a, sizeof a) < 0 || listen(fd, 64) < 0) { logf_("cannot listen on %s: %s", spec, strerror(errno)); return -1; }
  socklen_t al = sizeof a;
  getsockname(fd, (struct sockaddr *)&a, &al);
  *port_out = ntohs(a.sin_port);
  fcntl(fd, F_SETFL, O_NONBLOCK);
  return fd;
}

static void usage(void) {
  fprintf(stderr,
          "usage: voice-host [--listen HOST:PORT] [--provider google|local] [--agent gemini|echo]\n"
          "                  [--model NAME] [--project ID] [--budget-msat N] [--max-speakers N] [--verbose]\n"
          "  default: --listen 127.0.0.1:8787 --provider google --agent gemini\n");
}

int main(int argc, char **argv) {
  const char *listen_spec = "127.0.0.1:8787";
  for (int i = 1; i < argc; i++) {
    const char *a = argv[i];
    const char *v = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--listen") && v) { listen_spec = v; i++; }
    else if (!strcmp(a, "--provider") && v) {
      if (!strcmp(v, "google")) g_provider = PROVIDER_GOOGLE;
      else if (!strcmp(v, "local")) g_provider = PROVIDER_LOCAL;
      else { usage(); return 2; }
      i++;
    } else if (!strcmp(a, "--agent") && v) {
      if (!strcmp(v, "gemini")) g_agent = AGENT_GEMINI;
      else if (!strcmp(v, "echo")) g_agent = AGENT_ECHO;
      else { usage(); return 2; }
      i++;
    } else if (!strcmp(a, "--model") && v) { snprintf(g_speech.model, sizeof g_speech.model, "%s", v); i++; }
    else if (!strcmp(a, "--project") && v) { snprintf(g_speech.project, sizeof g_speech.project, "%s", v); i++; }
    else if (!strcmp(a, "--budget-msat") && v) { g_budget_msat = strtoull(v, NULL, 10); i++; }
    else if (!strcmp(a, "--max-speakers") && v) {
      size_t n = strtoull(v, NULL, 10);
      g_max_speakers = n >= 1 && n <= sizeof accounts / sizeof accounts[0] ? n : g_max_speakers;
      i++;
    }
    else if (!strcmp(a, "--verbose")) g_verbose = true;
    else { usage(); return 2; }
  }
  signal(SIGPIPE, SIG_IGN);
  speech_init();
  g_speech.verbose = g_verbose;
  if (g_provider == PROVIDER_GOOGLE || g_agent == AGENT_GEMINI) {
    char err[256];
    if (!speech_google_ready(err, sizeof err)) {
      logf_("%s; use --provider local --agent echo for offline work", err);
      return 1;
    }
  }
  int port = 0;
  int lfd = listen_on(listen_spec, &port);
  if (lfd < 0) return 1;
  if (pipe(wake_pipe) < 0) return 1;
  fcntl(wake_pipe[0], F_SETFL, O_NONBLOCK);
  for (int i = 0; i < WORKERS; i++) {
    pthread_t t;
    pthread_create(&t, NULL, worker, NULL);
    pthread_detach(t);
  }
  warm();  // token and connections ready before the first turn
  printf("listening on 127.0.0.1:%d provider=%s agent=%s\n", port, g_provider == PROVIDER_GOOGLE ? "google" : "local",
         g_agent == AGENT_GEMINI ? g_speech.model : "echo");
  fflush(stdout);

  struct pollfd pfd[MAX_SESSIONS + 2];
  int idx[MAX_SESSIONS + 2];
  for (;;) {
    int np = 0;
    pfd[np].fd = lfd; pfd[np].events = POLLIN; idx[np++] = -1;
    pfd[np].fd = wake_pipe[0]; pfd[np].events = POLLIN; idx[np++] = -2;
    bool speaking = false;
    for (int i = 0; i < MAX_SESSIONS; i++) {
      session *s = sessions[i];
      if (!s) continue;
      pfd[np].fd = s->fd;
      pfd[np].events = (short)(POLLIN | (s->out.n ? POLLOUT : 0));
      idx[np++] = i;
      if (s->speaking) speaking = true;
    }
    int r = poll(pfd, (nfds_t)np, speaking ? 10 : 1000);
    if (r < 0 && errno != EINTR) { logf_("poll: %s", strerror(errno)); return 1; }
    for (int k = 0; r > 0 && k < np; k++) {
      if (!pfd[k].revents) continue;
      if (idx[k] == -1) {
        for (;;) {
          int fd = accept(lfd, NULL, NULL);
          if (fd < 0) break;
          int slot = -1;
          for (int i = 0; i < MAX_SESSIONS; i++) if (!sessions[i]) { slot = i; break; }
          if (slot < 0) { close(fd); continue; }
          fcntl(fd, F_SETFL, O_NONBLOCK);
          int one = 1;
          setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
#ifdef SO_NOSIGPIPE
          setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one);   // Linux has none: SIGPIPE is ignored (main)
#endif
          session *s = calloc(1, sizeof *s);
          if (!s) { close(fd); continue; }
          s->fd = fd;
          s->serial = next_serial++;
          snprintf(s->sid, sizeof s->sid, "s%llu-%08x", (unsigned long long)s->serial, arc4random());
          sessions[slot] = s;
        }
      } else if (idx[k] == -2) {
        char drain[256];
        while (read(wake_pipe[0], drain, sizeof drain) > 0) {}
      } else {
        session *s = sessions[idx[k]];
        if (pfd[k].revents & (POLLIN | POLLHUP | POLLERR)) on_readable(s);
      }
    }
    // Finished provider work.
    pthread_mutex_lock(&q_lock);
    job *done = done_head;
    done_head = done_tail = NULL;
    pthread_mutex_unlock(&q_lock);
    while (done) {
      job *nx = done->next;
      on_job_done(done);
      done = nx;
    }
    uint64_t now = now_ms();
    for (int i = 0; i < MAX_SESSIONS; i++) {
      session *s = sessions[i];
      if (!s) continue;
      if (s->state == S_READY) pump_speech(s, now);
      flush_out(s);
      if (s->state == S_CLOSING) {
        flush_out(s);
        session_close(i);
      }
    }
  }
}
