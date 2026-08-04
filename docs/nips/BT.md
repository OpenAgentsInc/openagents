> Status: proposed draft from the Bitcoin OSS hardening program
> ([`2026-08-04-nostr-native-hardening-program.md`](../hardening/2026-08-04-nostr-native-hardening-program.md)).
> Application: optional campaign funding, contribution standing, and
> references to externally settled payouts.

NIP-BT
======

Bounties and Contribution Credit
--------------------------------

`draft` `optional`

This NIP defines three records for sponsored security and public-interest
work: a **Funding Pool** that states a campaign budget, a **Contribution
Credit** that gives receipt-backed standing for useful work, and a **Payout
Reference** that can point to external settlement evidence after a campaign
admits a payment rail.

The primary economic unit is verified contribution, not vulnerability count.
Coverage attestations, independent verdicts, reproductions, negative controls,
and refutations can earn credit. Credit does not require a payout rail and does
not depend on publishing a finding.

> Nothing in this NIP moves sats, reserves funds, escrows value, creates a debt,
> or grants a right to payout. A Funding Pool is a signed budget statement. A
> Contribution Credit is non-transferable standing. A Payout Reference records
> evidence produced by NIP-57, NIP-61, or a designated ledger after payment
> happened outside this NIP.

## Kinds

This NIP reserves:

| Kind | Type | Description |
| --- | --- | --- |
| 32490 | Addressable (unique `d`) | Funding Pool revision |
| 32491 | Addressable (unique `d`) | Contribution Credit revision |
| 32492 | Addressable (unique `d`) | Payout Reference revision |
| 32493-32499 | — | Reserved for future NIP-BT use |

Each `d` value below includes a dense revision number. Authors MUST append a
new address for a state change and MUST NOT replace an already published
address.

## Actors and authority

- The **sponsor authority** signs Funding Pool revisions. It is the sponsor or
  a key that the sponsoring Organization has explicitly admitted for campaign
  budget statements.
- The **credit authority** signs Contribution Credits. The current Funding
  Pool or campaign policy MUST name this key with a `p` tag whose marker is
  `credit_authority`.
- The **payout-recording authority** signs Payout References. The current
  Funding Pool or campaign policy MUST name this key with a `p` tag whose
  marker is `payout_authority`, and MUST cite an admitted rail policy before
  the key can publish a valid Payout Reference.
- The **contributor** is the principal credited for the receipt-backed work.
  The credit authority pubkey MUST differ from the contributor pubkey. A
  campaign policy MAY require stronger separation between owners or roles.
- The **verifier** evaluates produced work. For an independent verdict, the
  verifier and producer MUST satisfy the independence floor in NIP-EV or the
  stronger policy named by the campaign.

A signature identifies the signer; it does not prove that the signer holds one
of these roles. Clients MUST resolve the authority declaration from the
Funding Pool or campaign policy. Relay admission grants no campaign, credit,
disclosure, payout, or settlement authority.

Authority rotation MUST be explicit in the campaign policy. A successor
inherits the campaign's revision sequence and idempotency set; rotation does
not permit the successor to issue the same credit or payout again.

## Common values and tags

### Campaign and scope

`campaign_ref` is one stable, public-safe campaign identifier. It MUST be
either a Nostr event address (`<kind>:<pubkey>:<d>`) or the lowercase SHA-256
digest of pre-registered campaign terms. Every record uses:

```text
["campaign", "<campaign_ref>"]
```

Scope is repeated as needed:

```text
["scope", "<scope_type>", "<scope_ref>"]
```

`scope_type` is one of:

- `target`: one repository, project, package, or artifact;
- `target_class`: a declared group of targets;
- `source_set`: one materialized source-set or revision ref;
- `profile`: one pre-registered scan or evaluation profile;
- `invariant`: one security invariant or regression watch; or
- `work`: another bounded work/objective ref.

`scope_ref` MUST be a stable public-safe ref or a digest, never raw private
target material. A record with several `scope` tags applies to their
intersection. An empty scope is invalid.

### Contribution classes

`contribution_class` is exactly one of:

- `coverage_attestation`;
- `independent_verdict`;
- `reproduction`;
- `negative_control`;
- `confirmed_finding`; or
- `other_verified`, when the referenced credit scheme defines the class.

