# Coldcard forensic development benchmark

This directory is the checked-in OFR-004 benchmark pack. It is development
data, not an untouched security holdout and not evidence that any later
reproduction suite has run.

The pack contains:

- `benchmark-manifest.v1.json`: five arms, the frozen six-link source rubric,
  budgets, split ownership, and content digests for every subordinate file;
- `reproduction-manifest.v1.json`: vulnerable, fixed, dependency, and pinned
  postmortem revisions plus the eight-rung claim lattice;
- `arm-fixtures.v1.json`: dependency presence, five semantic transformations,
  and three clean-control cases;
- `dataset-splits.v1.json`: separately owned train, development, untouched
  holdout, and clean-holdout descriptors;
- four `suite-*.v1.json` manifests for code-to-artifact,
  generator/owned-fixture, historical fingerprint, and evidence-graph work;
  inputs owned by later issues remain `required_unmaterialized`, never zero or
  silently present; and
- `artifact-witness-fixtures.v1.json`: deterministic vulnerable, fixed, and
  provider-removal fault-build capture fixtures for the OFR-014 evaluator.
  Every capture in the file declares `provenance.kind = "conformance_vector"`,
  so it exercises the schema and the evaluator and nothing else. Its digests
  are placeholders, no Coldcard firmware build produced them, and
  `evaluateColdcardArtifactWitnessSuite` refuses the file with
  `blocker.artifact_witness.provenance_not_admitted`. An acceptance-level
  artifact-witness claim requires captures whose provenance is an
  `admitted_worker_run` bound to one exact OpenAgents Cloud managed-sandbox
  generation, guest image digest, and its emitted receipts; and

- `artifact-witness-live-run.v1.json` plus
  `artifact-witness-live-captures.v1.json.gz`: **three real Coldcard MK4
  firmware builds**, not conformance values. Each capture was produced by
  `scripts/cloud/coldcard-build-driver.mjs` running inside its own admitted
  OpenAgents Cloud `live_gce` managed sandbox on guest image
  `oa-msb-guest-coldcard-9296-v2`, which carries `arm-none-eabi-gcc 12.2.1`
  and the two pinned firmware trees. The run file holds the per-variant
  assertions and the toolchain and mutation description; the gzipped file
  holds the captures themselves, including every artifact digest, the
  enumerated symbol inventory of every object the link consumed, the measured
  call edges, and the preprocessed macro value. The provenance block on each
  capture is stamped by
  `scripts/cloud/coldcard-artifact-witness-live.ts` from the managed-sandbox
  runtime receipts it observed, never by the guest: a guest cannot attest to
  its own admission. `evaluateColdcardArtifactWitnessSuite` returns `Verified`
  for this set, and `packages/forensic-contract/test/coldcard-artifact-witness-live.test.ts`
  also checks that the same assertions applied to the wrong build are
  violated, so the file cannot become a rubber stamp; and
- `generator-vectors.v1.json` and `generator-live-capture.v1.json.gz`: frozen
  vulnerable, 32-bit-reseeded, approved-provider, and
  guard/provider/initialization/call-trace/reseed mutation vectors, plus a
  synthetic public-material match and a work-factor throughput measurement
  recorded as a candidate count over an elapsed interval rather than as a
  written-in rate. Every vector's expected digests were produced by libngu's
  own `ngu/random.c` at commit `537519a8` — the submodule revision both pinned
  Coldcard firmware trees carry — compiled verbatim and executed inside an
  admitted `live_gce` managed sandbox, so `goldenVectorSource.kind` is
  `independent_implementation` and `provenance.kind` is `admitted_worker_run`.
  The `.gz` file is that capture: per vector it holds the target pass through
  the pinned `my_random_bytes` and `_rand_below`, a single-step mirror of the
  same compiled functions, and the agreement between them, which the guest
  requires before it will emit a vector at all.
  `packages/forensic-contract/test/coldcard-generator-live.test.ts` repackages
  the capture and refuses any drift between it and the corpus, and
  `admitColdcardGeneratorEvidence` now admits the file. It contains no
  mnemonic, xprv, or live value oracle input; and
- `historical-scan-fixture.v2.json` plus the three
  `historical-bundle-*.v1.json.gz` files: **frozen mainnet chain data**, not a
  synthetic scenario. Blocks 960,189, 960,359, and 960,365-960,367 were
  extracted read-only from our own archival Bitcoin Core node (`oa-bitcoind`,
  unpruned, `txindex=1`, `disablewallet=1`) with
  `extract-historical-bundle.py`, using only `getblockchaininfo`,
  `getblockhash`, `getblock`, and `getrawtransaction`. Between them the blocks
  carry all eight known-positive transactions the Coldcard postmortem
  published; the fixture pins each bundle's content digest, its uncompressed
  canonical-byte digest, and its compressed-byte digest, so the checked-in
  bytes cannot drift from what the node answered. The capability record holds
  no RPC endpoint, cookie, credential, wallet method, external IP, or
  live-wallet query — only an opaque node-identity digest and server binding
  ref; and
- `historical-wide-scan-ledger.v1.json`: the per-block record of the
  1,701-block, 7,122,744-transaction scan run on the same node on 2026-08-01,
  with the block hash, eligible-transaction count, match count, and
  prevout-error count for every block, folded into three eras and a fee-rate
  histogram. Its per-million figures are recomputed from those counts rather
  than asserted, and each era pins the digest of the append-only raw artifact
  it was folded from; and
