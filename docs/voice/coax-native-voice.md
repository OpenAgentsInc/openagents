# Coax-native voice: a proposal

*Draft, 2026-10-28. Nothing here is built yet. The owner decided all
thirteen questions on 2026-10-28; each is marked **Decided** below.*

This proposes one voice system for OpenAgents, written in Coax, with its
media path and server on machines we operate. Speech recognition and
speech synthesis use Google's models (owner decision, 2026-10-28). It covers two things people will do with their
voice: talk to agents (the "just talk and see the right actions happen"
mode from [episode 251](../transcripts/251.md)) and talk to each other in
the Verse. It also covers a third, slower use: rendering spoken lines for
NPCs and films.

We have built voice about twenty times since 2024. The
[2026-10-21 voice audit](#reading-the-history) records what each attempt
taught us. This proposal starts from those lessons and from the reset
(`0b401561a0`), which removed every line of the Rust voice stack the audit
recommended building on. What's left is Coax, Metareal, the protocol, and
our own GCP project. That turns out to be a good place to start.

## What the owner has asked for

In the owner's own words, across the transcripts:

- One toggle, then a two-way stream: "no start/stop/wait-for-transcription,
  just one toggle on bidirectional stream so you just talk and see the
  right actions happen" ([251](../transcripts/251.md)).
- First-class voice, without the product being described as voice-first
  ([184](../transcripts/184.md)).
- A free basic version and a paid one "with best TTS/STT for high-quality
  Jarvis-y replies" ([251](../transcripts/251.md)).
- Off provider infrastructure: a provider session "can't be moved between
  nodes", and voice sessions must survive the node they were born on
  ([271](../transcripts/271.md)). OpenAI Realtime refused Sarah's custom
  voice ([270](../transcripts/270.md)). The standing rule for vendors is
  "use, never depend" ([279](../transcripts/279.md)).
- Owner direction recorded on 2026-10-21: the owned engine is the product
  default, not a fallback, and voices are assets that their owners license
  and get paid for.

## What we learned

The audit is long. These are the lessons that shape the design:

1. **Hosted media servers cost more than they returned.** Self-hosted
   LiveKit took 13.7 to 15.2 s to the first audio. Its GKE cluster
   (`oa-livekit-prod`, 6 nodes) and `oa-livekit-redis` HA instance kept
   running for three months after the code was deleted, until we deleted
   them on 2026-10-28.
2. **Chained cloud speech-to-text was slow.** Google STT finals took 2.2 to
   6.2 s. Local models were fast and worked first time: whisper.cpp and
   Parakeet TDT both did, with no account and no audio leaving the machine.
3. **The voice model must not answer for itself.** In August a speaker
   model claimed an action it never took. Since #12059 the voice layer only
   relays to a real agent. Keep that.
4. **Turn detection was always someone else's.** Every system used push to
   talk or a provider's voice activity detector. None owned the decision
   "you've finished speaking", which is the part that most decides how a
   conversation feels.
5. **Numbers got lost.** Every latency figure came from a receipt deleted
   with its system. The features that worked at the reset had no numbers.
6. **Almost nothing was tested on a real phone with real speech.**
7. **Rewrites, not moves.** Working parts died because they lived inside
   host apps that were deleted.

Lesson 7 is the strongest argument for Coax. A voice core written as Coax
libraries with C headers can sit under the iOS app, a Linux server and,
later, other shells, the way the camera and wallet already do. It can't
die with one app.

## Where we are today

**Voice code** (status 2026-10-28; this section was written before any of it existed):

- The schema: `proto/openagents/voice/v1alpha1/voice.proto`, including
  per-frame `FrameFeatures` and `FrameDecision`.
- The Coax engine: `metareal/voice.coax`, `voice-wire.coax`, and the two
  `#authority` files `voice-floor.coax` and `voice-meter.coax` (zero
  surviving mutants), behind the C face `metareal/platform/voice.h`.
- The dev voice host: `metareal/voice-host/` (C), running on this Mac only.
  It calls Google Chirp 3, Chirp 3 HD and Vertex Gemini. How to run it:
  [dev.md](dev.md).
- The phone: `apps/openagents-ios/OpenAgents/Voice/`, with the voice toggle
  on the home. `tools/voice-sim-test.sh` passes in the simulator against the
  Mac's host, offline and live. It has not run on a physical iPhone yet.
- [INVARIANTS.md](../../INVARIANTS.md) now allows the microphone through the
  voice toggle only, and audio leaves the phone only inside a turn and only
  to our voice host.

**Coax** (`coax0` 0.10.0) gives us:

- Fixed-width integers including `s16`, `f32`/`f64` with no fused
  operations, fixed arrays and views, structs, enums, `defer`.
- `#export_c` libraries with generated headers that assert layout. A view
  crosses to C as `{data, count}`, so the host can hand Coax a buffer of
  samples without copying.
- No heap, no hidden allocation, no globals. On an audio thread this is a
  feature: a Coax procedure can't allocate or take a lock behind your back.
- `#test`, `#forall`, `coax test --steps N` (a bounded step budget) and
  `#authority` with `coax mutate`. Decision code with zero surviving mutants
  is how the wallet was built.
- `#import_proto`: generated protobuf codecs, the same path the home uses
  for `HudInvocation`.
- `coax metal`: an f32/bool shader profile, emitted as Metal.

It doesn't yet have SIMD, jobs or threads, pointers, dynamic arrays, or
`#foreign` imports that take views (`#foreign` is scalar-only). Those gaps
decide where the line between Coax and the host falls.

**Metareal** already does the shape of spatial sound. `haunt.coax`
computes a bounded, symmetric gain and pan for each sound from the eye and
heading (`hh_gain`, `hh_pan`, with a `#test` for exactly that), and the
Swift host plays what Coax decided.

**Protocol** pieces exist but carry no audio:

- [NIP-LIVE](../../nips/openagents/NIP-LIVE.md) and `openagents.live.v1`
  (spec §23.6, status Partial). It separates whose microphone reaches an
  agent (input floors) from when the agent may speak (speaking floors).
  It also defines consent records, media epochs that fence stale audio
  after a reconnect, and barge-in that records how far playback got. It
  leaves the media transport to adapters.
- `openagents.verse.media.v1` (Verse §10.23): `SpatialAudioZone {volume,
  live_session, falloff_m}`. `verse.doc.v1` has up to 16 `Voice` entries
  for NPC lines, and authoring commands can come from `COMMAND_ORIGIN_VOICE`.
- Private record range 1100 to 1199 is reserved for live plans, captures
  and transcripts.

**Infrastructure we run** (GCP project `openagentsgemini`, checked today):

| Machine | What it is | Relevance |
| --- | --- | --- |
| `oa-pylon-gpu-1` | g2-standard-8, one L4, us-central1-a, running | Not needed: speech runs on Google's models, so the voice host needs no GPU |
| `coder-sync-1` | e2-standard-2 behind the global HTTPS load balancer, 24-hour WebSocket timeout | The pattern for a long-lived socket service |
| `clef-decisions-h100`, `clef-decisions-a100` | GPU hosts running decision models | Not for voice; shows we can hold GPUs |
| TPU v6e-1, asia-northeast1-b | Held permanently | Too far from US users for live audio |
| `coderos-4080`, `macbook-pro-m5` | The owner's machines | Development and the first measurements |
| `oa-livekit-prod`, `oa-livekit-redis` | Leftover LiveKit | Deleted 2026-10-28 (decision 1) |

## The design

```
 iPhone (Swift host + Coax)                 Voice host (C host + Coax), our GCP
 ───────────────────────────                ─────────────────────────────────
 mic ─► VoiceProcessingIO (AEC) ─► [] s16 ─► voice.coax                   ┌──► agent relay
                                  frames    level, VAD, end of turn,     │    (saved agent,
                                            barge-in, jitter buffer      │     Foreman)
                                               │                         │
                                  voice-wire.coax frames ── socket ──►  voice-wire.coax
                                               ▲                         │
 speaker ◄─ playback ◄─ voice-space.coax ◄──── frames ◄──────────────── ├──► Google STT (Chirp 3)
            gain/pan from where people stand                             ├──► Google TTS (Chirp 3 HD)
                                                                         └──► forwarder: who hears
                                            voice-floor.coax (#authority)     whom (voice-space.coax)
                                            voice-meter.coax (#authority)
```

The same Coax libraries run on both ends. The host on each side does only
what Coax can't: talk to the audio hardware, open sockets, and call
Google's speech APIs.

### What Coax owns and what the host owns

| Concern | Owner | Why |
| --- | --- | --- |
| Capture, playback, audio session, route changes (AirPods, car) | Swift host | Platform effects, per the Coax-first rule |
| Echo cancellation, gain control, noise suppression | Apple's VoiceProcessingIO on the phone | It's on the device, free, and hard to beat. Writing our own AEC is a later choice, not a start |
| Frames, ring buffers, resampling 48 kHz to 16 kHz, levels | `voice.coax` | Plain loops over views; the filter table is built at compile time with `#run` |
| Voice activity and end of turn | `voice.coax` | The part every past system rented. We own it and test it on recorded speech |
| Barge-in: stop playback when the person talks over the agent | `voice.coax` | A small state machine with `#forall` checks |
| Jitter buffer and packet-loss concealment | `voice.coax` | Deterministic, replayable from a packet trace |
| Wire frames (audio, control, transcript, floor) | `voice-wire.coax` via `#import_proto` | No second hand-written codec, like the home |
| Consent, input floors, speaking floors, media epochs | `voice-floor.coax`, `#authority` | It decides who may be heard, so it gets the wallet's rules: zero surviving mutants |
| Who hears whom in the Verse; gain and pan | `voice-space.coax` | Grows out of `haunt.coax`'s `hh_gain`/`hh_pan` |
| Metering speech minutes and characters, prices, free-tier limits | `voice-meter.coax`, `#authority` | It decides money |
| Sockets, TLS | C host on the server, Swift on the phone | TLS ends at the load balancer, like `sync.openagents.com` |
| Codec (Opus) | Host C, if we choose Opus (decision 5) | `#foreign` can't pass views yet, so the host encodes |
| Speech recognition and synthesis | Google Cloud Speech-to-Text v2 (Chirp 3) and Text-to-Speech (Chirp 3 HD), called by the voice host | Owner decision. The phone never talks to Google directly, and Google credentials stay on the server |

An audio callback runs every 10 to 20 ms on a real-time thread and must
never block. We'd add one rule for any Coax procedure called there: it
must have a `#test` that runs it on a worst-case frame under
`coax test --steps`, so its cost is bounded and checked on every build.

### Voice primitives: one set for every speaker

**Decided:** many different players, NPCs and agents will speak and
listen, so the system has no special case for any of them. It's a small
set of primitives that every surface reuses, defined in the protocol so
they're reliable across the phone, the voice host, world documents and
other clients.

Anything that speaks or listens is a **speaker**. A player, an NPC, an
agent, a narrator and a film's voiceover are all speakers that differ only
in their bindings:

| Primitive | What it is | Where it lives |
| --- | --- | --- |
| `VoiceProfile` | A named, versioned voice: the Google voice, language, speaking rate, pitch, volume and a short style note. Published revisions never change; a new sound is a new revision | `openagents.voice.v1` (new) |
| `VoiceCatalog` | The profiles a world, app or account may use, with limits | `openagents.voice.v1` |
| `Speaker` | Who is speaking: a player's key, an NPC id or an agent id, its role, its profile, and the world entity its voice comes from (or none, for a flat narrator) | `openagents.voice.v1` |
| `Utterance` | Something a speaker says: text, speaker, priority, whether it can be interrupted, spatial or flat, and live or batch | `openagents.voice.v1` |
| `Render` | The audio made for an utterance, keyed by a hash of profile revision and text, so the same line is never synthesized twice | `openagents.voice.v1` |
| `TurnEvent` | What happened in a turn: started, partial text, final text, interrupted (with how far playback got), ended | `openagents.voice.v1` |
| `Route` | Where a final transcript goes: an agent, an NPC's brain, or other players. Chosen by explicit binding (whose floor it is, who you're facing), never by reading the words | `openagents.voice.v1` |
| Consent, floors, participants, media epochs | Who may be heard and when | `openagents.live.v1` (exists) |
| `Voice` in a world document | An NPC's voice and lines. Gains a `VoiceProfile` reference beside its existing `LineSet` | `verse.doc.v1` (exists) |
| `SpatialAudioZone` | Areas where voices carry or stop | `verse.media.v1` (exists) |

