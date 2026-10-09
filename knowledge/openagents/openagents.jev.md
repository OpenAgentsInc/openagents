---
id: openagents.jev
version: 3
kind: product
title: "Jev and TypeSafe"
summary: >-
  Jev is TypeSafe's small, fast decision model, not an OpenAgents product:
  a partner model we use to route chat replies and to judge inside runs.
tags: [jev, typesafe, decisions, model]
applies_when: >-
  The user asks what Jev or TypeSafe is, or what the small model that reads
  messages does. Never when listing what OpenAgents offers: Jev is
  TypeSafe's, not ours.
answer: >-
  Jev is TypeSafe's small, fast decision model, not an OpenAgents
  product: TypeSafe makes it, and we use it as a partner model. It answers
  typed questions, such as which of several options fits or how likely
  something is, in a fraction of a second. We use it to choose how to reply
  to each chat message, and inside Coder's runs to judge progress. It writes
  no reply text itself.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/jev/src/lib.rs
    - docs/deployment/chat-worker.md
    - docs/verse/agent-trainer-leveling.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-02: BYOK (#10176): Jev runs on the person's own keys when they chose them."
  - "2026-10-09: v3: a reply listed Jev among 'our core products'; the note says plainly that Jev is TypeSafe's, a partner model we use for routing, never an OpenAgents product (owner)."
---

## Answer

Jev is TypeSafe's small, fast decision model, not an OpenAgents product: TypeSafe makes it, and we use it as a partner model. It answers typed questions, such as which of several options fits or how likely something is, in a fraction of a second. We use it to choose how to reply to each chat message, and inside Coder's runs to judge progress. It writes no reply text itself.

## Details

- Jev is TypeSafe's product, not ours. Never list it among what OpenAgents offers.
- Its answers are Choice (one of listed options with probabilities), Noul (a probability of yes), and Score (ordered levels).
- A Jev answer is a model's judgment, not a grader.
- With your own keys on (**Use my keys for everything**), Jev runs on your TypeSafe key, then your Vercel AI Gateway key (`typesafe-ai/jev`), then your OpenRouter key (`typesafe/jev-1.13`), never on ours (#10176).

## Sources

- `crates/jev/src/lib.rs`
- `docs/deployment/chat-worker.md`
- `docs/verse/agent-trainer-leveling.md`
