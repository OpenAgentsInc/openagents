> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 4 — hosts, outcomes, and public trust.

NIP-OC
======

Outcome Closeout
----------------

`draft` `optional`

This NIP defines the terminal economics records of the All Work system:
the **Accepted Outcome** — the unit the whole engine prices — and the
**Closeout** that reconciles every ending, accepted or not.

The accepted outcome is the unit of account this program exists to
produce: not tokens, not completions, not merged commits, but an outcome
a stranger can check — scoped in advance, evidenced, independently
verified, and accepted by the accountable owner. This NIP gives that unit
a wire form and binds it to the attribution refs that let contributors be
paid for it, without itself moving a satoshi.

> Nothing in this NIP is settlement. Settlement lives with its designated
> ledger — NIP-AC receipts, NIP-LBR closeouts, or an external authority —
> and this NIP only references it.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 39580 | Addressable | Accepted Outcome |
| 39581 | Addressable | Closeout |
| 39582-39589 | — | Reserved for future NIP-OC use |

Authority-signed, in the `39xxx` agent-economics neighborhood beside
NIP-SA, NIP-AC, NIP-TRN, and NIP-HP.

## 1. Accepted Outcome (`kind:39580`)

One accepted Work outcome and its complete derivation. Address:

```text
39580:<authority_pubkey>:<work_ref>
```

### 1.1 Required tags

- `d`: the `work_ref`
- `org`
- `a` with marker `disposition`: the NIP-EV Owner Disposition with
  `decision=accepted` — the record without which this kind cannot exist
- `a` with marker `objective`: the exact NIP-WK Objective revision
  accepted against
- `accepted_at`
- `revision`, `published_at`

### 1.2 Recommended tags

- `e`/`a` with markers `verification`, `evidence`: the NIP-EV chain the
  disposition considered
- `a` with marker `edge`: NIP-EV Receipt Edges rooting the receipt graph
- `p` with marker `contributor`: each attributed principal — agents,
  people, reviewers, verifiers — with the contribution class in a
  fourth element (`["p", "<pubkey>", "", "contributor:delegate"]`)
- `a` with markers `session`, `run`: the producing Sessions and Runs
- `component`: versioned capability/skill/model refs that contributed
- `a` with marker `split`: the Split Definition ref governing allocation
- `a` with marker `settlement`: settlement refs as they appear (NIP-AC
  `kind:39244`, NIP-LBR closeouts, external ledger refs)
- `cost`: bounded cost records with provenance labels

### 1.3 Rules

- **Derived, never asserted.** An Accepted Outcome is publishable only
  when its `disposition` ref resolves to an admitted `accepted`
  disposition on the same Work and objective revision. There is no other
  path to this kind.
- **Attribution is receipt-backed.** A `contributor` or `component` tag
  is admissible only with a traversable path through the receipt graph
  (sessions, activities, evidence, verdicts). Presence in a Run is not
  attributable value, and the graph cannot infer missing contribution.
- **Provenance labels are mandatory on economics.** Every `cost` value
  and derived metric carries its label — `modeled`, `measured`,
  `verified`, `paid`, `settled` — and later rungs are never inferred.
- **Revision-scoped.** A later Objective revision does not retract an
  Accepted Outcome; it opens new Work-to-accept. The record is a
  permanent fact about the revision it names.

## 2. Closeout (`kind:39581`)

The terminal reconciliation for Work or a bounded engagement, whatever
the ending. Address:

```text
39581:<authority_pubkey>:<work_ref>
```

### 2.1 Required tags

- `d`: the `work_ref`
- `org`
- `status`: `accepted`, `rejected`, `waived`, `cancelled`, `superseded`,
  `refunded`, or `no_reward`
- `closed_at`
- `revision`, `published_at`

### 2.2 Recommended tags

- `a` with marker `outcome`: the Accepted Outcome, when `status=accepted`
- `a` with marker `disposition`: the terminal NIP-EV disposition
- `a` with marker `settlement`: settlement refs, each with its state
  (`["a", "<ref>", "", "settlement:pending"]` /
  `settlement:settled`)
- `reason`: typed reason for non-accepted endings
- `a` with marker `successor`: replacement Work on supersession

### 2.3 Rules

- Every Work that ends gets a Closeout — the unhappy endings are as
  load-bearing for the economics as the accepted ones, because
  acceptance rate, rework, and refund rates are computed from them.
- `refunded` and `no_reward` reference their settlement-side evidence
  where it exists; a Closeout asserting refund with no ref is a claim
  awaiting its ledger.
- A Closeout closes accounting for one Work revision. It does not delete
  history, revoke evidence, or prevent a successor.

## 3. Measurement from the wire

Because outcomes, closeouts, dispositions, sessions, and costs are all
signed records, the program's north-star metrics are computable by any
authorized reader without a private analytics store:

- accepted outcomes per time window, per Work Class, per contributor;
- acceptance rate (accepted / eligible closeouts) with the rejected,
  cancelled, and reworked denominators intact;
- cost per accepted outcome, with provenance labels preserved;
- review minutes and verification cost per accepted outcome where those
  receipts exist; and
- accepted outcomes per kilowatt-hour, exactly when measured energy
  inputs exist on the referenced dispatch and session records — and
  labeled `modeled` otherwise.

A metric computed over records with missing refs inherits the gaps: loss
accounting applies to analytics too.

## Security considerations

- **Outcome inflation.** The derivation rule is the defense: no
  disposition, no outcome. A fleet of self-verified, self-accepted Work
  fails at the NIP-EV independence floor before it ever reaches this
  kind.
- **Attribution gaming.** Receipt-backed attribution plus signed Split
  Definitions make allocation auditable; disputes attach as NIP-EV
  `disputes` edges rather than edits.
- **Metric laundering.** Provenance labels are not interchangeable;
  clients MUST NOT render `modeled` economics beside `settled` ones
  without the labels.
- **Privacy.** Outcome records reveal who did what for whom. Private
  Organizations keep them on restricted relays; public publication is
  the deliberate act of building a portable track record.

## References

- NIP-01
- NIP-WK, NIP-EV (layer 0) — objectives, dispositions, receipt graph
- NIP-AS, NIP-AV (layer 2) — the producing records
- NIP-HP (this layer) — dispatch and energy inputs
- NIP-AC `kind:39244`, NIP-LBR — settlement rails this NIP references
- `docs/allwork/README.md` — the accepted-outcome thesis
- `docs/omega/GLOSSARY.md` §13 — outcome-economics terms and provenance
  labels

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: derived Accepted Outcomes with receipt-backed
  attribution, universal Closeouts, and wire-computable outcome
  economics.
