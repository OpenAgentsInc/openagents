# 268. Execute Inkling 2 Mini on CPU

> **Status: unimplemented — design draft.** This chapter specifies full CPU
> execution of the original Inkling 2 Mini checkpoint, including tokenization,
> chat template formatting, and logit/decode parity. No full-model CPU run is
> implemented here yet.

Paralleling [chapter 64](ch064-execute-gpt-oss-20b.md), this chapter
assembles the NVFP4/MXFP8 weight loader from
[chapter 266](ch266-original-nvfp4-and-mxfp8-weights.md) and the 3-pass looped
MoE runner from [chapter 267](ch267-a-looped-moe-transformer.md) to execute the
actual ~20 GB Inkling 2 Mini checkpoint on CPU.

CPU execution is not the interactive target—it is the bit-auditable reference
that Metal kernels in [chapter 269](ch269-inkling-2-mini-on-metal.md) must
match.

## What the chapter will build

1. **Tokenizer and chat template.** Load Inkling 2 Mini's tokenizer and
   special tokens, formatting system, user, assistant, thinking/CoT and tool
   turns identically to the upstream template.
2. **Streaming layer-by-layer CPU forward pass.** Run the 26.4B total / 3.4B
   active parameter graph on CPU with explicit memory bounds so a 32 GB machine
   can verify single-prompt logits without swapping out.
3. **Logit and greedy-decode verification.** Record top-k logits, per-pass
   router selections, and a deterministic greedy continuation on fixed prompts.

## Acceptance checks

- **Worked trace:** show exact token IDs for a short prompt with system,
  user and tool-definition turns, and verify the first generated token.
- **Independent reference:** match top-k logits and greedy-decoded tokens
  against an independent reference runtime (llama.cpp / vLLM / SGLang) on the
  same pinned checkpoint revision.
- **Refused inputs:** reject truncated shards, unknown special tokens, and
  mismatched `config.json` loop counts.