A sketch of the new package (field numbers and limits get settled in V1,
with compatibility snapshots like every published contract):

```proto
package openagents.voice.v1;

message VoiceProfile {
  optional string id = 1;              // stable, e.g. "foreman"
  optional uint32 revision = 2;        // published revisions are immutable
  optional string provider_voice = 3;  // the Google Chirp 3 HD voice name
  optional string language = 4;        // BCP-47, e.g. "en-US"
  optional float speaking_rate = 5;    // 0.25-2.0, default 1.0
  optional float pitch = 6;            // semitones, -20..20
  optional float volume_gain_db = 7;   // -96..16
  optional string style = 8;           // short direction note, <= 160 bytes
}
message VoiceCatalog { repeated VoiceProfile profiles = 1; } // <= 64

enum SpeakerRole { SPEAKER_ROLE_UNSPECIFIED = 0; SPEAKER_ROLE_PLAYER = 1;
  SPEAKER_ROLE_NPC = 2; SPEAKER_ROLE_AGENT = 3; SPEAKER_ROLE_NARRATOR = 4; }
message Speaker {
  optional string id = 1;              // player pubkey, NPC id or agent id
  SpeakerRole role = 2;
  optional string profile = 3;         // VoiceProfile id
  optional uint32 profile_revision = 4;
  optional string entity = 5;          // world entity the voice comes from; empty = flat
}

message Utterance {
  optional string id = 1;
  optional string speaker = 2;
  optional string text = 3;            // <= 600 bytes, like an NPC AskReply
  optional uint32 priority = 4;        // higher interrupts lower
  optional bool interruptible = 5;
  optional bool spatial = 6;
  optional bool batch = 7;             // render ahead and cache, not live
}
message Render { optional bytes key = 1; optional string utterance = 2;
  optional uint32 sample_rate = 3; optional uint64 samples = 4; }

enum TurnPhase { TURN_PHASE_UNSPECIFIED = 0; TURN_PHASE_STARTED = 1;
  TURN_PHASE_PARTIAL = 2; TURN_PHASE_FINAL = 3; TURN_PHASE_INTERRUPTED = 4;
  TURN_PHASE_ENDED = 5; }
message TurnEvent { optional string turn = 1; optional string speaker = 2;
  TurnPhase phase = 3; optional string text = 4;
  optional uint64 played_samples = 5; optional uint64 media_epoch = 6; }

message Route { optional string turn = 1; optional string target = 2;
  optional string reason = 3; }        // e.g. "floor", "facing"; never parsed from speech
```

