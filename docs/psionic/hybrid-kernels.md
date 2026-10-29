# Hybrid forward primitives: progress toward chapter 7

The chapter 7 model path has three bounded Coax primitives and separately
composed causal linear and full-attention layers. These are building blocks for
the pinned Elephant and Mammoth checkpoints, not yet a complete model,
decision service, or independently qualified upstream runtime. The existing [chapter 6](../../psionic/book/ch06-attention-without-training.md)
remains an educational dense block with its own different rotary convention.

- [delta.coax](../../psionic/lib/delta.coax) implements width-four causal
  depthwise convolution with SiLU and the grouped Q/K gated delta recurrence.
  Its caller supplies already projected Q/K/V, gate, beta, decay and norm
  inputs as one packed span. It stages and publishes convolution history,
  recurrent state and output together on success.
- [qwen-attention.coax](../../psionic/lib/qwen-attention.coax) prepares
  per-head query/gate, normalized key/value and partial split-half rotary
  coordinates; it then provides grouped, masked, sigmoid-gated attention.
  The caller supplies explicit positional coefficients and a mask.
- [qwen-matvec.coax](../../psionic/lib/qwen-matvec.coax) projects original
  unaligned little-endian BF16 or F32 rows against finite F32 input. It checks
  all original encodings, stages output and uses compensated binary64 products
  and accumulation. It accepts up to 32,768 rows and columns, enough for
  the individual layer projections; embedding and readout can select rows.

The source comparison is the [Transformers 5.3.0 Qwen3.5 implementation](https://github.com/huggingface/transformers/blob/v5.3.0/src/transformers/models/qwen3_5/modeling_qwen3_5.py),
read as an implementation reference. In particular, its query projection
interleaves a query and gate **within each head**; Q/K use `(1 + weight)` RMS
scales; rotary is split-half and may rotate only part of a head. The delta path
normalizes Q/K, repeats them across value heads, applies decay before the delta
update and performs gated RMS normalization after recurrence. The Coax CPU
profile uses wide compensated arithmetic and explicit F32 staging; it does not
claim bitwise parity with PyTorch/BF16 kernels or a captured upstream run.

The delta primitive admits up to 16 key heads, 48 value heads, dimensions 128,
and width-four convolution. Its maximum recurrent matrix holds 786,432 F32
values. Value heads must divide evenly into key-head groups. The full-attention
primitive admits up to 32 query heads, 16 KV heads, dimension 256, and 16,384
visible positions. Its explicit byte mask permits future decision-mask work;
masking itself does not yet implement branch isolation. Each profile refuses
non-finite operands, invalid dimensions, short spans and overlaps; it never
truncates. Delta `A_log` above 80 is refused in this profile. Callers retain
truthful extents, lifetime, alignment and exclusive access; C observes address
ranges only.

[The owned delta oracle](../../psionic/tests/delta_oracle.py) computes a
three-token scalar reference with nonzero initial convolution and recurrent
states. [The attention oracle](../../psionic/tests/qwen_attention_oracle.py)
checks split-half rotary preparation, grouped attention, masks and output
gates. Native tests exercise actual storage, aliases and non-finite values.
The [verification record](../verification/psionic-hybrid-2026-10-29.md)
contains the precise test scope.

[Chapter 7](../../psionic/book/ch07-a-hybrid-sequence-model.md) composes the
causal layers and documents their state. Still required are model tensor
mapping, causal full-model execution, independent upstream layer comparisons,
and general combined attention/recurrent state handling. Chapters 8–11 need
the decision mask, branch forks,
readout, complete original checkpoint execution and serving. Neither these
kernels nor partially downloaded checkpoints prove those outcomes.
