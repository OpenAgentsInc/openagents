# Private semantic comparison

`psionic/lib/mesh/compare.coax` is the pure Chapter21 comparison policy.
It takes the original and replayed Chapter16 response strings, the same
admitted request and its still-live pools, and checker-owned configuration:

```coax
comparison := mc_compare(original, replica, request, pools,
    exact_identity, "centred-log-f64-max8-v1",
    configured_compatible_backend, maximum_distance_ppb);
```

The result has `verdict` (1 pass, 2 fail, 3 inconclusive), `reason`, `worst`
distance, and `question` (first index attaining that distance, or -1 when
inconclusive). Reason constants are `MC_OK`, `MC_CONTEXT`, `MC_REQUEST`,
`MC_ORIGINAL`, `MC_REPLICA`, `MC_DECISION` and `MC_DISTANCE`.

Unconfigured Coax bools initialize false; the caller passes this bool
explicitly after approving a backend scope. It is **not** decoded from a witness, receipt,
beacon, result or provider claim. The Chapter21 checker integration requires an
explicitly configured `psionic-toy-cpu-v1` backend scope and its exact pinned
toy identity, and records the configured profile in verdict metadata. This is
operator-approved comparison scope. It is neither hardware/model attestation
nor a measured tolerance calibration. Other models and backend combinations
need an explicit future policy.

Both sides must carry the exact configured `service.identity` and its model
name. The comparator projects only model, answers and identity, then uses
the immutable Chapter20 `mf_answer` validator with a local-shaped service.
Routes, provider names, latency, usage and request/attempt metadata do not
affect semantic equality. Signature, ciphertext, provider, request/attempt,
receipt and witness binding remain the private wire reader's responsibility;
the projection does not replace them. All strings and pools must stay valid
and immutable for this synchronous call. Each semantic projection has a
262,144-byte scratch limit; overflow is inconclusive.

Malformed or missing fields, duplicates of semantic keys, wrong identities,
invalid question counts, unsupported profile/shape, unknown backend context,
and invalid configuration return inconclusive. They are never treated as
matching answers. Validation precedes tolerance and logarithms: confidence,
noul, every probability and each score's type-dependent physical bounds
remain strict even at the largest configured comparison tolerance. The
existing emitted decimal profile has up to nine fractional digits and no
exponent notation.

For supported valid responses:

- Choice and score retain the validated discrete selection. Different
  selections fail even if the numerical distance is otherwise allowed.
- Choice/score vectors have 2–8 entries in admitted request order. The broader
  Chapter16 interface's 9/10-level scores and larger choices are inconclusive
  in this first bounded comparison profile.
- Noul is the Bernoulli vector `[noul, 1-noul]`.
- For each vector, replace values below 0.000001 with that floor, take binary64
  natural logs, subtract their fixed-order mean, and compare the largest
  absolute coordinate difference. This is the bounded Chapter160 primitive;
  importing the simulated-round experiment is unnecessary.
- The worst question distance must be at most
  `maximum_distance_ppb / 1_000_000_000`. The configured integer range is
  0–1,000,000,000, inclusive. Zero requires exact equality of the computed
  centred logs; this is appropriate for the deliberately pinned toy demo,
  not an assertion about real multi-backend numerical noise.

A failure is a statement about this configured private replay comparison,
not proof of intent. Missing evidence, unsupported configurations, and
unavailable replays stay inconclusive. No plaintext prompt/result or
compatibility claim needs to be published by this unit.

Run the synthetic consumer with `coax run psionic/examples/mesh-compare.coax`.
The example never signs or publishes a verdict. Its ordinary emitted C is
also exercised under ASan/UBSan by
`psionic/tests/mesh-compare-native.sh OUT_DIR`. The policy's source checks
include an independent hand-derived log-distance reference, all three answer
kinds, exact bounds, semantic metadata variation, malformed inputs and a
poisoned compatibility claim. Root-only mutation qualifies this new unit;
it does not requalify the unchanged Chapter20 validators or crypto.
