// The browser's face over the voice engine (verse/voice*.coax through
// verse/platform/voice.c), as apps/openagents-ios/OpenAgents/Voice is the
// phone's: the engine decides turns, barge-in, gain and the wire; voice.js
// only moves samples and bytes. One engine per page, in this module's memory.
//
// Strings and samples cross in fixed buffers: JS writes into vw_in (input
// samples), vw_text (a message's strings, back to back) or vw_frame (a frame
// from the socket); encoders write the frame to send into vw_out and return
// its length; vw_decode fills the fields read with vw_field and vw_string.
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "voice.h"

#define EXPORT(name) __attribute__((export_name(name)))

static oa_voice *v;
static uint32_t in_rate = 16000;
static int16_t in_buf[OA_VOICE_MAX_INPUT_SAMPLES];
static int16_t frame16k[OA_VOICE_FRAME_SAMPLES];
static int16_t preroll[OA_VOICE_PREROLL_SAMPLES];
static int16_t play[OA_VOICE_MAX_DOWNLINK_SAMPLES];
static uint8_t text[2048];
static uint8_t frame[OA_VOICE_MAX_FRAME_BYTES];
static uint8_t out[OA_VOICE_MAX_FRAME_BYTES];
static oa_voice_features feat;
static oa_voice_msg got;
static uint32_t sequence;

EXPORT("vw_in") int16_t *vw_in(void) { return in_buf; }
EXPORT("vw_text") uint8_t *vw_text(void) { return text; }
EXPORT("vw_frame") uint8_t *vw_frame(void) { return frame; }
EXPORT("vw_out") uint8_t *vw_out(void) { return out; }
EXPORT("vw_play") int16_t *vw_play(void) { return play; }

// A fresh engine for input at rate (16, 24, 44.1 or 48 kHz; else 16 kHz).
// Returns the samples in one 20 ms input frame. The media epoch moves on.
EXPORT("vw_start") uint32_t vw_start(uint32_t rate) {
  uint64_t was = v ? oa_voice_epoch(v) : 0;
  if (!v) v = calloc(1, oa_voice_size());
  else memset(v, 0, oa_voice_size());
  in_rate = rate == 16000 || rate == 24000 || rate == 44100 || rate == 48000 ? rate : 16000;
  oa_voice_init(v, in_rate);
  if (was) oa_voice_set_epoch(v, was + 1);
  sequence = 0;
  return oa_voice_input_frame_samples(v);
}
EXPORT("vw_rate") uint32_t vw_rate(void) { return in_rate; }

// One input frame from vw_in: OA_EV_* bits; the 16 kHz frame is kept for vw_audio.
EXPORT("vw_uplink") uint32_t vw_uplink(void) { return oa_voice_uplink(v, in_buf, frame16k, &feat); }
EXPORT("vw_level") float vw_level(void) { return feat.rms_db; }
EXPORT("vw_turn_state") uint32_t vw_turn_state(void) { return feat.turn_state; }
EXPORT("vw_preroll") uint32_t vw_preroll(void) { return oa_voice_preroll(v, preroll, OA_VOICE_PREROLL_SAMPLES); }
EXPORT("vw_epoch") double vw_epoch(void) { return (double)oa_voice_epoch(v); }
EXPORT("vw_set_epoch") void vw_set_epoch(double e) { oa_voice_set_epoch(v, (uint64_t)e); }
EXPORT("vw_playing") void vw_playing(int on) { oa_voice_playing(v, on != 0); }
EXPORT("vw_cut") void vw_cut(void) { oa_voice_cut(v); }
EXPORT("vw_cancel_turn") void vw_cancel_turn(void) { oa_voice_cancel_turn(v); }
EXPORT("vw_flush") void vw_flush(void) { oa_voice_jitter_flush(v); }
EXPORT("vw_depth") uint32_t vw_depth(void) { return oa_voice_jitter_depth(v); }

static oa_voice_bytes at(uint32_t from, uint32_t len) {
  return (oa_voice_bytes){len ? text + from : NULL, len};
}

