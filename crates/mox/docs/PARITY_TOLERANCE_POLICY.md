# Mox Parity Tolerance Policy

This document records the default backend parity policy introduced by
`MOX-119`.

The source of truth for the reusable policy lives in
`crates/mox/mox-runtime/src/parity.rs` as `BackendParityPolicy`.

## Reference Rule

CPU is the numeric reference until a tighter backend-independent oracle exists.

That means:

- relative tolerance is computed against the CPU/reference value
- backend claims are evaluated as "candidate backend stays within the documented
  budget of CPU" rather than "any two backends can drift arbitrarily"

The numeric comparison formula follows the standard Tinygrad-style `allclose`
shape:

```text
abs(expected - actual) <= atol + rtol * abs(expected)
```

## Default Budget Table

| Surface | Path | Expectation | Budget |
| --- | --- | --- | --- |
| Embeddings | dense / non-quantized | `numerical` | `atol=1e-5`, `rtol=1e-5`, cosine similarity `>= 0.99999` |
| Embeddings | quantized | `numerical` | `atol=2e-3`, `rtol=2e-3`, cosine similarity `>= 0.999` |
| Logits | dense / non-quantized | `numerical` | `atol=1e-5`, `rtol=1e-5`, top-token rank drift `= 0` |
| Logits | quantized | `numerical` | `atol=5e-3`, `rtol=5e-3`, top-token rank drift `<= 1` |
| Generation with seed | all paths | `exact` | token choices exact, decoded text exact, termination exact |
| Generation without seed | all paths | `semantic` / `exact` | token choices semantic, decoded text semantic, termination exact |

## Why These Classes Exist

`MOX-119` draws a hard line between three kinds of parity:

- `exact`
  - byte-for-byte / token-for-token identity is required
  - used for seeded generation because determinism claims are otherwise not
    honest
- `semantic`
  - the backend may sample differently, but it must preserve the same semantic
    contract
  - used for unseeded generation outcomes so we do not fake deterministic
    parity where sampling entropy is expected
- `numerical`
  - element-wise drift is allowed only inside an explicit budget
  - used for embeddings and logits because floating-point and quantized kernels
    can differ slightly even when behavior is still correct

## Quantized vs Dense Paths

Quantized paths have explicit budgets separate from dense paths.

That is intentional:

- dense paths should stay very tight
- quantized paths may drift more, but the budget must still be small enough to
  catch real correctness regressions
- quantized execution is not allowed to reuse dense budgets silently or to hide
  behind loose thresholds without review

## Seed Determinism Rule

If a request carries a seed, backend parity is `exact` for:

- token choices
- decoded text
- termination reason

If a request does not carry a seed, backend parity is still `exact` for
termination reason, but sampled token/text comparison is only `semantic`.

This prevents two opposite mistakes:

- requiring impossible token-for-token equality from unseeded sampling paths
- claiming determinism for seeded requests while tolerating backend drift

## Current Consumers

The policy is wired into:

- `crates/mox/mox-serve/tests/metal_embeddings_parity.rs`
- `crates/mox/mox-serve/tests/metal_text_generation_parity.rs`
- `crates/mox/mox-serve/src/conformance.rs` embed comparisons
- `crates/mox/mox-backend-metal/src/lib.rs` policy documentation tests

Future backend parity work (`MOX-131`, `MOX-146`, `MOX-153`, `MOX-156`) should
reuse this policy instead of introducing local constants.
