# From checkpoint inventory to a hybrid block

This records the original-weight building blocks used by
[chapter 10](../book/ch010-execute-elephant.md) and
[chapter 11](../book/ch011-execute-mammoth.md), which exercise one original-weight
text decision each for Elephant and Mammoth.

The separate rank-five inventory and bundle code retains all five extents,
including Elephant's `model.visual.patch_embed.proj.weight` shape
`[1152, 3, 2, 16, 16]`. It admits the entire BF16/F32 payload: unique names,
valid shape products, disjoint ranges and complete byte coverage. Bundle
binding verifies original config/tokenizer/shard hashes, the full trusted
name/dtype/shape/shard inventory and, when present, the complete shard index.
The existing chapter 4 `original-weights-v1` contract still admits rank 0–4.
The new inventory's borrowed names and raw bytes must remain immutable and
alive; its caller supplies and authenticates the trusted pins. Draft metadata
may change on refusal but public inventory is only published after success.

The [owned manifest tools](../tests/elephant_manifest.py) and
[Mammoth tool](../tests/mammoth_manifest.py) require
the pinned original size and SHA-256 before generating or checking the
[Elephant inventory](../fixtures/models/elephant/manifest.json) and
[Mammoth inventory](../fixtures/models/mammoth/manifest.json).
This small reviewable inventory is metadata, not checkpoint weights.
`--header` produces an immutable C data table from that fixture for the
[real-file harness](../tests/elephant_bundle.c). The harness maps
original bytes; Coax performs parsing, hashing and admission. Supplying an
inventory generated from an unauthenticated file would not establish identity.

The [hybrid block executor](../lib/qwen-model.coax) runs an ordered
list of linear and full layers over one block. Linear layers advance once per
token; full layers see every token in the block and the earlier KV prefix.
It stages the linear state, the live prefix and new cache entries, and two
alternating activation blocks. It publishes output and new state only after
all layers succeed. Unused cache tails stay unchanged. The caller supplies
layer weights, correctly positioned rotary coefficients, coherent shapes,
valid storage and model identity; this executor does not authenticate them.
The workspace reserves both layer workspaces rather than choosing a maximum.

The independent references cover nonzero recurrent execution and a nonzero
cached full-attention suffix, including multiple layer and prefix boundaries.
The native adapter observes nested aliases, but callers retain the pointer,
extent, alignment, lifetime and exclusive access obligations. Those component
checks alone do not establish original-weight numerical parity, resource
feasibility, request serialization, or a serving API. The separate
[chapter 10 record](../../docs/verification/psionic-ch10-2026-10-29.md) describes
the real-model comparisons and their limits.

The [layer mapper](../lib/model-io/layer-map.coax) checks the exact
names and dimensions of the four-layer Qwen schedule before borrowing BF16
matrices and decoding its small vectors. It does not authenticate the inventory;
that must happen first. All 32 Elephant layers and all 64 Mammoth layers have
been mapped from their original pinned inventories.

The [token block](../lib/qwen-tokens.coax) gathers original-format
embeddings, runs the complete hybrid block, normalizes the final token, projects
the entire vocabulary in bounded chunks and performs slot readout. It stages
state until the full head and readout succeed. The separate
[text rotary generator](../lib/qwen-rope.coax) implements the pinned
text-only theta and 64 rotated dimensions. Callers must bind the exact model,
weights, tokenizer, slot IDs, positions and branch state; neither primitive
is a complete decision request or service. The chapter composes these
primitives with authenticated session binding and typed request preparation.
