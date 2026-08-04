> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 3 — automation and attention.

NIP-TP
======

Triage Proposals
----------------

`draft` `optional`

This NIP defines **Triage Proposals**: reviewable suggested changes from a
triage engine — routing, duplicates, labels, priority, ownership,
readiness — with confidence, evidence, expiry, and an explicit
disposition.

The rule:

> A proposal never mutates Work by itself. A human disposition, or an
> admitted auto-apply policy, converts a proposal into a NIP-WI intent —
> and the intent passes ordinary admission. Explanation is not proof, and
> confidence is a self-assessment, not evidence.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32350 | Addressable | Triage Proposal |
| 32351-32359 | — | Reserved for future NIP-TP use |

Authority-signed. The proposing engine (an agent under a NIP-AD grant, a
Loop, or a service) submits proposals through NIP-WI
(`triage.propose`); dispositions are `triage.dispose` intents.

## 1. Triage Proposal (`kind:32350`)

Address:

```text
32350:<authority_pubkey>:<proposal_ref>
```

### 1.1 Required tags

- `d`: unique `proposal_ref`
- `org`
- `subject`: `a` ref with marker `subject` — the Work/Issue, or the
  NIP-WI Work Candidate, the proposal concerns
- `kind_label`: what is proposed (see 1.3)
- `proposal`: the proposed value — a ref (team, state, label, principal,
  duplicate target) or bounded literal
- `confidence`: `high`, `medium`, or `low` (a calibrated deployment MAY
  add a numeric `confidence_score`)
- `state`: `open`, `accepted`, `edited`, `rejected`, `auto_applied`, or
  `expired`
- `p` with marker `proposer`: the engine's principal
- `exp`: expiry after which the proposal cannot be applied
- `published_at`

### 1.2 Recommended tags

- `e`/`a` with marker `evidence`: the source facts the proposal rests on
  — similar prior Work (NIP-WR lineage), matching signals (NIP-CN),
  text-match refs, history refs
- `reason`: bounded machine-readable rationale code
- `alternative`: repeated lower-ranked candidate values
- `p` with marker `disposer` plus `disposed_at`: filled at disposition
- `e` with marker `applied_intent`: the NIP-WI intent that applied it

`content` MAY carry a bounded human-readable rationale. Rationale text is
a claim by the proposer; the `evidence` refs are what a reviewer checks.

### 1.3 Proposal kinds

| `kind_label` | Proposes |
| --- | --- |
| `route_team` / `route_project` | Team or Project placement |
| `duplicate` | A duplicate relation to a canonical Work |
| `label` / `priority` | Label attachment or priority value |
| `assignee` / `delegate` | Accountable owner or agent delegate |
| `state` | A Workflow State move |
| `sla` | An SLA policy attachment |
| `readiness` | Ready-for-agent-investigation assessment |
| `question` | Required missing information (pairs with a NIP-AV elicitation when accepted) |
| `candidate_admit` / `candidate_reject` | Disposition of a NIP-WI Work Candidate |

Unknown kinds are preserved and displayed as unknown.

### 1.4 Disposition rules

- **One disposition per proposal.** `accepted` applies the proposed
  value; `edited` applies a disposer-modified value (recorded in the
  applied intent); `rejected` applies nothing; all three are terminal.
- **Auto-apply is a named policy.** `auto_applied` is admissible only
  under the deployment's auto-apply policy revision, which names eligible
  `kind_label`s, minimum confidence, and excluded subjects. Low-confidence
  or conflicting proposals MUST NOT auto-apply, and two open proposals
  proposing different values for the same field suspend both for human
  disposition.
- **Expiry is honest.** An expired proposal is `expired`, not silently
  gone; late acceptance is refused.

### 1.5 Measurement

Because proposals, dispositions, and applied intents are all signed
records, precision is computable from the wire: acceptance rate, edit
rate, rejection rate, and downstream reversal rate per `kind_label` and
per proposer. Deployments SHOULD gate `auto` promotion (here and in
NIP-AL) on these measured rates, not on demonstrations.

## Security considerations

- **Confidence inflation.** `confidence` is proposer-asserted. Auto-apply
  policies keyed on it MUST also require measured historical precision
  for the proposer — a new engine starts in suggest regardless of its
  self-reported confidence.
- **Evidence-free persuasion.** A proposal whose rationale is fluent but
  whose `evidence` refs are absent or irrelevant is the triage-shaped
  false green; clients render evidence-less proposals with a warning.
- **Injection via subject.** Proposals quote candidate and Issue text.
  That text is untrusted data in every rendering and in the disposer's
  context.
- **Routing leaks.** A proposal's evidence refs can reveal private prior
  Work to a broader triage audience; audience filtering runs before
  publication, and refs the audience cannot read are omitted, not
  teased.

## References

- NIP-01
- NIP-WI (intents, candidates), NIP-WR (duplicate/lineage evidence),
  NIP-CN (signals), NIP-WS (labels, states, SLA)
- NIP-AL (this layer) — Loops as proposers
- `docs/omega/GLOSSARY.md` — Triage Engine, Triage Proposal

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: proposal record, kind vocabulary, disposition
  and auto-apply rules, and wire-measurable precision.
