# Voice Session Evidence Records

Status: implemented for issue #373 / `OPENAGENTS-LATE-013`.

## Purpose

Voice should be a fast input and steering surface, but it must not become
hidden command execution. This contract records voice sessions as evidence:
transcript segment refs, confidence, provider refs, command route proposals,
source refs, redaction policy refs, approval receipts, and execution receipts.

Implementation:

- `workers/api/src/omni-voice-session-evidence.ts`
- `workers/api/src/omni-voice-session-evidence.test.ts`

## Session Report

The session report records:

- provider kind and provider ref;
- capture state;
- language ref;
- transcript segment refs;
- command proposal refs;
- source refs;
- evidence refs;
- receipt refs;
- redaction policy refs;
- approval receipt refs;
- caveat refs; and
- workroom ref.

Projection labels use friendly time strings, not raw timestamps.

Supported capture states:

- `not_recorded`;
- `recorded`;
- `transcribed`;
- `redacted`; and
- `discarded`.

Transcribed or redacted sessions require transcript segments.

## Transcript Segments

Transcript segments record speaker role, segment ref, text ref, source refs,
evidence refs, redaction policy refs, start offset, duration, and confidence in
basis points. The contract stores transcript evidence as refs only; it rejects
raw transcript text, raw audio, provider payloads, private names, contact
information, secrets, payment/wallet material, and raw timestamps.

## Command Proposals

Command proposals support these route kinds:

- `site_revision_feedback`;
- `forum_post`;
- `customer_order`;
- `crm_send`;
- `coding_write`;
- `runner_launch`;
- `payment`;
- `provider_action`;
- `public_claim`;
- `pylon_setup`; and
- `unknown`.

Proposal states are:

- `draft`;
- `proposed`;
- `needs_approval`;
- `approved`;
- `rejected`;
- `executed`;
- `blocked`; and
- `expired`.

Proposed, approval-needed, approved, and executed commands require source
transcript segment refs and evidence refs. The source segment refs must point
at transcript segments in the same session report.

Writes, sends, payments, provider actions, public claims, Pylon setup, and
runner launches require approval. High and critical risk proposals also require
approval. Approved or executed proposals require approval receipt refs;
executed proposals require execution receipt refs; blocked proposals require
blocked reason refs; and expired proposals require an expiry timestamp.

## Authority Boundaries

Voice session evidence is read-only. It cannot:

- capture audio;
- mutate transcripts;
- mutate proposals;
- approve work;
- execute commands;
- spend wallets;
- mutate provider accounts; or
- upgrade public claims.

Any actual command execution must go through a separate server-authoritative
route with scoped grants, idempotency, approval policy, and receipts.

## Projection Audiences

Supported projection audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public, agent, and customer projections redact private provider, idempotency,
receipt, segment, session, text, source, proposal, language, title, and
approval refs as appropriate. Operator and team projections can retain the full
safe ref set, but even those projections reject raw secrets, provider payloads,
raw transcript/audio, private repo refs, payment/wallet material, and raw
timestamps.

## Tests

Coverage includes:

- voice session projection and counts;
- transcript/source validation;
- confidence and duration validation;
- proposal state transition requirements;
- approval and execution receipt requirements;
- blocked and expired proposal requirements;
- public redaction; and
- hard false audio capture, transcript, proposal, approval, execution,
  payment, provider, and public-claim mutation authority.
