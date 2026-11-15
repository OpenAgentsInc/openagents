# 263. Export a policy and supervise its steps

> **Status: unimplemented — design draft.** This chapter specifies a robotics
> policy package, interchange checks and a local execution contract. Existing
> bounded ONNX support is reused but does not yet cover every proposed actor.

The training program knows more than the weight tensors. It knows the
observation normalizer, joint map, action scale, history layout and which
assembly it modeled. Deployment needs those facts too. A weight file loaded
successfully can still produce the wrong physical action on every step.

Package an admitted controller, then supervise every proposed action. Model
admission and motor authority are different decisions. An artifact can be
valid for offline evaluation while being ineligible for the attached robot.

## Folding a normalizer exactly once

For an illustrative scalar observation, let the stored mean be 1, denominator
2 and raw input 3. A layer with weight 0.25 and bias 0.10 produces:

```text
normalized x = (3 - 1) / 2 = 1
output       = 0.25 * 1 + 0.10 = 0.35.
```

Folding normalization into the affine layer gives weight 0.125 and bias
−0.025, so the raw-input result is again 0.35. If the runtime normalizes first
and the exported graph normalizes again, its graph input becomes zero and
the result is 0.10. All tensor shapes are still legal.

This is a real-arithmetic calculation. An actual export freezes storage,
rounding boundaries and comparison tolerances; algebraic folding is not a
promise of bitwise F32 identity. Retain raw-input vectors, intermediate
normalized values and action outputs from an independent evaluator.

## An admitted package

The package binds model/weights, graph/operator profile, normalization,
observation/action mapping, controller/skill semantics, assembly/calibration
requirements, precision, memory and the expected evaluation vectors. Define
which calibration values are per-unit and which change the model's identity.
Do not require one physical serial number for every compatible unit; require
the declared assembly plus a qualified unit-specific calibration.

Use [Safetensors interchange](ch008-models-travel-between-frameworks.md) and
the existing [ONNX reader](ch220-read-a-bounded-onnx-graph.md) and
[writer](ch221-write-a-bounded-onnx-graph.md). Extend shared supported operators
only when the selected actor needs them. Keep unknown operations, unsupported
attributes, external tensors and dynamic shapes as explicit refusals.

Register any new package/companion schema in `proto/` before writing its one
codec. Preserve published checkpoint identities. Robotics metadata cannot be
stored by relabeling the tokenizer field in the existing training checkpoint.
A package writer reads back what it wrote before publishing the release.

## One local step

The runtime reads a complete sensor generation, verifies identity/freshness,
computes a bounded proposal, applies declared limits, validates its command
lease and dispatches the effect. All asynchronous receipts retain concrete
ownership. A stale result cannot replace a newer admitted proposal.

Suppose the sensor was captured at 100 ms, inference returns at 130 ms and
the actuator application would occur at 150 ms. With a maximum state age of
40 ms, the command is stale at application: 150 − 100 = 50 ms. Checking only
the 20 ms since inference returned would admit the wrong command. Host receipt
time, device time and simulation step require explicit mapping and uncertainty.

Refuse old authority epochs, duplicate/out-of-order sequences and expired
commands. A refusal leaves the active controller's permitted local behavior
in effect; the precise hold/stop behavior depends on the qualified mechanism.
If the device effect may already have happened, record an incomplete or
halted transition rather than claiming motion was undone.

## Prove software semantics before connecting hardware

Use a fake device effect that records commands and returns controlled receipts.
It is a test adapter, not evidence of hardware behavior. Exercise delayed and
reordered replies, loss, cancellation, restart, bad sensor values and a
failed dispatch. Check complete output preservation on ordinary refusal and
the documented halted state after a partial external effect.

Independently compare native policy output and supported ONNX execution over
the same raw observations. Poison normalization, joint signs, calibration,
operator identity and action scale one at a time. Every mismatch must fail
before a command is emitted. Require mutation witnesses for admission policy.

Reuse existing model-file authentication, tensor admission, clocks, crypto
and supervisor components. C owns only actual file/device/clock effects.
[RB-10](../docs/robotics/microduck-implementation-roadmap.md#rb-10-policy-package-and-supervised-execution)
implements this boundary; [chapter 264](ch264-run-the-policy-on-a-real-robot.md)
then measures it on one separately authorized physical target.
