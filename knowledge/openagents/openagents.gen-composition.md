---
id: openagents.gen-composition
version: 1
kind: product
title: "What the composition is made of in OpenAgents 1.0.0"
summary: >-
  OpenAgents is the general agent people talk to; Coder is its first
  specialist member; its layers are front, answers, conversation, specialist
  agent, engines, plugins, evaluation, and credit.
tags: [essay, general-agent, composition, layers, coder, plugin]
applies_when: >-
  The user asks what our general agent is made of, or what its layers are.
answer: >-
  In OpenAgents 1.0.0 the composition is: the front (the chat router, Jev
  decisions over NIP-DEC carried by NIP-CJ), answers (reviewed answers and
  NIP-KB knowledge entries), conversation (a hosted chat model), a specialist
  agent (Coder, on the person's own computer), engines (Codex, Claude Code,
  Grok Build, OpenCode, and Devin, chosen by policy, capacity, and the
  person's request), plugins, evaluation (the Gym), and credit (NIP-XP
  awards).
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/essays/2026-10-01-the-return-of-the-general-agent.md
evidence:
  - "2026-10-01: written from the essay The Return of the General Agent and checked against its text (#10099); the answer text awaits the owner's copy review."
---

## Answer

In OpenAgents 1.0.0 the composition is: the front (the chat router, Jev decisions over NIP-DEC carried by NIP-CJ), answers (reviewed answers and NIP-KB knowledge entries), conversation (a hosted chat model), a specialist agent (Coder, on the person's own computer), engines (Codex, Claude Code, Grok Build, OpenCode, and Devin, chosen by policy, capacity, and the person's request), plugins, evaluation (the Gym), and credit (NIP-XP awards).

## Details

- OpenAgents is the general agent people talk to, on a phone, a desktop, or a terminal; Coder is its first specialist member, and the other members are the agents Coder delegates to and the plugins people add.
- A plugin can contain skills, workflows, knowledge, Wasm, and tests; plugins ship as NIP-EXT releases, NIP-PRG programs, Wasm guests, and NIP-KB entries.
- It is an agent of agents in a literal sense: one request can pass through four layers of agency, each chosen by a decision someone can inspect.
- Where it comes from: the essay The Return of the General Agent, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md#what-the-composition-is-made-of`.

## Sources

- `docs/essays/2026-10-01-the-return-of-the-general-agent.md`
