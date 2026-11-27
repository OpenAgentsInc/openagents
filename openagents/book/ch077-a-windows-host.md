# 77. A Windows host

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Six of the seven platforms Episode 289 named have a host. This chapter builds
the seventh: a Windows host for the engine and the platform layer. It is an
additional platform. The web and phone apps ship without it (chapter 76), and
Windows is qualified on its own evidence.

## Draft

Episode 289 lists them: Windows, Mac and Linux, iOS and Android, web and
terminal. When the initial port audit ran, Windows had nothing behind it.
Since then, **the Win32 + Vulkan OpenAgents Desktop host has shipped** in
[`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows)
(`#14046`, with WebView2 integration in `browser_webview2.c`, cross-build
tooling in `build.sh`, Wine smoke harnesses `wine-smoke.sh` and
`wine-window.sh`, and Linux/Windows `Inkling-2-Mini` CUDA serving and variant
selection on RTX 4080-class GPUs). What remains open for this chapter is the
full **Metaverse Engine** Windows host (`metaverse-engine/host/windows/`)
passing [`conformance.coax`](../../metaverse-engine/host/conformance.coax)
with pixel-for-pixel frame parity against Linux Vulkan.

A host is the layer between Coax code and a platform. It opens a window, reads
input, plays sound, talks to the graphics card, opens sockets and reads files.
The OpenAgents Metaverse Engine defines what a host must provide, and has conformance checks that a
host must pass. The tree has hosts for the browser, for Apple's platforms, for
Linux Vulkan, and the Win32 + Vulkan desktop carrier in
[`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows).

The graphics decision (audit decision 5) is settled: **Vulkan** is the shared
desktop graphics interface across both Linux and Windows, avoiding a separate
Direct3D backend. Windowing and input use native Win32, embedded browser panes
use WebView2, and Coax compiles cleanly to Windows executables via C output.

Windows matters more than its place at the end of the book suggests. Chapter
70's sellers are people with gaming computers, and most gaming computers run
Windows. Psionic's CUDA serving backend (`psionic-inkling-serve`) and the
desktop Win32/Vulkan carrier already target NVIDIA Windows hardware; finishing
full Metaverse Engine host conformance completes the seventh platform.

Because a host is a contract with checks, this is a bounded job. The chapter
is done when the Windows host passes the same conformance run as the others
and the same realm draws the same frame.

## What we build

Shipped today:

- Win32 + Vulkan OpenAgents Desktop host ([`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows), `#14046`), including WebView2 pane support (`browser_webview2.c`), build scripts (`build.sh`), and Wine smoke checks (`wine-smoke.sh`, `wine-window.sh`).
- Windows/Linux NVIDIA CUDA serving support and `Inkling-2-Mini` variant catalog selection.

Remaining for full chapter completion:

- A Windows host for the Metaverse Engine (`metaverse-engine/host/windows/`): window, input, Vulkan graphics, sound, sockets, and files passing the engine conformance suite.
- Full Pylon and CLI release qualification on native Windows CI hardware.
- The host conformance run passing on Windows, with a frame that matches the
  other hosts.

It builds on what exists today:

- [`apps/openagents-desktop/windows/README.md`](../../apps/openagents-desktop/windows/README.md)
- [`apps/openagents-desktop/windows/main.c`](../../apps/openagents-desktop/windows/main.c)
- [`apps/openagents-desktop/windows/browser_webview2.c`](../../apps/openagents-desktop/windows/browser_webview2.c)
- [`metaverse-engine/host/README.md`](../../metaverse-engine/host/README.md)
- [`metaverse-engine/host/conformance.coax`](../../metaverse-engine/host/conformance.coax)
- [`metaverse-engine/host/build-vulkan.coax`](../../metaverse-engine/host/build-vulkan.coax)
- [`coax/platform`](../../coax/platform)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`psionic/lib/pylon`](../../psionic/lib/pylon)

### The chapter's check

- **Source:** `metaverse-engine/host/windows/`, run through
  [`conformance.coax`](../../metaverse-engine/host/conformance.coax), plus
  the Pylon and the command line on Windows.
- **Trace:** the conformance result: one line per check, the same lines the
  Linux host prints, and a hash of the test realm's frame equal to the other
  hosts' hash.
- **Refusal:** an operation the Windows host does not yet provide reports
  its check as refused, not passed, and the run fails.
- **Independent check:** the same conformance run on the Linux and browser
  hosts, and the Windows frame compared pixel by pixel with the Linux host's
  frame.

Planned paths: `metaverse-engine/host/windows/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, Windows, Mac and Linux
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 5:00, gaming computers
  - [Episode 283, *Coder for Gamers?*](../../docs/transcripts/283.md): gamers as an audience
  - [Episode 280, *CoderOS*](../../docs/transcripts/280.md): an operating system of our own
- **Old code at `coder-v1.0.0`, crates:** `rust-native`, `rust-native-desktop`, `openagents-desktop`, `terminal-app`
- **Also:**
  - The audit's gap table, row P16, and its decision 5
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Do we need a Windows machine in the build fleet, or do we build from
   another platform and test on a rented one?