For `independent_verdict`, `result` is REQUIRED and is one of `confirmed`,
`refuted`, or `inconclusive`. A `refuted` verdict is eligible work. For
`negative_control`, a correctly detected seed and an honestly reported miss
can both be eligible when the pre-registered credit scheme says how each is
scored. A campaign MUST NOT silently discard negative or refuting work after
it was performed under an eligible policy.

### Economic provenance

The labels below are separate facts:

- `modeled`: a planning estimate;
- `pledged`: a sponsor-signed commitment, without proof of custody or payment;
- `paid`: an admitted external rail reports that a payment happened; and
- `settled`: the designated settlement authority reports final settlement.

`modeled` does not imply `pledged`; `pledged` does not imply `paid`; and `paid`
does not imply `settled`. Clients MUST preserve the label on storage,
aggregation, and display. They MUST NOT render a sum that combines labels
without separate subtotals.

Monetary amounts use whole sats in canonical base-10 form with no sign, leading
zero, separator, or decimal point. Contribution units are non-monetary positive
integers defined by a versioned credit scheme. No conversion from contribution
units to sats exists unless an external campaign policy states one, and that
policy still creates no payout authority under this NIP.

## 1. Funding Pool (`kind:32490`)

A sponsor-signed campaign budget statement for a bounded scope and period.
Exact address:

```text
32490:<sponsor_authority_pubkey>:<campaign_ref>:pool:<n>
```

The `d` tag is `<campaign_ref>:pool:<n>`, where `n` starts at `0` and increases
by one for each revision from this sponsor authority.

### 1.1 Required tags

- `d`: `<campaign_ref>:pool:<n>`
- `campaign`: the stable `campaign_ref`
- `org`: the sponsoring Organization ref
- one or more `scope` tags
- `state`: `planned`, `open`, `paused`, `closed`, or `cancelled`
- `amount`: `["amount", "<sats>", "modeled" | "pledged", "budget"]`
- `p` with marker `credit_authority`: the admitted Contribution Credit signer
- `starts_at`, `ends_at`: campaign period as Unix timestamps, with
  `starts_at < ends_at`
- `revision`: the same integer `n`
- `published_at`

### 1.2 Recommended tags

- `scheme`: versioned contribution-credit policy ref
- `verification_policy`: NIP-EV or campaign-specific independence policy ref
- `eligible`: one tag per eligible `contribution_class`
- `p` with marker `payout_authority`, only when a payout-recording authority is
  admitted
- `rail_policy`: versioned admitted-rail policy ref, only when one exists
- `disclosure_policy`: the finding/disclosure gates that a payout must satisfy
- `e` with marker `previous`: the prior Funding Pool revision for `n > 0`
- `a` with marker `campaign`: the campaign's Nostr record, when
  `campaign_ref` is a digest rather than that record's address
- `t`: discovery topics

`content` MAY contain a bounded public-safe campaign summary. It MUST NOT carry
raw findings, private coverage gaps, invoices, wallet material, payment
preimages, credentials, or private target data.

### 1.3 State and budget rules

- The initial state is `planned`, `open`, or `paused`. `open` and `paused`
  require `amount` provenance `pledged`. A `planned` pool MAY remain
  `modeled`.
- Valid transitions are `planned -> open | cancelled`,
  `open -> paused | closed | cancelled`, and
  `paused -> open | closed | cancelled`. `closed` and `cancelled` are terminal.
- A revision MAY reduce or withdraw a future pledge. It does not erase credits
  already issued or change an external payment fact.
- A pledge is not proof that funds exist or are reserved. `paid` and `settled`
  totals are derived only from valid Payout References and stay separately
  labeled.
- Pool eligibility MUST reward `coverage_attestation`,
  `independent_verdict`, or both. A scheme MUST NOT price work solely by the
  number or severity of findings, and MUST NOT award a race based on reveal or
  publication time.

## 2. Contribution Credit (`kind:32491`)

A credit-authority-signed, non-transferable record that one contributor
performed eligible receipt-backed work. Exact address:

```text
32491:<credit_authority_pubkey>:<campaign_ref>:credit:<credit_id>:<n>
```

The `d` tag is `<campaign_ref>:credit:<credit_id>:<n>`. `credit_id` is a stable
lowercase identifier selected by the credit authority. `n` starts at `0`.

### 2.1 Required tags

