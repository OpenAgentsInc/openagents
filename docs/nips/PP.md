> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 4 — hosts, outcomes, and public trust.

NIP-PP
======

Product Promises
----------------

`draft` `optional`

This NIP defines the public trust registry as signed events: **Product
Promise** records — one per named capability, carrying its public claim
and current state — **Promise Transitions** recording every state change
with evidence, and the **Registry Head** that versions the whole set.

It makes the existing `openagents.com/promises` contract portable: any
client can render the promise registry, verify who signed each state, and
audit the transition history, from relay data alone.

The governing rule, inherited from the registry it encodes:

> The registry is the public claim authority. Roadmaps, marketing copy,
> screenshots, code, demos, or individual receipts cannot silently flip a
> promise state. A state changes only through an authority-signed
> transition citing its evidence.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32440 | Addressable | Product Promise |
| 32441 | Addressable (unique `d`) | Promise Transition |
| 32442 | Addressable | Registry Head |
| 32443-32449 | — | Reserved for future NIP-PP use |

Signed by the Organization's promise-registry authority key — declared in
the NIP-OT Organization record with marker `promise_authority`, and MAY be
the All Work authority key or a distinct one.

## 1. Product Promise (`kind:32440`)

One named capability's public claim contract. Address:

```text
32440:<registry_pubkey>:<promise_id>
```

### 1.1 Required tags

- `d`: stable `promise_id`
- `org`
- `claim`: the exact public claim text (bounded; the full statement MAY
  live in `content`)
- `state`: `green`, `yellow`, `red`, `degraded`, or `planned`
- `registry_version`: the Registry Head version this record belongs to
- `revision`, `published_at`

### 1.2 Recommended tags

- `scope`: bounded statement of what the claim covers and excludes
- `e`/`a` with marker `evidence`: current supporting evidence refs
  (NIP-EV receipts, NIP-OC outcomes, public verification records)
- `e`/`a` with marker `blocker`: current blocker refs for non-green
  states
- `verify`: verification guidance — how an outside party can check the
  claim themselves
- `e` with marker `transition`: the latest Promise Transition
- `last_verified_at`: when the claim was last re-proven
- `t`: discovery topics

`content` carries the full public claim statement and scope prose. This
kind is public by design: an encrypted promise is a contradiction, and
deployments with private capability tracking use ordinary Work records
instead.

### 1.3 State meanings

- `green`: the claim holds, with current evidence
- `yellow`: the claim holds with qualifications named in blockers
- `red`: the claim currently fails
- `degraded`: partially holds; scope tags name the working subset
- `planned`: claimed intent, no capability claim yet — a planned promise
  is a roadmap fact, not a product fact, and clients MUST render it
  distinctly

`state` is distinct from Work State, Proof Rung, release status, and
service availability. A promise can be `green` while related Work is
open, and `red` while the marketing page still exists — the registry is
what tells the truth.

## 2. Promise Transition (`kind:32441`)

One state change, append-only. Address:

```text
32441:<registry_pubkey>:<promise_id>:tr:<n>
```

### 2.1 Required tags

- `d`: `<promise_id>:tr:<n>` (unique, dense per promise)
- `org`
- `from` and `to`: the prior and new states
- `e`/`a` with marker `evidence`: the evidence justifying the transition
  — REQUIRED for transitions into `green` or `yellow`; transitions into
  `red`/`degraded` cite the failing observation where one exists
- `reason`: bounded machine-readable reason
- `p` with marker `actor`: the transition-admitting principal
- `occurred_at`, `published_at`

### 2.2 Rules

- **Evidence-gated upward.** No transition into `green` without
  resolvable evidence refs. Weakening an oracle to make a claim pass is
  a registry change requiring its own recorded decision, not a quiet
  transition.
- **Downward is fast, upward is proven.** Deployments SHOULD admit
  degradations on a single credible failing observation and require the
  full evidence chain to climb back — asymmetry in favor of honesty.
- **Dense and append-only.** Transition `n` follows `n-1`; a gap in the
  sequence is a visible integrity defect in the registry itself.

## 3. Registry Head (`kind:32442`)

The versioned index. Address: `32442:<registry_pubkey>:registry`.

Required tags: `d` (`registry`), `org`, `registry_version`, repeated `a`
refs with marker `promise` naming every current Product Promise,
`published_at`. Recommended: `x` — a digest over the sorted
`(promise_id, state, revision)` set, so any reader can verify they hold
the complete registry at the named version and detect a withheld or
stale promise.

Registry passes (adding, withdrawing, or rescoping promises) bump
`registry_version`; a withdrawn promise's record remains resolvable with
its final state and a `["withdrawn_at", ...]` tag.

## Security considerations

- **The registry key is the trust root.** Its compromise is a
  public-claim incident; rotation follows the NIP-OT two-sided pattern,
  and clients pin the promise authority from the Organization record.
- **Evidence-link rot.** A `green` promise whose evidence refs no longer
  resolve is displayable as green-with-unverifiable-evidence at best;
  clients MUST distinguish resolvable from dangling evidence.
- **Cherry-picked registries.** The head digest defeats serving a
  flattering subset: a reader holding the head can prove completeness or
  detect omission.
- **Promise-shaped marketing.** Only this registry's records are promise
  states. A quotation of a promise elsewhere inherits nothing; verifiers
  resolve the address.

## References

- NIP-01
- NIP-OT (authority declaration), NIP-EV (evidence and verification),
  NIP-OC (accepted outcomes as evidence)
- `docs/promises/` — the registry contract this NIP encodes
- <https://openagents.com/promises> and
  <https://openagents.com/api/public/product-promises> — the live
  surfaces this makes portable
- `docs/omega/GLOSSARY.md` — Product Promise, Promise Registry, Promise
  State, Public Claim, No-Evidence-No-Claim Rule

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Product Promise records, evidence-gated
  append-only Transitions with honest asymmetry, and the digest-bearing
  Registry Head.