The Coax side gets these as generated codecs through `#import_proto`, so
`voice.coax` and the voice host share one definition, the way the home
shares `HudInvocation`. A new character in the Verse is then just data: a
`VoiceProfile` in the world's catalog, a `Speaker` bound to its entity,
and a `Route` to whatever brain answers for it.

### Talking to an agent

1. You tap the voice toggle once. The phone captures, and `voice.coax`
   runs voice activity detection and turn detection on each 20 ms frame.
2. Audio streams to the voice host while you speak, so speech-to-text
   produces partial text as you go and the screen shows it.
3. When `voice.coax` decides you've finished (silence plus a learned turn
   check, decision 6), the final transcript goes to the agent as an
   ordinary message, the same relay as #12059. The voice layer never
   answers, acts or claims anything on its own.
4. The agent's reply comes back as text. A spoken summary (aethyr-cpp
   capped these at 30 words) goes to text-to-speech, and audio streams
   back as it's made.
5. If you talk over it, the phone stops playback within one frame and
   tells the host how far playback got, as NIP-LIVE requires. The agent
   hears that it was interrupted.

The same flow serves any speaker. In the Grid, walking up to the Foreman
and talking is one binding: being near its body grants the input floor,
and its `Route` goes to its agent. A global voice toggle is another
binding of the same primitives, routed to your default agent. Neither is
built into the system.

