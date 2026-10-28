/*
 * metareal/platform/voice.h -- THE CONTRACT for Coax-native voice.
 *
 * This is the plain-C face of the voice engine. The voice host
 * (metareal/voice-host/) and the iOS app (apps/openagents-ios/OpenAgents/Voice/)
 * both code against exactly this file. Changes from here on are additive only:
 * new functions, new constants, new fields appended at the END of a struct
 * (callers zero-initialize structs, so a zero new field must mean "absent" or
 * "the old behavior"). Never renumber, reorder, or repurpose anything below.
 *
 * The logic lives in Coax (metareal/voice.coax, voice-wire.coax,
 * voice-floor.coax, voice-meter.coax); metareal/platform/voice.c only owns
 * storage and forwards to the Coax exports. Design: docs/voice/coax-native-voice.md.
 * Schema: proto/openagents/voice/v1alpha1/voice.proto. Every enum value below
 * is numbered exactly as that proto.
 *
 * Threading: an oa_voice is confined to one thread at a time (the audio
 * thread, typically). Nothing here allocates, locks, or does I/O, and every
 * per-frame call has a fixed worst-case cost (checked by `coax test --steps`).
 *
 * Audio is mono signed 16-bit PCM in host byte order (little-endian on every
 * platform we ship).
 */
#ifndef METAREAL_PLATFORM_VOICE_H
#define METAREAL_PLATFORM_VOICE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------ constants */

#define OA_VOICE_PROTOCOL 1            /* Hello.protocol / Welcome.protocol */
#define OA_VOICE_RATE 16000            /* processing (and STT) rate, Hz */
#define OA_VOICE_FRAME_MS 20
#define OA_VOICE_FRAME_SAMPLES 320     /* 20 ms at 16 kHz */
#define OA_VOICE_MAX_INPUT_SAMPLES 960 /* 20 ms at 48 kHz: the largest uplink frame */
#define OA_VOICE_MAX_FRAME_BYTES 16384 /* Frame.max_message_bytes */
#define OA_VOICE_MAX_AUDIO_BYTES 9600  /* Audio.data max_bytes: 200 ms of 24 kHz s16 */
#define OA_VOICE_MAX_DOWNLINK_SAMPLES 4800 /* one jitter-buffer packet: 200 ms at 24 kHz */
#define OA_VOICE_JITTER_SLOTS 16       /* jitter buffer capacity, packets */

/* Events returned (OR-ed) by oa_voice_uplink / oa_voice_downlink. */
#define OA_EV_TURN_START (1u << 0) /* speech began: a turn starts with this frame */
#define OA_EV_TURN_END   (1u << 1) /* the turn ended with this frame */
#define OA_EV_BARGE_IN   (1u << 2) /* speech over playback: stop playback (epoch bumped) */
#define OA_EV_FORWARD    (1u << 3) /* this uplink frame belongs to a turn: send it */
#define OA_EV_DECISION   (1u << 4) /* the built-in policy produced a decision: oa_voice_last_decision */
#define OA_EV_EPOCH      (1u << 5) /* the media epoch changed during this call: oa_voice_epoch */
#define OA_EV_TOO_LONG   (1u << 6) /* with TURN_END: the turn hit max_turn_ms */
#define OA_EV_DROPPED    (1u << 7) /* a DROP decision suppressed forwarding this frame */
#define OA_EV_UNDERRUN   (1u << 8) /* jitter pop: no packet ready, concealment/silence played */

/* ------------------------------------------------------------ enums (voice.proto) */

typedef enum { /* SpeakerRole */
  OA_SPEAKER_ROLE_UNSPECIFIED = 0,
  OA_SPEAKER_ROLE_PLAYER = 1,
  OA_SPEAKER_ROLE_NPC = 2,
  OA_SPEAKER_ROLE_AGENT = 3,
  OA_SPEAKER_ROLE_NARRATOR = 4
} oa_voice_speaker_role;

typedef enum { /* TurnPhase */
  OA_TURN_PHASE_UNSPECIFIED = 0,
  OA_TURN_PHASE_STARTED = 1,
  OA_TURN_PHASE_PARTIAL = 2,
  OA_TURN_PHASE_FINAL = 3,
  OA_TURN_PHASE_INTERRUPTED = 4,
  OA_TURN_PHASE_ENDED = 5
} oa_voice_turn_phase;

