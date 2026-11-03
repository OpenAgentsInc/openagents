# `openagents dictate`: microphone to local Parakeet text (#12667)

Checks of the standalone dictation command (proposal phase D0) through the
real CLI: every case runs `apps/openagents-cli/build/openagents dictate`,
which runs the dictation host `build/openagents-dictate`. No microphone was
opened and no sound was played. Audio came from the recorded-audio capture
backend (`--capture fixture`, `OPENAGENTS_DICTATE_FIXTURE=verse/voice-host/fixtures/wait-stop.wav`
or 1.5 s of digital silence) on a virtual clock, or, for the Linux recorder
pipe, from a stand-in `pw-record` on `PATH` that writes zeros, exits at once,
or is absent.

Harness: `coax/web/tool.sh apps/openagents-cli/tests/dictate-check.coax [MODEL_DIR [CASE]]`.
Log: [check-linux-cpu.txt](check-linux-cpu.txt).

## Setup

| | |
| --- | --- |
| Machine | coderos-4080: Intel Core i7-14700K (28 threads), NixOS, gcc 15.2.0, shared with other agents (load average 7 to 13) |
| Model | `nvidia/parakeet-tdt-0.6b-v3` @ `541d1f99`, `~/models/parakeet-tdt-0.6b-v3-541d1f99` (hashes checked on open) |
| Device | CPU (`--device cpu`, the Linux default). Parakeet's GPU path is Metal only |
| Compiler | the cached checked build `~/.cache/coax-release/f8545e6cc2dc…` (installed by `coax/build.sh`) |
| Base | `origin/main` at `8ebf571b91` |

## What runs where

- `apps/shared/dictation.coax` and `dictation-policy.coax` (unchanged; Coder
  for Mac's controller) decide the timeline, Verse's resampling and VAD,
  silence admission, tickets, cancellation, the 60 s cap, exact segments
  (`dgp_segment`, at most 3 calls) and the bounded text join. The host links
  it as its own C unit, as the Mac app does.
- `apps/openagents-cli/dictate-rules.coax` (`#authority`): capture backends,
  the recognition device, `--seconds`, input names, recorder arguments,
  control keys, a stalled input, the four endings and their exit codes.
  `coax mutate --library`: **256 mutants, 256 killed** (three survivors on the
  first run were closed with sharper checks: `"060"`, `"1:"` and `"-"`).
- `dictate.coax`: one dictation, in Coax: model, input, the listening loop,
  recognition segment by segment, the text or JSON result.
- `dictate-native.c` (effects only): the terminal and signals, the recorder
  pipe, the controller's C face, and the resident engine
  (`psionic_parakeet_open/text/clear/close`) on a worker thread so a cancel
  never waits for a model call. `dictate-mac.m`: macOS permission, CoreAudio
  inputs and an AudioQueue.

## Results

All 26 cases pass. The first full run passed 25 of 26: the stand-in
recorder for `recorder-zeros` could not find `cat` because the harness had
replaced `PATH`; after the harness fix only that case was rerun.

| Case | What it shows | Result |
| --- | --- | --- |
| final-json | one JSON object; `samples` 44,800 = 1000 ms to Enter at 2.6 s + 200 ms tail | exit 0 |
| final-text | stdout is exactly `Wait, stop.\n` | exit 0 |
| events | 4 status lines (`opening_input`, `listening`, `finishing`, `recognizing`), no text in any, then one `final` | exit 0 |
| seconds | `--seconds 1` ends by itself: 19,200 samples | exit 0 |
| cap | 60 s cap: 952,800 samples, `limited:true`, 3 segments joined once each | exit 0 |
| silence | `no_speech`/`silence`, 0 segments: no recognition call | exit 9 |
| tap | a 200 ms hold: `no speech detected (too_short)`, empty stdout | exit 9 |
| cancel-script, cancel-escape, cancel-signal | a scripted cancel, the Escape byte on stdin, SIGINT: no text | exit 130 |
| start-enter | `--wait-for-enter`, started 1 s in: 42,400 samples = the warm ring's 450 ms pre-roll + 2 s + tail | exit 0 |
| fault-permission / device / program | denied permission, a missing input, no recorder | exit 4 / 6 / 8 |
| fault-stall / ended / format / recognition | audio pauses, the input ends, an 8 kHz input, a failed model call | exit 1, `capture`/`capture`/`format`/`recognition` |
| model-missing | no model directory: refused before any input opens, no status lines | exit 6 |
| model-bad | empty model files: `model` error, `cannot map model.safetensors` | exit 1 |
| usage | `--seconds 0`, `--capture coreaudio` on Linux, no fixture file, `--words`, a file argument | exit 2 |
| gpu-linux | `--device gpu` on Linux: `this build has no Apple Metal backend`, never the CPU | exit 1 |
| recorder-missing | `--capture pipewire` with no `pw-record` on `PATH` | exit 8, `program` |
| recorder-ends | a `pw-record` that exits at once | exit 1, `capture` |
| recorder-zeros | a `pw-record` writing zeros through the real pipe path | exit 9, `silence`, recognizer `parakeet` |
| parakeet | `wait-stop.wav` through the resident model: exactly `Wait, stop.` (the reference recorded in [verse-parakeet-2026-11-02](../verse-parakeet-2026-11-02/README.md)) | exit 0 |

The cases without a model use a stand-in recognizer
(`OPENAGENTS_DICTATE_FIXTURE_TEXT`, only with `--capture fixture`); its
results say `"recognizer":"fixture"`, never `parakeet`.

## Time

The `parakeet` case took 30.6 s for 2.8 s of audio, including the model's
load (9 to 11 s on this machine, as in #12669): about 7x real time for the
recognition itself on a shared CPU. The recorder cases each loaded the model
(9.7 to 11.7 s) before opening the input. Interactive use needs the Mac's
Metal path or a Linux GPU path (#12672).

## Not claimed

- No real microphone, macOS permission prompt, or PipeWire/PulseAudio/ALSA
  device was used. `dictate-mac.m` was not compiled (no macOS SDK here).
  Both are owner actions in [`NEEDS_OWNER.md`](../../../NEEDS_OWNER.md).
- No partial transcripts, vocabulary bias or streaming: final text only.
- The Linux recorder clock is the sample count from the first chunk's
  arrival, not a hardware timestamp; a stalled recorder is caught by the
  2 s first-audio and 1 s gap limits.
- No sanitizer run of `dictate-native.c` and no Metal latency.
