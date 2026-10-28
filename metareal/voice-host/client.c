// voice-client: the dev test client. Streams a WAV to the voice host at real
// time in 20 ms frames, deciding turns with oa_voice_uplink exactly as the
// phone does, prints every event, and saves the reply audio. See docs/voice/dev.md.
#include "../platform/voice.h"
#include "wav.h"
#include "ws.h"
#include <arpa/inet.h>
#include <ctype.h>
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

static uint64_t now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (uint64_t)t.tv_sec * 1000u + (uint64_t)t.tv_nsec / 1000000u;
}

static oa_voice_bytes B(const char *s) {
  oa_voice_bytes r = {(const uint8_t *)s, s ? (uint32_t)strlen(s) : 0};
  return r;
}

static int fd = -1;
static bool quiet = false;

static bool send_all(const uint8_t *p, size_t n) {
  while (n) {
    ssize_t w = send(fd, p, n, 0);
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) return false;
    p += w;
    n -= (size_t)w;
  }
  return true;
}

static bool send_msg(const oa_voice_msg *m) {
  uint8_t enc[OA_VOICE_MAX_FRAME_BYTES], fr[OA_VOICE_MAX_FRAME_BYTES + 16];
  size_t n = oa_voice_encode(enc, sizeof enc, m);
  if (!n) { fprintf(stderr, "voice-client: encode failed (kind %u)\n", m->kind); return false; }
  return send_all(fr, ws_build(fr, sizeof fr, WS_BINARY, enc, n, true));
}

static const char *phase_name(uint32_t p) {
  static const char *n[] = {"UNSPECIFIED", "STARTED", "PARTIAL", "FINAL", "INTERRUPTED", "ENDED"};
  return p < 6 ? n[p] : "?";
}
static const char *action_name(uint32_t a) {
  static const char *n[] = {"KEEP", "HOLD_TURN", "END_TURN", "BARGE_IN", "DUCK", "PAUSE", "RESUME", "DROP"};
  return a < 8 ? n[a] : "?";
}

// Lowercase, letters/digits/spaces only, single spaces: for --expect.
static void normalize(const char *in, char *out, size_t cap) {
  size_t o = 0;
  bool space = true;
  for (; *in && o + 1 < cap; in++) {
    unsigned char c = (unsigned char)*in;
    if (isalnum(c)) { out[o++] = (char)tolower(c); space = false; }
    else if (!space) { out[o++] = ' '; space = true; }
  }
  while (o && out[o - 1] == ' ') o--;
  out[o] = 0;
}

static void usage(void) {
  fprintf(stderr,
          "usage: voice-client [--url ws://127.0.0.1:8787/] [--observe] [--out reply.wav] [--expect TEXT]\n"
          "                    [--barge FILE.wav] [--turns N] [--timeout SECONDS] [--rate-out HZ]\n"
          "                    [--profile ID] [--speaker ID] [--decide-gain DB] [--quiet] FILE.wav\n");
}

typedef struct {
  int finals, partials, speeches, speech_updates, speech_ends, interrupted, errors, features, decisions, stale, audio_in, meters;
  size_t downlink_samples;
  int peak;
  bool matched;
} tally;

