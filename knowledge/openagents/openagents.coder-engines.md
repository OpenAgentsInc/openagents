---
id: openagents.coder-engines
version: 2
kind: product
title: "How Coder runs on your computer"
summary: >-
  Coder runs on your computer through the coding agents its settings allow,
  first preferred, with failover; the coder.providers setting sets the order.
tags: [coder, engine, model, provider, capacity, settings, claude-code, codex]
applies_when: >-
  The user asks which coding agent, engine, or model Coder uses on their
  computer, how to make Coder use or prefer Claude Code, Codex, or another
  agent by default, whether they can pick the model from the phone, what
  happens when a provider runs out, or whether a switched login needs a
  restart.
answer: >-
  Coder tries the coding agents your computer's settings allow, first
  preferred, and moves to the next when one isn't signed in or is out of
  capacity. To prefer Claude Code over Codex, put it first in the
  `coder.providers` setting: run `openagents settings set coder.providers
  claude,codex,grok`, or on the desktop app's Coder settings turn Codex off
  and on again, which puts it last. To use it for one task, say "run this
  with Claude Code". Each run uses the Claude Code login signed in at its
  start, so a switched account needs no restart.
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
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-30: an engine named in chat now reaches the computer from the phone too (#10081); checked against INVARIANTS.md."
  - "2026-10-01: the engine that runs is told the request for it is done and works on the task itself (#10084); checked against INVARIANTS.md."
  - "2026-10-02: names the coder.providers setting, the desktop's agent order, and the login read at each start (#10140, #10105); checked against the cited settings and Coder sources."
---

## Answer

Coder tries the coding agents your computer's settings allow, first preferred, and moves to the next when one isn't signed in or is out of capacity. To prefer Claude Code over Codex, put it first in the `coder.providers` setting: run `openagents settings set coder.providers claude,codex,grok`, or on the desktop app's Coder settings turn Codex off and on again, which puts it last. To use it for one task, say "run this with Claude Code". Each run uses the Claude Code login signed in at its start, so a switched account needs no restart.

## Details

- A task can also hand work to OpenCode or Devin, and that session shows inside its chat.
- A named engine, such as "run this with Claude Code", goes first for that task when the settings allow it on that computer; Coder says why when it can't.
- A device sends a workspace label, a title, a prompt, and at most the engine you asked for. That engine only goes first among the ones the owner already allows; a device can't add an engine or choose the model or limits.
- The engine that runs is told your request for it is already done, so it works on your task itself; it never starts another engine's app to do it. A message that only asks for a test delegation gets a small, harmless look at the project.
- `coder.providers` lists the coding agents Coder may use, first preferred: `codex`, `claude`, `grok`, `opencode:PROVIDER/MODEL`, or `devin`, comma-separated, each optionally `NAME:MODEL`. The default is `codex,claude,grok`. It lives in `~/.openagents/settings.json`, which `openagents settings`, `openagents chat`, the desktop app, and a host on that computer all read.
- In the desktop app, Coder's settings list the agents Coder may run. Coder tries the ones that are on from the top; one turned on goes last, and at least one stays on.
- Before a start, Coder reads the usage of the login signed in now, so it follows a switched Claude Code account on the next run.
- There's no model picker in the app yet.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
- `crates/openagents-cli/src/settings.rs`
- `crates/openagents-desktop/src/settings.rs`
- `crates/coder/src/task/local.rs`
