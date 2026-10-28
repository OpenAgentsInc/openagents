# Coax-native voice: a proposal

*Draft, 2026-10-28. Nothing here is built yet. Decisions the owner has
made are marked **Decided**; the rest have a recommendation.*

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
   LiveKit took 13.7 to 15.2 s to the first audio. Its GKE cluster is still
   running two months after its code was deleted (checked today:
   `oa-livekit-prod`, 6 nodes, plus the `oa-livekit-redis` HA instance).
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

**Voice code:** none. The only audio in the tree is `AVAudioPlayer`
playing Haunt's clips (`apps/openagents-ios/OpenAgents/Platform/HauntAssets.swift`).
The phone app has never asked for the microphone, and
[INVARIANTS.md](../../INVARIANTS.md) says the home screen does not acquire
device permissions. Turning voice on is an invariant change.

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
| `oa-livekit-prod`, `oa-livekit-redis` | Leftover LiveKit, running | Should be deleted (decision 1) |

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

In the Grid this has an obvious home: walk up to the Foreman and talk.
Being near an agent's body can be what grants the input floor, so
proximity becomes the push-to-talk button.

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

## Phases

Each phase ends with a measurement committed and a real-phone run.

- **V0. Clean up.** Delete `oa-livekit-prod` and `oa-livekit-redis` after
  the owner confirms. Check whether `one-prod` and `one-stg` still serve
  anything.
- **V1. The Coax core and its tests.** `voice.coax` (frames, resampler,
  levels, VAD, end of turn, barge-in, jitter buffer), checked on recorded
  fixtures. No network, no models. Exit: replay checks pass, every
  audio-thread procedure has a bounded step test.
- **V2. Phone loopback.** Microphone permission (with the invariant
  change), VoiceProcessingIO capture, and Coax deciding turns live on the
  phone. Playback can just echo your own speech back. Exit: turn-end
  latency measured on a real iPhone.
- **V3. Voice host with speech-to-text.** The C host plus Coax libraries on
  a small VM, calling Google's streaming speech-to-text, frames over a WebSocket through the load balancer, partial and
  final transcripts on screen. Exit: harness numbers for steps 1–2.
- **V4. Text-to-speech and the agent relay.** The first full conversation:
  talk to the Foreman in the Grid. Exit: every row of the budget measured.
- **V5. Players.** The forwarder, `voice-space.coax`, and Opus if decision
  5 says so. Exit: two phones in the Grid hear each other placed in space,
  mouth-to-ear measured.
- **V6. Batch lines.** NPC and film lines rendered as cached jobs.
- **V7. Voices as assets.** Enrollment with consent, licensing, metering,
  payouts (see decision 13).

## Decisions to make

1. **Delete the leftover LiveKit infrastructure?** `oa-livekit-prod` (6
   nodes) and `oa-livekit-redis` (HA) are running with no code behind
   them. At list prices, six 8-vCPU nodes plus an HA Redis likely cost well over $1,000 a month.
   *Recommendation: delete both now. Check `one-prod` and `one-stg`
   separately.*
2. **Where does the voice host run?** It needs no GPU now. (a) Alongside
   `coder-sync-1` on that VM, (b) its own small VM in us-central1 behind
   the same load balancer, (c) `coderos-4080` over the tailnet for
   development only. *Recommendation: (c) while building V1 to V3, then
   (b), so a voice bug can't take down sync and the reverse.*
3. **Speech-to-text model.** **Decided:** Google Cloud Speech-to-Text v2,
   Chirp 3, streaming.
4. **Text-to-speech model.** **Decided:** Google Cloud Text-to-Speech,
   Chirp 3 HD voices, streaming for live replies and batch for lines.
5. **Codec.** (a) Opus in host C: the standard for a reason, with loss
   concealment and low bitrate. (b) A Coax codec (16-bit PCM or IMA-ADPCM):
   simple and fully checkable, but 64 to 256 kbps. *Recommendation: PCM
   frames for the agent path in V3/V4 (one stream, a decent network), and
   Opus in the host before player voice in V5.*
6. **Turn detection.** (a) Energy and silence rules only: fully Coax,
   deterministic, worse at telling "um" from "done". (b) Add a small
   learned turn model (Silero VAD, smart-turn) through the host runtime.
   *Recommendation: (a) in V1 with fixtures that show its failures, then
   (b) when the fixtures prove it's needed. Port the model into Coax once
   Coax has SIMD.*
7. **Model runtime.** **Decided by 3 and 4:** none. No speech model runs
   on our machines.
8. **On-phone speech for offline use.** (a) Apple's on-device
   SpeechAnalyzer and AVSpeechSynthesizer (what aethyr-cpp used), (b) our
   own small models on the phone through Metal, (c) no offline voice at
   first. *Recommendation: (c) until V4 works, then (a) as an explicit
   offline mode, because the Khala app removed Apple's TTS after one day.*
9. **Transport.** (a) WebSocket over TLS through the existing load
   balancer, (b) QUIC datagrams on our own UDP endpoint. *Recommendation:
   (a) through V4. It works on every network and we already run one.
   Decide on (b) with V5 numbers, since TCP stalls hurt group voice more
   than one-to-one.*
10. **Which agent answers?** The voice layer relays; something must be on
    the other end now that the old chat stack is gone. *Recommendation:
    the Foreman in the Grid, through whatever agent endpoint the app adopts
    for text. Voice adds no agent of its own.*
11. **Free and paid tiers.** What's free (minutes per day? the basic
    voice?) and what costs sats. *Recommendation: decide the price at V4,
    once Google's measured cost per conversation minute is known.*
12. **Language work for Coax.** Voice wants `#foreign` that takes views
    (to call the codec without the host in between), SIMD, and
    a way to declare which procedures may run on the audio thread.
    *Recommendation: add `#foreign` views in V3, when it first blocks, and
    leave SIMD to the GPU and SIMD stages already planned in
    [the spec](../../coax/spec.md).*
13. **Licensed and custom voices.** Voices as assets that people own and
    license (V7) now depend on what Google allows: Google offers custom
    voices only on its terms. (a) Build licensed voices on Google's custom
    voice offering, (b) keep licensed voices out of scope while Google does
    speech, (c) render licensed voices with an owned model for batch lines
    only, as an exception to decision 4. *Recommendation: (b) for now, and
    revisit after V4. It's the one place where the lock-in lesson
    (OpenAI refusing Sarah's voice) applies directly.*

## What we're not doing

- No LiveKit, Pipecat or other hosted media framework, in any hosting.
- No speech provider besides Google, and no audio from the phone straight
  to Google: it goes through our voice host.
- No peer-to-peer media or TURN.
- No voice model that answers or acts on its own.
- No new languages: Coax, the host C compiler, and Swift for platform
  effects.

## Reading the history

The full audit and the code it describes are in history:

```sh
git show 0b401561a0^:docs/audits/2026-10-21-voice-infrastructure.md
git ls-tree -r --name-only 0b401561a0^ | grep -i voice
```

The aethyr-cpp iOS voice path (SpeechAnalyzer, WebRTC VAD, spoken
summaries) is in that repository at `69848ca`, under
`apps/openagents-ios/OpenAgents/Audio/` and `Chat/`.
