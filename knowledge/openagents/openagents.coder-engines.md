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
  owner sets the engine and model on the host; the phone doesn't choose them.
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
---

## Answer

Coder runs on your computer, with the access you granted that computer. Its host sends each task to a connected model provider that has capacity, and fails over to another when one runs out. A task can also hand work to OpenCode or Devin, and that session shows inside its chat. The computer's owner sets the engine and model on the host; the phone doesn't choose them.

## Details

- A device sends only a workspace label, a title, and a prompt, and can't choose the engine, model, or limits.
- There's no model picker in the app yet.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
