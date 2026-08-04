> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 3 — automation and attention.

NIP-AL
======

Automation Loops
----------------

`draft` `optional`

This NIP defines **Loops**: bounded recurring or event-driven agent
workflows — the automation layer that turns "when X happens, have an
agent do Y" into a versioned, budgeted, circuit-broken record instead of
an invisible cron job.

The safety posture is structural:

> A Loop's output event classes are excluded from its own trigger graph
> by construction. Loops start in suggest mode. Duplicate events, echoes,
> retry storms, and external outages must not create duplicate or
> unbounded Work.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32340 | Addressable | Loop Definition |
| 32341 | Addressable | Loop State |
| 32342-32349 | — | Reserved for future NIP-AL use |

Authority-signed; mutated through NIP-WI (`loop.draft`, `loop.publish`,
`loop.pause`, `loop.resume`, `loop.revoke`, `loop.archive`).

## 1. Loop Definition (`kind:32340`)

Address:

```text
32340:<authority_pubkey>:<loop_ref>
```

### 1.1 Required tags

- `d`: stable `loop_ref`
- `org`
- `name`: bounded display name
- `mode`: `suggest` or `auto` (see 1.4)
- `draft_revision` and `published_revision`: the published revision is
  immutable — a change drafts and republishes
- `p` with marker `agent`: the executing Agent Member
- `p` with marker `owner`: the accountable human owner of the Loop
- `state`: `draft`, `published`, `paused`, `revoked`, or `archived`
- `published_at`

### 1.2 Recommended tags

- `trigger`: repeated typed trigger selectors (see 1.3)
- `condition`: condition-spec ref evaluated before eligibility
- `a` with markers `skill` (NIP-SKL, version-pinned), `guidance`
  (NIP-GB revisions), `grant` (the NIP-AD grant template runs execute
  under)
- `tool_policy`, `budget`, `concurrency`, `retry`: policy refs and
  bounds
- `breaker`: circuit-breaker policy ref (failure threshold, cool-down,
  half-open rule)
- `exclusion`: repeated event classes structurally excluded from this
  Loop's triggers — MUST include every class the Loop can emit

### 1.3 Triggers

Recommended selector vocabulary:

- Work/Issue lifecycle: `work.created`, `work.state_changed`,
  `work.blocked`, `work.stale`, `candidate.pending` (NIP-WI intake)
- Collaboration: `review.requested`, `changes.requested`,
  `decision.recorded`, `signal.recorded` (NIP-CN)
- Execution: `session.failed`, `session.awaiting_input`,
  `verification.failed`, `sla.breached` (NIP-WS)
- Code: NIP-34 patch/PR/status events from bound repositories
- Schedule: `cron` with a bounded expression
- Workroom: an admitted actor's NIP-WA event classes

A trigger makes a Loop *eligible*. Eligibility still passes condition
evaluation, concurrency and budget checks, and — for every effect — NIP-WI
admission under the Loop's grant. A Loop never bypasses admission.

### 1.4 Modes

- **suggest**: the Loop's runs produce NIP-TP Triage Proposals or draft
  intents requiring a human disposition. This is the mandatory starting
  mode.
- **auto**: runs may submit admitted mutations directly. Publishing in
  `auto`, or promoting to it, requires the deployment's admitted
  auto-apply policy revision — measured suggest-mode precision is the
  expected evidence, and the policy ref is recorded on the definition.

### 1.5 Loop laws

1. **Self-trigger exclusion.** The trigger graph is checked at publish
   time against the Loop's declared output classes plus the transitive
   outputs of its skill; overlap refuses publication.
2. **Idempotent triggering.** One triggering event produces at most one
   run per Loop (`idem` derived from the trigger event id); duplicate
   deliveries and relay replays collapse.
3. **Bounded concurrency and budget.** Runs beyond `concurrency` queue
   or drop per policy; budget exhaustion pauses the Loop with a typed
   state, never a silent stall.
4. **Circuit breaking.** Failures beyond the breaker threshold move the
   Loop to `paused` with `reason=circuit_open`; resumption is explicit
   or half-open per the policy.
5. **Revisions are immutable.** Every run records the exact published
   revision it executed; drafts never run.

## 2. Loop State (`kind:32341`)

The operational state head. Address:
`32341:<authority_pubkey>:<loop_ref>`.

Required tags: `d` (the `loop_ref`), `org`, `state` (`idle`, `eligible`,
`running`, `cooling`, `circuit_open`, `budget_exhausted`), `revision`,
`published_at`. Recommended: `next_eligible_at`, `run` refs to the
Sessions the Loop most recently created (NIP-AS), counters
(`runs_total`, `failures_recent`), and `reason` on pause states.

Loop runs themselves are ordinary Agent Sessions on ordinary Work — the
Loop is discoverable from the session's initiator ref, and all evidence,
activity, and review flow through the existing NIPs.

## Security considerations

- **Automation storms.** The exclusion check, trigger idempotency, and
  breaker are defense in depth against the classic loop-feeds-loop
  cascade — including across Loops: deployments SHOULD check pairwise
  output/trigger overlap at publish time, not just self-overlap.
- **Grant inflation.** A Loop's grant template is bounded like any
  NIP-AD grant; `auto` mode changes who dispositions the run, never what
  the grant permits.
- **Trigger snooping.** Trigger selectors reveal operational structure;
  private Organizations keep definitions on restricted relays.
- **Suggest-mode laundering.** A suggest-mode Loop whose proposals are
  rubber-stamped by another automation is `auto` in fact; the auto-apply
  policy applies to the composition, not the label.

## References

- NIP-01
- NIP-WI, NIP-AD, NIP-AS (grants and sessions)
- NIP-GB, NIP-TP (this layer), NIP-WS (SLA breaches), NIP-CN (signals)
- NIP-SKL — version-pinned procedures
- `docs/omega/GLOSSARY.md` — Loop Definition, Trigger, Circuit Breaker

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: immutable published revisions, trigger
  vocabulary, suggest/auto modes, the five loop laws, and the Loop
  State head.