### Talking to each other in the Verse

Player voice needs no speech models at all, only moving audio between
people. The voice host also runs a forwarder: each client sends one
stream up; the forwarder sends each listener the streams they should hear.
It doesn't decode or mix. "Who should hear whom" is a Coax decision made
from positions the realm already replicates (64 m radius today) and from
`SpatialAudioZone`s. Each phone then places voices in space with
`voice-space.coax`.

Because the server is ours and every client connects out to it, we need
no TURN servers and no peer-to-peer NAT traversal. That's most of what
made WebRTC stacks heavy.

### Lines for NPCs and films

Lines in a `verse.doc` `Voice`, NPC answers, and film voiceovers don't
need to be live. They're batch jobs: text and a voice in, a WAV out,
cached by content hash. This is where higher-quality voices and, later,
licensed cloned voices start, because a few seconds of render time
doesn't matter there.

### Where the speech models run

**Decided:** Google's models do all speech recognition and synthesis.

- **Speech to text:** Cloud Speech-to-Text v2 with Chirp 3, as a streaming
  recognition with interim results. The voice host opens one stream per
  turn. When `voice.coax` decides the turn is over, the host closes the
  stream rather than waiting for Google's own endpointing, so the final
  transcript comes back right away. V3 measures whether that holds.
- **Text to speech:** Cloud Text-to-Speech with Chirp 3 HD voices,
  streamed, so the first audio plays while the rest is still being made.
  It was the fastest thing we ever measured for first audio (163 to
  198 ms).
