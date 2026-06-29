# Khala Chat Pylon Context Audit

Date: 2026-06-28
Status: implementation note
Owner: openagents.com Worker `/api/khala/chat`

## Problem

The public `/chat` page sends stateless turns to `POST /api/khala/chat`.
Before this change, that route only assembled the Khala identity prompt and the
conversation history, then opened the generic inference stream. It did not pass
any OpenAgents Pylon registry state to the model and did not have a deterministic
answer path for basic Pylon questions.

The result was visible in local testing: asking "what pylons are connected"
returned generic prose about electrical grids or Protoss/StarCraft-style pylons
instead of the OpenAgents product surface's registered Pylon nodes. That is a
product bug, not a copy problem.

## Relevant Existing Authority

Blueprint records live under `workers/api/src/blueprint` and are evidence,
contract, and approval-bound planning material. Program Types and Program
Signatures model governed behavior families; they are not prompts and do not
grant runtime write authority. Action Submissions are the write-side proposal
boundary and cannot execute effects at intake. The Blueprint boundary therefore
supports scoped Pylon assignments and evidence, but it is not a shortcut that
lets public chat mutate infrastructure.

The current Pylon API is the product-owned runtime registry:

- `GET /api/pylons`
- `GET /api/pylons/{pylonRef}`
- `GET /api/pylons/{pylonRef}/assignments`
- `POST /api/pylons/register`
- `POST /api/pylons/{pylonRef}/heartbeat`
- `POST /api/pylons/{pylonRef}/wallet-readiness`
- `POST /api/pylons/{pylonRef}/payout-target-admission`
- `POST /api/operator/pylons/assignments`
- `POST /api/pylons/{pylonRef}/assignments/{assignmentRef}/...`

Public reads return public-safe projections. Presence and lifecycle writes
require an OpenAgents agent bearer token and an `Idempotency-Key`; the Worker
explicitly rejects NIP-98 as Pylon presence authority because registrations are
bound to the owning agent token. Operator assignment creation is admin-gated and
the dispatch gate denies stale, blocked, wrong-capability, duplicate, paused, or
policy-incomplete requests.

The public stats projection already defines the live-count semantics used on the
site: an online Pylon is an active v0.2.5+ registration with a fresh heartbeat
and an online/ready status. Wallet-ready and assignment-ready are separate
states. Online is not payout evidence, accepted-work evidence, or settlement
evidence.

Provider discovery fields on `/api/pylons` are public-safe when the provider
Pylon declares the NIP-90 lane: Nostr pubkey/npub, market relay refs, and lane
refs. Those fields repeat public relay identity, not wallet/payment material.

## Required Chat Behavior

Khala chat should answer basic Pylon questions from the real product surface,
without roleplay:

- what Pylons are connected or online;
- how many are registered, online, seen recently, wallet-ready, or
  assignment-ready;
- what capabilities/coding capacity registered Pylons advertise;
- how to register or heartbeat a Pylon programmatically;
- which API surfaces can read, update, or dispatch work to Pylons.

For public `/api/khala/chat`, "interact" must be bounded:

- public chat may read and explain public Pylon projections;
- owner agents may register, heartbeat, report wallet readiness, and report
  assignment progress through bearer-token routes;
- operators may create bounded assignment leases through admin routes;
- authenticated Khala coding delegation may route caller-owned coding workflows
  to linked local Codex/Claude-capable Pylons through the separate
  OpenAI-compatible API path;
- public chat must not claim it can dispatch paid work, approve payout targets,
  spend bitcoin, settle providers, or mutate Pylon state by itself.

## Implementation Shape

Add a small typed Pylon context layer for Khala chat:

1. Load recent Pylon registrations from the existing `PylonApiStore`.
2. Reuse the public stats projection for aggregate live-count semantics.
3. Build a public-safe, bounded context block for the generic model path.
4. Use a narrow modeled parser for exact Pylon operational questions, returning
   deterministic answers for list/status/register/interaction/capability
   prompts before opening the provider stream.

The parser is intentionally narrow and Pylon-specific. It is not a general
semantic router or tool selector; it only recognizes bounded operational Pylon
question forms after the request is already inside the Khala chat route.

## Safety Notes

- Do not include owner token prefixes, raw paths, private payout destinations,
  invoices, payment hashes, preimages, mnemonics, exact balances, or raw
  provider credentials in chat context.
- Do not infer earning, payment, or settlement from registration/heartbeat
  state.
- If live Pylon context is unavailable, say so plainly and point at
  `GET /api/pylons` / `GET /api/public/pylon-stats` instead of hallucinating.
- Keep API guidance concrete but token-free: name auth requirements and
  idempotency keys, never secrets.

## Verification Targets

- Unit tests for deterministic Pylon list/status answers from a stub context.
- Unit tests that the generic inference request receives Pylon context in its
  system prompt.
- Unit tests that registration/interaction questions do not open the provider
  stream.
- Local `curl` against `/api/khala/chat` should stream a Pylon-aware answer.
- Local browser `/chat` should use the same API path.