int main(int argc, char **argv) {
  const char *url = "ws://127.0.0.1:8787/", *wav = NULL, *barge = NULL, *out_path = NULL, *expect = NULL;
  const char *profile = "", *speaker = "dev";
  bool observe = false, have_gain = false;
  float gain_db = 0;
  int turns = 0, timeout_s = 30;
  uint32_t rate_out = 24000;
  for (int i = 1; i < argc; i++) {
    const char *a = argv[i], *v = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--url") && v) { url = v; i++; }
    else if (!strcmp(a, "--observe")) observe = true;
    else if (!strcmp(a, "--quiet")) quiet = true;
    else if (!strcmp(a, "--out") && v) { out_path = v; i++; }
    else if (!strcmp(a, "--expect") && v) { expect = v; i++; }
    else if (!strcmp(a, "--barge") && v) { barge = v; i++; }
    else if (!strcmp(a, "--turns") && v) { turns = atoi(v); i++; }
    else if (!strcmp(a, "--timeout") && v) { timeout_s = atoi(v); i++; }
    else if (!strcmp(a, "--rate-out") && v) { rate_out = (uint32_t)atoi(v); i++; }
    else if (!strcmp(a, "--profile") && v) { profile = v; i++; }
    else if (!strcmp(a, "--speaker") && v) { speaker = v; i++; }
    else if (!strcmp(a, "--decide-gain") && v) { have_gain = true; gain_db = (float)atof(v); i++; }
    else if (a[0] != '-' && !wav) wav = a;
    else { usage(); return 2; }
  }
  if (!wav) { usage(); return 2; }
  if (!turns) turns = barge ? 2 : 1;

  uint32_t rate = 0, brate = 0;
  size_t ns = 0, nb = 0;
  int16_t *pcm = wav_read(wav, &rate, &ns);
  if (!pcm) { fprintf(stderr, "voice-client: cannot read %s (16-bit PCM WAV)\n", wav); return 2; }
  int16_t *bpcm = NULL;
  if (barge) {
    bpcm = wav_read(barge, &brate, &nb);
    if (!bpcm || brate != rate) { fprintf(stderr, "voice-client: cannot read %s at %u Hz\n", barge, rate); return 2; }
  }
  oa_voice *v = calloc(1, oa_voice_size());
  oa_voice_init(v, rate);
  uint32_t fs = oa_voice_input_frame_samples(v);

  // ws://host:port/path
  char host[128] = "127.0.0.1", path[256] = "/";
  int port = 8787;
  if (strncmp(url, "ws://", 5) != 0) { fprintf(stderr, "voice-client: only ws:// URLs\n"); return 2; }
  const char *h = url + 5, *slash = strchr(h, '/'), *colon = strchr(h, ':');
  size_t hl = slash ? (size_t)(slash - h) : strlen(h);
  if (colon && (!slash || colon < slash)) { hl = (size_t)(colon - h); port = atoi(colon + 1); }
  if (hl >= sizeof host) return 2;
  memcpy(host, h, hl);
  host[hl] = 0;
  if (slash) snprintf(path, sizeof path, "%s", slash);

  struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *ai = NULL;
  char ps[16];
  snprintf(ps, sizeof ps, "%d", port);
  if (getaddrinfo(host, ps, &hints, &ai) != 0) { fprintf(stderr, "voice-client: cannot resolve %s\n", host); return 1; }
  fd = socket(AF_INET, SOCK_STREAM, 0);
  if (connect(fd, ai->ai_addr, ai->ai_addrlen) < 0) { fprintf(stderr, "voice-client: cannot connect to %s: %s\n", url, strerror(errno)); return 1; }
  freeaddrinfo(ai);
  int one = 1;
  setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
  setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one);
  char req[512];
  send_all((const uint8_t *)req, ws_client_request(host, port, path, req, sizeof req));
  static uint8_t in[1 << 20];
  size_t in_n = 0;
  for (;;) {
    ssize_t r = recv(fd, in + in_n, sizeof in - in_n, 0);
    if (r <= 0) { fprintf(stderr, "voice-client: handshake failed\n"); return 1; }
    in_n += (size_t)r;
    uint8_t *end = memmem(in, in_n, "\r\n\r\n", 4);
    if (end) {
      size_t hn = (size_t)(end - in) + 4;
      if (!ws_client_accepts((const char *)in, hn)) { fprintf(stderr, "voice-client: upgrade refused\n"); return 1; }
      memmove(in, in + hn, in_n - hn);
      in_n -= hn;
      break;
    }
  }

  oa_voice_msg hello = {.kind = OA_MSG_HELLO};
  hello.u.hello.protocol = OA_VOICE_PROTOCOL;
  hello.u.hello.client = OA_CLIENT_DEV;
  hello.u.hello.has_speaker = true;
  hello.u.hello.speaker.id = B(speaker);
  hello.u.hello.speaker.role = OA_SPEAKER_ROLE_PLAYER;
  hello.u.hello.route = B("foreman");
  hello.u.hello.input_sample_rate = OA_VOICE_RATE;  // we send voice.coax's 16 kHz output
  hello.u.hello.output_sample_rate = rate_out;
  hello.u.hello.codec = OA_CODEC_PCM_S16LE;
  hello.u.hello.profile = B(profile);
  hello.u.hello.observe = observe;
  send_msg(&hello);

  tally t = {0};
  int16_t *reply = NULL;
  size_t reply_n = 0, reply_cap = 0;
  char final_text[700] = "", last_utt[65] = "";
  bool welcomed = false, in_turn = false, playing = false, barge_started = false, barge_done = !barge;
  char turn[16] = "";
  uint32_t turn_no = 0, up_seq = 0;
  uint64_t play_start = 0, play_end = 0, played_ok = 0;
  size_t pos = 0, bpos = 0;
  uint64_t t0 = now_ms(), next_frame = 0, done_at = 0;
  // Latency from this client's TURN_END to the FINAL and to the first reply audio.
  uint64_t t_turn_end = 0, t_final = 0;
  bool want_first_audio = false;
  int16_t frame[OA_VOICE_MAX_INPUT_SAMPLES];
  int rc = 0;

  for (;;) {
    uint64_t now = now_ms();
    if (now - t0 > (uint64_t)timeout_s * 1000) { fprintf(stderr, "voice-client: timed out\n"); rc = 1; break; }
    int wait = welcomed ? (int)(next_frame > now ? next_frame - now : 0) : 100;
    struct pollfd p = {.fd = fd, .events = POLLIN};
    int pr = poll(&p, 1, wait);
    if (pr > 0) {
      ssize_t r = recv(fd, in + in_n, sizeof in - in_n, 0);
      if (r <= 0) { fprintf(stderr, "voice-client: connection closed\n"); rc = 1; break; }
      in_n += (size_t)r;
      for (;;) {
        ws_frame f;
        int k = ws_parse(in, in_n, sizeof in, &f);
        if (k < 0) { fprintf(stderr, "voice-client: bad WebSocket frame\n"); return 1; }
        if (k == 0) break;
        oa_voice_msg m;
        memset(&m, 0, sizeof m);
        if (f.opcode == WS_BINARY && oa_voice_decode(f.payload, f.length, &m)) {
          now = now_ms();
          switch (m.kind) {
          case OA_MSG_WELCOME:
            welcomed = true;
            next_frame = now;
            if (!quiet) printf("welcome session=%.*s provider=%.*s in=%u out=%u max_turn_ms=%u free_ms_left=%llu\n",
                               (int)m.u.welcome.session.len, m.u.welcome.session.ptr, (int)m.u.welcome.provider.len,
                               m.u.welcome.provider.ptr, m.u.welcome.input_sample_rate, m.u.welcome.output_sample_rate,
                               m.u.welcome.max_turn_ms, (unsigned long long)m.u.welcome.meter.free_ms_left);
            if (have_gain) {
              oa_voice_msg d = {.kind = OA_MSG_DECISION};
              d.u.decision.has = OA_DECISION_HAS_GAIN | OA_DECISION_HAS_CONFIDENCE;
              d.u.decision.stream = OA_STREAM_DOWNLINK;
              d.u.decision.action = OA_FRAME_ACTION_UNSPECIFIED;
              d.u.decision.gain_db = gain_db;
              d.u.decision.confidence = 1.0f;
              d.u.decision.decider = B("dev-client");
              send_msg(&d);
              if (!quiet) printf("sent decision DOWNLINK gain_db=%.1f\n", gain_db);
            }
            break;
          case OA_MSG_TURN_EVENT: {
            oa_voice_turn_event *e = &m.u.turn_event;
            if (!quiet) printf("turn %.*s %s%s%.*s%s epoch=%llu\n", (int)e->turn.len, e->turn.ptr, phase_name(e->phase),
                               e->text.len ? " \"" : "", (int)e->text.len, e->text.ptr, e->text.len ? "\"" : "",
                               (unsigned long long)e->media_epoch);
            if (e->phase == OA_TURN_PHASE_FINAL) {
              t.finals++;
              t_final = now;
              want_first_audio = true;
              snprintf(final_text, sizeof final_text, "%.*s", (int)e->text.len, e->text.ptr);
              if (expect) {
                char a[700], b[700];
                normalize(final_text, a, sizeof a);
                normalize(expect, b, sizeof b);
                if (strstr(a, b)) t.matched = true;
              }
            }
            if (e->phase == OA_TURN_PHASE_PARTIAL) t.partials++;
            if (e->phase == OA_TURN_PHASE_INTERRUPTED) t.interrupted++;
            break;
          }
          case OA_MSG_ROUTE:
            if (!quiet) printf("route %.*s -> %.*s (%.*s)\n", (int)m.u.route.turn.len, m.u.route.turn.ptr,
                               (int)m.u.route.target.len, m.u.route.target.ptr, (int)m.u.route.reason.len, m.u.route.reason.ptr);
            break;
          case OA_MSG_SPEECH: {
            // A repeated utterance id is the same reply with more text known
            // (the host resends Speech as a streamed reply grows): an update.
            bool update = m.u.speech.utterance.id.len == strlen(last_utt) &&
                          !memcmp(m.u.speech.utterance.id.ptr, last_utt, m.u.speech.utterance.id.len);
            if (update) t.speech_updates++;
            else {
              t.speeches++;
              snprintf(last_utt, sizeof last_utt, "%.*s", (int)m.u.speech.utterance.id.len, m.u.speech.utterance.id.ptr);
            }
            oa_voice_set_epoch(v, m.u.speech.media_epoch);
            if (!quiet) printf("%s %.*s \"%.*s\" rate=%u epoch=%llu\n", update ? "speech-update" : "speech", (int)m.u.speech.utterance.id.len,
                               m.u.speech.utterance.id.ptr, (int)m.u.speech.utterance.text.len, m.u.speech.utterance.text.ptr,
                               m.u.speech.sample_rate, (unsigned long long)m.u.speech.media_epoch);
            break;
          }
          case OA_MSG_AUDIO: {
            oa_voice_audio *a = &m.u.audio;
            if (a->media_epoch < oa_voice_epoch(v)) { t.stale++; break; }  // cut by barge-in: never played
            uint32_t k = a->data.len / 2;
            if (k > OA_VOICE_MAX_DOWNLINK_SAMPLES || a->sample_rate != rate_out) { fprintf(stderr, "voice-client: bad downlink audio\n"); rc = 1; break; }
            int16_t chunk[OA_VOICE_MAX_DOWNLINK_SAMPLES];
            memcpy(chunk, a->data.ptr, k * 2);
            oa_voice_features ff;
            oa_voice_downlink(v, chunk, k, rate_out, &ff);
            if (reply_n + k > reply_cap) {
              reply_cap = (reply_n + k) * 2;
              reply = realloc(reply, reply_cap * 2);
            }
            memcpy(reply + reply_n, chunk, k * 2);
            reply_n += k;
            for (uint32_t i = 0; i < k; i++) {
              int s = chunk[i] < 0 ? -chunk[i] : chunk[i];
              if (s > t.peak) t.peak = s;
            }
            t.audio_in++;
            if (want_first_audio && t_turn_end) {
              want_first_audio = false;
              printf("latency final_ms=%llu first_audio_ms=%llu (from TURN_END)\n",
                     (unsigned long long)(t_final - t_turn_end), (unsigned long long)(now - t_turn_end));
            }
            t.downlink_samples += k;
            if (play_end < now) { play_end = now; if (!playing) play_start = now; }
            play_end += (uint64_t)k * 1000 / rate_out;
            played_ok += k;
            break;
          }
          case OA_MSG_CONTROL:
            if (m.u.control.kind == OA_CONTROL_SPEECH_END) {
              t.speech_ends++;
              if (!quiet) printf("speech_end %.*s played_samples=%llu epoch=%llu\n", (int)m.u.control.utterance.len,
                                 m.u.control.utterance.ptr, (unsigned long long)m.u.control.played_samples,
                                 (unsigned long long)m.u.control.media_epoch);
            } else if (m.u.control.kind == OA_CONTROL_PONG && !quiet) printf("pong\n");
            break;
          case OA_MSG_METER:
            t.meters++;
            if (!quiet) printf("meter recognized_ms=%llu synthesized_chars=%llu free_ms_left=%llu free_chars_left=%llu owed_msat=%llu\n",
                               (unsigned long long)m.u.meter.recognized_ms, (unsigned long long)m.u.meter.synthesized_chars,
                               (unsigned long long)m.u.meter.free_ms_left, (unsigned long long)m.u.meter.free_chars_left,
                               (unsigned long long)m.u.meter.owed_msat);
            break;
          case OA_MSG_ERROR:
            t.errors++;
            printf("error code=%u \"%.*s\" turn=%.*s\n", m.u.error.code, (int)m.u.error.message.len, m.u.error.message.ptr,
                   (int)m.u.error.turn.len, m.u.error.turn.ptr);
            break;
          case OA_MSG_FEATURES: {
            oa_voice_features *x = &m.u.features;
            t.features++;
            if (!quiet) printf("features %s seq=%u rms=%.1f peak=%.1f speech=%.2f noise=%.1f turn=%u play=%u speech_ms=%u silence_ms=%u zc=%.3f\n",
                               x->stream == OA_STREAM_UPLINK ? "up" : "down", x->sequence, x->rms_db, x->peak_db, x->speech,
                               x->noise_db, x->turn_state, x->playback_state, x->speech_ms, x->silence_ms, x->zero_crossings);
            break;
          }
          case OA_MSG_DECISION: {
            oa_voice_decision *d = &m.u.decision;
            t.decisions++;
            if (!quiet) printf("decision %s %s from=%u decider=%.*s\n", d->stream == OA_STREAM_UPLINK ? "up" : "down",
                               action_name(d->action), d->apply_from_sequence, (int)d->decider.len, d->decider.ptr);
            break;
          }
          default: break;
          }
        } else if (f.opcode == WS_CLOSE) {
          fprintf(stderr, "voice-client: host closed the connection\n");
          rc = 1;
        } else if (f.opcode == WS_BINARY) {
          fprintf(stderr, "voice-client: malformed frame from host\n");
          rc = 1;
        }
        memmove(in, in + f.consumed, in_n - f.consumed);
        in_n -= f.consumed;
      }
      if (rc) break;
    }
    now = now_ms();
    if (!welcomed || now < next_frame) continue;
    next_frame += OA_VOICE_FRAME_MS;
    if (next_frame + 200 < now) next_frame = now;  // stalled (debugger); don't burst

    // Playback model: audio plays at real time from when it arrives.
    bool was = playing;
    playing = now < play_end;
    if (playing != was) oa_voice_playing(v, playing);

    // The next 20 ms of microphone: the WAV, then (with --barge, once reply
    // audio has played 300 ms) the barge-in WAV, then silence.
    if (pos < ns) {
      size_t k = ns - pos < fs ? ns - pos : fs;
      memset(frame, 0, sizeof frame);
      memcpy(frame, pcm + pos, k * 2);
      pos += k;
    } else if (barge && !barge_done && (barge_started || (playing && now - play_start >= 300))) {
      barge_started = true;
      size_t k = nb - bpos < fs ? nb - bpos : fs;
      memset(frame, 0, sizeof frame);
      memcpy(frame, bpcm + bpos, k * 2);
      bpos += k;
      if (bpos >= nb) barge_done = true;
    } else {
      memset(frame, 0, sizeof frame);
    }
    int16_t out16k[OA_VOICE_FRAME_SAMPLES];
    oa_voice_features f;
    uint32_t ev = oa_voice_uplink(v, frame, out16k, &f);
    if (ev & OA_EV_BARGE_IN) {
      uint64_t heard = (now - play_start) * rate_out / 1000;
      if (heard > played_ok) heard = played_ok;
      oa_voice_msg c = {.kind = OA_MSG_CONTROL};
      c.u.control.kind = OA_CONTROL_BARGE_IN;
      c.u.control.media_epoch = oa_voice_epoch(v);
      c.u.control.played_samples = heard;
      send_msg(&c);
      play_end = now;
      playing = false;
      oa_voice_playing(v, false);
      if (!quiet) printf("barge-in epoch=%llu played_samples=%llu\n", (unsigned long long)oa_voice_epoch(v), (unsigned long long)heard);
    }
    if (ev & OA_EV_TURN_START) {
      snprintf(turn, sizeof turn, "t%u", ++turn_no);
      in_turn = true;
      oa_voice_msg c = {.kind = OA_MSG_CONTROL};
      c.u.control.kind = OA_CONTROL_TURN_START;
      c.u.control.turn = B(turn);
      c.u.control.media_epoch = oa_voice_epoch(v);
      send_msg(&c);
      int16_t pre[OA_VOICE_PREROLL_SAMPLES];
      uint32_t pn = oa_voice_preroll(v, pre, OA_VOICE_PREROLL_SAMPLES);
      for (uint32_t off = 0; off < pn;) {
        uint32_t k = pn - off > OA_VOICE_MAX_AUDIO_BYTES / 2 ? OA_VOICE_MAX_AUDIO_BYTES / 2 : pn - off;
        oa_voice_msg a = {.kind = OA_MSG_AUDIO};
        a.u.audio.turn = B(turn);
        a.u.audio.sequence = up_seq++;
        a.u.audio.sample_rate = OA_VOICE_RATE;
        a.u.audio.codec = OA_CODEC_PCM_S16LE;
        a.u.audio.data.ptr = (const uint8_t *)(pre + off);
        a.u.audio.data.len = k * 2;
        a.u.audio.media_epoch = oa_voice_epoch(v);
        send_msg(&a);
        off += k;
      }
      if (!quiet) printf("turn-start %s at %u ms of input (pre-roll %u ms)\n", turn, (unsigned)((pos + bpos) * 1000 / rate), pn / 16);
    }
    if (in_turn && (ev & (OA_EV_FORWARD | OA_EV_TURN_START))) {
      oa_voice_msg a = {.kind = OA_MSG_AUDIO};
      a.u.audio.turn = B(turn);
      a.u.audio.sequence = up_seq++;
      a.u.audio.sample_rate = OA_VOICE_RATE;
      a.u.audio.codec = OA_CODEC_PCM_S16LE;
      a.u.audio.data.ptr = (const uint8_t *)out16k;
      a.u.audio.data.len = sizeof out16k;
      a.u.audio.media_epoch = oa_voice_epoch(v);
      send_msg(&a);
    }
    if (in_turn && (ev & OA_EV_TURN_END)) {
      in_turn = false;
      oa_voice_msg c = {.kind = OA_MSG_CONTROL};
      c.u.control.kind = OA_CONTROL_TURN_END;
      c.u.control.turn = B(turn);
      send_msg(&c);
      t_turn_end = now_ms();
      if (!quiet) printf("turn-end %s (%u ms)%s\n", turn, oa_voice_turn_ms(v), ev & OA_EV_TOO_LONG ? " too long" : "");
    }
    if (observe && (ev & OA_EV_DECISION) && !quiet) {
      oa_voice_decision d;
      if (oa_voice_last_decision(v, &d)) printf("client-decision up %s\n", action_name(d.action));
    }

    bool sources_done = pos >= ns && barge_done;
    if (!done_at && sources_done && !in_turn && !playing && t.speech_ends + t.interrupted >= turns) done_at = now;
    if (done_at && now - done_at > 300) break;
  }

  printf("summary finals=%d partials=%d speeches=%d speech_updates=%d speech_ends=%d interrupted=%d audio_in=%d downlink_samples=%zu "
         "stale=%d features=%d decisions=%d meters=%d errors=%d peak=%d\n",
         t.finals, t.partials, t.speeches, t.speech_updates, t.speech_ends, t.interrupted, t.audio_in, t.downlink_samples, t.stale,
         t.features, t.decisions, t.meters, t.errors, t.peak);
  if (out_path && reply_n) {
    if (!wav_write(out_path, reply, reply_n, rate_out)) { fprintf(stderr, "voice-client: cannot write %s\n", out_path); rc = 1; }
    else if (!quiet) printf("wrote %s (%zu samples at %u Hz)\n", out_path, reply_n, rate_out);
  }
  if (!t.finals) { fprintf(stderr, "voice-client: no final transcript\n"); rc = 1; }
  if (t.errors) { fprintf(stderr, "voice-client: the host sent %d error(s)\n", t.errors); rc = 1; }
  if (expect && !t.matched) { fprintf(stderr, "voice-client: no final transcript contains \"%s\" (last: \"%s\")\n", expect, final_text); rc = 1; }
  close(fd);
  return rc;
}