- `d`: `<campaign_ref>:credit:<credit_id>:<n>`
- `campaign`: the stable `campaign_ref`
- `org`: the campaign Organization ref
- `p` with marker `contributor`: the credited pubkey
- one or more `scope` tags
- `contribution_class`: one value from the enum above
- `result`: required only for `independent_verdict`, as defined above
- `scheme`: the exact versioned credit policy used
- `credit_units`: a positive integer and scheme-defined unit:
  `["credit_units", "<integer>", "<unit>"]`
- one or more `e` or `a` refs with marker `contribution`: the signed source
  receipt or protocol record that binds the contributor to the work
- one or more `e` or `a` refs with marker `verification`: the independent
  verification or verdict records required by §2.3
- `idempotency`: the lowercase SHA-256 fingerprint defined in §2.4
- `state`: `issued`, `disputed`, `revoked`, or `superseded`
- `credited_at`: Unix timestamp of the initial credit decision; immutable in
  later revisions
- `revision`: the same integer `n`
- `published_at`

### 2.2 Recommended tags

- `a` with marker `pool`: the Funding Pool revision used for eligibility
- `criteria`: each pre-registered criterion satisfied by the work
- `reason`: a bounded machine-readable reason for a dispute, revocation, or
  supersession
- `e` with marker `previous`: the prior Contribution Credit revision for
  `n > 0`
- `a` with marker `successor`: the replacement credit when
  `state=superseded`

`content` MAY carry a bounded public-safe summary. The contribution refs and
digests are sufficient for a credit; neither content nor refs need to reveal a
finding.

### 2.3 Admission and state rules

- The initial state MUST be `issued`. Valid transitions are
  `issued -> disputed | revoked | superseded` and
  `disputed -> issued | revoked | superseded`. `revoked` and `superseded` are
  terminal.
- Campaign policy determines the evidence sufficient for each class. A
  `coverage_attestation`, `reproduction`, `negative_control`, or
  `confirmed_finding` credit MUST cite independent verification. An
  `independent_verdict` credit MAY use the verdict itself as the verification
  ref, but its signer MUST be independent of the evaluated producer.
- A refutation, an inconclusive independent evaluation, an attested absence,
  or a scan that finds nothing MAY receive credit when it satisfies the
  pre-registered scheme. A finding is not required.
- `confirmed_finding` credit requires an independently confirmed finding or
  verdict ref. It MUST be ordered by the pre-registered commitment or work
  record, never by reveal or publication time.
- Credit issuance MUST NOT require public finding content. Group-visible,
  encrypted, or digest-only receipts are valid when authorized verifiers can
  resolve and verify them.
- A Contribution Credit is standing only. It is not transferable, spendable,
  assignable, collateral, a balance, an invoice, or a promise of payout.
- Revocation records a later decision and does not delete the initial work,
  verification, dispute, or audit history. Clients SHOULD display the latest
  valid state and retain the full chain.

### 2.4 Idempotency and double-credit prevention

Before issuance, the credit authority computes:

```text
SHA256("nip-bt-credit-v1\n" ||
       campaign_ref || "\n" ||
       scheme_ref || "\n" ||
       contributor_pubkey || "\n" ||
       contribution_class || "\n" ||
       result_or_empty || "\n" ||
       sorted_scope_tags || "\n" ||
       sorted_contribution_refs)
```

`sorted_scope_tags` and `sorted_contribution_refs` are the lexicographically
sorted UTF-8 tag arrays serialized as compact JSON. Nostr `a` and `e` refs are
serialized with the tag name, ref value, relay hint, and marker; absent trailing
elements are empty strings.

The resulting digest is the `idempotency` value. Within one campaign and
scheme, an idempotency value MAY be issued only once, including after the
credit becomes disputed, revoked, superseded, or authority keys rotate.
Splitting one receipt set into several credits, changing `credit_id`, replaying
on another relay, or republishing under a successor key does not create new
credit. Credits under distinct campaigns or schemes remain distinct and MUST
retain those labels when displayed or aggregated.

## 3. Payout Reference (`kind:32492`)

A payout-authority-signed binding from one Contribution Credit to evidence
created by an admitted external settlement rail. It can exist only after the
external payment evidence and all applicable disclosure gates exist. Exact
address:

```text
32492:<payout_authority_pubkey>:<campaign_ref>:payout:<credit_id>:<n>
```

The `d` tag is `<campaign_ref>:payout:<credit_id>:<n>`. One campaign has at
most one Payout Reference chain for a `credit_id`; `n` starts at `0`.

### 3.1 Required tags