typedef enum { /* Codec */
  OA_CODEC_UNSPECIFIED = 0,
  OA_CODEC_PCM_S16LE = 1,
  OA_CODEC_OPUS = 2
} oa_voice_codec;

typedef enum { /* Client */
  OA_CLIENT_UNSPECIFIED = 0,
  OA_CLIENT_IOS = 1,
  OA_CLIENT_WEB = 2,
  OA_CLIENT_DEV = 3
} oa_voice_client;

typedef enum { /* ControlKind */
  OA_CONTROL_UNSPECIFIED = 0,
  OA_CONTROL_TURN_START = 1,
  OA_CONTROL_TURN_END = 2,
  OA_CONTROL_BARGE_IN = 3,
  OA_CONTROL_PLAYBACK = 4,
  OA_CONTROL_CANCEL = 5,
  OA_CONTROL_PING = 6,
  OA_CONTROL_PONG = 7,
  OA_CONTROL_SPEECH_END = 8
} oa_voice_control_kind;

typedef enum { /* ErrorCode */
  OA_ERROR_UNSPECIFIED = 0,
  OA_ERROR_PROTOCOL = 1,
  OA_ERROR_UNAUTHORIZED = 2,
  OA_ERROR_LIMIT = 3,
  OA_ERROR_PROVIDER = 4,
  OA_ERROR_TOO_LONG = 5,
  OA_ERROR_INTERNAL = 6
} oa_voice_error_code;

typedef enum { /* Stream */
  OA_STREAM_UNSPECIFIED = 0,
  OA_STREAM_UPLINK = 1,
  OA_STREAM_DOWNLINK = 2
} oa_voice_stream;

typedef enum { /* TurnState */
  OA_TURN_STATE_UNSPECIFIED = 0,
  OA_TURN_STATE_IDLE = 1,
  OA_TURN_STATE_SPEAKING = 2,
  OA_TURN_STATE_TRAILING = 3,
  OA_TURN_STATE_ENDED = 4
} oa_voice_turn_state;

typedef enum { /* PlaybackState */
  OA_PLAYBACK_STATE_UNSPECIFIED = 0,
  OA_PLAYBACK_STATE_SILENT = 1,
  OA_PLAYBACK_STATE_PLAYING = 2,
  OA_PLAYBACK_STATE_DUCKED = 3,
  OA_PLAYBACK_STATE_PAUSED = 4
} oa_voice_playback_state;

typedef enum { /* FrameAction */
  OA_FRAME_ACTION_UNSPECIFIED = 0,
  OA_FRAME_ACTION_HOLD_TURN = 1,
  OA_FRAME_ACTION_END_TURN = 2,
  OA_FRAME_ACTION_BARGE_IN = 3,
  OA_FRAME_ACTION_DUCK = 4,
  OA_FRAME_ACTION_PAUSE = 5,
  OA_FRAME_ACTION_RESUME = 6,
  OA_FRAME_ACTION_DROP = 7
} oa_voice_frame_action;

/* Frame oneof field numbers: oa_voice_msg.kind. 0 = empty/invalid. */
typedef enum {
  OA_MSG_NONE = 0,
  OA_MSG_HELLO = 1,
  OA_MSG_WELCOME = 2,
  OA_MSG_AUDIO = 3,
  OA_MSG_CONTROL = 4,
  OA_MSG_TURN_EVENT = 5,
  OA_MSG_SPEECH = 6,
  OA_MSG_ROUTE = 7,
  OA_MSG_METER = 8,
  OA_MSG_ERROR = 9,
  OA_MSG_FEATURES = 10,
  OA_MSG_DECISION = 11
} oa_voice_msg_kind;

/* ------------------------------------------------------------ value types */

/* A borrowed string or byte run. Not NUL-terminated. {NULL,0} = absent/empty.
 * After oa_voice_decode, ptr points into the decoded input buffer. */
typedef struct {
  const uint8_t *ptr;
  uint32_t len;
} oa_voice_bytes;

