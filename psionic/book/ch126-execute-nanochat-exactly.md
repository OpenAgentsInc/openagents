# 126. Execute nanochat exactly

> **Status: unimplemented — design draft.** Nothing here runs yet.

[Chapter 125](ch125-execute-the-pinned-nanochat-gpt.md) runs the pinned
nanochat `gpt.py` forward pass and loads a `torch.save` checkpoint without
executing its pickle. This chapter completes the inference profile around
that model: the pinned tokenizer, the conversation rules and cached
generation from text to text. The pin is `karpathy/nanochat` revision
`92d63d4e`.

## What this chapter covers

- The pinned BPE tokenizer. nanochat trains it with `rustbpe` and runs it
  with `tiktoken`. Its pretokenization pattern uses two-digit number groups.
  You read its merges and special tokens from an inert export, not from
  `tokenizer.pkl`.
- The conversation and tool tokens in their defined order, and the rendering
  of a conversation into token ids.
- An inert checkpoint export of tensors, configuration and tokenizer, with
  source hashes and explicit version readers.
- Every enabled forward feature from chapter 125, run from rendered text.
- Cached inference: incremental decoding equals the full pass at every
  position.
- Multi-token generation through the sampling of chapter 53.

## Dependencies

- [Chapter 9](ch009-text-becomes-a-model-request.md) and
  [chapter 123](ch123-learn-a-tokenizer.md): the byte-BPE engine and the
  nanochat pretokenization pattern.
- [Chapter 125](ch125-execute-the-pinned-nanochat-gpt.md): the pinned
  forward pass and checkpoint admission.
- [Chapter 53](ch053-choosing-the-next-token.md): seeded sampling and stop
  conditions.

## Done means

- Token ids for a fixed set of texts and conversations equal the upstream
  tokenizer's recorded ids.
- Logits for a recorded prompt match an independent reference within the
  stated profile, and cached logits equal full-pass logits.
- Greedy and seeded generation from a real exported checkpoint reproduce
  recorded upstream outputs, or the chapter reports the first divergent
  token.
- No pickle is executed at any step.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
