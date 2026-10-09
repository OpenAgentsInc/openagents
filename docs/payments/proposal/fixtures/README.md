# Proposal fixtures

Test vectors for the proposal docs in the parent folder. Synthetic and
draft — see each file's `scope` field for exactly what is and isn't asserted.

## `buyer-attestation-v1.json`

Drives a test-first implementation of the merchant-side **`BuyerAttestation::verify`**
hook from the [merchant-facilitator API sketch](../2026-10-09-x402-merchant-facilitator-api-sketch.md) §2.
Each vector is a self-contained `(input → verdict)` case: a
[NIP-SOV](../../../../nips/openagents/NIP-SOV.md) `sovereign-profile.v1`, a
[NIP-CAP](../../../../nips/openagents/NIP-CAP.md) `grant.v1` with the `spend` effect,
a `price_msat`, `now`, and merchant policy (`trusted_authorities`, `min_reputation`),
mapped to either `ok {buyer, owner, max_per_tx_msat}` or a single `error` code.

### What the models stand in for
- **Signatures are not computed.** A control flag `signature_valid` drives the
  signature case (the existing x402 fixture likewise asserts no real signatures).
- **Admission is a trusted set.** NIP-SOV: *"self-publication cannot establish that
  trust."* So the merchant supplies `trusted_authorities`; a profile whose
  `authority` isn't in it fails. A production verifier replaces this with real
  host-admission evidence.
- **ArtifactRefs/digests are placeholders.** No POL/CAP/SOV artifact bytes or
  RFC8785 canonicalization is asserted.
- **Ceiling source** for the `over_ceiling` check is `grant.bounds.max_per_tx_msat`.
  A full verifier also cross-checks the SOV treasury aggregate allowance and
  reconciliation authority — out of scope for these single-call vectors.

### Fail-fast evaluation order
The verifier applies checks in this order; the first failure wins (each negative
vector isolates exactly one failing condition, so order is unambiguous here):

| # | Check | Error if it fails |
|---|---|---|
| 1 | attestation present (profile + grant + signature) | `missing` |
| 2 | signature valid over the bound request | `signature` |
| 3 | `profile.authority` ∈ merchant `trusted_authorities` | `authority` |
| 4 | `grant.principal == profile.agent` | `principal_mismatch` |
| 5 | `grant.purpose == "operational"` | `purpose_not_operational` |
| 6 | `"spend"` ∈ `grant.effects` | `effect_absent` |
| 7 | `now < grant.expires_at` | `expired` |
| 8 | `price_msat <= grant.bounds.max_per_tx_msat` | `over_ceiling` |
| 9 | `reputation >= min_reputation` | `reputation_too_low` |
| 10 | all passed | → `ok {buyer, owner, max_per_tx_msat}` |

### Vectors and expected verdicts

| Vector | Verdict | Exercises |
|---|---|---|
| `ok-minimal` | `ok` | happy path, no reputation gate |
| `ok-reputation-met` | `ok` | reputation 80 ≥ required 50; extra `network` effect alongside `spend` |
| `err-missing` | `missing` | no attestation → refuse before issuing an invoice |
| `err-signature` | `signature` | signature doesn't verify |
| `err-authority-untrusted` | `authority` | authority not admitted/trusted (self-publication) |
| `err-principal-mismatch` | `principal_mismatch` | grant issued to a different agent |
| `err-lineage-rotated-key` | `principal_mismatch` | rotated agent key; grant not re-authorized to the new identity |
| `err-purpose-evaluation` | `purpose_not_operational` | arena/eval grant can't buy real goods |
| `err-effect-absent` | `effect_absent` | grant lacks the `spend` effect |
| `err-expired` | `expired` | `now ≥ expires_at` |
| `err-over-ceiling` | `over_ceiling` | price 60,000 sats > ceiling 50,000 sats |
| `err-reputation-too-low` | `reputation_too_low` | reputation 30 < required 50 |

### Note for the API sketch
Two vectors (`purpose_not_operational`, `effect_absent`, and the
`principal_mismatch`/lineage pair) exercise checks beyond the error enum currently
in the sketch's `AttestationError`. Adopting this fixture means **extending that enum**
to the nine-code vocabulary in the fixture's `error_vocabulary` — the recommended set.

### Using it
Load the file, run each vector's `input` through `verify`, and assert the result
equals `expect`. Pseudocode:

```
for v in fixture.vectors:
    got = verify(v.input)                     # Ok(Attestation) | Err(code)
    assert got == v.expect                    # ok{...} or error "<code>"
```
