> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 0 — foundation.

NIP-EV
======

Evidence, Verification, and Dispositions
----------------------------------------

`draft` `optional`

This NIP defines the trust wire of the All Work system: how produced
evidence, independent verification, and the accountable owner's decision
become separate, signed, digest-bound records.

The design encodes three rules the whole OpenAgents program depends on:

1. **No evidence, no claim.** A readiness, completion, verification, or
   outcome claim is unavailable until the record that proves it exists and
   resolves.
2. **Producer-verifier separation.** Producing an effect or artifact never
   grants authority to verify it.
3. **No rung implies the next.** Evidence is not verification. Verification
   is not acceptance. Acceptance is not release, public claim, or
   settlement. Each is its own signed record, and clients report only the
   highest rung actually reached.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32190 | Addressable (unique `d`) | Evidence Receipt |
| 32191 | Addressable (unique `d`) | Verification Receipt |
| 32192 | Addressable (unique `d`) | Owner Disposition |
| 32193 | Addressable (unique `d`) | Receipt Edge |
| 32194-32199 | — | Reserved for future NIP-EV use |

Unlike NIP-WK records, kinds `32190` and `32191` are signed by their
**producer** and **verifier** keys respectively, not by the All Work
authority. The signature identifies who stands behind the record; the
authority attaches admitted receipts to Work through NIP-WK
`evidence_attached` / `verification_recorded` events, which is what makes a
receipt part of canonical Work history.

## Common tags

- `["d", "<identifier>"]` — unique receipt identifier
- `["work", "<work_ref>"]` — the Work this record concerns
- `["org", "<organization_ref>"]` — owning Organization
- `["criteria", "<criterion_ref>"]` — acceptance criterion ref, repeated
- `["x", "<sha256>"]` — digest of the exact evidenced bytes
- `["url", "<location_hint>"]` — optional off-relay location hint
- `["e" | "a", ..., "<marker>"]` — typed references
- `["produced_at" | "verified_at" | "decided_at", "<unix>"]` — timestamps

## 1. Evidence Receipt (`kind:32190`)

A producer-signed record that bound material exists in support of named
criteria. Address:

```text
32190:<producer_pubkey>:<evidence_ref>
```

### 1.1 Required tags

- `d`: unique `evidence_ref`
- `work`: the target Work
- `kind_label`: evidence kind — `test_run`, `build`, `diff`, `screenshot`,
  `log_digest`, `benchmark`, `formal_model`, `replay`, `report`,
  `artifact`, or deployment-declared
- `x`: digest of the exact evidence bytes
- `produced_at`

### 1.2 Recommended tags

- `criteria`: each criterion the evidence addresses (for example a
  ProductSpec criterion ref or an Objective revision)
- `a` with marker `objective`: the NIP-WK Objective revision the evidence
  was produced against
- `a` with marker `session`: the producing Session (NIP-AS / NIP-SA)
- `url`: where the bytes can be fetched by an authorized party
- `env`: environment or target-revision ref the evidence ran against

`content` MAY carry a bounded public-safe summary. Raw logs, prompts, and
private artifacts stay off-relay behind the digest.

An Evidence Receipt proves that its signer committed to exact bytes at a
time. It does not prove the bytes are correct, complete, or sufficient —
that is verification's job.

## 2. Verification Receipt (`kind:32191`)

A verifier-signed record of an admitted evaluation of evidence against
criteria. Address:

```text
32191:<verifier_pubkey>:<verification_ref>
```

### 2.1 Required tags

- `d`: unique `verification_ref`
- `work`: the target Work
- `e` with marker `evidence`: each evaluated Evidence Receipt, repeated
- `verdict`: `passed`, `failed`, or `inconclusive`
- `policy`: the verification policy or AssuranceSpec-equivalent ref the
  evaluation ran under
- `verified_at`

### 2.2 Recommended tags

- `criteria`: the criteria evaluated
- `x`: digest of the verifier's output or report
- `reason`: typed reason on `failed` or `inconclusive`
- `env`: the environment the verification executed in

### 2.3 Independence rule

