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
- `historical-import.v1.json`: Episode 264 Arm A as
  `completed_incomplete` and Arm B as an unverified `source_observed` hit.

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

The historical import deliberately has no numeric wall-time or token value:
the source record did not preserve them. `unavailable` carries a reason and no
`value` property.

Run the contract and benchmark checks with:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
```
