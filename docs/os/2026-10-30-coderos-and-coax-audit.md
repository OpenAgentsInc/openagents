# CoderOS after the Nuke, and what a Coax OS could be

Written 2026-10-30. Everything here was checked read-only that day, from git
history, the private host flake on GitHub, and an SSH session to
`coderos-4080`. The only thing run on that machine was a throwaway Coax
build in `/tmp/coax-probe` (see [Coax on Linux today](#coax-on-linux-today)).

## Summary

- **CoderOS still runs, but nobody can change it from here.** The Nuke
  (`0b401561a0`, 2026-10-27) deleted `os/`, `docs/os/` and the Rust crates
  CoderOS builds. `coderos-4080` keeps working because the private flake
  `AtlantisPleb/coderos-hosts` pins `openagents` to `2ca255550e`
  (2026-10-19). Every edit to a public module now means checking out a
  pre-Nuke revision, and `nix flake update openagents` would break the host,
  because `?dir=os` no longer exists on `main`.
- **Unfreeze it first, in Nix, privately.** Copy the pinned modules, scripts
  and package definitions into `coderos-hosts`, and pin the Rust binaries to
  the frozen revision as their own input. That restores the edit loop in an
  afternoon and does not put Nix back in this repository.
- **Then rewrite the parts we own in Coax, one at a time.** The parts are
  the programs and policy, not NixOS: the update agent, the desk command, the
  camera daemon, the recording scripts, the CPU limits, and last the
  compositor. Linux's desktop stack (libwayland, wlroots, libinput, libdrm,
  GBM, PipeWire, V4L2) is C, and Coax compiles to C, so these programs link
  their real dependencies with no binding layer once Coax can import C
  headers and pass pointers.
- **Keep nixpkgs and the NixOS module system.** Replacing a package set of
  more than 100,000 packages and a lazy configuration language is a different
  project from ours. Coax should own a typed, checked host description that
  generates the NixOS configuration, not a new Nix.
- **The interesting C-level bets** are a Coax `sched_ext` CPU scheduler for
  the frame-plus-inference mix, the engine as its own DRM/KMS session with no
  compositor, and engine and inference in one process sharing GPU memory.
  Each needs a Coax feature the bootstrap does not have yet. Writing kernel
  modules in Coax is not one of the bets: see [What not to do](#what-not-to-do).

## What the Nuke removed

From `git show 0b401561a0^`, the tree as it stood before the Nuke:

| Part | Size | What it was |
| --- | --- | --- |
| `os/flake.nix`, `flake.lock` | 276 lines | Exports `nixosModules.coderos`, packages and dev shells; pins nixpkgs to `c25784012c`; defines no host. |
| `os/modules/coderos/` | 15 modules | `default` (boot, SSH, Nix settings, `nix-ld`, Docker, sudo and `safe.directory` options), `desktop` (1,143 lines: Hyprland or the Coder compositor on tty1, binds, monitors), `desk`, `capture`, `presentation`, `recording-hud`, `browser`, `android`, `camera`, `camera-grant`, `hands`, `compositor`, `cpu-limits`, `coder-host`, `coder-update`, `tailscale`. |
| `os/bin/` | 21 scripts | `screen-record` (1,523 lines), `recording-hud` (807), `presentation-mode`, `coder-compositor-session`, `dictate-toggle`, `oa-workspace`, `coder-update`, `camera-overlay`, `coder-pane`, `coder-close`, and the Android launchers. |
| `os/pkgs/` | 6 files | Nix builds of `coder-desk`, `coder-compositor` and `coderos-camera` from the whole workspace (`workspace-src.nix`), the Android SDK, and a font. |
| `os/tests/` | 13 files | Stub hosts, an every-capability evaluation check, and shell tests for each script. |
| `os/examples/workstation/` | 3 files | A complete host with placeholders. |
| Rust crates | about 38,000 lines | `coder-compositor` (Smithay), `coder-wm`, `coder-desk`, `coder-desk-cli`, `coder-hands`, `coder-hands-measure`, `coderos-camera` (ONNX Runtime hand tracking), `coder-service` (trial update and rollback). |
| `docs/os/` | 4 files | The move audit of 2026-09-28 (`git show 0b401561a0^:docs/os/2026-09-28-coderos-audit.md`), camera and hands, the Jev judge for hands. |

`os/` alone was 13,273 lines of Nix and shell over 34 commits. All of it can
be read with `git show 0b401561a0^:os/<path>`.

Also gone: `crates/psionic`, the Rust inference engine (2,963 files), and
`crates/jev`. Psionic has since been rebuilt in Coax (`psionic/`), and it
already runs CUDA kernels that `coax cuda` emits on `coderos-4080`. The
game engine is `verse/` (it was `metareal/` when this was first written).

## What `coderos-4080` runs today

Checked over SSH on 2026-10-30 at 22:39 CDT:

| Item | Value |
| --- | --- |
| NixOS | 26.05.20260906.c257840 (Yarara), kernel 6.18.49 |
| Host flake | `AtlantisPleb/coderos-hosts`, last pushed 2026-10-26 |
| Pinned `openagents` | `2ca255550e`, "CoderOS: vendor the root workspace with fetchCargoVendor", 2026-10-19 |
| Session | `coder-compositor 0.5.0 --backend udev` under `uwsm`, with `coderos-camera serve` and `agent-notices` |
| User services | `coder-earn` (serves the mesh within the owner's limits) |
| Timers | `coderos-cpu-limits` every 5 minutes; `coder-update` hourly |
| Capabilities on | Coder host and update, CPU limits, NVIDIA with the container toolkit, `rasdaemon`, PostgreSQL, the desktop with recording, dictation, camera, presentation, browser, Android, and the private Zoom, deck, Battle.net, gaming, firmware, dev-terminal and GPU-browser modules, Tailscale |

`coder-update` builds `coder` from this repository's `main` and hands it to
the host service for a trial. Since the Nuke it logs `already running
6fa6244f; nothing to build` each hour and exits cleanly. It is harmless,
but it is a timer pointed at code that no longer exists, and it will never
deliver another update.

### Where it breaks

1. **Editing a public module.** The modules live only at the pinned
   revision. The documented loop (`--override-input openagents
   git+file://$HOME/openagents?dir=os`) now points at a checkout with no
   `os/`.
2. **`nix flake update`.** Updating the `openagents` input moves it to
   `main` and evaluation fails. Updating nixpkgs alone is safe, because
   nixpkgs follows `openagents/nixpkgs`, which stays pinned.
3. **Rebuilding a Rust binary.** `workspace-src.nix` builds from the whole
   pre-Nuke workspace. That still works at the pinned revision, but any fix
   to the compositor or camera means a branch off a revision `main` has
   abandoned.
4. **Reinstalling.** A fresh machine works the same way as long as GitHub
   keeps serving `2ca255550e`, which it will, but the install path is
   undocumented anywhere current.

## Step 1: unfreeze CoderOS in the private flake

This repository's rule is Coax first, with Python and shell only as tooling,
so the Nix should not come back here. The private flake already holds the
host; it can hold the modules too.

1. Copy `os/modules/coderos/`, `os/bin/`, `os/pkgs/`, `os/tests/` and
   `os/flake.nix` from `2ca255550e` into `coderos-hosts`, with the commit
   message naming that revision.
2. Add a second input, `openagents-legacy`, pinned to
   `github:OpenAgentsInc/openagents/2ca255550ed17189815c8e1640baa28e55478596`
   as a plain source (`flake = false`). Point `workspace-src.nix` at it, so the
   compositor, camera and desk keep building byte for byte.
3. Drop the `openagents` flake input. `nix flake update` can then never pull
   in `main`.
4. Turn off `coderos.coderUpdate` until a Coax `coder` exists to update. The
   host service keeps the current binary.
5. Build the new toplevel and compare its store path with
   `/run/current-system` (`/nix/store/rfh1wx…-nixos-system-coderos-4080-…`).
   Identical paths mean nothing changed. Then switch.

After that, the edit loop is one repository and `nixos-rebuild switch
--flake ~/coderos-hosts#coderos-4080`. The camera, compositor and desk are
frozen Rust until their Coax replacements land, which is honest about what
they are.

## Step 2: rewrite what we own in Coax

NixOS is two things: nixpkgs and the module system, which we use and do not
own, and the programs and policy we wrote. The second half is what moves.
Each row is a drop-in replacement behind the same NixOS option, so the host
switches one program at a time and rolls back with the generation.

| Order | Part | Today | In Coax | What Coax needs first |
| --- | --- | --- | --- | --- |
| 1 | CPU limits | `cpu-limits.nix` + shell, every 5 min | A small program that reads sysfs and RAPL and writes the limits; the policy as an `#authority` file under `coax mutate` | File I/O in the host C, which a 30-line C face supplies |
| 2 | Update agent | `bin/coder-update` (245 lines of shell) + `coder-service` trial and rollback (Rust) | One Coax program: fetch, build with `coax build`, trial, roll back. The trial rules are policy, so `#authority` | Process spawn and file I/O in the host face; a Coax `coder` to update |
| 3 | Desk command | `coder-desk-cli` (Rust) speaking JSON to the compositor | Coax client of the same socket protocol, with the protocol in `proto/` | Unix socket in the host face |
| 4 | Recording and presentation | `screen-record` (1,523 lines), `recording-hud`, `presentation-mode`, `dictate-toggle` | Coax programs. Dictation shares the voice engine in `verse/voice.coax` (VAD, end of turn) | PipeWire through C; header import |
| 5 | Camera daemon | `coderos-camera` (Rust, V4L2, ONNX Runtime) | Coax capture over V4L2; the hand model as a Coax kernel | Pointer FFI; an inference kernel in Coax |
| 6 | Compositor | `coder-compositor` + `coder-wm` (Rust, Smithay, about 15,500 lines) | A Coax compositor over wlroots, or Coax window policy inside an existing wlroots compositor first | Header import (§17.2), callbacks, raw pointers into C structs |

Rows 1 to 3 are within reach of today's bootstrap plus a thin C face, the
same pattern as `verse/platform/host.c`. Rows 4 to 6 wait on the C
interface work the spec already plans: header import through libclang, and
foreign calls beyond scalars. That makes CoderOS a real consumer for
ROADMAP R1 (target ABI) and a Linux evidence row in §17.2, which today says
Linux "need[s] distinct runtime, linker, ABI and debugging evidence before
becoming [a] supported compiler target".

The NixOS side of each program is a 20-line derivation that runs `coax
build`. Coax's no-download, explicit-inputs build fits a Nix sandbox as it
is.

### The host description

The modules are Nix, and they can stay Nix. What Coax adds is the part
worth checking: which capabilities a host turns on, which devices it grants
to which program, the CPU limits, the update trial rules. Write that as a
Coax file that checks its invariants (`#test`, `#forall`, `#authority` where
it grants devices or spends power), and have `coax run` print JSON that a
fixed NixOS module reads with `builtins.fromJSON`. The Nix becomes a
stable adapter; the decisions become typed, tested code. The old audit's
stub-host evaluation check stays as the proof that the JSON is a valid
NixOS configuration.

## Step 3: what C everywhere makes possible

The kernel, the drivers' userspace interfaces, the desktop stack, Verse's
host and Coax's output are all C. That shared ABI is the opening. These are
bets, in the order I would try them.

### A Coax CPU scheduler with `sched_ext`

Kernel 6.12 and later, including the 6.18 kernel on `coderos-4080`, can
load a CPU scheduler as a BPF program (`sched_ext`). Clang compiles
restricted C to BPF, and the kernel's verifier refuses anything unbounded.
That restricted C is close to what Coax already emits for Metal: no
allocator, no recursion, bounded loops, explicit types. A Coax BPF profile,
shaped like the Metal emitter, could produce a scheduler that knows the
machine's real mix: a render thread with a 16.6 ms deadline, inference
batches that can wait, `cargo`-style builds that must never starve the
session (the 2026-09-11 out-of-memory incident the update module cites),
and a degrading CPU held at a power limit. Coax's `#forall` checks would
cover the policy before the verifier sees it, and a bad scheduler falls back
to the default one automatically. This is small, testable and unusual.

### The engine as the session

**First light, 2026-10-30:** `verse/kms/` runs Star Catcher on
`coderos-4080` this way, with DRM/KMS dumb buffers, evdev and a CPU
rasterizer over the realm's Coax light, at about 40 fps at 2560x1440. See
`verse/kms/README.md`.

Verse draws through Metal on Apple hardware. On Linux it could own the
display directly through DRM/KMS and GBM, as a session with no compositor:
boot, autologin, the engine on the screen, input from libinput. CoderOS
already has the shape for this; `desktop.nix` chooses the session program
per host. A kiosk-style Verse session is a smaller first step than a
Coax compositor, and it is also the path to a compositor: a compositor is an
engine that draws other programs' buffers as textures.

### Engine and inference in one process

The Metal emitter already turns Coax f32 procedures into shader code, and
`coax cuda` now emits CUDA C for NVRTC. That gives one kernel source
for the Mac, the iPhone and the RTX 4080. With the engine and an inference
engine both in Coax and both linking the same C runtime, they can share one
process and one GPU allocation: a model's output written into a buffer the
renderer reads, with no socket, no copy and no second scheduler. The voice
engine is already in Coax; hand tracking (row 5 above) is the first model
small enough to try. Psionic (`psionic/`) is the inference engine in Coax
that would share the process.

### Reproducible builds as derivations

Spec §16.1 says a reproducible Coax build records the compiler hash, target
description and every input, and §17.1 says import never downloads. That is a Nix
derivation in all but name. Generating a Nix derivation per Coax build, or
later a Coax-native content-addressed store, would let CoderOS describe
everything above the kernel with one build model. This one is last: Nix
already does it well, and the payoff only comes once most of the system is
Coax.

## What not to do

- **Don't write kernel modules in Coax yet.** Kernel C is GNU C with no
  libc, no floating point, and APIs that are mostly macros and inline
  functions. Rust for Linux took years of bindings and compiler work to get
  there. Coax's output assumes a libc and its print and check helpers, and
  the spec has no atomics API, memory model or freestanding profile yet
  (§12: "No job syntax, scheduler integration, atomic API or static race
  checker exists in the bootstrap yet"). `sched_ext` gets the scheduling
  benefit through BPF without any of that.
- **Don't replace nixpkgs.** A Coax package set is a decade of work with no
  product behind it.
- **Don't expect an all-Coax stack below the GPU.** The NVIDIA kernel module
  is open, but its userspace (CUDA, the GL and Vulkan drivers) is a closed
  binary. Coax can call it through C; it can't own it. An AMD or Intel GPU
  with Mesa is the only fully open path, if that ever matters.
- **Don't bring the Nix back into this repository** unless the owner decides
  CoderOS should be a public distribution again. Today it describes one
  private machine, and AGENTS.md names Coax and the host C compiler as the
  only toolchains.

## Coax on Linux today

On 2026-10-30 a clean `git archive HEAD coax` was built on `coderos-4080`
in `/tmp/coax-probe`, inside `nix-shell -p gcc python3`. The C11 seeds
(lexer, parser, checker, sources) and the C++20 host compiled with gcc and
linked `stage0` with no changes. That is the first evidence that coax0
builds on Linux. The full `coax/build.sh`, which runs the compiler's own
mutation gates, had not finished when this was written, so the final
compiler and the examples have not been run on Linux yet.

## Status (2026-10-31)

- **Step 3, the engine as the session: done.** All 16 realms run natively
  on the 4080 over Vulkan, with shaders written in Coax, and travel works
  between them (docs/verse/native-gpu.md; release 2a94f5fab2). The owner's
  play-through is in NEEDS_OWNER.md, and the Chromium kiosk stays until it
  passes.
- **Step 1: not started.** It waits on decision 1 below. `coderos-hosts`
  still imports `openagents?dir=os` at 2ca255550e.
- **Step 2 row 1, CPU limits: written, not installed.**
  `os/cpu-limits.coax` (the tool) and `os/cpu-limits-policy.coax`
  (`#authority`, 0 mutants survive) replace the shell in
  `coderos-cpu-limits`. A `--dry-run` on coderos-4080 finds the same zone
  (intel-rapl:0, both constraints) and the same values (125 W, 88 percent)
  as the unit's last run, and prints the same lines. A fake sysfs check
  shows it leaves subzones and intel-rapl-mmio alone, as the shell does.
  Installing it means pointing the unit's ExecStart at a 20-line
  derivation that runs `coax build`, which is a NixOS change that needs
  the owner's yes.
- **Step 2 row 3, the desk command: written, not installed.**
  `os/desk.coax` is `coder-desk` in Coax: the desk protocol over a Unix
  socket (`cxsock_connect_unix`, new in `coax/platform/socket-native.c`),
  every verb, and the Rust client's arguments, output and exit codes. On
  coderos-4080, `list`, `screens`, `focused`, `status` and `reading`
  against the live desk print the same bytes as the Rust client, and the
  requests for all 21 other verbs, sent to a stand-in socket, match it byte
  for byte. The Hyprland backend is not ported; with no desk socket it says
  so. Installing it in place of the Rust binary is part of step 1.
- **Step 2 row 4, recording: written, not installed.** `os/record-policy.coax`
  (`#authority`, 0 mutants survive) is how `screen-record` places a take's
  sound against its picture: the microphone-clock fit, the holes, the
  unplaced shortfall, the clock note and each piece's delay.
  `os/record-place.coax` reads the same ledger and clock files and prints
  what the shell's awk printed. On 3,000 random ledgers and clock logs it
  matches the shell's own functions byte for byte. `record-place mux`
  builds the mux's ffmpeg command, and on 1,000 random takes it matches
  `mux_recording`'s argument for argument. `os/microphone.coax` picks the
  microphone as `pick_microphone` does: the take's node, then the host's
  pattern, then the default. It matches the shell's jq on 1,500 random
  PipeWire graphs. The pattern is a regular expression, so it needed
  `coax/lib/text/regex.coax`, a small search that refuses anything outside
  its subset (`#authority`, 0 mutants survive, 2,780 pinned cases, and
  4,000 random cases that agree with Python's `re`).
  `os/record-audio.coax` is the microphone's supervisor. It opens a segment
  per capture with its offset in the ledger, watches the file and the node,
  cuts a capture that stalls or loses its node, finds the microphone again,
  and keeps the outage marker and the HUD's mic report. On coderos-4080 it
  ran against stand-ins for `pw-record`, `pw-dump` and `wpctl`, never the
  real devices: a clean take, a stalled file, a recorder that exits, a node
  that disappears and comes back, and no microphone at first. In all five
  it wrote the same segments, arguments, warnings, reports and markers as
  the shell's `supervise_audio`, with offsets within 0.4 s of each other.
  `os/screen-record.coax` is the command. Its `status` and `repair` print
  the shell's lines and leave the same files: on coderos-4080, seven status
  states (none, a stale pid, a live take with and without a mic report, an
  outage and reattachments, a broken or empty report) and two repair sets
  (nothing to do; a silent take, one that has sound, an empty capture, a
  broken video, a hidden file, a take with no capture) matched exactly.
  `stop` (`os/record-stop.coax`, with the HUD meter sync check in
  `os/record-sync.coax`) ends both captures, places and muxes the sound,
  checks it by the HUD meter, and tags the file. Twelve prepared takes on
  coderos-4080 (none, a stale pid, one piece, a hole, no sound, a slow
  microphone clock, a sync check by a given crop and by the desk's HUD
  window, a region take, a camera shift, no ledger, a proven end) gave the
  same output, files, streams and tags as the shell. The pieces of the
  sync check matched awk and jq on 500 trends, 3,000 levels and 2,000
  crops. Two changes: a sync check with too few windows gives its real
  counts (the shell always said "0 of 0"), and the placement limits are
  policy constants rather than environment overrides.
  `start` (`os/record-start.coax`) completes the command: it checks the
  microphone, starts `wf-recorder`, and starts the supervisor and the
  `--duration` timer as detached runs of itself (the process face gained
  `proc_detach`, `cxproc_self_path` and `cxproc_local_stamp`). On
  coderos-4080, twelve start-to-stop scenarios matched the shell: usage,
  bad flags, the microphone report, a plain take, audio with sync and fps,
  a silent or missing microphone, the fallback microphone, system audio, a
  region from slurp with presentation mode, `--duration`, and a second
  start. Every outside program in those runs was a stand-in, never the
  screen, the microphone or the desk. `os/screen-record.coax` now does
  everything `ops/screen-record.sh` did, and is not yet installed (owner).
  `os/presentation-mode.coax` replaces `presentation-mode`. Against a
  stand-in desk (a script that keeps screens, windows and focus and logs
  every call) it gave the shell's output, exit codes, state file and desk
  calls in twelve scenarios: on, status, off, toggle, a camera that has to
  be opened or is never there, one that will not stay put, a screen that
  goes away, a scale that will not change, no desk, a given socket, and
  bad options.
  `os/dictate-toggle.coax` replaces `dictate-toggle`: the recorder, the
  tone and the watcher that says when the microphone closed by itself.
  With stand-ins for ffmpeg, pw-play, notify-send, whisper-cli, wtype and
  curl, fourteen scenarios gave the shell's output, warnings and calls,
  except that a failed whisper-cli is now reported as a failure (the
  shell's pipeline reported it as "no speech").
  `os/recording-hud.coax` replaces `recording-hud`, the strip under the
  camera view that the sync check reads. Its frames matched the shell's
  draw byte for byte in 300 generated states. With stand-ins for the desk,
  `screen-record`, `pw-record` and `foot`, it made the same desk calls,
  recorder calls, foot command and messages for start (with and without a
  camera, already up, low on the screen), run (keys, mouse, arrows,
  recording on and off, the camera going away), status and stop. Two
  differences: it stops the meter's `pw-record` when it ends (the shell
  killed only the last process of its pipeline and left the microphone
  open), and it reads the microphone report field by field (the shell's
  tab-split read merged empty fields). On a pseudo-terminal it took the
  window's size and put echo and line mode back on exit. The platform
  gained `cxterm_*` (raw mode, size, key reads, signals), `cxproc_exec`,
  `cxproc_read_bytes` and `cxproc_cmdline` for it.
  Row 4 is written in full; installing it is the owner's yes in
  NEEDS_OWNER.md (the screen recorder option), and the four programs go in
  together. On 2026-10-31 the 4080's PipeWire graph had no microphone at
  all, so the Blue microphone was not delivering then. Checking them means recording on the 4080, which
  needs an agreed time with the owner.
- **Step 2 row 5, the camera daemon: written, not installed.** `os/camera.coax` is the
  command side of `coderos-camera`: every verb but `serve`, as one JSON
  line on the daemon's control socket. On coderos-4080 it matched the Rust
  command's output, `--json` output, exit status and requests in 33 cases
  against a stand-in daemon (every reply type, refusals, answers serde
  would refuse, a daemon that closes or never answers, bad words) and on
  `status` and `outputs` against the live daemon (read-only verbs only;
  the camera itself was not opened).

  `serve` is `os/camera-serve.coax`, with its effects in
  `coax/platform/camera-native.c` (V4L2 capture, the v4l2loopback node,
  Unix sockets). It is one poll loop where the Rust daemon had a thread
  per output. Each frame is decoded once with `coax/lib/image/jpeg.coax`,
  and an MJPEG frame that has no Huffman tables gets the standard ones.
  The recording keeps Rust's one-frame mailbox and its drop count. On
  coderos-4080, run next to the Rust daemon on a camera path that does not
  exist, the two matched in every CLI answer: `status`, `outputs`, `record
  start` twice, `record stop` with and without a recording, `hands off`,
  `stop`, in text and `--json`. The stderr logs were identical too,
  including ffmpeg's failure to tag an empty recording. One difference:
  the recording's length, 1.2 s against 1.3 s, a wall-clock difference. A
  camera path that names a regular file is read as concatenated JPEG
  frames, and with ffmpeg test-pattern frames, with and without Huffman
  tables, the daemon held 15.0 fps. It wrote a 45-frame H.264 recording
  with the right receipt and tag, and counted a hands reader. At 1280x720
  and 30 fps it kept up using 79% of one core and 142 MB.

  The hand tracker is `os/hands.coax`. The pinned MediaPipe landmark model
  runs on `coax/lib/nn/onnx.coax`, a protobuf reader and f32 interpreter
  for the nine ops the model uses, in place of onnxruntime. The tracker
  runs as the daemon's child (`coderos-camera hands-worker`) fed one
  224x224 frame at a time, so a slow frame holds the tracker and not the
  camera, as its thread did. On coderos-4080:
  - Every output float matched onnxruntime to within 2.3e-4 relative,
    over four test frames.
  - On a hand from OpenCV Zoo's demo (presence 0.98), the landmarks
    matched to within 6.6e-7. Every number on the line was the shortest
    that reads back as its float, and the pose matched a port of
    `recognize` ("open hand").
  - A frame takes about 100 ms on one core. Tracking a file source of that
    hand at 15 fps, it published 9 lines a second with the drops counted,
    while a recording at the same time kept all 75 frames.
  - The model file is checked by SHA-256 in Coax and fetched with curl
    when missing, as before.

  Differences from Rust:
  - Stopping a recording holds the loop for the second ffmpeg takes to
    finish.
  - A request that does not parse is refused in our own words, not
    serde's.

  Left:
  - The real camera and loopback, which wait for an agreed time because
    the running daemon holds `/dev/video0` and `/dev/video10` is the
    owner's.
  - Installing it in place of the Rust daemon (an owner yes).

  On 2026-10-31 two `coderos-camera serve` processes were running there,
  and the one that answers reports that another process streams
  `/dev/video0`.
- **Step 2 row 6, the compositor: started with its window policy.**
  `os/wm.coax` is `coder-wm`: dwindle tiling as the Hyprland session lays
  it out, nine desks over several screens, floats, fullscreen, monocle and
  pinned windows. Rust's boxed tree becomes a pool of nodes changed in
  place. `os/wm-check.coax` runs a file of window commands through it and
  prints the whole state after each one: every window's desk and
  rectangle as f32 bits, focus, the tiles each screen draws, and which
  screen shows which desk. A Rust driver linking the original crate prints
  the same. Over 1,300 random sessions (390,565 commands) the two matched
  line for line, bit for bit.

  In 18 of those sessions Rust panicked. Super+right-drag on a float
  placed near the right or bottom edge calls `f32::clamp` with its bounds
  reversed, which would take the compositor down. The Coax version takes
  the upper bound, and the sessions were compared up to the panic.

  The Wayland server is also Coax now: `os/compositor.coax`, with only
  sockets, descriptors and shared memory in
  `coax/platform/wayland-native.c`, and no wlroots or Smithay. It speaks
  wl_compositor, wl_subcompositor, wl_shm, wl_seat, wl_output,
  wl_data_device_manager and xdg-shell, and places every toplevel with
  `os/wm.coax`. Clients get the US keymap from `os/compositor-us.xkb`,
  compiled once with `xkbcli`. This first step is headless and runs on its
  own socket: it reads no input device, draws to no display, and doesn't
  touch the running session. A test channel on stdin moves the pointer,
  types, runs policy verbs and writes the composited screen as a PPM.

  On coderos-4080, two unmodified `foot` terminals connected. They tiled
  at 640x720 each, with their own title bars drawn as subsurfaces. Text
  typed through the channel ran in the focused shell (`echo hello from
  coax`, `ls /`), and the unfocused window showed as inactive. Focus moved
  with `movefocus`. `close` closed the first terminal, and the second took
  the full 1280x720.

  The CoderOS key bindings also run in it now: the Super chords from the
  Hyprland configuration, which follow Omarchy's tiling keys, as one pure
  table (`cx_bind`) ahead of the focused window. In the headless check,
  Super+Return three times opened three terminals through the launch
  directory. They tiled the way dwindle tiles: a left half, then a right
  half split top and bottom. Super+Left moved the focus, and Super+Shift+2
  took the window to desk 2 and followed it there. Super+F made it
  fullscreen. Super+1 then Super+W closed desk 1's focused window, and the
  last one filled the screen.

  Super+W decides the way `os/bin/coder-close` did, now in Coax. It
  collects the window's process (SO_PEERCRED) and every process under it,
  then asks `coder activity --pid ...` without blocking the compositor. A
  window with work in flight closes only on a second press within 5
  seconds, and the first press says what is running. A window it can't
  ask about closes on the first press. With a stand-in `coder` first on
  PATH, a busy terminal warned and then closed on the second press, an
  idle one closed on the first, and each question named the terminal's
  process and its shell. The notice goes to `notify-send` with
  `coder-close`'s arguments, and to the compositor's output.

  The compositor's two faces to the machine are written, after
  `verse/kms`: `coax/platform/screen-native.c` takes one screen through
  KMS dumb buffers with page flips, and grabs keyboards and mice through
  evdev. `--screen` uses both, draws a pointer arrow, and puts the screen
  back on Ctrl+Alt+Backspace. `--input FILE` plays recorded evdev records
  through the same path on their own clock. Played headless from such a
  file, two Super+Return presses opened two terminals. Moving the mouse
  500 px left moved the focus, the typed `echo mouse` ran in that
  terminal, a click reached it, Super+Shift+Right swapped the two, and
  Ctrl+Alt+Backspace quit cleanly.

  The KMS output has not run on a screen yet. It needs a free VT, and the
  owner's session is on this machine, so it waits for the owner
  (NEEDS_OWNER.md, "Try the Coax compositor").

  The clipboard works. There is one selection, offered to the client
  with the keyboard, and a paste hands the reader's pipe to the copying
  client: `wl-paste` read what `wl-copy` offered, with all five of its
  types, and saw it cleared.

  Windows look the way the Hyprland session drew them. Under
  xdg-decoration the compositor draws the frames, so clients drop their
  own title bars. It uses the session's `general` settings: 6 px gaps at
  the screen edge, 3 px between windows, and a 1 px border, white on the
  focused window and #4a4a4a on the rest. A fullscreen window has no gap
  and no border.

  It composites only when something changed, but then it composites the
  whole screen. With three terminals open, a frame takes 1.8 ms at
  1280x720, 6.4 ms at 2560x1440 (the 4080's screen) and 13.9 ms at 4K, on
  one core of coderos-4080. That was 2.5, about 9 and 20.8 ms before the
  blit worked out each row's visible span once and checked a pool's size
  once per surface. The screenshot came out byte-identical.

  The window rules and mouse bindings are in too. A window titled
  `selfie` (the camera view) or `recording-hud` floats with no border and
  is pinned, whether it names itself before or after it maps. Super+C
  starts the camera program, and Super+Shift+E exits. Super + left drag
  moves a float, and Super + right drag resizes it from the nearest
  corner. Headless, a `selfie` window floated borderless, moved by the
  drag (-300, -150), and followed to desk 2. A Super + right drag on it
  keeps its shape (keep_aspect_ratio): a 160 px wider drag gave 480x480.
  Left: the floats' first size (320 px when the window hasn't drawn yet),
  and host rules such as the Android emulator's class.

  The session's start list (Hyprland's `exec-once` rows) is
  `LAUNCH-DIR/start`, run once the socket is up. A start program that
  opened a terminal gave a focused window at login, and a launch
  directory without one starts nothing.

  A client's own pointer image (`wl_pointer.set_cursor`) replaces the
  arrow while the pointer is over that client: over a terminal, foot's
  I-beam was drawn at its hotspot.

  Also left: damage tracking (redrawing only what changed), and the protocols foot still reports missing
  (primary selection, fractional scale, activation, text input).
- **Step 2 row 2: not started.** It waits on a Coax `coder` to update.
- **Rows 4 to 6 and the compiler.** `#foreign` already passes views of
  integers and floats (a pointer and a length), so a C face can fill a Coax
  buffer, and opaque handles are integers. Capture (row 4) and V4L2
  (row 5) need no header import. The compositor (row 6) still needs
  callbacks and struct access, and that is the compiler work in §17.2.

## Decisions for the owner

1. **Approve step 1:** copy the CoderOS modules into the private
   `coderos-hosts` flake and pin the Rust binaries to `2ca255550e`. This
   touches only the private repository and the host's next switch.
2. **Turn off `coder-update`** on `coderos-4080` until a Coax `coder`
   exists to update.
3. **Pick the first Coax program for CoderOS.** I recommend the CPU limits
   (row 1): it is small, its policy is exactly what `#authority` and `coax
   mutate` are for, and it runs every five minutes, so it proves itself
   quickly.
4. **Decide whether the `sched_ext` scheduler is worth a spike.** It needs a
   BPF emitter in Coax, about the size of the Metal one.
