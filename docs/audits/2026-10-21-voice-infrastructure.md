# Voice infrastructure audit, past and present

Status: audit, 2026-10-21, at `origin/main` `f5fb59bfa9`. Owner request: a
deep audit of every voice system OpenAgents has built, in this repository and
in other repositories, with a recommendation for one voice stack. This
document proposes issues but files none.

Sources:

- This repository's full history (`git log --all`, 2023-11-07 to 2026-10-21),
  including deleted trees read with `git show HASH:PATH`. Documents deleted by
  the 2026-09-18 commit `dabc08102f` ("Nuke") are read at `dabc08102f~1`.
- Issues and pull requests on
  [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents).
- The transcript archive in [`docs/transcripts/`](../transcripts/).
- [OpenAgentsInc/voice](https://github.com/OpenAgentsInc/voice) and 17 other
  OpenAgentsInc repositories found among the organization's 468. Most are
  private; this document describes their designs and quotes no code, prompts,
  endpoints, or secrets.
- Sibling checkouts under `~/work`, as reference only.
- A read-only check of the `openagentsgemini` Google Cloud project on
  2026-10-21.

A note on hashes: `git log --all` also finds a rewritten copy of early
history, so some early commits exist twice. This document cites the hash
that is an ancestor of `origin/main`.

## Summary

- **We have built voice at least 20 times since November 2024.** About 14
  systems lived in this repository and 9 more in separate repositories. They
  used every language we have written: Rust, TypeScript, Python, Swift,
  Kotlin, Elixir, and Go (vendored LiveKit).
- **Almost none ended because the voice technology failed.** Most were
  deleted with their host during a repository reset, a language rule, or a
  product pivot: "Zero base", the Rust deprecation, the Bun/Effect rebuild,
  the Electron deletion, the TypeScript purge, and the move from Zeron. The
  exceptions are Sarah's avatar (an owner order and a likeness policy), the
  Khala app's text-to-speech ("sounded bad"), and the LiveKit path's 14 s to
  first audio.
- **What works today** is two features in the OpenAgents apps, imported from
  Zeron on 2026-10-16 (`dba20df768`):
  - On-device dictation with NVIDIA Parakeet TDT 0.6B v3 (Rust, ONNX Runtime
    on CPU) on the desktop, and from a browser through a paired computer.
  - "Talk with Codex" realtime calls on the desktop, iOS, Android, and the
    web. These use the user's own Codex subscription voice over WebRTC, relay
    each request to a saved agent (Foreman by default), and never carry audio
    through our servers.
- **Neither feature has a recorded latency number or an end-to-end test with
  real speech on a real device.** Every October voice issue closed
  code-complete with device checks deferred.
- **Leftover infrastructure is still running.** The self-hosted LiveKit
  cluster `oa-livekit-prod` (six GKE nodes) and its high-availability Redis
  are `RUNNING` on 2026-10-21, 54 days after the code that used them was
  deleted in `d7574f6669`.
- **Top recommendation:** one Rust voice engine, `crates/voice-engine`, with
  two interchangeable backends behind the call surface the apps already have:
  the existing Codex realtime relay, and an owned cascade (Silero VAD,
  streaming Parakeet, the chosen agent, then a Psionic text-to-speech voice)
  that runs on the user's own computer or a Pylon. Measure before you build:
  a latency harness comes first. See [Recommendations](#recommendations).
- **Owner direction, 2026-10-21:** relying on Codex for voice is not
  enough. The owned engine is required, not a fallback, and it must also
  serve **voices as creator assets**: people enroll their own voice, list
  it, and are paid each time a creator uses it in Game Lab, Video Lab or
  the Verse, next to commissioned human voice actors. See
  [Owned engine and the voice marketplace](#owned-engine-and-the-voice-marketplace).

## Timeline

Abbreviations: STT is speech to text, TTS is text to speech, VAD is voice
activity detection, AEC is acoustic echo cancellation, SFU is a selective
forwarding unit (a WebRTC media server), and RTF is real-time factor
(processing time divided by audio length; under 1.0 keeps up).

### 2024–2025: Onyx and hosted Whisper

**Onyx mobile app** (Dec 2024 to Feb 2025)
- Repository: [OpenAgentsInc/onyx](https://github.com/OpenAgentsInc/onyx),
  public, React Native and TypeScript. Created 2024-11-27, last push
  2025-02-14.
- Architecture: push to talk, with Groq-hosted Whisper `whisper-large-v3`
  for STT and Groq Llama 3.1 8B for replies. No TTS.
- Shipped in the episodes "Speak to Onyx" ([`151.md`](../transcripts/151.md),
  line 13: "You can now talk to Onyx") and "Code by Voice"
  ([`152.md`](../transcripts/152.md), line 65, a GitHub fix directed from the
  phone).
- Ended: no longer updated after 2025-02-14. A later Expo repository,
  [OpenAgentsInc/app](https://github.com/OpenAgentsInc/app) (private,
  2025-07-14 to 07-17), repeated the Groq Whisper input.

**`transcribe` binary** (2025-02-03 to 2025-03-22)
- Repository and path: this repository, `src/bin/transcribe.rs`, Rust. Added
  in `d7d41d9057` (#665, #666).
- Architecture: batch only. `ffmpeg` extracts audio, Groq `whisper-large-v3`
  transcribes, and Groq Llama cleans up the text and writes
  `docs/transcripts/*.md`. This is how the transcript archive started.
- Ended: deleted by the whole-repository reset "Zero base" (`3337f49d01`,
  #754). No voice-specific reason.

**Microphone buttons with no backend** (2025)
- The shadcn chat kit's `use-audio-recording.ts` was added to `apps/coder`
  (`99560675d6`, #810) and copied into `apps/website` and `apps/projects`.
  `transcribeAudio` was an optional prop that nothing supplied. Deleted in
  the June 2025 "Zero base" (`acc72ba4ca`).
- `ios/OpenAgents/FloatingMicButton.swift` (`616c7ab539`) had the action
  `TODO(#1447): voice input`. Issue
  [#1447](https://github.com/OpenAgentsInc/openagents/issues/1447) asked for
  the Apple Speech framework and closed the same day without it.

**v5s** (July 2025)
- Repository: [OpenAgentsInc/v5s](https://github.com/OpenAgentsInc/v5s),
  private, Rust server and Swift iOS app, 2025-07-17 to 07-22.
- Architecture: on-device `SFSpeechRecognizer` or server-side `whisper-rs`
  (`small.en`), then Groq STT; Groq or Ollama for the reply; ElevenLabs, then
  Piper (a British female voice) for TTS; a WebSocket to a Rust server with
  Postgres and pgvector memory.
- Latency: commits mention a 5 to 10 s response problem; docs claim 0.8 to
  2.2 s of API latency (not measured).
- The owner described it in episode 183
  ([`183.md`](../transcripts/183.md), line 17) as "a voice app that I built
  using Swift … with the Rust backend, Piper text-to-speech." Abandoned
  within a week.

**Phone sales calls** (July to August 2025)
- Repositories, all private: [client1](https://github.com/OpenAgentsInc/client1)
  (Python and Flask, Twilio with ElevenLabs and OpenAI),
  [voice-python](https://github.com/OpenAgentsInc/voice-python),
  [dialer](https://github.com/OpenAgentsInc/dialer), and the `voice-server/`
  inside [dashboard](https://github.com/OpenAgentsInc/dashboard).
- Architecture: Pipecat 0.0.77 on Twilio's media WebSocket; Silero VAD;
  Deepgram Nova-2 STT; Groq `llama-3.3-70b-versatile`; Cartesia Sonic TTS.
  The persona was an outbound sales agent for a payments customer.
- Latency: claimed only (TTS about 200 ms, STT about 150 ms, LLM about
  500 ms).
- Ended: `dashboard`'s last commit (2025-08-27) is a voice-server crash fix.
  This is the only Python voice product we built, and the only one that
  reached the telephone network.

**Zed call audio, vendored** (2025-12-10 to 2025-12-13)
- Path: `crates/audio`, `crates/livekit_api`, and `crates/livekit_client`
  from the Zed import (`336d0c1b32`, `e9b6eb3cc0`). Rust, GPL-3.0, with
  Zed's fork of the LiveKit Rust SDK and `libwebrtc`.
- Nothing in OpenAgents called them. Deleted with the Zed fork in
  `f33e72a55c` ("thx zed bye4now").

**Video-series transcription** (scripts, not product)
- `scripts/transcribe_video_series.py` (`bb46192ddd`, 2025-12-16): `yt-dlp`
  and local `openai-whisper`. Deleted in `714ea04717`.
- `scripts/transcribe-video-series.py` with
  `docs/VIDEO_SERIES_TRANSCRIPTION_RUNBOOK.md` (`a4bb560fce`, 2026-05-31):
  OpenAI `gpt-4o-transcribe-diarize`. Deleted in the Bun/Effect rebuild
  `f5919c7669`.

### January to March 2026: local Whisper and Google Chirp in Rust

**Onyx editor voice, then `crates/voice` and `crates/voice-daemon`**
(2026-01-06 to 2026-02-25)
- Path: first `crates/onyx/src/voice/` (inside `7e9417cdfc`), then extracted
  by `a463ee5294` ("Add voice crate and voice-daemon for global voice
  transcription"). Rust, macOS. Here Onyx was a Rust Markdown editor, not
  the 2024 phone app.
- Architecture: `cpal` capture resampled to 16 kHz; whisper.cpp through
  `whisper-rs` 0.14 with Metal; `base.en` downloaded from Hugging Face; hold
  Right Command (a `CGEventTap`) to talk; paste the text through the
  clipboard. No VAD, no TTS.
- Ended: `voice-daemon` was deleted in `d58fda3075` ("Prune autopilot MVP
  workspace", 2026-01-12). `crates/voice` lost its only consumer when Onyx
  was retired (`a8b4e8e5c5`) and was deleted in `78a91d6b78` ("remove dead
  workspace library crates", 2026-02-25).

**Autopilot desktop voice playground** (2026-03-18 to 2026-06-09)
- Path: `apps/autopilot-desktop/src/voice_playground.rs`, Rust.
- Plan: `docs/audits/2026-03-18-autopilot-voice-integration-audit.md`
  (`a46faca09e`, since deleted) recommended push-to-talk STT and spoken
  replies first, realtime and barge-in later, and said not to revive the
  archived Whisper crate.
- Built: `afae54bba9` (STT) and `380e79ef1c` (TTS), from issues
  [#3951](https://github.com/OpenAgentsInc/openagents/issues/3951),
  [#3952](https://github.com/OpenAgentsInc/openagents/issues/3952), and
  [#3953](https://github.com/OpenAgentsInc/openagents/issues/3953). Google
  Speech-to-Text V2 `chirp_3`; Chirp 3 HD TTS (voice Charon, then Aoede);
  `cpal` capture and `rodio` playback; a `gcloud` access token.
- Psionic's MLX audio package
  ([#3877](https://github.com/OpenAgentsInc/openagents/issues/3877),
  [#3878](https://github.com/OpenAgentsInc/openagents/issues/3878)) moved to
  the Psionic repository and never shipped here.
- Ended: the app was renamed deprecated in April and deleted in the
  Bun/Effect rebuild `f5919c7669` (2026-06-09).

### May 2026: six voice repositories in four weeks

These all lived outside this repository. Most are private.

| Repository | Dates | Stack | Outcome |
|---|---|---|---|
| [eagle](https://github.com/OpenAgentsInc/eagle) | 05-06 to 05-08 | LiveKit Agents React starter on LiveKit Cloud | Marked "prototype/reference only". The LiveKit Cloud agent stopped answering when LiveKit Inference ran out of STT and TTS quota. |
| [lyra](https://github.com/OpenAgentsInc/lyra) | 05-05 to 05-08 | Rust control service on Cloud Run; PCM over WebSocket from an AudioWorklet; OpenAI STT and LLM; TTS from Psionic CSM; a Cartesia adapter; "no LiveKit dependency" | Superseded by `autopilot`. No streaming, barge-in, or WebRTC. |
| [autopilot](https://github.com/OpenAgentsInc/autopilot) | 05-07 to 07-17 | TypeScript and Rust (`crates/voice-runtime`); server-side PCM VAD; Google STT with OpenAI STT as backup; Psionic CSM TTS with OpenAI `nova` as fallback; about 30 `docs/voice-*.md` specs | README says "deprecated legacy". |
| [autopilot3](https://github.com/OpenAgentsInc/autopilot3) | 05-13 to 07-01 | Cloudflare Workers and Convex; OpenAI Realtime | Replaced by `autopilot4`. |
| [autopilot4](https://github.com/OpenAgentsInc/autopilot4) | 05-17 to 05-30 | Rust: Google STT V2 `chirp_3`, Gemma 4, Chirp 3 HD; rejected Piper and OpenAI Realtime | No longer updated after 05-30. |
| [uplink](https://github.com/OpenAgentsInc/uplink) | 05-19 to 06-24 | Swift iOS: `SFSpeechRecognizer` plus `autopilot4`'s voice WebSocket | Stopped with `autopilot4`. |

### June 2026: voice as records, and a one-day native app

**Voice ingest records** (2026-06-06 to 2026-07-05)
- TypeScript Workers code that modeled voice sessions as evidence records
  and command proposals. `voice-program-ingest.ts` (`7717bdd05f`, #4992)
  states that it "never calls an STT vendor".
- Removed in `a1435e6b89` because it was "never armed" and "superseded by
  native SwiftUI voice app"
  ([#8386](https://github.com/OpenAgentsInc/openagents/issues/8386)).

**Native Khala voice app** (2026-06-26)
- Path: `clients/mobile/Khala`, Swift. Specified in `d12f02c99e` and
  scaffolded in `475c2e3fd7`: `AVAudioEngine` capture, on-device
  `SFSpeechRecognizer`, `AVSpeechSynthesizer` replies, and chat completions
  from our API.
- TTS was removed the same day in `8a54389bd1`: "the AI no longer speaks
  responses (sounded bad + double-played on a 2nd response)". All in-app
  voice was removed hours later in `91edb870c3`.
- An Expo module `khala-push-to-talk-stt` in `clients/khala-mobile`
  rejected every call on both platforms and was retired in `9e99de2ab2`.

### July 2026: Sarah, persistent desktop voice, and OpenAI Realtime

**Sarah v1, the avatar sales agent** (2026-07-07 to 2026-07-10)
- Before the monorepo: [sarah-prev](https://github.com/OpenAgentsInc/sarah-prev)
  (private, Next.js) used the Vercel AI Gateway's realtime tokens for
  `openai/gpt-realtime-2`.
- Path: `apps/sarah` (`974490e7ba`,
  [#8594](https://github.com/OpenAgentsInc/openagents/issues/8594)),
  TypeScript on Bun, with GPU services on a separate GCE host.
- Brain: Gemma 4 (`gemma-4-31b-it`) with a deterministic pricing guard that
  runs before the model, because that lane had no tool calling
  (`158a361260`).
- Avatar, stage 1: HeyGen LiveAvatar in FULL mode with its custom LLM add-on
  ([#8598](https://github.com/OpenAgentsInc/openagents/issues/8598)).
  LiveAvatar did the STT and TTS.
- Avatar, stage 2: an owned pipeline, because LiveAvatar only allows
  first-person likenesses and Sarah was a synthetic character. Browser Web
  Speech API for STT (final results only); Chirp 3 HD and a CosyVoice2
  voice clone for TTS; MuseTalk 1.5 lip sync; WebRTC (WHEP) delivery
  (`f5f9cb3725`).
- Measured (`ebb024d9da`, one L4 GPU):
  - CosyVoice2-0.5B: RTF 0.897, cold load 20.2 s, and 3 to 5.8 s to the
    first chunk
    ([#8613](https://github.com/OpenAgentsInc/openagents/issues/8613)), so
    Chirp 3 HD stayed the live voice at 163 to 185 ms to the first chunk
    warm.
  - MuseTalk: 20 FPS against a 24 FPS target, about 60 s to warm up, and
    one session for each GPU
    ([#8621](https://github.com/OpenAgentsInc/openagents/issues/8621)).
- A Pipecat audit (`65731bed6d`) said not to adopt Pipecat as the
  foundation, because it has no local lip sync and its asyncio loop would
  compete with MuseTalk. It recommended taking patterns instead: Silero VAD,
  the `smart-turn` end-of-turn model, interruption handling, and
  time-to-first-audio metrics.
- Ended: `13bc1e7443` removed the whole surface on the owner's order
  (192 files). [#8610](https://github.com/OpenAgentsInc/openagents/issues/8610)
  closed as WONTDO with "Do not dispatch avatar, opener, voice, video,
  ASR/VAD … work." The GPU host was terminated.

**Persistent desktop voice mode** (2026-07-12)
- Epic [#8733](https://github.com/OpenAgentsInc/openagents/issues/8733),
  leaves #8734 to #8741.
- Paths: `apps/openagents-audio` and `apps/openagents-audio-edge`
  (TypeScript and Effect on Cloud Run), `packages/audio-contract`, and
  `crates/oa-desktop-audio` (a Rust helper for the Electron app).
- Architecture: the Rust helper owned `cpal` capture and playback,
  resampling, packet framing, a TLS media WebSocket, and flushing the
  playback buffer on barge-in. The gateway streamed 24 kHz PCM16 to Google
  STT V2 `chirp_3` and from Chirp 3 HD streaming TTS (voice Sulafat). Audio
  was optionally retained, encrypted, in Cloud Storage with a 30-day expiry.
  The decision record chose this TypeScript-plus-Rust split
  (`docs/voice/2026-07-12-effect-vs-rust-audio-architecture-decision.md`).
- Built in one day: `d23978eee2` (contract) through `bf1ba778c1` ("prove
  retained real-microphone MVP").
- Measured (`docs/voice/evidence/`, deleted in `dabc08102f`):

  | Measurement | Value |
  |---|---|
  | STT final transcript, wall time | 2,207 to 6,153 ms |
  | Chirp 3 HD first byte | 185 to 198 ms |
  | Barge-in acknowledgement, p95 | 2 ms (target 500 ms or less) |
  | 60 s real-microphone fault run | 562 of 562 packets acknowledged |

- Ended: the Electron host was deleted in `a299a2ef43` (2026-08-05), the
  Rust helper in `3975253ba3`, and the TypeScript apps in `d7574f6669`.
  [#8741](https://github.com/OpenAgentsInc/openagents/issues/8741) records
  that the landed parts were left as "dormant substrate".

**Sarah v2, the owner's orchestrator voice** (2026-07-18 to 2026-08-28)
- Path: `packages/sarah` (`385820682b`), TypeScript, with the Expo mobile
  app.
- TTS only at first (`a9c50fa401`): OpenAI `gpt-4o-mini-tts`, voice
  `marin`.
- Managed realtime gateway (`82e9f00fdb`,
  [#9272](https://github.com/OpenAgentsInc/openagents/issues/9272)):
  the client sends PCM16 at 24 kHz over a custom WebSocket to a Cloud Run
  bridge, which talks to OpenAI Realtime `gpt-realtime-2.1` with semantic VAD
  and `gpt-4o-mini-transcribe`. Credits were reserved and settled for each
  session. The mobile client (`ff8fdd2ae5`) shipped as TestFlight build 127
  ([#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)).
  Credit limits were removed in `65e7b80f9d`.
- In the transcripts, the owner could not upload a custom voice to OpenAI
  ([`270.md`](../transcripts/270.md), line 33), tool calls did not run in
  voice mode (line 231), and calls failed repeatedly ("voice session
  failed", lines 61, 113, 219).
- Ended: the mobile app was retired in `090b7a4a9a` and `packages/sarah` was
  deleted in `fae80bde79`.

**Sarah on self-hosted LiveKit** (2026-07-30 to 2026-08-28)
- Issues [#9283](https://github.com/OpenAgentsInc/openagents/issues/9283)
  to [#9286](https://github.com/OpenAgentsInc/openagents/issues/9286).
- Paths: `apps/sarah-livekit-agent` (`0ec4ea19b6`), `@livekit/agents`
  1.6.0 with a patched OpenAI plugin; `infra/livekit*` and
  `infra/modules/livekit-*` Terraform (`bcbc1aa073`).
- Architecture: a regional GKE cluster with one SFU for each node, a pool of
  three agent workers, high-availability Redis, and TURN. Sarah joined each
  room as an OpenAI Realtime agent. No LiveKit Cloud. A teardown
  (`3e838a65fc`) noted that LiveKit adds rooms and multi-party calls but
  "does not automatically improve Sarah's current admission, command
  safety, accounting, or transcript privacy."
- Measured (two-room acceptance, `c4906fbd89`), timed from the harness
  start:

  | Measurement | Value |
  |---|---|
  | Admission | 255 to 544 ms |
  | Room connected | 1.2 to 2.0 s |
  | Microphone published | 7.0 to 7.3 s |
  | First Sarah audio | 13.7 to 15.2 s |
  | Interrupt acknowledgement | 67 to 110 ms |

- Cost: one unattended Codex thread spent 16 h 58 min and about 1.91 billion
  tokens on this program, with 21 Cloud Run deploys and 30 GKE rollouts
  (`84459a026d`,
  `docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md`).
- Ended: code deleted in `d7574f6669` ("delete TypeScript voice and LiveKit
  stack"). **The cluster was not.** See
  [Leftover infrastructure](#leftover-infrastructure).

### August 2026: Rust LiveKit, then Elixir

**OpenAgentsInc/voice** (2026-08-06 to 2026-08-10)
- Repository: [OpenAgentsInc/voice](https://github.com/OpenAgentsInc/voice),
  private, not archived. 74 commits by one author, from `b704c0f`
  ("Bootstrap private Rust voice infrastructure") to `188bbbc` ("Deploy the
  component-show runtime").
- Languages: about 10,100 lines of Rust in nine crates (agent, contract,
  control, three providers, a provider cascade, a conversation simulator,
  ops), and a vendored LiveKit server v1.13.5 in Go. The LiveKit Rust SDK is
  pinned.
- Architecture: the browser joins a self-hosted LiveKit room with a
  short-lived grant that only allows joining, subscribing, and publishing a
  microphone. A Rust agent joins as a participant. A Rust control service
  issues sessions, grants, and events. It ran on GKE with images deployed by
  digest.
- Providers, in order: OpenAI Realtime `gpt-realtime-2.1`; Gemini Live
  (`gemini-2.5-flash-native-audio`, then `gemini-3.1-flash-live-preview`);
  then a "Google cascade" with Gemini Live used only for STT and turn
  detection, Gemma 4 31B for the reply, and Gemini Flash TTS sentence by
  sentence. A silent executor model ran workspace tools through MCP.
- Turn ending: the provider's VAD plus a silence timer of 0 to 2.8 s that
  depends on whether the transcript looks finished. No Silero or Whisper.
- Measured: locally, agent audio heard 505 ms after admission and Rust first
  output audio at 324 ms. In production logs the first spoken text came
  about 0.9 s after the user stopped speaking, but an MCP tool call took
  up to 29 s. The same day the speaker model invented facts and claimed an
  action it never took; the fix was prompt rules.
- Ended: imported into [OpenAgentsInc/one](https://github.com/OpenAgentsInc/one)
  (private, Rust, 2026-08-11 to 08-17) under new crate names. `one` found
  that the pinned LiveKit Rust SDK does not build for `wasm32` and that
  Gemini STT heard "no" as Portuguese "Não" unless English was forced. `one`
  stopped when `sarah` started.

**sarah and openagents.com, Elixir** (2026-08-16 to 2026-09-04)
- Repositories: [OpenAgentsInc/sarah](https://github.com/OpenAgentsInc/sarah)
  (private, Phoenix LiveView) and
  [OpenAgentsInc/openagents.com](https://github.com/OpenAgentsInc/openagents.com)
  (public read-only mirror).
- Architecture: OpenAI Realtime speech to speech (`gpt-realtime-2.1`, voice
  `marin`, semantic VAD), WebRTC directly between the browser and OpenAI,
  with Phoenix proxying call setup and holding a server-side control
  WebSocket. Calls recorded in stereo.
- Decision: `sarah`'s `docs/voice/OPENAI_REALTIME_DECISION.md` (2026-08-16)
  rejected both the LiveKit and Rust stack and the chained STT-LLM-TTS path,
  citing STT and TTS latency and a larger interruption state machine.
- Measured (staging, few samples): call setup p50 865 ms, first remote audio
  p50 969 ms.
- Ended: on 2026-08-19 the core went public as `openagents.com`, now a
  read-only mirror, and product work returned to Rust in this repository.

### October 2026: what is on main today

The `apps/openagents` tree, including its voice, came from Zeron (the Coder
Desktop fork, [OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop),
now archived) in merge commit `dba20df768` (2026-10-16). Upstream commits
keep their own pull request numbers, which refer to the fork, not to this
repository.

**Desktop dictation** (upstream `80b946b153`, 2026-10-02)
- Path: `apps/openagents/crates/voice` (`openagents-voice`), with the UI in
  `apps/openagents/crates/ui/src/dictation*`. Rust.
- Model: NVIDIA Parakeet TDT 0.6B v3, INT8 ONNX (CC-BY-4.0), 670,479,942
  bytes, pinned by revision and SHA-256 in `crates/voice/model.json`,
  downloaded on opt-in and never bundled.
- Runtime: `parakeet-rs` 0.3.8 on ONNX Runtime 1.28 (CPU), `cpal` 0.17.3
  capture, and `rubato` resampling to 16 kHz.
- Behavior: hold Cmd or Ctrl+D to talk, up to 60 s; decoded once when you
  release (no streaming, no partial results); 25 European languages; audio
  never leaves the device.
- Docs: [`apps/openagents/docs/reference/desktop-dictation.md`](../../apps/openagents/docs/reference/desktop-dictation.md)
  and [`apps/openagents/docs/design/desktop-parakeet-v3.md`](../../apps/openagents/docs/design/desktop-parakeet-v3.md).
- Windows support was restored in `fbf7f330cc`
  ([#12182](https://github.com/OpenAgentsInc/openagents/issues/12182)).
  ONNX Runtime has no Intel macOS build.

**Browser dictation through a paired computer** (2026-10-20,
[#12292](https://github.com/OpenAgentsInc/openagents/issues/12292))
- The browser module `apps/openagents/web/dictation` (wasm) captures and
  downsamples to 16 kHz and sends chunks over the computer's device room
  (`ece9ce1f3d`). The engine (`apps/openagents/crates/engine/src/rpc/dictation.rs`,
  `a1c255e9c1`) transcribes with the same Parakeet model on the user's own
  computer.

**Talk with Codex: realtime voice calls** (upstream `4a5fb8b443`,
2026-10-07, and post-merge work to 2026-10-20)
- Provider: Codex's own subscription voice, started with
  `thread/realtime/start` on the user's Codex install and billed to the
  user's ChatGPT plan. There is no OpenAI API key, no `gpt-realtime` model
  string, and no Gemini Live anywhere in the tree.
- Transport: WebRTC directly from the device to OpenAI. Our servers carry
  only SDP signaling and controls over the owner-only device relay.
  Capture, Opus, AEC, barge-in, and playout all happen in the WebRTC
  endpoint.
- Rust crates in `apps/openagents/crates`:
  - `harness/src/codex/realtime.rs`: attaches realtime to an idle Codex
    thread, requires a ChatGPT login, and records only final transcript
    items.
  - `harness/src/codex/voice_profile.rs`: a fresh, private Codex home for
    each call (`4b814d9a37`,
    [#12060](https://github.com/OpenAgentsInc/openagents/issues/12060)),
    after one shared voice thread grew to about 24 million input tokens and
    picked up a user's plugin.
  - `voice-media`: speaks to Codex's packaged `codex-voice-host` helper
    (GStreamer and Opus, not redistributed by us; see
    [`apps/openagents/dist/voice/README.md`](../../apps/openagents/dist/voice/README.md)).
  - `voice-session`: the shared call lifecycle and a view reducer for phase,
    captions, levels, and the orb.
  - `proto/src/voice.rs`, `engine/src/voice*`, `engine/src/rpc/voice_*`,
    and `doc/src/voice.rs`: the contract, the one voice call allowed for
    each engine, client-owned media, and the transcript chat.
- Relay design (owner decision 2026-10-17, "for now, voice = Foreman"):
  the voice model only relays. It gets three tools, `get_voice_agent`,
  `send_message`, and `wait_for_turn`, passes each spoken request to the
  chosen saved agent, and speaks back a short summary (`75e3db449e`,
  `83be482194`; [#12059](https://github.com/OpenAgentsInc/openagents/issues/12059),
  [#12061](https://github.com/OpenAgentsInc/openagents/issues/12061)). The
  trigger was a call where the voice model answered by itself and Foreman
  never saw the request.
- Shells:
  - Desktop (GPUI): `ui/src/voice.rs` and `ui/src/shell/voice_stage.rs`.
  - iOS (Swift): `apps/openagents/apps/ios/OpenAgents/Voice/`, with
    `stasel/WebRTC` 150.0.0 and a Live Activity.
  - Android (Kotlin): `AndroidWebRtcMediaDriver.kt` (`9df3c231b0`) and audio
    routing, with call screens in Rust
    (`apps/openagents/crates/mobile/src/voice_ui.rs`, `223294f911`,
    `b6cb916370`; [#12094](https://github.com/OpenAgentsInc/openagents/issues/12094),
    [#12097](https://github.com/OpenAgentsInc/openagents/issues/12097)).
  - Web: `apps/openagents/web/client/src/voice.rs` (`5ab0702863`,
    [#12293](https://github.com/OpenAgentsInc/openagents/issues/12293)).
- Validation: offline only, against Python fake peers in
  `apps/openagents/crates/harness/tests/fixtures/`. The one live run
  (2026-10-01, Linux, Codex 0.159) passed sign-in and negotiation but could
  not open audio devices. [`codex-voice-remote.md`](../../apps/openagents/docs/research/codex-voice-remote.md)
  says "G0–G4 remain pending live validation".
- Release blockers recorded in
  [`codex-voice.md`](../../apps/openagents/docs/research/codex-voice.md):
  an in-app consent step for App Review guideline 5.1.2(i), the privacy
  manifest, the privacy policy, and the full WebRTC license list.

**Other voice code on main**
- **CoderOS push to talk:** `os/bin/dictate-toggle` (`651d51f4f4`), a shell
  script that records with `ffmpeg`, transcribes with whisper.cpp
  `whisper-cli` and `ggml-base.en`, and types the text. Bound to Super+V in
  `crates/coder-binds`. Infrastructure, Linux only.
- **Psionic text to speech:** Sesame CSM-1B in
  `crates/psionic/crates/psionic-serve/src/csm_speech.rs` and
  `psionic-models/src/csm.rs`, served at `/v1/audio/speech` on CPU (imported
  in `737f94a17c`). Nothing calls it. The older Psionic checkout measured
  2,702 ms to the first audio for 160 ms of speech on a warm CPU, with
  12.6 s to load. The Silero-style `psionic-vad` crate was not imported.
  Pylon (`crates/pylon`) serves no audio.
- **NIP-LIVE:** [`nips/openagents/NIP-LIVE.md`](../../nips/openagents/NIP-LIVE.md)
  and `crates/live-host` (`d55b47041a`,
  [#11738](https://github.com/OpenAgentsInc/openagents/issues/11738))
  implement consent, speaking floors, and media epochs for text profiles
  only. No audio transport exists.
- **NIP-A0 voice messages:** `crates/nostr/src/domain/voice.rs`
  (`cded8b842c`), the upstream short voice note event kinds. Nothing sends
  or plays them.
- **Sales voice sessions:** `crates/coder-task/src/task/sales/voice.rs`
  (`9a46a330bd`, [#10882](https://github.com/OpenAgentsInc/openagents/issues/10882)),
  an authorization record for consented, human-supervised calls. Off by
  default, with no audio.
- **Unused AEC prototype:** `apps/openagents/crates/audio`, with `cpal`
  capture and WebRTC AEC3 behind a feature. No crate depends on it.
- **Video Lab:** `crates/coder-new/src/media/transcript.rs` uses whisper.cpp
  for word timings on recorded clips, not conversation.

## Comparison

| System | Dates | Repository and path | Language | Capture and transport | VAD and turns | STT | LLM | TTS | Best recorded latency | Why it ended |
|---|---|---|---|---|---|---|---|---|---|---|
| Onyx phone | 2024-11 to 2025-02 | `onyx` | TypeScript | Expo, HTTP | Push to talk | Groq Whisper large-v3 | Groq Llama 3.1 8B | None | None recorded | Abandoned |
| `transcribe` | 2025-02 to 2025-03 | `src/bin/transcribe.rs` | Rust | Files | None | Groq Whisper | Groq Llama | None | Batch | "Zero base" reset |
| v5s | 2025-07 | `v5s` | Rust, Swift | WebSocket | Push to talk | SFSpeech, whisper-rs, Groq | Groq, Ollama | ElevenLabs, Piper | Claimed 0.8 to 2.2 s | Abandoned in a week |
| Phone dialer | 2025-07 to 2025-08 | `voice-python`, `dialer`, `dashboard` | Python | Twilio WebSocket, Pipecat | Silero | Deepgram Nova-2 | Groq Llama 3.3 70B | Cartesia Sonic | Claimed only | No updates after 2025-08-27 |
| Whisper daemon | 2026-01 to 2026-02 | `crates/voice`, `crates/voice-daemon` | Rust | `cpal`, local | Hold key | whisper.cpp base.en | None | None | None recorded | Workspace prunes |
| Chirp playground | 2026-03 to 2026-06 | `apps/autopilot-desktop` | Rust | `cpal`, HTTPS | Push to talk | Google Chirp 3 | None | Chirp 3 HD | None recorded | Bun/Effect rebuild |
| Lyra, Autopilot, Autopilot4 | 2026-05 | `lyra`, `autopilot`, `autopilot4` | Rust, TypeScript | PCM over WebSocket | Server PCM VAD | Google, OpenAI | OpenAI, Gemma 4 | Psionic CSM, OpenAI, Chirp 3 HD | None recorded | Replaced each other |
| Khala app | 2026-06-26 | `clients/mobile/Khala` | Swift | On device | Push to talk | SFSpeech | Our chat API | AVSpeech | None recorded | TTS "sounded bad" |
| Sarah v1 avatar | 2026-07-07 to 07-10 | `apps/sarah` | TypeScript, Python GPU | WebRTC (WHEP) | Browser | Web Speech API | Gemma 4 | Chirp 3 HD, CosyVoice2 | Chirp 163 ms to first chunk | Owner order, likeness policy |
| Desktop voice mode | 2026-07-12 | `apps/openagents-audio`, `crates/oa-desktop-audio` | TypeScript, Rust | Rust helper, WebSocket | Google endpointing, barge-in | Google Chirp 3 | Registered actions | Chirp 3 HD | 185 ms TTS first byte; 2.2 s STT final | Electron deleted |
| Sarah realtime | 2026-07-28 to 08-28 | `packages/sarah` | TypeScript | WebSocket bridge | Semantic VAD | In model | `gpt-realtime-2.1` | In model | None recorded | TypeScript purge |
| Sarah LiveKit | 2026-07-30 to 08-28 | `apps/sarah-livekit-agent`, `infra/livekit` | TypeScript, Terraform | LiveKit WebRTC on GKE | Semantic VAD | In model | `gpt-realtime-2.1` | In model | 13.7 s to first audio | TypeScript purge |
| voice / one | 2026-08-06 to 08-17 | `voice`, `one` | Rust, Go | LiveKit WebRTC on GKE | Provider VAD and a timer | Gemini Live | Gemma 4, Gemini | Gemini TTS | 0.9 s to first spoken text | Replaced by Elixir |
| sarah (Elixir) | 2026-08-16 to 09-04 | `sarah`, `openagents.com` | Elixir | WebRTC to OpenAI | Semantic VAD | In model | `gpt-realtime-2.1` | In model | 969 ms first audio, p50 | Product returned to Rust |
| Parakeet dictation | 2026-10-02 to now | `apps/openagents/crates/voice` | Rust | `cpal`, local | Hold key | Parakeet TDT 0.6B v3 | None | None | None recorded | **Live** |
| Talk with Codex | 2026-10-07 to now | `apps/openagents/crates/voice-*` | Rust, Swift, Kotlin | WebRTC to OpenAI | In model | In model | Codex voice, relays to a saved agent | In model | None recorded | **Live** |

## What exists today

### Works and is wired

- Desktop Parakeet dictation (macOS arm64, Linux, Windows).
- Browser dictation through a paired computer.
- Talk with Codex on the desktop, iOS, Android, and the web, with the
  relay to a saved agent. Tested only against fakes.
- CoderOS push to talk (`os/bin/dictate-toggle`).

### Dead code on main

- `apps/openagents/crates/audio`: the AEC prototype, unused. It pins a
  different `cpal` version (0.18.2) from `crates/voice` (0.17.3).
- Psionic CSM-1B TTS: built, served, and called by nothing.
- `crates/nostr/src/domain/voice.rs` (NIP-A0): parsed and unused.
- `crates/openagents-web/src/pages/chat.rs` lines 3920 to 3925: the web
  composer's "Voice input" button is commented out until it works.
- `voice-experimental` in `apps/openagents/crates/ui/Cargo.toml`: an empty
  compatibility feature.

### Documented but unbuilt

- Promise B12, "You can talk to the agent and hear it answer", is `later` in
  [`crates/openagents-web/content/promises.toml`](../../crates/openagents-web/content/promises.toml).
  X10 (a named voice agent such as Onyx or Sarah) is `dropped` and returns
  as B12.
- NIP-LIVE audio and video transport.
- Streaming dictation with partial results (listed as investigation only in
  `desktop-parakeet-v3.md`) and dictation on the phone.
- Scrawl-style capture with a time-aligned transcript
  ([`docs/desktop/scrawl-capture.md`](../desktop/scrawl-capture.md)).
- Zeron gap rows VOI-01 to VOI-07
  ([`docs/research/zeron/media-and-appearance.md`](../research/zeron/media-and-appearance.md)),
  including physical-audio evidence (VOI-07).
- Voice in the Verse. "Voice" in `crates/verse*` means sound-mixer voices
  and text speech bubbles; [`docs/verse/chat.md`](../verse/chat.md) is text
  only.
- "Voice edit": the owner's oldest voice idea, editing files and the UI by
  talking ([`111.md`](../transcripts/111.md), line 13: "what if we edit
  these files via voice?"; episode 251, lines 26 to 31: "just rattle them
  off and have it update live"). No issue or design exists for it.

### Leftover infrastructure

A read-only check on 2026-10-21 of the `openagentsgemini` project found:

- GKE cluster `oa-livekit-prod` in `us-central1`, `RUNNING`, with node pools
  `oa-livekit-prod-app` (`e2-standard-8`) and `oa-livekit-prod-sfu`
  (`c2-standard-8`), six nodes in all.
- Memorystore Redis `oa-livekit-redis`, `STANDARD_HA`, `READY`.

The code that used them was deleted on 2026-08-28 (`d7574f6669`, whose
message calls the cluster "residual infra, not a reason to keep this
source"). `docs/cloud/2026-10-02-cloud-parallel-execution-audit.md` noted
the cluster on 2026-10-02. The IAM deny policy for it still exists
(`docs/deployment/automation-account.md`). The clusters `one-prod` and
`one-stg` from the `one` repository are also running; this audit did not
check what they serve.

## Lessons

### What worked

- **Local STT in Rust.** whisper.cpp (`a463ee5294`) and Parakeet
  (`80b946b153`) both worked on the first try, needed no account, and kept
  audio on the device. Parakeet is the only voice feature that has survived
  a move between repositories.
- **Chirp 3 HD for fast TTS.** It was measured three times at 163 to 198 ms
  to the first byte (#8613, AUDIO-7, AUDIO-8).
- **A narrow Rust audio helper.** `oa-desktop-audio`'s capture, packet
  framing, and buffer flushes passed a 60 s real-microphone fault run with
  every packet acknowledged and a 2 ms p95 barge-in acknowledgement.
- **Making the voice model a relay.** Since #12059, the voice model passes
  requests to a real agent and does not answer on its own. This addresses
  the failure seen in `voice` on 2026-08-10, where the speaker model claimed
  an action it never took.
- **Media straight from the device to the provider.** Both the Elixir
  `sarah` and Talk with Codex keep audio off our servers, which removes an
  SFU, TURN, and a bridge from what we run.

### What failed

- **Hosted media servers.** Self-hosted LiveKit took 13.7 to 15.2 s to the
  first audio, cost an unattended agent run about 1.91 billion tokens, and
  is still running two months after its code was deleted. LiveKit Cloud
  stopped answering when its inference quota ran out (`eagle`).
- **Chained cloud STT.** Google STT finals took 2.2 to 6.2 s, which is why
  the Elixir `sarah` chose speech-to-speech instead.
- **Owned voice cloning and avatars on one GPU.** CosyVoice2 needed 3 to
  5.8 s to its first chunk, and MuseTalk served one session for each L4 GPU
  at 20 FPS.
- **Platform TTS.** The Khala app's `AVSpeechSynthesizer` was removed after
  one day.
- **Provider lock-in on voice identity.** OpenAI Realtime would not take
  Sarah's custom voice ([`270.md`](../transcripts/270.md), line 33), and
  HeyGen would not take a synthetic likeness.
- **Tools inside realtime models.** Tool calls did not run in Sarah's voice
  mode, the LiveAvatar connector forwarded no tool events, and an MCP call
  in `voice` took 29 s.

### Recurring problems

1. **Rewrites instead of moves.** Voice was rebuilt from nothing about 20
   times. Each rewrite discarded working parts (the Rust audio helper, the
   Chirp adapter, the Whisper daemon) because they lived inside a host app
   that was deleted.
2. **Latency was measured once and then lost.** Every number in this audit
   came from a receipt deleted with its system. The two features that work
   today have no latency numbers at all.
3. **No real-device evidence.** From the AUDIO-8 acceptance that stayed open
   to the October issues closed with device checks deferred, voice has
   rarely been tested end to end with real speech on a phone.
4. **Infrastructure outlives code.** The LiveKit cluster, the Redis
   instance, the IAM deny policy, and the `one` clusters all remain.
5. **Language and repository churn.** Rust, TypeScript, Python, Swift,
   Kotlin, Elixir, and Go have each held a voice stack. Under today's rules
   only the Rust parts are kept.
6. **Turn detection is always someone else's.** Every system used push to
   talk or a provider's VAD. None owned end-of-turn detection, which the
   Pipecat audit identified as the part to take (Silero plus `smart-turn`).

## Recommendations

### One stack

Build one Rust voice engine and keep each shell thin. Specifically:

- **Contract and lifecycle:** keep `voice-session` and `proto/src/voice.rs`
  as the one call contract. Every shell already renders it.
- **Engine:** add a `VoiceBackend` trait with two backends, chosen for each
  call:
  - `codex-realtime`, the current Talk with Codex path. It costs us nothing
    and needs the user's ChatGPT login.
  - `cascade`, an owned path for users without Codex, for offline use, and
    for voices we control.
- **The cascade, all Rust:**
  - Capture and AEC: `apps/openagents/crates/audio` (`cpal` and WebRTC AEC3)
    on the desktop. Platform capture stays in Swift and Kotlin on phones,
    passing frames to Rust.
  - VAD and end of turn: Silero VAD and `smart-turn` as ONNX models through
    `ort`, which Parakeet already uses.
  - STT: Parakeet TDT 0.6B v3, extended to stream partial results.
  - LLM: none of its own. The cascade relays to the chosen saved agent with
    the same three tools, and summarizes for speech.
  - TTS: a Psionic voice. CSM-1B on CPU is too slow (2.7 s to first audio),
    so serve it from a GPU Pylon, and add a small local voice such as
    Kokoro-82M through ONNX for computers without a GPU.
  - Transport: the user's own computer runs the cascade. Phones and
    browsers reach it over the existing device room and NIP-REACH direct
    channel, with WebRTC only where the shell already has it. No SFU, no
    TURN service, and no cloud bridge of our own.
- **Inference placement:** on the user's computer first, a Pylon second,
  and a cloud provider never by default. This follows the Psionic and Pylon
  direction and the existing "audio never touches our servers" rule.
- **Not recommended:** LiveKit (in any hosting), Pipecat (Python), and a
  hosted voice gateway. Each was built and measured here, and each cost more
  than it returned.

### Owned engine and the voice marketplace

Owner direction after this audit, 2026-10-21:

1. **We run our own voice engine.** Talk with Codex depends on each user's
   ChatGPT login and on a voice we can't choose, extend or sell. It stays as
   a backend, but the `cascade` backend becomes the product default and the
   first thing we build after the harness. The order is V1, V3, V4, V5, V6.
2. **Voices are assets creators own and sell.** The co-founder's idea: where
   ElevenLabs sells AI voices, the Verse and Game Lab honor human labor.
   - **Commission a human.** A creator pays a voice actor (for example $20)
     to record lines for a game or film. The recording comes back as a
     source in Video Lab or Game Lab.
   - **License a trained voice.** A person gives the Verse their voice once,
     a voice model is made from it with their consent, and they list it. A
     creator who uses it pays for each use, and the owner is paid without
     recording every line or renting a studio. This lowers the barrier for
     people with no recording setup.

How this changes the stack:

- **Two speeds of speech.** Live calls need speech to start in under
  500 ms. Lines for a game, a film or an NPC are a **batch render**: a
  job that makes a WAV file, the way `lab_voice` already places a recorded
  voiceover on a cut. Batch rendering is where cloning works first, because
  speed matters much less there (CosyVoice2's 3 to 5.8 s to the first chunk,
  under [What failed](#what-failed), is fine for a batch job).
  Realtime cloned voices come later, on a GPU Pylon.
- **The voice is a private asset.** A voice model and its enrollment
  recordings follow the MetaHuman rule in `docs/video-lab/README.md`:
  they are never uploaded to a shared bucket in raw form, never put in a
  dataset, trace or Gym run, and every render made with them carries
  `no_training` and the voice's identity. They live on the owner's computer,
  or on a Pylon the owner chooses, as `private-local` inputs under Labs job
  placement (`docs/labs/README.md`, "Jobs and placement").
- **Consent is a record, not a checkbox.** Enrollment has the person read a
  prompt with a fresh challenge phrase, which ties the recording to the
  account that lists the voice. The listing states what it allows (games,
  films, ads, live NPCs, explicit content) and can be withdrawn. Withdrawal
  stops new uses; renders already made stay licensed.
- **Every use is metered.** A render job that uses a listed voice records
  the voice, the creator, the length in seconds and the price. Payouts reuse
  the account earnings and Lightning address work (#12307) instead of a new
  payment path.
- **Provider lock-in is the reason to own it.** The audit records OpenAI
  Realtime refusing Sarah's custom voice (`270.md`, line 33). A marketplace
  of people's own voices can't depend on a provider that decides which
  voices exist.

### Draft issues

These are drafts. None has been filed.

**V0. Tear down the leftover LiveKit infrastructure.**
Delete GKE cluster `oa-livekit-prod`, Memorystore `oa-livekit-redis`, the
related DNS records, and the IAM deny policy in
`docs/deployment/automation-account.md`, after the owner confirms. Check
what `one-prod` and `one-stg` serve, and remove them if nothing uses them.
Acceptance: `gcloud container clusters list` no longer shows them.

**V1. Voice latency harness.**
Add a Rust harness that plays recorded speech into a voice backend and
records time to the first partial, time to the final transcript, time to
the first audio, barge-in stop time, and RTF. Check its results in under
`bench/voice/`. Run it against Talk with Codex and Parakeet first.
Acceptance: one committed result for each of the two live features.

**V2. Real-device voice evidence.**
Run Talk with Codex on a physical iPhone, an Android phone, a Mac, and
Chrome, with real speech, and record the G0 to G4 results. The owner's
steps go in `NEEDS_OWNER.md`.

**V3. `VoiceBackend` trait in `voice-session`.**
Move the Codex realtime path behind a trait without changing behavior, so
a second backend can be added. Acceptance: existing voice tests pass with
the trait in place.

**V4. Streaming Parakeet with Silero VAD.**
Extend `openagents-voice` to stream partial results and end turns with
Silero VAD (and later `smart-turn`) through `ort`. Use it for desktop
dictation first. Acceptance: partial text appears while you speak; the V1
harness records time to the first partial.

**V5. Psionic speech on a Pylon.**
Serve CSM-1B (or a faster voice) from a GPU Pylon as a NIP-CJ job with
streamed audio, and add a local ONNX voice for computers without a GPU.
Acceptance: under 500 ms to the first audio on a GPU Pylon, measured by the
V1 harness.

**V6. Cascade backend.**
Put capture, VAD, STT, the relay to the saved agent, and TTS together as
the `cascade` backend, run on the user's computer, reachable from phones
and the web over the device channel. Acceptance: a call with no Codex login
works end to end, measured by V1.

**V7. Voice edit.**
Design the owner's long-standing "talk and see the UI update" mode on top
of the relay: spoken edits go to the active agent as messages and show
their results live. Start with a design note and an acceptance journey.

**V8. Retire dead voice code.**
Either wire `apps/openagents/crates/audio` into V6 or delete it; align its
`cpal` version with `crates/voice`; remove the empty `voice-experimental`
feature; and decide whether NIP-A0 voice notes belong in chat.

**V9. Voice enrollment and consent records.**
A person records a short enrollment with a challenge phrase from any
OpenAgents surface. The engine stores it as a `private-local` voice asset
with a consent record (owner, allowed uses, withdrawable) and makes a voice
model with an owned cloning model on the owner's computer or a chosen
Pylon. Acceptance: one test account enrolls, a sample line renders in the
enrolled voice, withdrawal blocks the next render, and nothing from the
enrollment reaches a shared bucket, dataset or trace.

**V10. Voice lines as a Lab job.**
Add a `speak` job (text, voice, style) to the Labs job types that returns a
WAV artifact, usable from `lab_voice` in Video Lab and for Game Lab NPC
lines. It inherits `no_training` and names the voice in the artifact.
Acceptance: a cut gets a voiceover rendered from text in an enrolled voice,
and `cut.py check` passes on it.

**V11. Voice marketplace: listings, metering and payouts.**
List enrolled voices with samples, allowed uses and a price for each use or
each second. Meter every `speak` job that uses a listed voice and pay the
owner through account earnings (#12307). Acceptance: a test creator renders
lines in a test voice owner's listed voice, and the owner's earnings show
the payment with the job as its receipt.

**V12. Commissioned human voice work.**
A creator posts lines with a fixed fee, and a voice actor records them in
the browser or uploads them. The recording arrives as a Video Lab or Game
Lab source with the actor credited, and payment is released on acceptance.
Acceptance: one end-to-end commission between two test accounts.