- **Batch lines** for NPCs and films use the same voices through ordinary
  (non-streaming) synthesis, cached by content hash.

The voice host is the only thing that calls Google. It holds the
credentials (a service account key in Secret Manager), meters every call,
and forwards results to the phone in our own frames. That keeps three
things ours even with Google doing speech: the media path, the turn and
floor decisions, and the option of switching models later behind the same
frames. Because no model runs on our machines, the voice host needs no
GPU: a small CPU VM is enough.

## Latency budget

These are targets for the parts we own. The agent's own thinking time is
separate and shown in the UI.

| Step | Target (p50) | What we had before |
| --- | --- | --- |
| You stop speaking → turn decided | ≤ 350 ms | Fixed 850 ms silence timer (aethyr-cpp) |
| Turn decided → final transcript | ≤ 300 ms | Google STT, 2.2–6.2 s, while waiting on Google's own endpointing; we close the stream ourselves instead |
| Reply text → first audio on the phone | ≤ 300 ms | Chirp 3 HD 163–198 ms to first byte |
| You talk over it → playback stops | ≤ 100 ms | 67–110 ms (LiveKit); 2 ms acknowledgement (Rust helper) |
| Player to player, mouth to ear, same region | ≤ 250 ms | Never measured |

Measurement comes first this time, and it lives in the repository:

- **Replay checks.** Recorded speech fixtures (consented, ours) run through
  `voice.coax` in `coax test`. Turn detection, barge-in and the jitter
  buffer are deterministic, so their results are exact and stay in the
  tests.
- **A latency harness.** It plays the fixtures into a real voice host and
  records each step above. Results are committed under `bench/voice/` with
  the machine, Google model and region, and build named. If the
  final-transcript step stays over a second, that's the signal to revisit
  the speech-to-text decision.
