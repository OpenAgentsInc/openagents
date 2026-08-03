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
- `generator-vectors.v1.json`: frozen vulnerable, 32-bit-reseeded, approved-
  provider, and guard/provider/initialization/call-trace/reseed mutation
  vectors, plus a synthetic public-material match and a work-factor throughput
  measurement recorded as a candidate count over an elapsed interval rather
  than as a written-in rate. Every vector declares
  `goldenVectorSource.kind = "self_generated"` and
  `provenance.kind = "conformance_vector"`: their expected digests came from
  the reproduction they test, so passing them locks our generator against
  drift and proves nothing about the target's generator.
  `admitColdcardGeneratorEvidence` refuses the file. It contains no mnemonic,
  xprv, or live value oracle input; and
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

Run the contract and benchmark checks with:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
```