- `extract-historical-bundle.py`: the read-only extractor that produced the
  bundles. Exact integer satoshi arithmetic throughout, no float, no wallet
  RPC, no node credential in its output; and
- `historical-import.v1.json`: Episode 264 Arm A as
  `completed_incomplete` and Arm B as an unverified `source_observed` hit.
- `evidence-derivation-fixture.v1.json`: a constructed boundary fixture for the
  OFR-017 evidence graph — dust exactly at the threshold, an explicit change
  output, a victim-confirmed report, and a traversal that reaches its bound.
  Retained deliberately, because real evidence does not supply boundary cases on
  demand. It is boundary construction, never measurement, and contains no
  private keys or live-wallet oracle.
- `evidence-derivation-incident-wave6.v1.json`: the **real** Coldcard wave-6
  incident, blocks 960,359 and 960,367, extracted read-only from our own
  archival node. Seeds are thirteen third-party-published addresses at
  `published_unconfirmed` and thirteen fingerprint candidates carried from the
  frozen OFR-016 bundles with their content and revision digests. There are no
  victim reports in it: nobody has reported to us, and a published address is
  not a victim report. Everything else — spending transactions, destinations,
  values, components, episodes — is derived from chain records.
- `extract-incident-transactions.py`: the read-only extractor that produced the
  incident fixture. It keeps a transaction because the chain says one of its
  prevouts belongs to a known address, never because the transaction looked like
  a sweep; and
- `coldcard-published-figures.v1.json`: independently published figures
  transcribed from the postmortem author's own dataset at a pinned commit and
  file digest (`Kelbie/coldcard-rng-postmortem`, `src/data/chain.json`). Nothing
  in it is vendored and nothing in it is ours; every reconciliation item must
  name that digest, so the "published" column cannot become a number the
  comparison's own author chose.

## Tree digest procedure

Every repository revision uses `sha256_git_ls_tree_r_z_v1`: the lowercase
SHA-256 of the exact byte stream emitted by:

```sh
git -C <repository> ls-tree -r -z <40-character-commit> | shasum -a 256
```

The stream binds path, mode, object type, and Git object identity recursively
without depending on checkout timestamps or local filesystem metadata. The
materializer must still verify actual source-object bytes; this tree pin does
not replace OFR-003 byte receipts.

## Contamination boundary

Postmortem-generated outputs are named only by `expectedComparisonRefs`.
Schemas reject any suite that also places one of those refs in its source or
evaluator inputs. Coldcard and its visible variants remain optimizer-visible
development data. The two evaluator-only holdout descriptors have different
owners and digests and contain no Coldcard arm refs.

## Frozen chain evidence, and what it cannot carry

The bundles and the wide-scan ledger are real mainnet data, which changes what
a green test proves and what it does not.

It proves that the published fingerprint reproduces from chain bytes nobody in
this repository controls, that a one-vbyte perturbation of the fee table
destroys every published positive, and that the false-match denominator is high
enough to bound what the fingerprint can support: 2,820-6,154 per million at
2 sat/vB and 24-701 per million at 30 sat/vB.

It does not prove that any matched transaction is a theft, an attacker
transaction, or connected to Coldcard at all. Every match is a program-
similarity candidate. The claim ceiling on every report and on the ledger is
`program_similarity_only`, and it is the schema that enforces it.

The three bundles are scanned under a *narrowed* fingerprint revision (version
2, single P2WPKH output, uniform table-listed input type, observed sequences).
The wide-scan ledger was produced under the *published* revision, which applies
only the non-coinbase, zero-locktime, table-typed, exact-integer-rate rule.
Both revision objects are carried in `historical-scan-fixture.v2.json` and both
digests are bound to the artifacts that used them, so the two numbers are never
silently compared.

The raw append-only hit files behind the ledger are 62 MB and stay out of the
repository. They are frozen content-addressed under
`gs://openagentsgemini-oa-artifacts/forensics/coldcard/ofr-016/`, keyed by the
same SHA-256 digests the ledger records.

The historical import deliberately has no numeric wall-time or token value:
the source record did not preserve them. `unavailable` carries a reason and no
`value` property.

## Loupe verification (OFR-007)

`loupe-control-plane-transcript.v1.json.gz` is the recorded wire traffic of a
live Loupe verification: every request the verifier sent to the staging
managed-sandbox control plane and every response it got back, in order, plus the
clock readings it took. It contains no plan, no evidence receipt, no verdict and
no worker receipt, because the verifier derives all of those. Replaying it
re-derives the whole verification, and the replay is exact: the recorded control
plane matches on request identity, so a driver that would ask a different
question is refused rather than handed the old answer.

`loupe-first-verdict-ledger/` is the durable first-verdict ledger. One file per
verification, created with `O_CREAT | O_EXCL`, so the first verdict written for
a verification is the one every later reader gets. `loupe-verification-live-run.v2.json`
is the human-readable summary the run derived, including the spec it was given
and the measured Google Cloud cost.

`loupe-verification-live-run.v1.json` and `loupe-first-verdict-ledger.v1.json`
are the previous run, retained as the historical record referenced by
openagents#9294. They belong to the shape where the verifier was handed its
evidence, and nothing reads them.

Run the contract and benchmark checks with:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
```