/* FrameFeatures: what voice.coax measured on one frame. */
typedef struct {
  uint32_t stream;          /* oa_voice_stream */
  uint32_t sequence;
  uint64_t media_epoch;
  float rms_db;             /* -120..0 dBFS */
  float peak_db;            /* -120..0 dBFS */
  float speech;             /* 0..1 */
  float noise_db;           /* dBFS */
  uint32_t turn_state;      /* oa_voice_turn_state */
  uint32_t playback_state;  /* oa_voice_playback_state */
  uint32_t speech_ms;
  uint32_t silence_ms;
  float zero_crossings;     /* 0..1 per sample */
} oa_voice_features;

/* FrameDecision presence bits (oa_voice_decision.has). A field whose bit is
 * clear is "omitted" in the proto sense. */
#define OA_DECISION_HAS_GAIN       (1u << 0)
#define OA_DECISION_HAS_DUCK       (1u << 1)
#define OA_DECISION_HAS_RATE       (1u << 2)
#define OA_DECISION_HAS_HOLD       (1u << 3)
#define OA_DECISION_HAS_CONFIDENCE (1u << 4) /* omitted confidence counts as 1.0 */
#define OA_DECISION_HAS_DEADLINE   (1u << 5) /* omitted deadline = no deadline */
#define OA_DECISION_HAS_APPLY_FROM (1u << 6) /* omitted = from the next frame */
#define OA_DECISION_HAS_EPOCH      (1u << 7) /* omitted = the current epoch */

/* FrameDecision: a bounded micro-change to a stream from apply_from_sequence
 * on. oa_voice_decide clamps every numeric field (gain -24..12 dB, duck
 * 0..40 dB, rate 0.8..1.25, hold 0..2000 ms, confidence 0..1). */
typedef struct {
  uint32_t has;               /* OA_DECISION_HAS_* */
  uint32_t stream;            /* oa_voice_stream */
  uint32_t apply_from_sequence;
  uint64_t media_epoch;
  uint32_t action;            /* oa_voice_frame_action */
  float gain_db;              /* absolute playback gain target, dB */
  float duck_db;              /* for DUCK: how far down, dB */
  float playback_rate;
  uint32_t hold_ms;           /* for HOLD_TURN: extra silence to wait */
  float confidence;
  oa_voice_bytes decider;     /* "voice.coax", "decisions:<model>" */
  uint32_t deadline_sequence;
} oa_voice_decision;

/* ------------------------------------------------------------ wire messages */

/* Nested messages: a has_* flag marks presence of a submessage. For scalar
 * fields, zero means omitted (encode skips zero scalars and empty strings);
 * the one exception is oa_voice_decision, which carries explicit `has` bits. */

typedef struct {
  oa_voice_bytes id;
  uint32_t role;              /* oa_voice_speaker_role */
  oa_voice_bytes profile;
  uint32_t profile_revision;
  oa_voice_bytes entity;
} oa_voice_speaker;

typedef struct {
  oa_voice_bytes id;
  oa_voice_bytes speaker;
  oa_voice_bytes text;        /* <= 600 bytes */
  uint32_t priority;
  bool interruptible;
  bool spatial;
  bool batch;
} oa_voice_utterance;

typedef struct {
  uint64_t recognized_ms;
  uint64_t synthesized_chars;
  uint64_t free_ms_left;
  uint64_t free_chars_left;
  uint64_t owed_msat;
} oa_voice_meter_report;

typedef struct {
  uint32_t protocol;
  uint32_t client;            /* oa_voice_client */
  bool has_speaker;
  oa_voice_speaker speaker;
  oa_voice_bytes route;
  uint32_t input_sample_rate;
  uint32_t output_sample_rate;
  uint32_t codec;             /* oa_voice_codec */
  oa_voice_bytes profile;
  oa_voice_bytes token;       /* never log */
  bool observe;
} oa_voice_hello;

typedef struct {
  uint32_t protocol;
  oa_voice_bytes session;
  uint32_t input_sample_rate;
  uint32_t output_sample_rate;
  oa_voice_bytes provider;    /* "google" | "local" */
  bool has_meter;
  oa_voice_meter_report meter;
  uint32_t max_turn_ms;
} oa_voice_welcome;

typedef struct {
  oa_voice_bytes turn;
  uint32_t sequence;
  uint32_t sample_rate;
  uint32_t codec;             /* oa_voice_codec */
  oa_voice_bytes data;        /* <= OA_VOICE_MAX_AUDIO_BYTES */
  uint64_t media_epoch;
  oa_voice_bytes utterance;
} oa_voice_audio;

