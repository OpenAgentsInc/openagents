---
id: openagents.gen-extensible
version: 1
kind: product
title: "Extensible by people and by their agents"
summary: >-
  A person extends the agent by writing a plugin; the same records can be
  written by agents, and Coder has landed changes to its own repository from a
  chat message.
tags: [essay, general-agent, extensible, plugin, agents, coder]
applies_when: >-
  The user asks how OpenAgents is extensible by people or by their agents.
answer: >-
  A person extends the agent by writing a plugin, which can contain skills,
  workflows, knowledge, Wasm, and tests, and ships as a signed release. From
  the app a person can draft a test set in chat, start a with-and-without run,
  and publish the result, with no account beyond a key and no one's
  permission. The same records can be written by agents, and the protocol does
  not care whether a signer is a person or an agent. Coder has used this on
  its own code, landing changes only when the repository's checks pass.
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

A person extends the agent by writing a plugin, which can contain skills, workflows, knowledge, Wasm, and tests, and ships as a signed release. From the app a person can draft a test set in chat, start a with-and-without run, and publish the result, with no account beyond a key and no one's permission. The same records can be written by agents, and the protocol does not care whether a signer is a person or an agent. Coder has used this on its own code, landing changes only when the repository's checks pass.

## Details

- Coder takes a GitHub issue from a chat message, posts a claim, works in its own worktree, runs the checks, fixes within a bound, and lands the change on main only when they pass. Its first runs landed 6d49408d6a, 57bf6e4778, and 619c7f203a, and on 2026-09-30 it made the slide deck an embeddable viewer in one turn.
- Two larger issues ran out of steps with correct partial work, which people finished; the flow now continues a turn that is still making progress.
- That is an agent extending the agent, through the same gate any contributor passes, at the scale of small changes.
- Where it comes from: the essay The Return of the General Agent, linked on GitHub at `https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md#by-people`.

## Sources

- `docs/essays/2026-10-01-the-return-of-the-general-agent.md`