The verifier pubkey MUST differ from the producer pubkey of every Evidence
Receipt it evaluates. A receipt violating this rule is self-verification
and MUST be treated as evidence at most, never as verification. A
deployment's verification policy MAY impose stronger independence (distinct
owners, distinct runtimes, quorums); it MUST NOT weaken this floor.

Model agreement is not verification either: a panel of models concurring is
a diagnostic signal. A Verification Receipt requires executed evaluation
under the named policy.

## 3. Owner Disposition (`kind:32192`)

The accountable owner's separate decision, signed by the owner principal
named in the Work Record (or by the authority key carrying an explicit
`on_behalf_of` reference to a recorded out-of-band decision). Address:

```text
32192:<owner_pubkey>:<disposition_ref>
```

### 3.1 Required tags

- `d`: unique `disposition_ref`
- `work`: the target Work
- `decision`: `accepted`, `rejected`, `waived`, `deferred`, or `superseded`
- `decided_at`

### 3.2 Recommended tags

- `e` with marker `verification`: the Verification Receipts considered
- `e` with marker `evidence`: directly considered Evidence Receipts
- `reason`: bounded rationale ref or code
- `a` with marker `successor`: replacement Work on `superseded`

### 3.3 Boundaries

- Only the accountable human owner (or an explicitly admitted
  organizational role) records a disposition. An agent, a Delegate, a
  passing verification, a merged PR, or a completed Session cannot.
- `accepted` with a `waived` verification gap MUST say so: a disposition
  that accepts despite missing verification carries
  `["waiver", "<criterion_ref>"]` per waived criterion.
- A disposition closes the acceptance question for one Work revision. A
  later Objective revision reopens it.

## 4. Receipt Edge (`kind:32193`)

A typed edge connecting trust records into a receipt graph. Address:

```text
32193:<signer_pubkey>:<edge_ref>
```

Required tags: `d`, `edge` (one of `supports`, `verifies`, `disputes`,
`supersedes`, `settles_ref`, `attributes`), and two `e`/`a` refs with
markers `from` and `to`.

Edges let closeouts and audits traverse from an Accepted Outcome back
through disposition, verification, evidence, sessions, and settlement refs
(NIP-AC `kind:39244`, NIP-LBR closeouts, future NIP-OC) without collapsing
them into one record. An edge asserts a relation claimed by its signer; it
cannot infer missing contribution, verification, acceptance, or settlement.

## The proof-rung ladder

Clients presenting Work trust state MUST report the narrowest rung actually
reached:

```text
evidence exists          (32190)
  -> verification passed (32191, independent)
    -> owner accepted    (32192)
      -> released / publicly claimed   (separate authority, e.g. NIP-PP)
        -> settled                     (separate ledger, e.g. NIP-AC)
```

No record on this ladder implies the next. Absence of a rung is a display
fact, not an error.

## Security considerations

- **Digest discipline.** A receipt without a digest binds nothing. Clients
  MUST verify fetched evidence bytes against `x` before treating them as
  the evidenced material.
- **Key identity is not role identity.** A verifier key being distinct
  proves distinctness of keys. Deployments that need organizational
  independence must bind keys to principals through NIP-OT membership and
  Block NIP-OA attestation and check the principal, not just the key.
- **Backdated receipts.** Timestamps are claims. Where ordering matters,
  anchor receipts through NIP-WK Work Events (`evidence_attached`,
  `verification_recorded`), whose authority-assigned `seq` provides the
  order of record.
- **Disputes.** A `disputes` edge never deletes the disputed record. Both
  records and the disagreement remain visible until a new verification or
  disposition resolves them.
- **Privacy.** Evidence summaries are bounded and public-safe; sensitive
  evidence lives off-relay with access control, reachable only through the
  digest and an authorized `url`.

## References

- NIP-01, NIP-44
- NIP-WK, NIP-WI, NIP-OT (this program)
- NIP-AC: Agent Credit — settlement receipts (`kind:39244`)
- NIP-LBR: labor closeout refs
- NIP-SKL §4: attestation and assurance-tier patterns
- `docs/omega/GLOSSARY.md` §13 — evidence, verification, and claim terms

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Evidence Receipt, Verification Receipt, Owner
  Disposition, Receipt Edge, independence floor, and the proof-rung ladder.