typedef struct {
  uint32_t kind;              /* oa_voice_control_kind */
  oa_voice_bytes turn;
  uint64_t played_samples;
  uint64_t media_epoch;
  oa_voice_bytes utterance;
  uint64_t at_ms;
} oa_voice_control;

typedef struct {
  oa_voice_bytes turn;
  oa_voice_bytes speaker;
  uint32_t phase;             /* oa_voice_turn_phase */
  oa_voice_bytes text;
  uint64_t played_samples;
  uint64_t media_epoch;
} oa_voice_turn_event;

typedef struct {
  bool has_utterance;
  oa_voice_utterance utterance;
  bool has_speaker;
  oa_voice_speaker speaker;
  uint32_t sample_rate;
  uint64_t media_epoch;
  oa_voice_bytes turn;
} oa_voice_speech;

typedef struct {
  oa_voice_bytes turn;
  oa_voice_bytes target;
  oa_voice_bytes reason;      /* "floor" | "facing" | "default" */
} oa_voice_route;

typedef struct {
  uint32_t code;              /* oa_voice_error_code */
  oa_voice_bytes message;
  oa_voice_bytes turn;
} oa_voice_error;

/* One voice.v1alpha1.Frame. `kind` selects the union member. */
typedef struct {
  uint32_t kind;              /* oa_voice_msg_kind */
  union {
    oa_voice_hello hello;
    oa_voice_welcome welcome;
    oa_voice_audio audio;
    oa_voice_control control;
    oa_voice_turn_event turn_event;
    oa_voice_speech speech;
    oa_voice_route route;
    oa_voice_meter_report meter;
    oa_voice_error error;
    oa_voice_features features;
    oa_voice_decision decision;
  } u;
} oa_voice_msg;

/* ------------------------------------------------------------ the engine */

typedef struct oa_voice oa_voice; /* opaque; caller allocates oa_voice_size() bytes */

/* Tunables. oa_voice_init installs these defaults; zero fields passed to
 * oa_voice_configure keep the current value. All values are clamped. */
typedef struct {
  uint32_t end_silence_ms;   /* silence after speech that ends a turn (default 700, 200..3000) */
  uint32_t min_speech_ms;    /* speech shorter than this is not a turn (default 160, 40..2000) */
  uint32_t max_turn_ms;      /* a turn is force-ended here (default 30000, 1000..120000) */
  float confidence_floor;    /* decisions below this are ignored (default 0.5, 0..1) */
  float speech_threshold;    /* speech likelihood that counts as speech (default 0.6, 0.05..0.95) */
  uint32_t jitter_target;    /* packets buffered before playout starts (default 2, 1..8) */
} oa_voice_config;

size_t   oa_voice_size(void);
/* Zero v (oa_voice_size() bytes) first, or let init do it: init clears all
 * state. input_rate: 16000, 24000, 44100 or 48000; anything else is treated
 * as 16000. Epoch starts at 1. */
void     oa_voice_init(oa_voice *v, uint32_t input_rate);
void     oa_voice_configure(oa_voice *v, const oa_voice_config *c);
void     oa_voice_get_config(const oa_voice *v, oa_voice_config *out);
/* Samples in one 20 ms frame at the init input rate (320, 480, 882, 960). */
uint32_t oa_voice_input_frame_samples(const oa_voice *v);

/* Uplink: one 20 ms frame at input_rate in (oa_voice_input_frame_samples
 * samples); OA_VOICE_FRAME_SAMPLES samples at 16 kHz written to out16k (may
 * be NULL); features written to f (may be NULL). Runs the turn machine,
 * barge-in and the built-in policy, applies pending decisions. Returns OA_EV_*. */
uint32_t oa_voice_uplink(oa_voice *v, const int16_t *pcm, int16_t *out16k, oa_voice_features *f);

/* Pre-roll: on the frame that returned OA_EV_TURN_START, the 16 kHz audio of
 * the frames just before it (the speech onset that preceded the decision,
 * plus one leading frame), oldest first. Send it before that frame's audio
 * so recognition hears the first syllable. Copies up to cap samples, returns
 * the count (a multiple of OA_VOICE_FRAME_SAMPLES, at most
 * OA_VOICE_PREROLL_SAMPLES). Valid until the next oa_voice_uplink. */
