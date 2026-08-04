> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-RV
======

Reviews
-------

`draft` `optional`

This NIP defines the review surface of the All Work system: **Review
Requests**, reviewer-signed **Review Verdicts**, and regenerable
**Review Guides**, placing changed code beside the Work intent it claims
to satisfy.

The authority boundaries, stated up front:

- A review verdict is the reviewer's typed conclusion. It contributes
  evidence toward NIP-EV verification; it is not merge authority, not
  verification by itself, and not acceptance.
- A Review Guide explains; it never approves, verifies, or merges.
- Git (and NIP-34) remains canonical for commits, checks, review threads
  it hosts, and merges. This NIP cites Git state and never invents it.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32320 | Addressable | Review Request |
| 32321 | Addressable (unique `d`) | Review Verdict |
| 32322 | Addressable | Review Guide |
| 32323-32329 | — | Reserved for future NIP-RV use |

Review Requests and Guides are authority-signed. Verdicts are
**reviewer-signed** — like NIP-EV receipts, the signature identifies who
stands behind the conclusion, and the authority attaches admitted verdicts
to Work through Work Events.

## 1. Review Request (`kind:32320`)

The state record for one requested review. Address:

```text
32320:<authority_pubkey>:<review_ref>
```

### 1.1 Required tags

- `d`: stable `review_ref`
- `org`, `work`
- `subject`: what is under review — an `a`/`e` ref with marker `subject`
  naming a NIP-34 patch/PR, a NIP-CC Coding Session, an artifact digest,
  or a NIP-DD Document revision
- `state`: `requested`, `in_review`, `changes_requested`, `approved`,
  `rejected`, `dismissed`
- `revision`, `published_at`

### 1.2 Recommended tags

- `p` with marker `requester` and repeated `reviewer`
- `a` with marker `guide`: the Review Guide, when one exists
- `a` with marker `session`: the producing Agent Session — so reviewers
  see the intent, plan, and activity beside the diff
- `e` with marker `verdict`: admitted Review Verdicts
- `a` with marker `checklist`: a versioned Review Guide rubric ref
- `due`: requested-by timestamp (planning data)

The `state` is a fold over admitted verdicts under the Work's review
policy (how many approvals, whose), evaluated at admission — never a
client-side inference.

## 2. Review Verdict (`kind:32321`)

One reviewer's typed conclusion. Address:

```text
32321:<reviewer_pubkey>:<verdict_ref>
```

### 2.1 Required tags

- `d`: unique `verdict_ref`
- `org`, `work`
- `a` with marker `review`: the Review Request
- `verdict`: `approved`, `changes_requested`, `rejected`, or
  `inconclusive`
- `subject_revision`: the exact subject revision reviewed — the pinned
  commit, patch event id, or document revision
- `reviewed_at`

### 2.2 Recommended tags

- `x`: digest of the reviewer's full notes (off-relay when private)
- `e` with marker `comment`: NIP-22 comment threads carrying inline
  discussion
- `criteria`: rubric items the verdict addressed
- `reason`: typed reason on `changes_requested` / `rejected`

### 2.3 Rules

- **Revision-bound.** A verdict binds `subject_revision`. When the
  subject changes, existing verdicts remain historical facts about the
  old revision, and the Review Request's fold treats them as stale.
- **Self-review is labeled.** A verdict whose reviewer is the subject's
  producer (session agent, patch author) is admissible only where the
  review policy allows it, and clients MUST label it self-review. It can
  never satisfy an independence-requiring fold, mirroring the NIP-EV
  floor.
- **Agents review under grants.** An agent reviewer signs with its own
  key under a NIP-AD grant whose capabilities include review; its verdict
  weight is review policy, and an agent verdict never satisfies a
  human-required approval.
- **Change requests loop, bounded.** `changes_requested` verdicts flow
  into follow-up Work or session steering through NIP-WI; the verdict
  itself commands nothing.

## 3. Review Guide (`kind:32322`)

A regenerable, evidence-linked explanation of a change set, grouping the
diff by implementation purpose. Address:

```text
32322:<authority_pubkey>:<review_ref>
```

### 3.1 Required tags

- `d`: the `review_ref` it explains
- `org`, `work`
- `subject_revision`: the exact revision explained
- `x`: digest of the guide body
- `generated_at`

### 3.2 Recommended tags

- `chapter`: repeated bounded chapter titles in reading order
- `e`/`a` with marker `evidence`: NIP-EV refs each chapter cites
- `p` with marker `generator`: the agent/tool that produced it

`content` carries the guide body (public-safe or encrypted to the review
audience). A Guide is derived, disposable, and regenerable from its
subject; a stale guide (subject moved past `subject_revision`) is marked
stale, not trusted. It has exactly zero authority: it cannot approve,
merge, or stand in for reading the change.

## Security considerations

- **Approval laundering.** A rendered green checkmark must trace to
  admitted verdicts under the review policy — not to a Guide, a Reaction,
  a chat message, or an agent's own `result` activity.
- **Rubber-stamp agents.** Agent verdicts are grant-bounded and
  policy-weighted precisely so a fleet of agent approvals cannot
  manufacture a human approval.
- **Stale-revision review.** `subject_revision` binding prevents the
  classic race: approve, then push. The fold's staleness rule makes the
  race visible instead of exploitable.
- **Private diffs.** Inline comments and notes can quote code. Reviews of
  private repositories keep comment threads and guide bodies on
  restricted relays or encrypted, with digests public at most.

## References

- NIP-01, NIP-22, NIP-34, NIP-44
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-AS, NIP-CC, NIP-AD (this layer)
- `docs/omega/GLOSSARY.md` — Work Review, Review Guide, Review Verdict,
  Merge boundary terms

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Review Request folds, revision-bound
  reviewer-signed Verdicts, self-review labeling, and zero-authority
  Review Guides.
