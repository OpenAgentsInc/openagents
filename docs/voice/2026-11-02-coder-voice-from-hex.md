# Voice in Coder: what we take from Hex

*Proposal, 2026-11-02. Coder dictation is not built yet. The Parakeet engine
foundation described below is now qualified separately in Psionic chapter 153.*

This document does three things:

1. It summarizes [Hex](https://github.com/anomalyco/hex), anomalyco's local
   hold-to-talk dictation app, and lists the ideas worth taking.
2. It maps the voice we already have: Psionic's speech recognition, the
   Verse voice engine, and the plans in
   [coax-native-voice.md](coax-native-voice.md).
3. It proposes how to add voice input to Coder in the short term, in the
   terminal UI and in the Mac app, with phases and checks.

The reference clone is at `~/work/projects/repos/hex` (Hex 3.3.0, HEAD
`2626560`, MIT). Read it there; don't copy code from it. Paths below that
start with `hex/` are in that clone.

## Summary

- **Dictation first, conversation later.** In the short term Coder gets
  dictation: you hold a key, speak, and the words land in Coder's composer
  as a draft. You review and send it yourself. Talking with an agent out
  loud stays with the voice host and the `openagents.voice.v1` primitives.
- **Speech stays on your computer.** Psionic's Whisper already transcribes
  on the CPU and on Metal, faster than whisper.cpp
  ([transcribe.md](../../psionic/docs/transcribe.md)). Coder links
  `libpsionic-whisper.a` through `psionic/platform/psionic-stt.h`. No audio
  leaves the machine, no account is needed, and no speech provider is billed.
- **One engine, two hosts.** The rules (timing, levels, formatting, the
  vocabulary list) are Coax, shared by the terminal UI and the Mac app. The
  hosts do only microphone capture and key handling.
- **Code-aware from day one.** Coder knows the repository, so it can pass
  identifiers, file names and crate names to Whisper's word bias, and it
  can turn spoken symbols ("snake case user id", "dot rs") into code text.
- **Hex supplies the hard-won details:** a warm pre-roll so first words
  aren't lost, a minimum hold so stray presses are ignored, a release tail
  so last words aren't cut, a silence gate before inference, speculative
  partials with a whole-clip final, and an evaluation that measures
  vocabulary recall without hurting ordinary speech.

## What Hex is

Hex is one Rust binary (`hex`) that serves as the desktop app, the CLI, a
headless service and an MCP server, on macOS, Windows and Linux. You hold a
shortcut, speak and let go; Hex transcribes on the device and pastes into
the frontmost app.

| Part | How Hex does it | Where |
| --- | --- | --- |
| Capture | `cpal` on every OS; skips Bluetooth headset microphones and a closed laptop's built-in microphone; changes device only while idle | `hex/src/audio.rs`, `hex/src/audio/lid.rs` |
| Timeline | 10 s buffer, 450 ms warm pre-roll, 300 ms minimum hold, 200 ms tail after release, resampled to 16 kHz | `hex/src/dictation.rs:39-64` |
| Clock | Key event and audio timestamps on one clock, so the clip starts and ends exactly where the keys say | `hex/AGENTS.md:724-731` |
| Silence | The loudest 30 ms must pass −50 dBFS, then Silero VAD (threshold 0.4, minimum speech 160 ms), so silence never reaches the model and it never invents "thank you" | `hex/src/transcription/speech_detector.rs` |
| Pauses | A pause segmenter on 20 ms frames, and a cap that feeds a live stream only the edges of a long pause | `hex/src/transcription/pause_segmenter.rs`, `engine/streaming.rs` |
| Models | Parakeet (default), Whisper large-v3-turbo, Qwen3-ASR and others through ggml/GGUF on Metal or Vulkan; each pinned by repository, revision and SHA-256; a new model takes over only after it loads and warms up | `hex/src/transcription/models.rs` |
| Streaming | Speculative: a stream runs during speech for live text in Hex's own overlay, but the final always comes from the whole clip, and any doubt (failure, empty, no final in 1.5 s) falls back to it | `hex/AGENTS.md:744-756` |
| Warm-up | Model and microphone stay warm; a short silent inference before use keeps Metal weights resident | `hex/src/parakeet.rs:1225-1243` |
| Text | Fixed pipeline, each stage best-effort: spoken formatting ("new line"), spoken symbols ("underscore"), filler words, replacements, an optional LLM rewrite, user transforms | `hex/src/dictation/processor.rs:318-410` |
| Vocabulary | `{term, sounds_like[]}`; up to 64 terms biased per run, the current mode's terms first; Parakeet rescoring of look-alike spans, a context prompt for Whisper | `hex/src/dictation/vocabulary.rs` |
| Output | Clipboard paste with a guarded restore, Shift-Return for newlines so chat boxes don't send, workarounds for remote desktops and two Chromium versions | `hex/src/paste.rs`, `hex/src/keyboard.rs` |
| Keys | Hold by default, toggle optional, double-tap locks; Escape cancels only while recording | `hex/src/suppression.rs` |
| Control | `hex dictation start\|stop\|toggle`; `stop --print` writes the text to stdout and pastes nothing (exit 4: no speech); a loopback API with `/transcriptions`, `/streams` and `/dictations` | `hex/docs/cli.md`, `hex/docs/streams.md` |
| Tests | A dictation corpus replayed at release offsets −300 to +200 ms; a vocabulary eval scoring term recall, false replacements and word error rate on 200 LibriSpeech clips as a guard | `hex/src/dictation/corpus_tests.rs`, `hex/docs/research/custom-vocabulary.md` |

Two measured facts from Hex matter most for us:

- Releasing the key at 0 to +200 ms kept the last word in 10 of 10 clips;
  releasing 100 ms early kept it in 7 of 10. The 200 ms tail is what makes
  "let go when you're done" work.
- The warm pre-roll kept first words even when speech began 300 ms before
  the key went down.

## What we already have

| Piece | State | Where |
| --- | --- | --- |
| Psionic Whisper (`tiny`…`large-v3-turbo`), beam search, word bias (`--words`), CPU and Metal; WER at or below whisper.cpp | Shipped, `openagents transcribe` default | `psionic/lib/whisper/`, [transcribe.md](../../psionic/docs/transcribe.md), #12614 |
| `psionic-stt.h`: open a model once, transcribe 16 kHz samples in process | Shipped, used by the voice host (#12622) | `psionic/platform/psionic-stt.h` |
| Speed in the voice host: 184–240 ms per recognition of up to 4.3 s on `base.en` with the GPU encoder; re-recognizing every second makes the final ready when the turn ends | Measured | transcribe.md, "Engine API for in-game voice" |
| Voice engine: resample to 16 kHz, levels, noise floor, VAD, end of turn, barge-in | Shipped | `verse/voice.coax`, `verse/platform/voice.h` |
| Voice host on Cloud Run with Google and Psionic providers | Shipped | `verse/voice-host/`, [dev.md](dev.md) |
| Web voice in realms, iOS voice toggle | Web shipped; iOS simulator only | `coax/web/voice.js`, `apps/openagents-ios/OpenAgents/Voice/` |
| Psionic Parakeet TDT 0.6B v3 in Coax | Resident native API and explicit CLI backend; Metal encoder, CPU decoder; all 29 frozen recordings match actual NeMo tokens/text and intermediate tensors meet tolerance, with 25-language smoke scores | [Engine guide](../../psionic/docs/parakeet.md), [chapter 153](../../psionic/book/ch153-transcribe-with-parakeet.md) |
| Psionic text to speech | Rule-based teaching version only | `psionic/lib/speak.coax`, ch. 155 |
| Zeron's dictation in Coder Desktop (Rust): Parakeet v3 INT8 on ONNX, hold Cmd+D, at most 60 s, transcribes into the draft and never sends | Shipped in the sibling repo; not ported to the Coax Mac app | `~/work/coder-desktop/crates/voice`, `crates/ui/src/dictation.rs` |
| Coder terminal UI and CLI | Shipped, no voice | `coder/tui/coder.coax` (submit is `cd_submit`, line 1076) |
| OpenAgents Mac app (Coax port of Zeron) | Shipped, no voice; "Not ported from Zeron's input: … dictation" | `apps/openagents-desktop/`, [coax-desktop.md](../desktop/coax-desktop.md) |

[coax-native-voice.md](coax-native-voice.md) decided Google speech for
*conversations* through our voice host, and named no model running on our
machines (decision 7). Since then the voice host runs Psionic Whisper in
process. Dictation in Coder is a different job: short, private clips that
become text you edit. Local Psionic fits it better than Google on every
count: no latency to a server, no per-minute bill, no audio leaving the
machine, and it works offline. This proposal doesn't change the
conversation decisions.

## Proposal: dictation in Coder

### What you do

- **Mac app.** Hold **⌘D** (Zeron's default) or press and hold the
  microphone in the composer. Speak. Let go. The text appears in the
  composer at the caret, and you edit and send it. While you hold, a level
  meter and the words so far show in the composer.
- **Terminal UI.** Terminals don't report key releases reliably, so press
  **Ctrl+Space** to start and again to stop (Escape cancels). The status
  line shows "Listening" and the words so far; the final text goes into the
  editor.
- **Anywhere else.** `openagents dictate` listens until you press Enter (or
  for `--seconds N`) and prints the text on stdout. Exit 4 means no speech.
  This is Hex's `stop --print`, and it lets any agent or script take voice
  input.

Dictation never sends a message. Speech selects nothing and runs nothing;
it only types into a draft. That keeps the rule from #12059 and the Verse:
a transcript is text, never a command.

### How it works

```text
 microphone ─► host capture (Swift in the app, C in the CLI)
                 │ 48 kHz s16 frames, with timestamps
                 ▼
 verse/voice.coax  resample to 16 kHz, levels, noise floor, speech likelihood
                 │
                 ▼
 dictation.coax  timeline: 450 ms pre-roll, 300 ms minimum hold, 200 ms tail,
                 60 s cap; level gate; "is there speech at all"
                 │ 16 kHz f32 clip
                 ▼
 psionic_stt_text(clip, words)   Psionic Whisper, model mapped once and kept warm
                 │
                 ▼
 dictation.coax  spoken symbols, casing, filler words, join with the text
                 before the caret
                 │
                 ▼
 composer draft (app: apps/shared/zeron/composer.coax; TUI: Coder's editor)
```

| Concern | Owner | Notes |
| --- | --- | --- |
| Microphone, permission, device choice | Host: Swift (`AVAudioEngine`) in the Mac app; C (`AudioQueue` on macOS, ALSA/PipeWire later) for `openagents dictate` and the TUI | Platform effects, per the Coax-first rule. Skip Bluetooth headset microphones by default, as Hex does |
| Key down and key up, with timestamps | Host | The app has the real key events. The TUI uses toggle |
| Resampling, levels, VAD | `verse/voice.coax` | Already built and tested; no second copy |
| Timeline, gates, caps | New `apps/shared/dictation.coax` | Pure rules with `#test`, checked on recorded fixtures |
| Recognition | `libpsionic-whisper.a` via `psionic-stt.h` | Linked in process; `base.en` by default, `small.en` as an option |
| Text rules and vocabulary | `apps/shared/dictation.coax` | Pure, deterministic, `#test` per rule |
| Inserting the text | The composer | We own the input box, so we insert directly. No clipboard, no synthetic keys |

The TUI runs the capture and recognition in a helper process
(`openagents dictate --stream`), which prints partial and final lines as
JSON. Coder reads them the way it reads its engines. That keeps the
microphone and the model out of the TUI process and gives the CLI verb and
the TUI one implementation.

### Latency

Targets on an Apple M-series Mac, `base.en`, encoder on Metal:

| Step | Target (p50) | Basis |
| --- | --- | --- |
| Key down → listening | ≤ 50 ms | Microphone kept open while the app is focused and dictation is on; the pre-roll covers the rest |
| Key up → text in the composer, 5 s clip | ≤ 300 ms | Re-recognize the clip every second while you speak (the voice host's method), so the final pass only adds the last second |
| Key up → text, 30 s clip | ≤ 600 ms | One Whisper window |
| First use after launch | Model loaded and warmed in the background | Hex's silent warm-up keeps Metal weights resident |

Every number gets measured and committed under `bench/voice/` with the
machine, model and build, as coax-native-voice.md requires.

### Taking Hex's ideas

**Take now:**

1. **Timeline constants.** 450 ms pre-roll, 300 ms minimum hold (shorter is
   a tap: show "Hold to dictate"), 200 ms tail after release, 60 s cap.
   The app timestamps key events and audio on one clock.
2. **Silence first.** Run Whisper only if the loudest 30 ms passes
   −50 dBFS and `voice.coax` saw speech for at least 160 ms. Otherwise
   report "No speech detected". Whisper invents words on silence, so this
   gate matters more for us than for Parakeet.
3. **Speculative partials, whole-clip final.** Partials come from
   re-recognizing the clip so far. The final always comes from the whole
   clip. Only the final enters the draft.
4. **Project vocabulary.** Build the word list from the repository:
   identifiers near the caret and in open files, file and directory names,
   crate and package names, and names from recent commits. Put the most
   local terms first and cap the list (Hex caps at 64). Pass it as
   `psionic_stt_text`'s `words`. Add a user list in settings
   (`{term, sounds_like}`) for names the repository doesn't show. This uses
   the git-mined index from deterministic-first context (#11210), not a
   model call.
5. **Spoken symbols and casing, as fixed rules.** "underscore", "dot",
   "slash", "dash", "colon", "open paren", "backtick", "new line"; "snake
   case user id" → `user_id`, "camel case user id" → `userId`, "all caps
   max size" → `MAX_SIZE`. Each rule has a join direction (next word, both,
   previous), as `hex/src/dictation/spoken_symbols.rs` does. These are
   bounded text formatting after the route is already chosen (dictation
   into a draft), so they don't conflict with the rule against keyword
   intent routing. They never trigger an action.
6. **Filler words.** Drop "um", "uh", "erm", and give a dropped capital to
   the next word.
7. **Joining.** Join with the text before the caret: sentence case and
   spacing follow what's already there. Newlines are inserted as newlines,
   never as Enter.
8. **Pinned models.** Models are fetched into
   `~/.cache/openagents/models/whisper/` with their SHA-256 (already the
   case for `openagents transcribe`). Switching models takes effect only
   after the new one loads and warms up; a failed switch keeps the old one.
9. **Evaluation.** See [Checks](#checks).

**Take later:**

- **Parakeet in Coder.** Psionic's bounded resident engine now matches actual
  NeMo on chapter 153's frozen 29-recording qualification. Its measured offline
  baseline processes 298.435 s audio in 48.375 s inference on an M5 Max, with
  a 20.272 s fresh engine open (filesystem cache uncontrolled). That is not
  a 300 ms key-release result. Keep it resident, measure dictation timing and
  implement the 60-second segmentation policy before choosing it as default.
  Hex makes Parakeet the default for speed and accuracy, and Zeron ships it.
  Hex's vocabulary rescoring (rescore look-alike spans with the
  transducer's score instead of boosting tokens) is the method to copy
  then.
- **A cleanup pass** through our gateway (Google first), off by default:
  "return only the text", never change numbers, skip it for numbers-only
  text, a deadline, and the plain transcript if it fails. Hex's rules from
  `hex/src/dictation/processor.rs` are the starting point.
- **Pause trimming** for long dictations, if real use shows Whisper
  dropping words after long pauses.
- **Linux capture** in the C host, when Coder runs on Linux desktops.

**Don't take:**

- Pasting into other apps, clipboard restore, synthetic keystrokes and
  their workarounds. Coder owns its composer.
- User JavaScript transforms (QuickJS) and spoken command phrases ("send",
  "stop"). Spoken commands are keyword routing; keys and buttons stay the
  only controls.
- A loopback HTTP service for other apps. `openagents dictate` on stdout
  covers agents and scripts.
- Audio ducking, media pause, meetings and system-audio capture.

### Privacy and invariants

- Audio stays in memory for one dictation and is then dropped. It's never
  written to disk, a transcript store, telemetry or training data. Only the
  text enters the draft.
- Dictation is off until you turn it on in Settings, which says plainly that
  speech is processed on this computer. The Mac app adds
  `NSMicrophoneUsageDescription` to `apps/openagents-desktop/Info.plist`.
- `INVARIANTS.md` says the microphone opens only through the voice toggle
  and after the person sees that Google processes their speech. Local
  dictation needs an amended invariant in the same change as D1: Coder's
  dictation setting is a second opt-in path, its audio never leaves the
  machine, and no speech provider is called. The check is a test that the
  dictation path links no network code and writes no audio files.

## Phases

Each phase ends with its checks passing and a measurement committed.

- **D0. `openagents dictate`.** C capture on macOS through `AudioQueue`,
  `verse/voice.coax` for resampling and levels, `apps/shared/dictation.coax`
  for the timeline and gates, `psionic-stt.h` for recognition. Flags:
  `--model`, `--words`, `--seconds`, `--stream` (JSON lines: `partial`,
  `final`, `no_speech`), `--json`. Exit 0 with text, 4 with no speech.
- **D1. Coder terminal UI.** Ctrl+Space toggles dictation through
  `openagents dictate --stream`; partials in the status line; the final into
  the editor before `cd_submit` ever runs. The invariant amendment lands
  here.
- **D2. Mac app.** The microphone in the composer (Zeron's position and
  states: "Hold to dictate", "Listening", "Transcribing"), ⌘D hold to talk,
  a settings toggle and an input-device picker. Capture in Swift,
  recognition in process with `libpsionic-whisper.a`, the model warmed at
  launch when dictation is on.
- **D3. Code-aware text.** The project vocabulary, spoken symbols, casing
  and filler rules in `dictation.coax`, each with `#test`s, plus the user
  word list in settings.
- **D4. Evaluation and tuning.** The corpus and harness below, and the
  latency table filled in. Pick the default model (`base.en` or `small.en`)
  from the numbers.
- **D5. Later.** Integrate the now-qualified Parakeet engine: resident lifetime,
  silence admission, 20.47-second model-bound segmentation, vocabulary rescoring
  and measured release latency. The optional cleanup pass and Linux capture
  also remain here. Engine qualification does not complete D0–D4 or authorize
  opening a microphone.

Talking with Coder out loud (spoken replies, barge-in) is not in these
phases. When it comes, it uses the voice host and `openagents.voice.v1`:
a `Route` to the Coder session and the Mac app as one more client.

## Checks

- `coax test apps/shared/dictation.coax --library`: timeline cases (tap
  under 300 ms, release tail, 60 s cap), the level gate, and every text
  rule with its join direction.
- **A code-dictation corpus.** Clips made with macOS `say` from a fixed
  script (the way `openagents transcribe corpus` makes its clips), plus a
  few of the owner's own recordings kept private like `cut.wav`:
  - sentences naming this repository's terms (`cd_submit`, `psionic-stt.h`,
    `voice.coax`, `ZeronState`, Coax, Psionic, Verse);
  - spoken symbols and casing ("snake case", "dot rs", "open paren");
  - look-alike distractors so the vocabulary bias is caught when it
    replaces ordinary words.
- **Scores**, extending `openagents transcribe compare` and
  `psionic/lib/whisper/score.coax`:
  - term recall: vocabulary terms spelled exactly;
  - false replacements: distractors turned into terms;
  - word error rate on term-free clips, with and without the word list. The
    list may not raise it by more than 0.3 points (Hex's gate);
  - word error rate counted after normalizing terms, so a correct
    `OpenAgents` isn't two errors against "open agents".
- **A release sweep.** Replay each clip with the key released −300 to
  +200 ms from the end of speech, and pressed up to 300 ms after the start;
  report how often the first and last words survive. Target: 10 of 10 at
  0 ms and later.
- **Latency.** Key up to text in the composer, per clip length, committed
  under `bench/voice/`.
- **A real run.** The owner dictates into Coder on the Mac in D2. The steps
  go in `NEEDS_OWNER.md`, naming the composer's microphone and ⌘D.

## Open questions

These have a recommended answer; the owner may change them.

1. **Default model.** Recommend `base.en`: it spells our names right with
   beam search and the word list, and runs in about 200 ms per pass on
   Metal. `small.en` is the option for accents and noise.
2. **Shortcut in the terminal.** Recommend Ctrl+Space. Terminals with the
   kitty keyboard protocol could get hold-to-talk later.
3. **Microphone warm or cold.** Recommend warm while the app is focused and
   dictation is on, released when the app goes to the background. The
   macOS microphone indicator then shows only while Coder is in front.

## Reference

- Hex: `~/work/projects/repos/hex`. Start with `hex/AGENTS.md` (the
  architecture and invariants), `hex/docs/features/dictation.md`,
  `hex/docs/research/custom-vocabulary.md`,
  `hex/docs/research/vocabulary-boost-results.md` and `hex/docs/streams.md`.
- Zeron's dictation: `~/work/coder-desktop/crates/voice`,
  `crates/ui/src/dictation.rs`, `docs/reference/desktop-dictation.md`.
- Ours: [transcribe.md](../../psionic/docs/transcribe.md),
  [coax-native-voice.md](coax-native-voice.md), [dev.md](dev.md),
  [coax-desktop.md](../desktop/coax-desktop.md), and chapter 25 of the Coax
  book, [Building a voice system in pieces](../../coax/book/ch25-case-study-voice.md).
