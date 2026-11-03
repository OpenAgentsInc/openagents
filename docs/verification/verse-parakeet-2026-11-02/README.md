# Verse voice host: resident Parakeet recognition (#12669)

Recorded audio through the actual voice host (`verse/voice-host/build/voice-host
--recognizer parakeet --synthesizer local --agent echo`), the dev client in
hold-to-talk mode, and Aethyr's event gate (`aethyr/voice.coax`, linked into
the client with `--aethyr`). No microphone was opened and no sound was played:
replies are rendered to memory only. Harness:
`coax/web/tool.sh verse/voice-host/parakeet-check.coax MODEL_DIR [CASE]`.

## Setup

| | |
| --- | --- |
| Machine | coderos-4080: Intel Core i7-14700K (28 threads), NixOS, gcc 15.2.0. Shared with other agents: load average 17 to 43 during the runs |
| Model | `nvidia/parakeet-tdt-0.6b-v3` @ `541d1f99`, the pinned files at `~/models/parakeet-tdt-0.6b-v3-541d1f99` (hashes checked on open) |
| Device | CPU (`--parakeet-device cpu`, the Linux default). Parakeet's GPU path is Metal only, so the RTX 4080 is not used; Linux GPU inference is #12672 |
| Compiler | the cached release compiler `~/.cache/coax-release/be3d4467…/coax` |
| Base | `origin/main` at `72efea3013` |

## Results

All cases pass. The first full run (14 min) passed 11 of 13; the two
failures were the harness's own (a `--quiet` client prints no `turn` lines, and
`stale=` also matched the client's downlink counter). After fixing those, the
`realm` and `long` groups were rerun alone; nothing else was rerun.

| Case | Turn | Result |
| --- | --- | --- |
| `hello-foreman.wav` | 60,800 samples, 1 call | exactly `Hello Foreman. What are we building today?`; Aethyr: ACCEPTED, then DUPLICATE; replay matched |
| `build-red-house.wav` | 103,680, 1 call | exactly `Build a small red house next to a tall tree.` |
| `garden-pause.wav` | 77,120, 1 call | exactly `Let's take the long way through the garden, then pick a door.` |
| `wait-stop.wav` | 38,400, 1 call | exactly `Wait, stop.` |
| the four, 1 s apart | 343,680 (21.5 s), **2 calls** | every word in order, none split at the cut: `Hello Foreman, what are we building today? Build a small red house next to a tall tree. Let's take the long way through the garden. Then pick a door. Wait, stop.` Punctuation differs from the clips heard alone (`Foreman,`, `garden. Then`): longer context, not the seam. The harness now expects this recorded text |
| 1.5 s of silence | 0 voiced frames | no speech, decided without a model call; FINAL with no text, then ENDED; Aethyr applies NO_SPEECH |
| cancel after release, then `wait-stop` | | `t1 parakeet cancelled`, never delivered; `t2` exact, Aethyr request 2 epoch 1 ACCEPTED, finals=1 |
| barge-in after release, then `wait-stop` | | `t1 parakeet stale: 0 of 1 window(s)` (stopped before its call); `t2` exact |
| the realm cancels alone | | the host delivers `t1`; Aethyr refuses it STALE (`stale=1`), accepts `t2`; replay matched |

## Latency

The engine's own inference time per turn (host log), and the client's time
from TURN_END to the FINAL. Single runs on a loaded shared CPU: these are what
happened, not a benchmark.

| Turn | Audio | Inference | TURN_END to FINAL | Real-time factor |
| --- | --- | --- | --- | --- |
| hello-foreman | 3.8 s | 30.3 s (34.6 s in an earlier single run) | 30.4 s | 8.0 |
| build-red-house | 6.5 s | 94.4 s | 94.5 s | 14.6 |
| garden-pause | 4.8 s | 41.1 s | 41.2 s | 8.5 |
| wait-stop | 2.4 s | 18.4 s | 18.4 s | 7.7 |
| long turn, 2 windows | 21.5 s | 285.3 s, then 207.5 s when rerun | 285.4 s / 207.6 s | 13.3 / 9.7 |

Loading the resident model (with its SHA-256 checks) took 9.0 to 10.7 s, once
per host. The host's own overhead after inference is under 0.2 s. On this CPU
path local recognition takes roughly 8 to 15 times the audio's length, so it
is correct but not interactive here. Interactive use needs the Metal path
(Psionic's measured Mac numbers are in
[the engine qualification](../../../psionic/docs/verification/parakeet-2026-11-02/optimization/README.md))
or a Linux GPU path (#12672). The Mac measurement is an owner action in
`NEEDS_OWNER.md`.

## Quality

Exact transcripts on 4 of 4 single-clip turns; on the 2-window turn every word
is right and in order, with two punctuation differences from the separate
clips. Four short synthetic English clips are a smoke test, not a population
accuracy claim; the engine's 29-recording NeMo comparison stays the accuracy
reference.

## Checks

- `coax/bin/coax test verse/voice-asr.coax`: 13 checks; `coax mutate`: 198
  mutants, 198 killed.
- `coax/bin/coax test aethyr/voice.coax`: 31 checks (with `replay.coax`'s);
  `coax mutate --root-only`: 188 mutants, 188 killed (`replay.coax` keeps its
  own gate).
- `verse/voice-host/test.sh --offline` on Linux: every offline case passes with
  the old `--provider local` flags (health, hello, pause, observe, barge-in,
  decision gain, two sessions).
- `coax/web/tool.sh tools-check`: 0 new files without a reason.
- The Cloud Run `Dockerfile` build of `speech.c` compiles with
  `-DOA_VOICE_NO_WHISPER -DOA_VOICE_NO_PARAKEET`.