- `d`: `<campaign_ref>:payout:<credit_id>:<n>`
- `campaign`: the stable `campaign_ref`
- `org`: the campaign Organization ref
- `a` with marker `credit`: the Contribution Credit revision being paid
- `p` with marker `contributor`: the same credited pubkey
- `rail`: `nip57`, `nip61`, or `ledger`
- `rail_policy`: the admitted rail policy version
- `e` or `a` with marker `settlement_evidence`: the NIP-57 zap receipt
  (`kind:9735`), NIP-61 nutzap event, or designated-ledger receipt
- `amount`: `["amount", "<sats>", "paid" | "settled", "payout"]`
- `state`: `paid`, `settled`, or `reversed`; it MUST match the current amount
  provenance except that `reversed` cites the former provenance
- `disclosure_gate`: `not_applicable` or `satisfied`
- `idempotency`: the lowercase SHA-256 fingerprint defined in §3.3
- `recorded_at`, `revision`, `published_at`

### 3.2 Recommended tags

- `e` or `a` with marker `disclosure`: the disclosure-state or gate evidence;
  it is REQUIRED when `disclosure_gate=satisfied`
- `a` with marker `pool`: the applicable Funding Pool revision
- `e` with marker `previous`: the prior Payout Reference revision for `n > 0`
- `reason`: bounded reason and external evidence ref for `reversed`

`content` MUST be empty or a bounded public-safe summary. It MUST NOT contain
an invoice, payment preimage, wallet material, mint token, credential, private
ledger payload, raw finding, or embargoed disclosure material.

### 3.3 Rail, gate, and replay rules

- The payout authority MUST resolve an `issued` Contribution Credit, verify
  that the contributor matches, and verify the external evidence under the
  cited `rail_policy` before signing.
- The rail policy MUST identify the settlement authority, supported evidence
  kind, recipient binding, amount rule, finality rule, and replay domain. A
  NIP-57 or NIP-61 event is evidence under that policy; relay acceptance is not
  payment or settlement authority.
- `paid` records external evidence that payment happened but makes no finality
  claim. `settled` requires the designated ledger or rail authority's final
  settlement evidence. A client MUST NOT infer `settled` from `paid`.
- Initial state is `paid` or `settled`. A `paid` chain may transition to
  `settled` or `reversed`. A `settled` chain may transition to `reversed` only
  when the designated settlement authority supplies reversal evidence.
  `reversed` is terminal.
- A finding-related credit MUST satisfy the campaign's cited disclosure policy
  before a Payout Reference is valid. A gate can be satisfied by maintainer
  acknowledgement, remediation, coordinated-release approval, publication, or
  another terminal state that the pre-registered policy admits. This NIP does
  not require publication, and embargo expiry alone MUST NOT satisfy a gate.
- `disclosure_gate=not_applicable` is valid only for work with no finding or
  disclosure obligation, such as coverage, an independent refutation, or a
  negative control.
- The Payout Reference cannot be used as an invoice, wallet command, release
  authorization, or proof that the pool had custody. It records a relation to
  evidence after the external action.

The payout idempotency value is:

```text
SHA256("nip-bt-payout-v1\n" ||
       campaign_ref || "\n" ||
       contribution_credit_address || "\n" ||
       contributor_pubkey || "\n" ||
       rail || "\n" ||
       rail_policy_ref || "\n" ||
       settlement_evidence_ref)
```

A settlement evidence ref MUST appear in only one Payout Reference in the
campaign. One credit has at most one payout chain. Relaying, rewrapping,
authority rotation, a different `d`, or a later state revision MUST NOT count
the evidence or amount again. Clients calculating totals count the chain once
at its latest valid state.

## Append-only audit rules

For all three kinds:

1. `revision=0` has no `previous` ref. Every later revision increments by one
   and cites exactly one prior event from the same kind, campaign, logical
   record, and admitted authority lineage.
2. Immutable fields remain unchanged across a chain. These include campaign,
   Organization, logical ID, scope, contributor, contribution class, source
   refs, credit idempotency value, and payout settlement-evidence ref.
3. An author MUST NOT publish two events for one kind, `d`, and revision. Two
   valid signatures at one position are equivocation; clients MUST expose the
   fork and MUST NOT select a favorable branch by timestamp.
4. Missing revisions, unresolved `previous` refs, changed immutable fields,
   invalid transitions, and authority discontinuities make the later revision
   non-canonical. The signed event remains audit evidence.
