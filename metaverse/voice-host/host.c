// The voice host: one process, one thread for sockets and the audio path,
// worker threads for provider calls. See docs/voice/dev.md.
//
// Wire: RFC 6455 WebSocket on --listen (default 127.0.0.1:8787); each binary
// message is one openagents.voice.v1alpha1.Frame (oa_voice_encode/decode from
// metaverse/platform/voice.h). GET /health answers "ok" over plain HTTP.
//
// The voice engine (voice.coax via voice.h) is confined to the main thread:
// every oa_voice*, oa_voice_floor* and oa_voice_meter* call happens here.
// Workers only do HTTP (speech.c) and hand results back through a pipe.
#include "../platform/voice.h"
#include "speech.h"
#include "ws.h"
#include "VoiceReply.h"
#include "VoiceAsr.h"
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
#define QUIET_RESUME 0.3f       // a barged reply goes on at this gain until its words say echo or someone
#define PARTIAL_EVERY 16000     // re-recognize for a PARTIAL every ~1 s of new 16 kHz audio
#define HISTORY 10             // exchanges of conversation the agent sees
#define OUT_LIMIT (2u << 20)    // a client this far behind is dropped
#define TARGET_ROUTE 1          // the floor's id for the session's Hello.route

// Recognition and synthesis are configured apart (--recognizer, --synthesizer);
// --provider X sets both to X, as it always has.
static speech_provider g_recognizer = PROVIDER_GOOGLE, g_synth = PROVIDER_GOOGLE;
static const char *one_name(speech_provider p) {
  return p == PROVIDER_GOOGLE ? "google" : p == PROVIDER_PSIONIC ? "psionic" : p == PROVIDER_PARAKEET ? "parakeet" : "local";
}
// Welcome.provider: one name when one provider serves both, as before;
// otherwise "RECOGNIZER+SYNTHESIZER", e.g. "parakeet+local".
static const char *provider_name(void) {
  static char both[32];
  if (g_recognizer == g_synth) return one_name(g_recognizer);
  snprintf(both, sizeof both, "%s+%s", one_name(g_recognizer), one_name(g_synth));
  return both;
}
// The echo handling for a phone's loudspeaker (quiet turns, resuming a cut
// reply) assumes real recognition of Google's synthesized replies.
static bool echo_aware(void) { return g_recognizer == PROVIDER_GOOGLE && g_synth == PROVIDER_GOOGLE; }
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

#include "host-jobs.h"
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
  struct { int16_t *pcm; size_t n; bool ready; size_t end; } seg[MAX_SEGS];  // end: its text's end in text
  int nseg, appended;
  size_t base;               // text already in pcm before the segments (a resume's)
  char overflow[MAX_TEXT + 1];  // sentences past MAX_SEGS - 1, rendered as one
  bool text_done;            // the agent has finished (or failed)
  bool started;              // Speech sent
  // Latency, ms since the monotonic epoch.
  uint64_t t_end, t_final, t_sentence, t_audio;
  const char *plan;
  uint64_t gap_us;           // playback gaps: audio that wasn't ready in time
  bool quiet;                // a barged reply going on quietly until the barge-in's words decide
  char who[129];             // who says it: the page's character (Route "facing"), else "" for the route
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
  // Whom the page says this speaker faces (a client Route, reason "facing"):
  // "" while nobody, and the route's own agent answers. While someone, the
  // page answers each final transcript with that character's own words (the
  // same character chat as typed questions) in a client Speech for the turn.
  char facing[129];
  uint32_t page_turn_no;   // the final transcript the page answers next (0: none)
  char page_turn[65];
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
  uint8_t *sp;             // each frame of tb: the engine's speech likelihood times 255 (metaverse/voice-asr.coax)
  size_t nvoiced;          // frames of tb at or over the speech threshold
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
  uint8_t *fin_sp;         // its frames' speech likelihoods
  size_t fin_nvoiced;
  job *asr[4];             // Parakeet recognitions in flight, for cancel and barge-in
  int nasr;
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
  size_t cut_rest;            // where cut_text's unrendered sentences start
  uint64_t cut_at;            // now_ms() of the cut; 0: nothing kept
  int cut_resumes;            // resumes in a row, so an echo can't cut it forever
  uint32_t echo_turn;         // a turn already taken for the echo from its partial: its final is dropped
  bool barge_wait;            // a barge-in came before its turn: the quiet resume starts with the turn
  // Turns that started while a reply played and never barged in: the phone's
  // own engine judged them too weak to interrupt, so they are the reply's
  // echo or the room, not someone talking (deliver_final drops them).
  uint32_t quiet[8];
  int nquiet;
  uint64_t spoke_at;          // now_ms() when the last reply finished playing here
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
  free(s->fin_sp);
  s->fin_sp = NULL;
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
  free(s->sp);
  free(s->v);
  free(s->fl);
  if (s->state != S_HTTP) logf_("%s: closed", s->sid);
  free(s);
  sessions[idx] = NULL;
}

static uint32_t ms_of(size_t samples) { return (uint32_t)(samples * 1000 / OA_VOICE_RATE); }