- **Real phones.** Each phase ends with the owner on a physical iPhone,
  with steps in `NEEDS_OWNER.md` naming screens and taps.

## Identity, consent and privacy

- **Speech grants no authority.** A transcript is a message to an agent.
  It is never a command the system executes because of how it sounded.
  The Verse rule from October stays: spoken words select the same reviewed
  controls as buttons.
- **NIP-LIVE governs sessions.** A session is a `live.v1` plan with tracks,
  participants, consent and floors. Transport credentials are minted after
  admission, bound to one participant and session, short-lived, and never
  logged.
- **Audio goes to Google, and we say so.** The phone sends audio only to
  our voice host, which streams it to Google for recognition and gets
  speech back from Google. The first time someone turns voice on, the app
  says plainly that Google processes their speech. Google's optional data
  logging stays off.
- **We don't keep audio.** The voice host holds audio in memory for the
  turn and drops it. Transcripts are private records (range 1100–1199), kept
  only if the person chooses. Nothing becomes training data without an
  explicit, withdrawable grant.
- **Voices are private assets.** An enrolled voice follows the MetaHuman
  rule: never in a shared bucket raw, never in a dataset, `no_training` on
  every render, consent recorded with a challenge phrase and withdrawable.
- **Encryption is stated honestly.** TLS to our server is not end-to-end
  between players, and the UI must not say it is.

## Money

Google bills us per minute of audio recognized and per character
synthesized, and the owner asked for free and paid tiers.
`voice-meter.coax` counts both per account and decides, under
`#authority`, whether a turn may run. Paid use settles over Lightning from
the Spark wallet already in the app. Voice owners are paid per use for
licensed voices through the same path. Bitcoin only, as everywhere.

## Capacity: 100 people talking at once

*Estimated 2026-10-28 from Google's list prices; V3 and V4 replace these with measured numbers.*

Nothing is deployed yet: the voice host runs only on a development Mac.
After the 2026-10-28 cleanup, its future load balancer and the earlier
`coder-sync-1` socket pattern are gone, so hosted capacity starts from zero. Every figure here is a list-price
estimate for **100 people talking to agents at the same time**.

The assumptions:

- People speak about 30% of the time. The phone opens a speech-to-text stream
  only while `voice.coax` hears speech, and the voice host closes it at end
  of turn.
- One agent turn a minute per person, each answered with a spoken summary of
  up to 30 words, about 180 characters.
- Raw 16 kHz audio up (256 kbps per speaker) and 24 kHz audio down
  (384 kbps while playing). Opus comes in before player voice (decision 5).

### Speech APIs: most of the cost

| Item | Per hour at 100 concurrent | How it's worked out |
| --- | --- | --- |
| Speech-to-text, Chirp 3, $0.016 a minute | ~$29 | 100 × 60 min × 30% speech = 1,800 min |
| Text-to-speech, Chirp 3 HD, $30 per million characters | ~$32 | 100 × 60 turns × 180 chars = 1.08M chars |
| **Total** | **~$61 an hour**, about **$0.60 per person-hour** | |

If a stream stays open the whole session, speech-to-text rises to ~$96 an
hour. That's why turn-scoped streams matter. Talkative agents cost more:
two spoken replies a minute doubles text-to-speech to ~$65 an hour.

### Machines and network: small

| Item | Monthly | Notes |
| --- | --- | --- |
| Voice host: 2× e2-standard-4 (one active, one standby) in us-central1 | ~$200 | 100 sessions are about 26 Mbps up and 12 Mbps down, plus 100 gRPC streams to Google and Coax's VAD on 20 ms frames. One 4-vCPU VM carries that with room to spare; V3 measures the real load per session |
| HTTPS load balancer with WebSocket backend | ~$20 + traffic | Long timeout, as `coder-sync-1` had |
| Internet egress for spoken replies | ~$0.60 an hour at 100 concurrent | ~5 GB an hour of raw audio down. Opus drops it to ~$0.05 |

