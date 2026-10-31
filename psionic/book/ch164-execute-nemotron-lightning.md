# 164. Execute Nemotron Lightning

> **Status: unimplemented — design draft.** Nothing here runs yet.

NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 combines the Mamba-2 hybrid of
chapter 163 with a mixture of experts. It has about 30B total parameters and
about 3B active per token. This chapter executes it in floating point first,
before any NVFP4 variant or speculation.

## What this chapter covers

- The pinned revision and its authenticated files.
- The hybrid layer pattern from chapter 163 combined with MoE feed-forward
  layers.
- The router from chapter 61, with stated tie rules and router-margin
  records.
- Residency planning for every expert plus scratch memory. Active parameter
  count is not total checkpoint residency.
- Floating-point execution first. NVFP4 and speculation are later, separate
  profiles.

## Dependencies

- [Chapter 163](ch163-execute-nemotron-nano.md): the hybrid model.
- [Chapter 61](ch061-a-few-experts-then-a-router.md): the router.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the memory
  plan.
- [Chapter 17](ch017-qualify-honestly.md): qualification rules.

## Done means

- The memory plan admits the complete model on a named host, or refuses with
  a stated shortfall.
- Token, router and logit references match under chapter 17's rules,
  including near-tie router margins.
- Measured memory and tokens per second are reported.

## Sources

- [Chapter plan](PLAN.md), Part XVII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Nemotron Lightning
  entry.
