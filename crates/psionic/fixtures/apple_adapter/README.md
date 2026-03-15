# Apple Adapter Fixtures

This directory freezes the repo-owned Apple adapter conformance inputs introduced
for issue `#3616`.

## Scope

- `datasets/`: positive and negative JSONL training-data examples
- `packages/`: positive and negative `.fmadapter` inventory fixtures
- `lineage/`: positive and negative OpenAgents lineage payloads

## Important note about payload files

The `.bin` and `.mil` files in this fixture corpus are intentionally small
placeholder payloads. They are not executable Apple-exported weights.

Their job is to freeze:

- file inventory
- metadata shape
- compatibility anchors
- digest behavior
- rejection cases

That keeps the repo lightweight while still giving later Rust parser/writer
tests a stable corpus to validate against.

## Canonical docs

- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
- `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