Audio from the voice host to Google's APIs stays inside Google and isn't
billed as egress.

### Players talking to each other

Player voice uses no speech models, only the forwarder. If each of 100
players hears 5 others, that's 500 streams down. With Opus at 24 kbps that's
12 Mbps, ~5 GB an hour, about **$0.65 an hour**. With raw audio it would be
ten times that, so Opus has to ship before player voice (decision 5, V5).
The same voice host handles it.

### What it means

| Usage at a peak of 100 concurrent | Speech APIs | Machines + network | Total a month |
| --- | --- | --- | --- |
| 1 hour a day | ~$1,800 | ~$240 | ~$2,000 |
| 4 hours a day | ~$7,300 | ~$300 | ~$7,600 |
| All day, every day | ~$44,000 | ~$650 | ~$45,000 |

The machines fit easily under the account's $1,000-a-month cap. The speech
APIs don't, beyond about 25 person-hours a day, about $15 a day. So
`voice-meter.coax` is a launch requirement, not a later nicety:

- The free tier caps minutes per account per day.
- Paid use settles over Lightning at or above ~$0.60 a person-hour plus margin.
- The voice host refuses new turns when the day's spend reaches its budget.

Three levers lower the per-hour price if needed:

- Standard text-to-speech voices ($4 per million characters) for
  low-stakes lines, dropping text-to-speech by about 85%.
- Batch and cached synthesis for NPC lines that repeat.
- Speech-to-text volume tiers, which start at 500,000 minutes a month.

Later, on-device speech can supplement Google in the mobile app; the web app
stays on Google. aethyr-cpp's iOS builds 1036–1045 already run on-device
speech: SpeechAnalyzer transcription and system voices. The 2026-10-21 audit
recommended putting speech on the user's device first. **Owner direction
(2026-10-28):** infrastructure that works everywhere comes first, which is
why speech runs on Google. On-device speech in the app is a later addition,
not a replacement. Each mobile turn it handles saves the per-minute and
per-character cost, but the estimates above assume none of that.

## Phases

Each phase ends with a measurement committed and a real-phone run.

- **V0. Clean up.** Done 2026-10-28: LiveKit's cluster, Redis and
  addresses deleted.
- **V1. The protocol and the Coax core.** Done 2026-10-28. `openagents.voice.v1` in
  `proto/` with compatibility snapshots, then `voice.coax` (frames, resampler,
  levels, VAD, end of turn, barge-in, jitter buffer), checked on recorded
  fixtures. No network, no models. Exit: replay checks pass, every
  audio-thread procedure has a bounded step test.
- **V2. Phone loopback.** Simulator done 2026-10-28; the real-iPhone
  measurement is not done. Microphone permission (with the invariant
  change), VoiceProcessingIO capture, and Coax deciding turns live on the
  phone. Playback can just echo your own speech back. Exit: turn-end
  latency measured on a real iPhone.
- **V3. Voice host with speech-to-text.** Local only so far: the host runs
  on a Mac with per-turn REST recognition (not streaming), no VM, no load
  balancer. The C host plus Coax libraries on
  a small VM, calling Google's streaming speech-to-text, frames over a WebSocket through the load balancer, partial and
  final transcripts on screen. Exit: harness numbers for steps 1–2.
- **V4. Text-to-speech and the agent relay.** Local only so far: a full
  conversation with the Foreman works in the simulator. Median end of turn
  to first reply audio on one Mac, live: 1.76 s (recognition 292 ms, agent
  first sentence 668 ms, first speech 703 ms). Against the budget, which
  leaves out the agent's own time: the final transcript (292 ms) meets its
  300 ms target; first speech (703 ms) misses its 300 ms target. The first full conversation:
  talk to the Foreman in the Grid. Exit: every row of the budget measured.
- **V5. Players.** The forwarder, `voice-space.coax`, and Opus if decision
  5 says so. Exit: two phones in the Grid hear each other placed in space,
  mouth-to-ear measured.
