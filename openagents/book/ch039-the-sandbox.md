# 39. The sandbox

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A plugin is someone else's code. This chapter gives it a place to run where it
can do only what it was admitted to do: a Wasm sandbox with bounded time and
memory and a short list of host functions.

## Draft

You can't take rights away from code you don't control. You can build a room
with rules you enforce, and let the code in under those rules. The
test-time-capabilities essay calls the room an arena. Here it is a Wasm
sandbox.

A plugin compiles to a Wasm module. The module can compute, and it can call
the host functions the host chose to give it. It can do nothing else. It has
no file system, no network and no clock unless the host hands one over. The
host also bounds how long the module runs and how much memory it uses, and a
module that goes over stops with a reason.

We chose Wasm in [Episode 48](../../docs/transcripts/048.md) and loaded the
first plugins in [Episode 53](../../docs/transcripts/053.md). [Episode
71](../../docs/transcripts/071.md) adds host functions. The audit recommends
the same choice now. A plugin written in any language that compiles to Wasm
will run. Coax compiles to Wasm, so a plugin can also be written in the
language the rest of the system uses.

The other option is a native process inside an operating-system sandbox. It
runs existing tools without a port, but its rules differ on every platform,
and the apps run on seven platforms.

The sandbox is also the frame for measurement. A claim about a plugin (chapter 43)
is a claim about that plugin under a stated grant. If the sandbox didn't hold,
the claim would describe nothing.

## What we build

A plan for what this chapter ships once the draft is approved:

- A Wasm host for plugins: admitted host functions only, with time and memory
  bounds.
- A plugin kit so a plugin can be written in Coax and compiled to Wasm.
- Escape tests: a module that asks for more than its grant is refused, and the
  refusal is logged.

It builds on what exists today:

- [`coax/web`](../../coax/web)
- [`coder/tui/plugins.coax`](../../coder/tui/plugins.coax)
- [`coax/lib/bounded.coax`](../../coax/lib/bounded.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A call into a plugin and a call back to the host | `openagents.program.v1` `PacketRequest`, `PacketResponse`, `HostCall` ([spec 20.1](../../docs/protocol/openagents-protocol-spec.md#201-openagentsprogramv1)) | Binary, across the sandbox boundary | Spec sketch; no `.proto` file |

Planned paths: `openagents/plugins/sandbox/`.

## Sources

- **Episodes:**
  - [Episode 48, *Brainstorming a Plugin System*](../../docs/transcripts/048.md): choosing Wasm
  - [Episode 53, *Loading WASM Plugins*](../../docs/transcripts/053.md)
  - [Episode 71, *PHP Host Functions*](../../docs/transcripts/071.md): host functions
  - [Episode 72, *LLM Inference Plugin*](../../docs/transcripts/072.md): an inference plugin
  - [Episode 250, *Ready the Fleet*](../../docs/transcripts/250.md): typed capabilities
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `plugin-pdk`, `plugin-hook-guard`
- **NIPs:** [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability admission": the arena
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5

## Open questions for the owner

1. Wasm with grants, as the audit recommends, or native processes with an
   operating-system sandbox? This is the audit's decision 2.
2. Which Wasm runtime runs plugins on native hosts: one we write in Coax, or
   an existing one we link?