#define OA_VOICE_PREROLL_SAMPLES 5120 /* 16 frames, 320 ms */
uint32_t oa_voice_preroll(const oa_voice *v, int16_t *out16k, uint32_t cap);

/* Downlink: reply audio about to play, any length n at `rate`. Applies the
 * gain/duck/pause state in place, ramped per sample (no clicks); pause writes
 * silence. Playback rate is state, not a stretch: n samples in are n samples
 * out, and the player reads oa_voice_playback_rate() and sets its own rate.
 * Features for the chunk go to f (may be NULL); one call is one downlink
 * sequence number. Returns OA_EV_* bits. */
uint32_t oa_voice_downlink(oa_voice *v, int16_t *pcm, uint32_t n, uint32_t rate, oa_voice_features *f);

/* Submit a decision from any decider. Returns false when dropped: wrong or
 * unspecified stream, stale epoch, past its deadline, confidence below the
 * floor, or an unknown action. A kept decision takes effect at
 * apply_from_sequence (a later one for the same stream replaces it). */
bool     oa_voice_decide(oa_voice *v, const oa_voice_decision *d);
/* The decision the built-in policy produced on the last OA_EV_DECISION.
 * decider = "voice.coax" (static storage). False if none yet. */
bool     oa_voice_last_decision(const oa_voice *v, oa_voice_decision *out);

/* Tell the engine whether reply audio is audible right now (drives barge-in
 * and PlaybackState). */
void     oa_voice_playing(oa_voice *v, bool playing);
uint64_t oa_voice_epoch(const oa_voice *v);
/* Adopt an epoch from the other side; the epoch never goes backwards.
 * Returns the resulting epoch. */
uint64_t oa_voice_set_epoch(oa_voice *v, uint64_t epoch);
/* Cut playback: bump the epoch, flush the jitter buffer. Returns the new epoch. */
uint64_t oa_voice_cut(oa_voice *v);
/* Drop the current uplink turn (no TURN_END follows). */
void     oa_voice_cancel_turn(oa_voice *v);

/* Current downlink state. */
float    oa_voice_gain_db(const oa_voice *v);      /* applied gain incl. duck */
float    oa_voice_playback_rate(const oa_voice *v);
uint32_t oa_voice_current_turn_state(const oa_voice *v);     /* oa_voice_turn_state */
uint32_t oa_voice_current_playback_state(const oa_voice *v); /* oa_voice_playback_state */
uint32_t oa_voice_uplink_sequence(const oa_voice *v);   /* next uplink frame's sequence */
uint32_t oa_voice_downlink_sequence(const oa_voice *v); /* next downlink chunk's sequence */
/* Duration of the current (or just-ended) turn in ms. */
uint32_t oa_voice_turn_ms(const oa_voice *v);

/* Jitter buffer for downlink packets (deterministic: output depends only on
 * the push/pop call sequence). push: one Audio packet's samples (n <=
 * OA_VOICE_MAX_DOWNLINK_SAMPLES) with its sequence and epoch; returns false
 * when dropped (stale epoch, duplicate, too old, too far ahead, too long).
 * Before playout starts, a late earlier packet rebases the buffer; an
 * empty, stopped buffer rebases on the next push; a newer epoch flushes.
 * pop: fills exactly n samples (n <= OA_VOICE_MAX_DOWNLINK_SAMPLES) in
 * sequence order; gaps are concealed by fading the last packet, then
 * silence. Returns OA_EV_UNDERRUN when nothing was ready, else 0. */
bool     oa_voice_jitter_push(oa_voice *v, uint32_t sequence, uint64_t epoch, const int16_t *pcm, uint32_t n);
uint32_t oa_voice_jitter_pop(oa_voice *v, int16_t *out, uint32_t n);
uint32_t oa_voice_jitter_depth(const oa_voice *v);   /* packets buffered */
void     oa_voice_jitter_flush(oa_voice *v);

/* ------------------------------------------------------------ the wire */

/* Encode one Frame into out (cap bytes). Returns bytes written; 0 = error
 * (unknown kind, a field over its proto bound, or cap too small). */
size_t   oa_voice_encode(uint8_t *out, size_t cap, const oa_voice_msg *m);
/* Decode one Frame (n <= OA_VOICE_MAX_FRAME_BYTES). Strings/bytes in m borrow
 * from `in`, which must outlive m. False = malformed, over a bound, or not
 * exactly one body. Unknown fields are skipped. */
