# Raw peer coordinate admission

`smc_multiply` is the sole exported variable-base entry in `secp_mult_const.coax`. It accepts a public peer and now examines all three coordinates before `sg_valid_mask` and before the eagerly evaluated `sg_normalize(peer)`. Each of the first nine limbs of each coordinate must be at most `8*(2^26-1)` and each top limb at most `8*(2^22-1)`. These inclusive bounds match the reviewed field magnitude-eight premise. An oversized coordinate now returns `ok=false` and canonical infinity `(1,1,0)` without entering either field operation. Before this change oversized raw coordinates were outside the documented contract and could abort or overflow; this is an explicit refusal for those inputs.

The entry retains its public ABI, the bounded input path, and all existing complete-mask point selection. An independent source review confirmed the bounds and their placement before arithmetic. The new admission branch depends on the peer, which is required to be public. The existing all-parameters-secret source auditor correctly refuses this source with `secret-branch`; it is not evidence of an audit with a public peer and secret scalar. ECDH peer validation, a properly classified source audit, target timing, erasure, and production-secret approval remain separate.

## Evidence

The [machine-readable report](2026-10-29-secp-peer-admission.json) pins the tested source closure, compiler, test drivers, generated C, native and Wasm artifacts, source maps and target tool identities. A checked Coax test run passed 83/83 checks, including each coordinate/limb's inclusive boundary and refusal of oversized x, y and z at the exported entry.

For each of the checked and release compiler modes, the independent native and Wasm boundary harness compared 65 bounded inputs to the pre-change implementation, including generator, infinity, malformed points, both sides of each inclusive boundary, and the simultaneous magnitude-eight maximum. It separately checked 60 oversized inputs: the first outside value and maximum `u64` at every coordinate and limb both refused as canonical infinity. Wasm checks also guard the output buffer and confirm the input peer is unchanged. Native calls use the actual exported struct ABI. The generated headers matched after excluding only the first comment line identifying the source path. The test verifies compiler-recorded source map hashes against the current closure and the copied baseline closure. The baseline is the byte-identical original source at commit `c3484fd00e29ceb3d43b924138577c5bf434911a`; temporary generated artifacts are identified by hash but not retained in the repository.

The fixture serializes each 64-bit limb as a decimal string for Wasm so JavaScript cannot round it through a floating-point number. An earlier test-driver attempt sent maximum-width values as JSON numbers and produced a spurious Wasm mismatch; that driver error was corrected before the recorded run. The corrected run tests the intended `u64` values.

Reproduce with the pinned compiler and the native and WASI toolchain described in the report:

```sh
coax/bin/coax test coax/lib/crypto/secp_mult_const.coax
COAX_PEER_REPORT=/tmp/coax-peer-admission.json python3 coax/tests/nostr/secp_peer_admission.py
coax/bin/coax mutate coax/lib/crypto/secp_mult_const.coax --mutation-stats
python3 tools/coax-nostr/secret-check/aggregate.py coax/lib/crypto/secp_mult_const.coax --entry smc_multiply --report /tmp/coax-peer-secret-audit.json
```

The full authority mutation run killed 1,353 of 1,353 mutants with no survivors. Its scheduler ran 2,917 selective checks and skipped 39,688 unrelated checks, with no borrowed or unstable coverage; the complete mutant set was retained. The final command is expected to refuse under its all-parameters-secret classification; no successful classified secret-scalar audit is claimed here. The earlier conditional field proof pins the pre-change variable-base source and must be independently revalidated against this changed caller before it is used for the revised root.
