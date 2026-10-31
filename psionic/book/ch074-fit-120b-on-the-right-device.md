# 74. Fit 120b on the right device

> **Status: unimplemented — design draft.** Nothing here runs yet.

gpt-oss-120b has 36 layers and 128 experts, and its original files total about
65 GB. The RTX 4080's 16 GB cannot hold them. This chapter runs the same device
engine on hardware with enough resident memory.

## What this chapter covers

- Option one: the Metal engine of chapter 72 on the 128 GiB Mac, with every
  weight resident.
- Option two: an explicitly available larger CUDA device, or several CUDA
  devices.
- The same frozen comparison rules as the 20b device chapters, against
  chapter 73's CPU references.
- Measured resident memory and headroom on the device that ran.
- Hardware purchases and new hosted machines are owner decisions, recorded in
  `NEEDS_OWNER.md`, not chapter work.

## Dependencies

- [Chapter 72](ch072-gpt-oss-on-metal.md): the Metal engine.
- [Chapter 73](ch073-execute-gpt-oss-120b.md): the 120b CPU reference.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the residency
  plan.

## Done means

- The residency plan admits the complete original 120b payload on the chosen
  device, and actual allocation succeeds with measured headroom.
- Logits and routes meet the tolerance frozen before comparison with
  chapter 73's recorded references.
- Greedy token ids equal the CPU engine's on the fixture prompts.
- Decode and prefill rates are reported for the device that ran.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 9.
- [gpt-oss target](../docs/targets/gpt-oss.md): 120b shape and pinned files.
