# 136. Supervised conversations and tools

> **Status: unimplemented — design draft.** Nothing here runs yet.

A base model continues text. Supervised fine-tuning (SFT) teaches it to
answer in conversations and call tools. nanochat runs SFT as a separate
stage after pretraining. This chapter builds that stage with correct loss
masks and checks that the base model's skills survive.

## What this chapter covers

- Conversation rendering through chapter 126's pinned tokenizer and
  special-token order.
- Loss masks: the loss covers assistant and tool-call tokens, not user or
  tool-result tokens.
- Mask validation on fixtures, so a mask that includes the wrong tokens
  fails.
- Bounded subsets of Nemotron-Agentic-v1 and OpenCodeReasoning, each with
  recorded provenance.
- Held-out chat evaluation.
- Base-skill regression: held-out base metrics before and after SFT.

## Dependencies

- [Chapter 126](ch126-execute-nanochat-exactly.md): the tokenizer and
  conversation format.
- [Chapter 128](ch128-a-complete-pretraining-run.md): a pretrained base
  model.
- [Chapter 120](ch120-data-and-resumable-state.md): provenance and splits.

## Done means

- Mask fixtures confirm that only assistant and tool-call tokens contribute
  to the loss.
- Held-out chat evaluation improves over the base model.
- Base-skill metrics are reported before and after, with a regression bound
  stated before training.
- Every training example traces to its dataset subset and licence.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