5. NIP-09 deletion requests and relay retention do not erase the economic or
   audit fact for a client that already observed it. Campaign operators SHOULD
   use relays that retain every addressed revision and SHOULD publish the refs
   needed for independent reconstruction.

Timestamps are signer claims. Revision and `previous` establish campaign
ordering; relay arrival order does not.

## Composition

- **NIP-SP and NIP-SC:** pre-registered profiles, source sets, coverage
  attestations, and negative controls are eligible contribution refs.
- **NIP-FD:** commitments, independent verdicts, reproductions, finding state,
  and disclosure gates determine finding-related eligibility. Credit order
  follows the pre-registered commitment or Work order, not reveal time.
- **NIP-SI:** invariant and regression-watch receipts can be credited as
  coverage or other verified work under a declared scheme.
- **NIP-EV:** evidence and independent Verification Receipts supply the default
  trust chain. Evidence, verification, credit, disclosure, payment, and
  settlement remain distinct rungs.
- **NIP-OC:** Accepted Outcomes and Closeouts MAY reference BT credits and
  payout records. NIP-OC or another designated ledger remains the accounting
  authority; BT does not promote a payment label.
- **NIP-57 and NIP-61:** provide optional Nostr-native payment evidence. They
  are not required for Contribution Credit.
- **NIP-29, NIP-44, and NIP-59:** group scoping, encryption, and gift wrapping
  can restrict sensitive records. Implementations must remember that outer
  tags and relay metadata can still disclose relationships.

No composition rule permits a payout before its rail and disclosure policies
are admitted. No credit, payout, or settlement state releases a finding for
publication or grants a Product Promise state.

## Security and privacy considerations

- **Per-vulnerability racing.** Paying by finding count, severity, reveal
  time, or publication time rewards speed and inflation. Pre-register schemes,
  pay or credit coverage and independent verification, and bind finding credit
  to commitment order.
- **Self-credit and Sybil roles.** Distinct pubkeys are only a minimum. A
  campaign that needs organizational independence resolves NIP-OT membership
  and attestations and rejects common-control roles under its verification
  policy.
- **Replay and double credit.** Authorities keep campaign-wide idempotency
  indexes across relays and key rotations. Readers recompute fingerprints and
  reject duplicate receipt or settlement-evidence use.
- **False settlement.** Readers verify the external receipt signer, recipient,
  amount, currency, finality, and replay domain against the admitted rail
  policy. A sponsor, relay, or payout-recording authority cannot self-assert an
  external settlement fact.
- **Budget overstatement.** `pledged` proves only the sponsor's signed
  statement. Clients do not label it funded, paid, or settled, and do not infer
  pool balance from it.
- **Disclosure pressure.** Credit issuance requires receipts, not public
  findings. Payout gates follow the campaign disclosure policy and never make
  embargo expiry or publication the default. A payout record cannot release
  content.
- **Coverage-map leakage.** Specific gaps and target details can help an
  attacker. Publish aggregate scopes when possible; keep specific records on
  scoped relays or inside NIP-29 groups; encrypt content and use opaque,
  digest-bound refs.
- **Relationship and amount leakage.** Public credits and payout refs reveal
  contributors, sponsors, timing, and amounts even when content is encrypted.
  Private campaigns use restricted relays or wrapped events and publish only
  deliberately aggregated statements.
- **Authority compromise and forks.** Pin authorities from the campaign
  policy, require explicit rotation, preserve every revision, and display
  equivocation instead of resolving it by convenience.

## References

- NIP-01 — event format and addressable events
- NIP-09 — deletion requests
- NIP-29 — relay-based groups
- NIP-44 and NIP-59 — encrypted payloads and gift wrap
- NIP-57 — Lightning zaps and zap receipts
- NIP-61 — nutzaps
- NIP-EV — evidence, independent verification, and receipt edges
- NIP-OC — accepted outcomes, closeouts, and settlement references
- NIP-OT — Organization and role authority
- NIP-AC — Agent Credit settlement receipts as a possible designated ledger
- NIP-LBR — labor-platform closeout and payout receipt boundaries
- NIP-PP — public claims remain a separate authority
- `docs/hardening/2026-08-04-nostr-native-hardening-program.md` — program
  rationale, disclosure boundary, roadmap, and acceptance criteria

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: append-only Funding Pool, Contribution Credit, and
  Payout Reference records; receipt-backed coverage and verification credit;
  external settlement and disclosure gates; provenance and replay rules.
