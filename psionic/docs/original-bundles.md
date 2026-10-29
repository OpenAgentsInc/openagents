# Pinned original decision-model inventories

Psionic's separate rank-five bundle profile admits complete Safetensors shards,
including the vision tensor in the original decision checkpoints. It verifies
caller-provided config, tokenizer, index and shard SHA-256 pins; scans every
name, dtype, dimension and payload range; checks complete byte coverage and
compares the full inventory against trusted expected specs. For sharded bundles
it validates every index entry and, when provided, both `total_size` and
`total_parameters`. The original chapter 4 rank-four contract is unchanged.

The reviewable [Elephant inventory](../fixtures/models/elephant/manifest.json)
and [Mammoth inventory](../fixtures/models/mammoth/manifest.json)
are metadata derived from the exact pinned upstream originals after checking
all file hashes. Their owned generator/checkers require the original files as
inputs; `--header` produces a C data table for the real-file test harnesses.
They do not download or run model code. The large original weights are not
stored in this repository.

Inventory publication is atomic after complete admission. Names borrow the
original header and raw tensor views borrow shard storage. Callers retain
immutable bytes, truthful pins, live aligned allocations and exclusive access
for writable storage. Binding a digest or expected inventory supplied by an
untrusted source does not establish authenticity.

The [verification record](../../docs/verification/psionic-original-bundles-2026-10-29.md)
identifies exact checkpoints, tested machines and the distinction between
full-file admission, index checks and real model execution.
