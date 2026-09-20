# Replacement 4B evaluation

## Candidate and policy

This evaluation pins `jaredpalmer/kev-4b` at
`c4bfa11b0dc07691884f2d97f1c4c4c05c92e416`, the shared Qwen3-4B base at
`906bfd4b4dc7f14ee4320094d8b41684abff8539`, and upstream code at
`86db6d924cee68fa9a1319d2c3e6010b9b233d60`. The candidate's artifact lock
and independent fixture directory are
[`kev-4b-c4bfa11`](../../../crates/kev/fixtures/variants/kev-4b-c4bfa11/).
Historical artifacts and measurements keep their original bytes.

Before candidate calls, the measurement plan is all open calibration and
development items in `coder-turns-v2`, `program-selection-v2`,
`support-v2-three-way`, and `external-v1`: 76, 68, 157, and 160 items.
Questions and labels stay frozen. No calibration maps or thresholds are
fitted. The comparison is the historical 4B at `1a0cb0a` and fresh hosted
`jev-latest`, including the program-selection rows already recorded by
[#9457](https://github.com/OpenAgentsInc/openagents/issues/9457).

Both local models serve Metal bf16 with fp32 heads and fp32 LoRA merging,
eager block-causal attention, a 4,096-token packed limit, and a 4,096 MiB
forward-memory budget. That admits one forward for each variant. They run
serially on the same 128 GiB M5 Max. Oversized requests count as refusals;
raising the bound after seeing responses would be a separate experiment.
The hosted service reports its model name but no verifiable weight digest.

The program-selection acceptance rule is the one frozen in
[the v2 baseline](../../decision-models/2026-09-20-program-selection-v2.md).
For other families, report paired errors, Wilson accuracy intervals, and
paired bootstrap intervals separately by family and split. These are
benchmark descriptions, not independent deployment trials. No locked read
or default replacement follows from this open measurement alone. A material
4B gap can justify a separate 8B control; more parameters do not repair an
input refusal or an ambiguous label.
