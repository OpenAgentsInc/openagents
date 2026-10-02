---
id: openagents.coder-engines
version: 3
kind: product
title: "How Coder runs on your computer"
summary: >-
  Coder uses every coding agent signed in on your computer, Codex first, with
  failover; nothing needs enabling, and settings only turn one off or set the order.
tags: [coder, engine, model, provider, capacity, settings, claude-code, codex]
applies_when: >-
  The user asks which coding agent, engine, or model Coder uses on their
  computer, how to make Coder use or prefer Claude Code, Codex, or another
  agent by default, whether they can pick the model from the phone, what
  happens when a provider runs out, whether a switched login needs a
  restart, or why an agent such as Devin doesn't show or isn't used.
answer: >-
  Coder uses every coding agent signed in on your computer (Codex, Claude
  Code, Grok Build, Devin, OpenCode) with nothing to enable, Codex first,
  moving on when one isn't signed in or is out of capacity. If an agent
  doesn't show, sign in to it on that computer (for Devin, `devin auth
  login`). To prefer one, run `openagents settings set coder.providers
  claude,codex`; agents left out still follow. To stop using one, turn it
  off in Coder's settings or run `openagents settings disable devin`. For
  one task, say "run this with Claude Code".
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - INVARIANTS.md
    - bins/openagents-ios/README.md
    - crates/openagents-cli/src/settings.rs
    - crates/openagents-desktop/src/settings.rs
    - crates/coder/src/task/local.rs
    - crates/coder/src/task/settings.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-30: an engine named in chat now reaches the computer from the phone too (#10081); checked against INVARIANTS.md."
  - "2026-10-01: the engine that runs is told the request for it is done and works on the task itself (#10084); checked against INVARIANTS.md."
  - "2026-10-02: names the coder.providers setting, the desktop's agent order, and the login read at each start (#10140, #10105); checked against the cited settings and Coder sources."
  - "2026-10-02: agents are opt-out (#10184): every signed-in agent is used with nothing to enable; coder.disabled turns one off and coder.providers is only an order; checked against crates/coder/src/task/settings.rs."
---

## Answer

Coder uses every coding agent signed in on your computer (Codex, Claude Code, Grok Build, Devin, OpenCode) with nothing to enable, Codex first, moving on when one isn't signed in or is out of capacity. If an agent doesn't show, sign in to it on that computer (for Devin, `devin auth login`). To prefer one, run `openagents settings set coder.providers claude,codex`; agents left out still follow. To stop using one, turn it off in Coder's settings or run `openagents settings disable devin`. For one task, say "run this with Claude Code".

## Details

- A task can also hand work to OpenCode or Devin, and that session shows inside its chat.
- A named engine, such as "run this with Claude Code", goes first for that task unless it was turned off on that computer; Coder says why when it can't.
- A device sends a workspace label, a title, a prompt, and at most the engine you asked for. That engine only goes first among the ones the computer can use; a device can't add an engine or choose the model or limits.
- The engine that runs is told your request for it is already done, so it works on your task itself; it never starts another engine's app to do it. A message that only asks for a test delegation gets a small, harmless look at the project.
- No one needs to edit a setting to use an agent they've signed in to. `coder.disabled` lists the agents turned off (`openagents settings disable AGENT`, `openagents settings enable AGENT`); `coder.providers` only sets the order, each optionally `NAME:MODEL`, and agents it leaves out follow in the default order Codex, Claude Code, Grok Build, Devin, OpenCode. They live in `~/.openagents/settings.json`, which `openagents settings`, `openagents chat`, the desktop app, the terminal's `/settings`, and a host on that computer all read.
- OpenCode runs on the model its own configuration names (its `model` setting), or the one named as `opencode:PROVIDER/MODEL`.
- In the desktop app and the terminal's `/settings`, every agent is on until you turn it off; at least one stays on.
- Each run uses the login signed in at its start: before a start, Coder reads the usage of the login signed in now, so a switched Claude Code account needs no restart.
- There's no model picker in the app yet.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
- `crates/openagents-cli/src/settings.rs`
- `crates/openagents-desktop/src/settings.rs`
- `crates/coder/src/task/local.rs`