#include "host-turn.h"
#include "host-dispatch.h"
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
          "usage: voice-host [--listen HOST:PORT] [--provider google|psionic|local] [--agent gemini|echo]\n"
          "                  [--recognizer google|psionic|parakeet|local] [--synthesizer google|local]\n"
          "                  [--parakeet-model DIR] [--parakeet-device gpu|cpu]   (parakeet)\n"
          "                  [--model NAME] [--project ID] [--budget-msat N] [--max-speakers N] [--verbose]\n"
          "                  [--whisper-model PATH] [--words A,B,C]   (psionic)\n"
          "  default: --listen 127.0.0.1:8787 --provider google --agent gemini\n");
}

int main(int argc, char **argv) {
  const char *listen_spec = "127.0.0.1:8787";
  bool parakeet_device_set = false;
  for (int i = 1; i < argc; i++) {
    const char *a = argv[i];
    const char *v = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--listen") && v) { listen_spec = v; i++; }
    else if (!strcmp(a, "--provider") && v) {
      if (!strcmp(v, "google")) g_recognizer = g_synth = PROVIDER_GOOGLE;
      else if (!strcmp(v, "local")) g_recognizer = g_synth = PROVIDER_LOCAL;
      else if (!strcmp(v, "psionic")) g_recognizer = g_synth = PROVIDER_PSIONIC;
      else { usage(); return 2; }   // parakeet does not synthesize: use --recognizer parakeet
      i++;
    } else if (!strcmp(a, "--recognizer") && v) {
      if (!strcmp(v, "google")) g_recognizer = PROVIDER_GOOGLE;
      else if (!strcmp(v, "local")) g_recognizer = PROVIDER_LOCAL;
      else if (!strcmp(v, "psionic")) g_recognizer = PROVIDER_PSIONIC;
      else if (!strcmp(v, "parakeet")) g_recognizer = PROVIDER_PARAKEET;
      else { usage(); return 2; }
      i++;
    } else if (!strcmp(a, "--synthesizer") && v) {
      if (!strcmp(v, "google")) g_synth = PROVIDER_GOOGLE;
      else if (!strcmp(v, "local")) g_synth = PROVIDER_LOCAL;
      else if (!strcmp(v, "parakeet")) g_synth = PROVIDER_PARAKEET;   // refused below: recognition only
      else { usage(); return 2; }
      i++;
    } else if (!strcmp(a, "--parakeet-model") && v) {
      snprintf(g_speech.parakeet_model, sizeof g_speech.parakeet_model, "%s", v);
      i++;
    } else if (!strcmp(a, "--parakeet-device") && v) {
      if (!strcmp(v, "gpu")) g_speech.parakeet_gpu = 1;
      else if (!strcmp(v, "cpu")) g_speech.parakeet_gpu = 0;
      else { usage(); return 2; }
      parakeet_device_set = true;
      i++;
    } else if (!strcmp(a, "--agent") && v) {
      if (!strcmp(v, "gemini")) g_agent = AGENT_GEMINI;
      else if (!strcmp(v, "echo")) g_agent = AGENT_ECHO;
      else { usage(); return 2; }
      i++;
    } else if (!strcmp(a, "--whisper-model") && v) { snprintf(g_speech.whisper_model, sizeof g_speech.whisper_model, "%s", v); i++; }
    else if (!strcmp(a, "--words") && v) { snprintf(g_speech.whisper_words, sizeof g_speech.whisper_words, "%s", v); i++; }
    else if (!strcmp(a, "--model") && v) { snprintf(g_speech.model, sizeof g_speech.model, "%s", v); i++; }
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
  if (!metaverse_voice_asr_synthesizes(g_synth)) {
    logf_("%s does not synthesize speech; choose --synthesizer google or local", one_name(g_synth));
    return 2;
  }
  // The GPU unless --parakeet-device cpu: Metal on macOS, CUDA on Linux
  // (GitHub #12672). Without one the host refuses to start rather than run
  // the slow CPU path unasked.
  if (!parakeet_device_set) g_speech.parakeet_gpu = 1;
  if (g_recognizer == PROVIDER_PARAKEET) {
    char err[512];
    uint64_t t0 = now_ms();
    if (!speech_parakeet_open(err, sizeof err)) {
      logf_("%s (%s); see psionic/docs/parakeet.md for the pinned model", err, g_speech.parakeet_model);
      return 1;
    }
    logf_("parakeet resident on the %s in %llu ms", g_speech.parakeet_gpu ? "GPU" : "CPU", (unsigned long long)(now_ms() - t0));
  }
  if (g_recognizer == PROVIDER_PSIONIC) {
    char err[256];
    if (!speech_psionic_open(err, sizeof err)) {
      logf_("%s (%s); fetch it with: openagents transcribe fetch base.en", err, g_speech.whisper_model);
      return 1;
    }
  }
  if (g_recognizer == PROVIDER_GOOGLE || g_synth == PROVIDER_GOOGLE || g_agent == AGENT_GEMINI) {
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
  printf("listening on 127.0.0.1:%d provider=%s agent=%s\n", port, provider_name(),
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
