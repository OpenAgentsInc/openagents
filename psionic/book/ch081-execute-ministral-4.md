# 81. Execute Ministral 4

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter opens the Mistral ladder with its smallest model. The plan names
the chapter "Ministral 4"; the pinned target in the plan and the Mistral
survey is Ministral-3-3B-Instruct-2512-BF16 (Apache-2.0, BF16 originals,
15.4 GB of files). It is a dense decoder, so it exercises the shared path with
Mistral's own tokenizer and chat format.

## What this chapter covers

- The Tekken tokenizer (131,072 entries) through chapter 9's byte-BPE engine,
  with its own pinned tables.
- Mistral's chat and reasoning format.
- A dense grouped-query decoder: 26 layers, width 3072, 32 query heads and 8
  KV heads.
- Its long-context rotary scaling.
- Token, intermediate and logit references.

## What it builds

- A planned Ministral model profile on the shared dense decoder path.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): pinned original files.
- [Chapter 9](ch009-text-becomes-a-model-request.md): the byte-BPE engine.
- [Chapter 10](ch010-attention-without-training.md): grouped-query attention.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the memory plan.
- [Chapter 53](ch053-choosing-the-next-token.md): sampling.

## Done means

- Full file hashes and a complete tensor inventory are pinned.
- Tokenizer and chat-format fixtures match recorded references.
- Selected intermediate tensors and logits match references for recorded
  prompts under a tolerance frozen in advance.
- This model's evidence does not qualify any larger Mistral model.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
