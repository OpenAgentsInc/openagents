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
`crates/jev`. There is no inference engine in Coax in this repository yet;
the Coax engine that exists is Metareal (`metareal/`, about 7,700 lines).

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
| 4 | Recording and presentation | `screen-record` (1,523 lines), `recording-hud`, `presentation-mode`, `dictate-toggle` | Coax programs. Dictation shares the voice engine in `metareal/voice.coax` (VAD, end of turn) | PipeWire through C; header import |
| 5 | Camera daemon | `coderos-camera` (Rust, V4L2, ONNX Runtime) | Coax capture over V4L2; the hand model as a Coax kernel | Pointer FFI; an inference kernel in Coax |
| 6 | Compositor | `coder-compositor` + `coder-wm` (Rust, Smithay, about 15,500 lines) | A Coax compositor over wlroots, or Coax window policy inside an existing wlroots compositor first | Header import (§17.2), callbacks, raw pointers into C structs |

Rows 1 to 3 are within reach of today's bootstrap plus a thin C face, the
same pattern as `metareal/platform/host.c`. Rows 4 to 6 wait on the C
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

The kernel, the drivers' userspace interfaces, the desktop stack, Metareal's
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

Metareal draws through Metal on Apple hardware. On Linux it could own the
display directly through DRM/KMS and GBM, as a session with no compositor:
boot, autologin, the engine on the screen, input from libinput. CoderOS
already has the shape for this; `desktop.nix` chooses the session program
per host. A kiosk-style Metareal session is a smaller first step than a
Coax compositor, and it is also the path to a compositor: a compositor is an
engine that draws other programs' buffers as textures.

### Engine and inference in one process

The Metal emitter already turns Coax f32 procedures into shader code. The
same idea pointed at CUDA C (or SPIR-V for Vulkan) gives one kernel source
for the Mac, the iPhone and the RTX 4080. With the engine and an inference
engine both in Coax and both linking the same C runtime, they can share one
process and one GPU allocation: a model's output written into a buffer the
renderer reads, with no socket, no copy and no second scheduler. The voice
engine is already in Coax; hand tracking (row 5 above) is the first model
small enough to try. This needs an inference engine in Coax, which does not
exist yet; `crates/psionic` was Rust and went with the Nuke.

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
