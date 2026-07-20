# AFS Apple FM ambient context block

Date: 2026-07-20
Status: implemented
Issue: owner-reported desktop fix

## Purpose

This document records the design of the ambient context block for the on-device
OpenAgents assistant. The assistant runs on the Apple on-device model (Apple FM).
The block gives the assistant the environment facts that the application already
holds. The assistant can then answer a question about the environment with the
real facts.

## Problem

The owner asked the local assistant "what do you know about me". The assistant
answered that it had no information about the user. This answer was wrong. The
application already knows the working directory, the operating system, the date,
and the sovereign identity public key. The prompt did not carry these facts, so
the assistant could not state them.

The prompt in `apps/openagents-desktop/src/turn/apple-fm-prompt.ts` held an
honesty preamble, a connected-agent list, and a delegation template. The prompt
held no environment facts.

## Two-layer memory split

The design has two memory layers. The two layers are different.

- The ambient context layer holds the facts of the current session. The facts
  are the date, the operating system, the application name, the working
  directory, the owner-device flag, and the public identity key. The host owns
  these facts. The host resolves the facts at each turn. This document ships the
  ambient context layer.
- The durable user-profile layer holds facts about the user across sessions. The
  assistant does not have this layer yet. The follow-up work adds this layer on
  the AFS-10 experience-memory substrate (`@openagentsinc/agent-experience-memory`).
  The AFS-10 substrate keys each record by the sovereign identity. The follow-up
  work is out of scope for this change.

The prompt tells the assistant about the split. The context block ends with one
honest line. The line states that the assistant does not remember personal facts
across sessions. The line also states that the assistant must not invent personal
facts. The assistant gives the real environment facts and stays honest about the
missing durable layer.

## Design

The change adds a typed context and renders a small block.

- A new type `AppleFmEnvironmentContext` holds the host-owned facts. Each field
  is optional. Each field is fail-soft. A missing field omits its line.
- A new helper `renderAppleFmEnvironmentContext` renders the block. The block
  starts with the line "Here is the context you have about the user and this
  session. Treat every line as a true fact you know:". The block lists one line
  for each present fact. The block ends with an active instruction.
- The active instruction tells the model to answer from these facts. It applies
  when the user asks who they are or about their setup. It forbids the refusal
  that the model has no information. It keeps the honesty rules: the model must
  not invent facts, and the model does not remember facts across past sessions.
  The public identity belongs to the user, not to the model.
- A test against the live on-device model showed a defect in the earlier passive
  wording. The passive wording asked the model to state the facts only if asked.
  The small model ignored the block. The small model gave a canned refusal about
  access to personal data. The active wording makes the model report the real
  facts. The active wording is the verified fix, not a guess.
- The prompt builder `buildOpenAgentsAppleFmPrompt` takes the context as a new
  parameter. The builder puts the block after the honesty base and before the
  connected-agent list. The history stays last. The history still drops the
  oldest turn first to fit the character limit. The limit is 3900 characters.
- A new module `apple-fm-environment.ts` builds the context from raw host inputs.
  The module takes an injected clock. The module does not read the wall clock.
  The injected clock keeps the tests deterministic.
- The main process resolves the facts at each turn. The working directory comes
  from the workspace-root resolver. The operating system comes from
  `process.platform`. The application name comes from the application name value.
  The public key comes from the sovereign identity public projection.

## Safety rules

The change keeps the identity discipline.

- The prompt carries public identity only. The prompt carries the `npub` public
  key. The prompt never carries the seed, the mnemonic, the `nsec`, or the raw
  private key.
- The renderer has a tripwire. The renderer prints an identity value only when
  the value matches the `npub1` public-key shape. The renderer drops any other
  value. A wrong secret value cannot reach the prompt.
- Each fact is fail-soft. A missing fact or a failed lookup omits its line. The
  turn never blocks.
- The empty case is safe. With no environment facts and no connected agents, the
  preamble equals the earlier plain honesty base. No live behavior changes.

## Compiled path

The compiled prompt path also carries the context. The function
`buildCompiledAppleFmPrompt` takes the context and appends the block to the
compiled preamble. The compiled path uses the same renderer and the same
tripwire. The AFS-09 release channel serves the baseline path by default, so the
baseline path is the live path today.

## Verification

The tests assert the behavior:

- The block holds each present fact from a fixture. The fixture gives the
  working directory, the operating system, the date, and the public key.
- The render is deterministic for the same inputs.
- The tripwire drops a secret-shaped identity value.
- A missing fact omits its line.
- The empty environment and empty agent case equals the plain honesty base.
