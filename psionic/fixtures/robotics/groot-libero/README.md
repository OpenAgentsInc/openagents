# GR00T LIBERO: metadata and original-file range contract

This is the exact chapter 168 **candidate**:
[`nvidia/GR00T-N1.7-LIBERO`](https://huggingface.co/nvidia/GR00T-N1.7-LIBERO/tree/2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/libero_10),
revision `2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21`, subfolder `libero_10`.
The source/reference/license analysis is in
[the target contract](../../../docs/targets/groot.md).

The directory retains original config, processor, embodiment mapping and
safetensors index bytes. The two `*.header.json` files contain exact
HTTP 206 JSON-header bytes from the original safetensors shards, excluding
the eight-byte length prefix and **all tensor payload**. Their names deliberately
do not pretend to be complete checkpoint files. They retain original padding
spaces and are marked as binary diffs; the derived inventory is readable JSON.

- `provenance.json` records source URLs, revision, transforms, sizes and hashes.
- `tensor-inventory.json` derives all 1,031 tensor names, dtypes, shapes,
  file offsets and lengths from those headers and checks the index mapping.
- `libero-statistics.json` selects the original statistics object's
  `libero_sim` member without numeric transformation. The full original
  statistics file hash and the selection transform remain in provenance.
- `embodiment-contract.json` distinguishes eight observed state coordinates,
  seven decoded action coordinates, model width 132, model horizon 40,
  decoded horizon 16 and reference execution horizon eight.
- `category2-capture.json` records the four privately captured original
  state-encoder ranges and their independently observed component hashes.
  Component captures do not verify either complete shard.

The [pinned card](https://huggingface.co/nvidia/GR00T-N1.7-LIBERO/blob/2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21/README.md)
identifies the NVIDIA Open Model License Agreement. Its retained
[agreement PDF](NVIDIA-Open-Model-License-2025-10-24.pdf) has SHA-256
`4d2fb590aa9b30c47f2058bff17291df7fa2aa0c1bd775a20703da9bb267cfab`.
See [NOTICE](NOTICE). This evidence does not retroactively resolve the
different base checkpoint's conflicting included license.

Advertised whole-shard SHA-256 values are metadata claims until each complete
payload is downloaded and hashed. The source audit checked header range
sizes, BF16 widths, every interval, contiguous payload coverage and exact index
key/file agreement; it performed no model inference. External gated Cosmos
configuration/tokenizer/processor assets are not included here or substituted
with assumed equivalents. The owner's subsequent terms acceptance and login
resolved access: [cosmos-dependency.json](cosmos-dependency.json) pins the five
privately cached original asset names, sizes and hashes, plus derived config,
tokenizer and image-processing semantics. It contains metadata only and does
not revise the earlier header-only observation. Chapters 113 and 114 remain
incomplete until the whole original VLA and matching loop are qualified.

[embodiment-qualification.json](embodiment-qualification.json) pins the bounded
state/action converter's source hashes and independent checks.
[request-contract.json](request-contract.json) records the actual original
Cosmos token IDs, private derived table identity and bounded request reference
traces. The request implementation admits ASCII language only and refuses more
than 512 tokens; those are explicit Coax bounds, not NVIDIA truncation rules.
[request-qualification.json](request-qualification.json) pins the request source
files and checks. [image-qualification.json](image-qualification.json) records
the bounded original 256-square camera recipe, complete OpenCV pixel comparisons
and exact image source hashes. Images are synthetic arithmetic probes; original
model execution and simulated outcomes remain separate qualification gates.
[request-sdk-qualification.json](request-sdk-qualification.json) additionally
records direct installed-Transformers position/delta parity without allocating
learned weights or running a model forward pass.

The header/acquisition progress notes above retain their historical chapter
labels. Current [chapter168](../../../book/ch168-execute-a-groot-policy.md)
qualifies the prepared original learned network; [chapter169](../../../book/ch169-close-the-simulated-control-loop.md)
now drives the genuine Panda SDK loop. Its distinct
[simulation receipts](../libero-panda/README.md) separate measured task outcomes,
exact controls-only replay and retained failed hidden-state diagnostics.