- **V6. Batch lines.** NPC and film lines rendered as cached jobs.
- **V7. Voices as assets.** Enrollment with consent, licensing, metering,
  payouts (see decision 13).

## Decisions

1. **Delete the leftover LiveKit infrastructure?** **Decided and done,
   2026-10-28:** `oa-livekit-prod` (6 nodes) and `oa-livekit-redis` (HA)
   deleted, with the LiveKit program's reserved addresses. `one-prod` and
   `one-stg` belong to a different application and were left alone.
2. **Where does the voice host run?** **Decided:** develop on
   `coderos-4080`, then a dedicated small VM in us-central1 behind the
   existing load balancer, separate from `coder-sync-1`.
3. **Speech-to-text model.** **Decided:** Google Cloud Speech-to-Text v2,
   Chirp 3, streaming.
4. **Text-to-speech model.** **Decided:** Google Cloud Text-to-Speech,
   Chirp 3 HD voices, streaming for live replies and batch for lines.
5. **Codec.** **Decided:** raw 16 kHz 16-bit frames for talking to
   agents (V3, V4), with no codec; Opus in host C before player voice
   (V5), where bandwidth and packet loss matter.
6. **Turn detection.** **Decided:** energy and silence rules in Coax
   first, tested on recorded speech that shows where they fail. A small
   learned turn model comes only when those recordings prove it's needed.
   Google's own endpointing is never what ends a turn.
7. **Model runtime.** **Decided by 3 and 4:** none. No speech model runs
   on our machines.
8. **Offline voice.** **Decided:** none. Voice needs a connection, and
   the toggle says so when there isn't one. One speech path, one voice.
9. **Transport.** **Decided:** WebSocket over TLS through the existing
   load balancer for agent voice. Player voice is measured in V5, and we
   move to QUIC datagrams only if TCP stalls show up there.
10. **Which agent answers?** **Decided:** no single one. Many players,
    NPCs and agents will speak, so voice is a set of reusable primitives
    in a new `openagents.voice.v1` package (see
    [Voice primitives](#voice-primitives-one-set-for-every-speaker)).
    Which brain answers is a `Route` binding, chosen per speaker.
11. **Free and paid tiers.** **Decided:** free while only the owner and
    testers use it. After V4 measures what a real conversation minute
    costs on Google, we set a free daily allowance and a sats price per
    minute beyond it.
12. **Language work for Coax.** **Decided:** two additions. `#foreign`
    imports that take views, so Coax can call the Opus codec with sample
    buffers directly (added in V3, when it first blocks). And a marker
    for procedures that may run on the real-time audio thread, checked by
    the compiler on top of the bounded step tests. SIMD stays on the
    spec's existing schedule; 16 kHz mono doesn't need it.

13. **Licensed and custom voices.** **Decided:** out of scope for now.
    We ship with Google's stock Chirp 3 HD voices and revisit after V4.
    Owning voices is the one place where the lock-in lesson (OpenAI
    refusing Sarah's voice) applies directly, so it comes back when we
    get there.

## What we're not doing

- No LiveKit, Pipecat or other hosted media framework, in any hosting.
- No speech provider besides Google, and no audio from the phone straight
  to Google: it goes through our voice host.
- No peer-to-peer media or TURN.
- No voice model that answers or acts on its own.
- No new languages: Coax, the host C compiler, and Swift for platform
  effects.

## Code

A guided walk through the built pieces, with runnable examples, is Chapter 27 of the Coax book: [Building a voice system in pieces](../../coax/book/ch27-case-study-voice.md).

## Reading the history

The full audit and the code it describes are in history:

```sh
git show 0b401561a0^:docs/audits/2026-10-21-voice-infrastructure.md
git ls-tree -r --name-only 0b401561a0^ | grep -i voice
```

The aethyr-cpp iOS voice path (SpeechAnalyzer, WebRTC VAD, spoken
summaries) is in that repository at `69848ca`, under
`apps/openagents-ios/OpenAgents/Audio/` and `Chat/`.
