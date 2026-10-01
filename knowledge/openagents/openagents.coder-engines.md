---
id: openagents.coder-engines
version: 1
kind: product
title: "How Coder runs on your computer"
summary: >-
  Coder runs on your computer through the model providers connected there,
  with failover by capacity; the computer's owner sets the engine and model.
tags: [coder, engine, model, provider, capacity]
applies_when: >-
  The user asks which coding agent, engine, or model Coder uses on their
  computer, whether they can pick the model from the phone, or what happens
  when a provider runs out.
answer: >-
  Coder runs on your computer, with the access you granted that computer. Its
  host sends each task to a connected model provider that has capacity, and
  fails over to another when one runs out. A task can also hand work to
  OpenCode or Devin, and that session shows inside its chat. The computer's
  owner sets the engines and models on the host. When you ask for one by
  name, such as "run this with Claude Code", Coder asks your computer to start
  with it if the owner allows it there, and says why when it can't.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - INVARIANTS.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-30: an engine named in chat now reaches the computer from the phone too (#10081); checked against INVARIANTS.md."
  - "2026-10-01: the engine that runs is told the request for it is done and works on the task itself (#10084); checked against INVARIANTS.md."
---

## Answer

Coder runs on your computer, with the access you granted that computer. Its host sends each task to a connected model provider that has capacity, and fails over to another when one runs out. A task can also hand work to OpenCode or Devin, and that session shows inside its chat. The computer's owner sets the engines and models on the host. When you ask for one by name, such as "run this with Claude Code", Coder asks your computer to start with it if the owner allows it there, and says why when it can't.

## Details

- A device sends a workspace label, a title, a prompt, and at most the engine you asked for. That engine only goes first among the ones the owner already allows; a device can't add an engine or choose the model or limits.
- The engine that runs is told your request for it is already done, so it works on your task itself; it never starts another engine's app to do it. A message that only asks for a test delegation gets a small, harmless look at the project.
- There's no model picker in the app yet.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
