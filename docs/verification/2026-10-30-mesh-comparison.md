# Chapter21 private semantic comparison qualification

New scope: `psionic/lib/mesh/compare.coax`, its synthetic native consumer,
reader documentation and scoped harness. Base: `adefd63fbc`. No changes to
Chapter20 validation, cryptography, keys, signing, public receipts or money.

The pure checker compares choice, noul and score answers under the explicit
`centred-log-f64-max8-v1` profile. Configuration (exact identity, backend
compatibility, profile and inclusive PPB threshold) belongs to the checker.
An unknown backend or unsupported/invalid input is inconclusive. A witness
claiming compatibility cannot override checker configuration. Metadata
variation cannot manufacture a semantic mismatch. The wire/witness reader
still owns cryptographic and request/attempt admission.

Results on the final frozen source:

- 216/216 baseline checks, including the unchanged dependency checks.
- 168/168 entry-source procedure mutants killed; zero survivors, no waivers.
  The first scoped run exposed four missing checks around a genuinely
  oversized question span and exact/empty noul output capacity. Those checks
  were added before final qualification. No policy weakening or equivalent
  classifications were used.
- Actual emitted C consumer passes optimized native and ASan/UBSan builds,
  with byte-identical output. It exercises configured/unknown backend scope,
  a hand-derived nonzero choice distance, noul, a valid three-level score of
  1.5, and a type-aware out-of-range score becoming inconclusive.
- Source checks also cover choice and score selection disagreements, exact
  floor/threshold edges, malformed identities/numbers/duplicates, reordered
  and escaped semantic keys, altered route/provider/timing/usage/correlation,
  and a poisoned compatibility field.

Commands (all completed below 15 minutes):

```sh
coax/bin/coax test psionic/lib/mesh/compare.coax --library --run-steps 250000000
coax/bin/coax mutate psionic/lib/mesh/compare.coax --library --root-only --jobs 4 --run-steps 250000000
psionic/tests/mesh-compare-native.sh /tmp/mesh-compare-native-qualified
```

Compiler: `coax0 0.24.0`, installed from the qualified checked cache with
source fingerprint
`81ad04b7877f29c0f424154fb27589c1b1839ef1d05ed28d724c488fc72f28c5`.
Final source/artifact hashes are in
[`data/oct30-mesh-comparison/qualification.json`](data/oct30-mesh-comparison/qualification.json),
with actual baseline, first mutation gaps, final mutation and native logs.

This qualifies private comparison policy only. It makes no model/backend
attestation, empirical noise calibration, public relay/replay, receipt,
production-key or signer-audit claim. The full signed Chapter21 integration
has its independent end-to-end record.
