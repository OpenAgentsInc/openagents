# 62. A Windows host

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Six of the seven platforms Episode 289 named have a host. This chapter builds
the seventh: a Windows host for the engine and the platform layer.

## Draft

Episode 289 lists them: Windows, Mac and Linux, iOS and Android, web and
terminal. The audit checks each against today's tree and finds one with
nothing behind it. There is no Windows host.

A host is the layer between Coax code and a platform. It opens a window, reads
input, plays sound, talks to the graphics card, opens sockets and reads files.
The OpenAgents Metaverse Engine defines what a host must provide, and has conformance checks that a
host must pass. The tree has hosts for the browser, for Apple's platforms and
for Linux.

A Windows host has four pieces. The window and input come from the platform's
own interfaces. Graphics is a choice between the platform's own graphics
interface and Vulkan, which the Linux host already uses. Sound, sockets and
files are smaller. And the build has to produce a Windows program from Coax's
C output.

Windows matters more than its place at the end of the book suggests. Part
VIII's sellers are people with gaming computers, and most gaming computers run
Windows. A Pylon that does not run there misses the audience Episode 290
named. So the Pylon and the terminal are the first things to run on Windows,
before the full app.

Because a host is a contract with checks, this is a bounded job. The chapter
is done when the Windows host passes the same conformance run as the others
and the same realm draws the same frame.

## What we build

A plan for what this chapter ships once the draft is approved:

- A Windows host for the Metaverse Engine and the platform layer: window, input, graphics,
  sound, sockets and files.
- The Pylon and the command line running on Windows first.
- The host conformance run passing on Windows, with a frame that matches the
  other hosts.

It builds on what exists today:

- [`metaverse-engine/host/README.md`](../../metaverse-engine/host/README.md)
- [`metaverse-engine/host/conformance.coax`](../../metaverse-engine/host/conformance.coax)
- [`metaverse-engine/host/build-vulkan.coax`](../../metaverse-engine/host/build-vulkan.coax)
- [`coax/platform`](../../coax/platform)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`psionic/lib/pylon`](../../psionic/lib/pylon)

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

1. Which graphics interface does the Windows host use: the platform's own, or
   Vulkan as on Linux? This is the audit's decision 5.
2. Do we need a Windows machine in the build fleet, or do we build from
   another platform and test on a rented one?
