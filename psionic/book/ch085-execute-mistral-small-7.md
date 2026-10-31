# 85. Execute Mistral Small 7

> **Status: unimplemented — design draft.** Nothing here runs yet.

The plan names this chapter "Mistral Small 7"; its pinned target is
Mistral-Small-4-119B-2603 (Apache-2.0, FP8, 241.9 GB of originals). It joins
latent attention with a mixture of experts: 128 routed experts with four
selected per token, plus one shared expert.

## What this chapter covers

- Latent attention from chapter 84 (KV rank 256, query rank 1024).
- 128 routed experts, top four, and one shared expert, extending chapter 61's
  router.
- FP8 originals from chapter 82.
- Residency on one large-memory machine, or expert paging in the manner of
  chapter 73.
- References for selected layers and tokens, including router choices.

## Dependencies

- [Chapter 61](ch061-a-few-experts-then-a-router.md): the router.
- [Chapter 82](ch082-original-fp8-weights.md): FP8 weights.
- [Chapter 84](ch084-attention-through-a-latent.md): latent attention.
- [Chapter 73](ch073-execute-gpt-oss-120b.md): the large-model CPU pattern.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the residency
  plan.

## Done means

- File hashes and the complete tensor inventory are pinned.
- Router choices, intermediates and logits match references for selected
  layers and tokens.
- Resident memory is measured on the named machine, or the paging profile is
  named and measured.
- Ministral 3 evidence is not counted for this model.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