bool     oa_voice_decode(const uint8_t *in, size_t n, oa_voice_msg *m);

/* ------------------------------------------------------------ floor (#authority) */

/* Who may be heard. Participants are small integer slots (0..OA_VOICE_FLOOR_SLOTS-1)
 * that the host maps to Speaker ids; route targets are small integer ids
 * (> 0) the host maps to agent/NPC ids or "players". A participant is heard
 * only with consent; a transcript goes to a target only through an explicit
 * binding. Media epochs are per floor and only move forward. */
#define OA_VOICE_FLOOR_SLOTS 8

typedef struct oa_voice_floor oa_voice_floor; /* caller allocates oa_voice_floor_size() bytes */
size_t   oa_voice_floor_size(void);
void     oa_voice_floor_init(oa_voice_floor *fl);
/* Record or withdraw consent for a slot. False for a bad slot. */
bool     oa_voice_floor_consent(oa_voice_floor *fl, uint32_t slot, bool consent);
/* Bind a slot's turns to a route target (> 0); target 0 unbinds. False for a
 * bad slot or a slot without consent (binding requires consent). */
bool     oa_voice_floor_bind(oa_voice_floor *fl, uint32_t slot, uint32_t target);
/* Grant the input floor to a slot (-1 releases it). Only one slot holds the
 * input floor; granting requires consent. False if refused. */
bool     oa_voice_floor_grant(oa_voice_floor *fl, int32_t slot);
/* May this slot's audio be heard (forwarded to recognition) now? */
bool     oa_voice_floor_may_hear(const oa_voice_floor *fl, uint32_t slot);
/* Where this slot's final transcript goes: the bound target, or 0 = nowhere. */
uint32_t oa_voice_floor_route(const oa_voice_floor *fl, uint32_t slot);
uint64_t oa_voice_floor_epoch(const oa_voice_floor *fl);
/* Bump the epoch (barge-in, cancel, reconnect). Returns the new epoch. */
uint64_t oa_voice_floor_bump(oa_voice_floor *fl);
/* Is audio stamped with this epoch current (playable)? */
bool     oa_voice_floor_epoch_ok(const oa_voice_floor *fl, uint64_t epoch);

/* ------------------------------------------------------------ meter (#authority) */

/* Per-account per-day speech metering. Free tier: 600000 ms recognized and
 * 5000 chars synthesized per day; beyond it, OA_VOICE_MSAT_PER_MS per
 * recognized ms and OA_VOICE_MSAT_PER_CHAR per synthesized char are owed.
 * A turn may run while the free tier lasts, or while the account's budget
 * (msat, set by the host from the account's balance) covers the worst case. */
#define OA_VOICE_FREE_MS_PER_DAY 600000ull
#define OA_VOICE_FREE_CHARS_PER_DAY 5000ull
#define OA_VOICE_MSAT_PER_MS 1ull
#define OA_VOICE_MSAT_PER_CHAR 30ull

typedef struct oa_voice_meter oa_voice_meter; /* caller allocates oa_voice_meter_size() bytes */
size_t   oa_voice_meter_size(void);
/* day: any monotonic day number (e.g. Unix days). budget_msat: spend allowed beyond the free tier. */
void     oa_voice_meter_init(oa_voice_meter *m, uint32_t day, uint64_t budget_msat);
/* Roll to a new day: a later day resets usage; an earlier day is ignored. */
void     oa_voice_meter_day(oa_voice_meter *m, uint32_t day);
void     oa_voice_meter_budget(oa_voice_meter *m, uint64_t budget_msat);
/* May a turn of up to max_ms recognized plus max_chars synthesized run now?
 * True iff what would be owed after it is <= the budget. */
bool     oa_voice_meter_may_run(const oa_voice_meter *m, uint64_t max_ms, uint64_t max_chars);
/* Record actual usage (it happened, so it always counts; counters
 * saturate). Returns false when the account is now past its free tier plus
 * budget: refuse further turns. */
bool     oa_voice_meter_charge(oa_voice_meter *m, uint64_t recognized_ms, uint64_t synthesized_chars);
void     oa_voice_meter_report_get(const oa_voice_meter *m, oa_voice_meter_report *out);

#ifdef __cplusplus
}
#endif
#endif /* METAREAL_PLATFORM_VOICE_H */