// Hello: vw_text holds the speaker id (speaker bytes) then the route (route bytes).
EXPORT("vw_hello") uint32_t vw_hello(uint32_t speaker, uint32_t route, uint32_t out_rate) {
  oa_voice_msg m = {.kind = OA_MSG_HELLO};
  m.u.hello.protocol = OA_VOICE_PROTOCOL;
  m.u.hello.client = OA_CLIENT_WEB;
  m.u.hello.has_speaker = true;
  m.u.hello.speaker.id = at(0, speaker);
  m.u.hello.speaker.role = OA_SPEAKER_ROLE_PLAYER;
  m.u.hello.route = at(speaker, route);
  m.u.hello.input_sample_rate = OA_VOICE_RATE;
  m.u.hello.output_sample_rate = out_rate;
  m.u.hello.codec = OA_CODEC_PCM_S16LE;
  return (uint32_t)oa_voice_encode(out, sizeof out, &m);
}

// Control: vw_text holds the turn (turn bytes) then the utterance (utterance bytes).
EXPORT("vw_control") uint32_t vw_control(uint32_t kind, uint32_t turn, uint32_t utterance, double played, double at_ms) {
  oa_voice_msg m = {.kind = OA_MSG_CONTROL};
  m.u.control.kind = kind;
  m.u.control.turn = at(0, turn);
  m.u.control.utterance = at(turn, utterance);
  m.u.control.played_samples = (uint64_t)played;
  m.u.control.media_epoch = oa_voice_epoch(v);
  m.u.control.at_ms = (uint64_t)at_ms;
  return (uint32_t)oa_voice_encode(out, sizeof out, &m);
}

// Route: whom the speaker faces, the target in vw_text (target bytes; 0 for
// nobody). The page's own binding (a conversation), never read from speech.
EXPORT("vw_route") uint32_t vw_route(uint32_t target) {
  oa_voice_msg m = {.kind = OA_MSG_ROUTE};
  m.u.route.target = at(0, target);
  m.u.route.reason = (oa_voice_bytes){(const uint8_t *)"facing", 6};
  return (uint32_t)oa_voice_encode(out, sizeof out, &m);
}

// Speech: the page's answer for the host to speak, vw_text holding the turn
// it answers (turn bytes) then the words (text bytes, at most 600).
EXPORT("vw_say") uint32_t vw_say(uint32_t turn, uint32_t words) {
  oa_voice_msg m = {.kind = OA_MSG_SPEECH};
  m.u.speech.has_utterance = true;
  m.u.speech.utterance.id = at(0, turn);   // required; the host names the utterance it speaks
  m.u.speech.utterance.text = at(turn, words);
  m.u.speech.turn = at(0, turn);
  return (uint32_t)oa_voice_encode(out, sizeof out, &m);
}

// Audio for the turn in vw_text (turn bytes): the last uplink frame (from 0),
// or count pre-roll samples from offset (from 1).
EXPORT("vw_audio") uint32_t vw_audio(uint32_t turn, int from, uint32_t offset, uint32_t count) {
  const int16_t *pcm = from ? preroll + offset : frame16k;
  uint32_t n = from ? count : OA_VOICE_FRAME_SAMPLES;
  oa_voice_msg m = {.kind = OA_MSG_AUDIO};
  m.u.audio.turn = at(0, turn);
  m.u.audio.sequence = sequence++;
  m.u.audio.sample_rate = OA_VOICE_RATE;
  m.u.audio.codec = OA_CODEC_PCM_S16LE;
  m.u.audio.data = (oa_voice_bytes){(const uint8_t *)pcm, n * 2};
  m.u.audio.media_epoch = oa_voice_epoch(v);
  return (uint32_t)oa_voice_encode(out, sizeof out, &m);
}

// A frame from the socket, in vw_frame: its OA_MSG_* kind, or 0 when malformed.
EXPORT("vw_decode") uint32_t vw_decode(uint32_t n) {
  memset(&got, 0, sizeof got);
  if (n > sizeof frame || !oa_voice_decode(frame, n, &got)) return 0;
  return got.kind;
}

