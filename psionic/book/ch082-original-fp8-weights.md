# 82. Original FP8 weights

> **Status: unimplemented — design draft.** Nothing here runs yet.

Several Mistral releases ship original weights in FP8 E4M3 with a scale per
block. This chapter decodes that format exactly. It keeps FP8 originals
separate from the derived quantized profiles of chapter 50.

## What this chapter covers

- E4M3 values with one scale per 128×128 block.
- Large 4's power-of-two (UE8M0) scales.
- Exact decode of every code at every admitted scale against an independent
  table.
- Refusal of partial blocks and out-of-range scales.
- FP8 originals kept distinct from derived profiles.
- The reference's dynamic FP8 activation quantization as a separate
  approximate profile.
- First consumers: Ministral-3-14B-Instruct-2512 and
  Devstral-Small-2-24B-Instruct-2512.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): original weights.
- [Chapter 60](ch060-original-mxfp4-weights.md): the pattern for an original
  block-scaled format.
- Native FP8 scalar types in Coax remain planned; packed storage and decoders
  run in the declared profiles until then.

## Done means

- Every E4M3 code decodes exactly at every admitted scale, for both regular
  and UE8M0 scales.
- Partial blocks and invalid scales are refused.
- The dynamic activation profile has its own name and stated tolerance.

## Sources

- [Chapter plan](PLAN.md), Part VIII and the Coax feature map.
- [Mistral target survey](../docs/targets/mistral.md).