// The decoded frame's numbers: 0 phase or kind or code, 1 sequence, 2 sample
// rate, 3 media epoch, 4 audio samples, 5 max turn ms (Welcome).
EXPORT("vw_field") double vw_field(int i) {
  switch (got.kind) {
  case OA_MSG_TURN_EVENT: return i == 0 ? got.u.turn_event.phase : i == 3 ? (double)got.u.turn_event.media_epoch : 0;
  case OA_MSG_CONTROL: return i == 0 ? got.u.control.kind : i == 3 ? (double)got.u.control.media_epoch : 0;
  case OA_MSG_ERROR: return i == 0 ? got.u.error.code : 0;
  case OA_MSG_SPEECH: return i == 2 ? got.u.speech.sample_rate : i == 3 ? (double)got.u.speech.media_epoch : 0;
  case OA_MSG_WELCOME: return i == 2 ? got.u.welcome.output_sample_rate : i == 5 ? got.u.welcome.max_turn_ms : 0;
  case OA_MSG_AUDIO:
    return i == 1 ? got.u.audio.sequence : i == 2 ? got.u.audio.sample_rate : i == 3 ? (double)got.u.audio.media_epoch
         : i == 4 ? got.u.audio.data.len / 2 : 0;
  }
  return 0;
}

// The decoded frame's strings, copied into vw_text: 0 its text (a transcript,
// a reply, an error), 1 the utterance, 2 the turn, 3 who speaks (Speech).
// Returns the length.
EXPORT("vw_string") uint32_t vw_string(int i) {
  oa_voice_bytes b = {0};
  switch (got.kind) {
  case OA_MSG_TURN_EVENT: b = i == 0 ? got.u.turn_event.text : i == 2 ? got.u.turn_event.turn : b; break;
  case OA_MSG_SPEECH:
    b = i == 0 && got.u.speech.has_utterance ? got.u.speech.utterance.text
      : i == 1 && got.u.speech.has_utterance ? got.u.speech.utterance.id : i == 2 ? got.u.speech.turn
      : i == 3 && got.u.speech.has_speaker ? got.u.speech.speaker.id : b;
    break;
  case OA_MSG_CONTROL: b = i == 1 ? got.u.control.utterance : i == 2 ? got.u.control.turn : b; break;
  case OA_MSG_AUDIO: b = i == 1 ? got.u.audio.utterance : i == 2 ? got.u.audio.turn : b; break;
  case OA_MSG_ERROR: b = i == 0 ? got.u.error.message : b; break;
  case OA_MSG_WELCOME: b = i == 0 ? got.u.welcome.provider : i == 1 ? got.u.welcome.session : b; break;
  }
  uint32_t n = b.len < sizeof text ? b.len : sizeof text;
  if (n) memcpy(text, b.ptr, n);
  return n;
}

// The decoded Audio's samples into the jitter buffer; false when dropped.
EXPORT("vw_push") int vw_push(void) {
  if (got.kind != OA_MSG_AUDIO) return 0;
  uint32_t n = got.u.audio.data.len / 2;
  if (!n || n > OA_VOICE_MAX_DOWNLINK_SAMPLES) return 0;
  int16_t pcm[OA_VOICE_MAX_DOWNLINK_SAMPLES];
  memcpy(pcm, got.u.audio.data.ptr, n * 2);
  return oa_voice_jitter_push(v, got.u.audio.sequence, got.u.audio.media_epoch, pcm, n);
}

// n reply samples at rate into vw_play, in order, with the engine's gain,
// duck and pause applied. Returns OA_EV_UNDERRUN when nothing was ready.
EXPORT("vw_pop") uint32_t vw_pop(uint32_t n, uint32_t rate) {
  if (n > OA_VOICE_MAX_DOWNLINK_SAMPLES) n = OA_VOICE_MAX_DOWNLINK_SAMPLES;
  uint32_t ev = oa_voice_jitter_pop(v, play, n);
  oa_voice_downlink(v, play, n, rate, NULL);
  return ev;
}
