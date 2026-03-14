# Activation Fingerprint Proofs

Psionic now carries an OpenAgents-owned activation-fingerprint proof path for
inference products that benefit from compact, verification-friendly artifacts.

## Current Scope

- The first shipped adapter is
  `psionic.activation_fingerprint.quantized.v1`.
- The first integrated product family is `psionic.embeddings`.
- Text generation and sandbox execution currently declare the proof layer as
  `unavailable`.
- Embeddings receipts declare the proof layer as `supported` and attach a
  concrete proof artifact on successful execution.

## Artifact Shape

The quantized adapter records:

- request digest
- product ID
- model ID
- runtime backend
- quantization config
- deterministic per-sample compact digests
- a stable artifact digest for the whole proof

Each sampled vector keeps:

- a stable label such as `embedding:0`
- original vector length
- number of values retained after deterministic sampling
- a mean quantized bucket
- an L2-style bucket sum
- a stable digest across the sampled bucket sequence

## Sampling And Quantization

- Sampling is deterministic and evenly spaced across the input vector.
- Values are clamped before quantization.
- Quantization uses configurable bucket width and clamp range.
- Verification regenerates the compact artifact from candidate vectors and
  compares stable digests.

This deliberately tolerates minor floating-point variance while keeping the
artifact small enough to attach to receipts.

## Posture

Canonical proof bundles now carry
`ExecutionProofAugmentationPosture` for the activation-fingerprint layer:

- `unavailable`: the product family does not surface this proof layer
- `supported`: the product family can attach the proof layer when applicable
- `required`: reserved for future products that make this layer mandatory

This keeps the market contract explicit before kernel-level product policy grows
into proof-sensitive product declarations.

## Cost Measurement

`QuantizedActivationFingerprintAdapter::benchmark` reports:

- generation iterations
- verification iterations
- average generation nanoseconds
- average verification nanoseconds
- sample count
- total values sampled

That gives Psionic a repeatable cost surface before a future product or kernel
layer upgrades this proof from `supported` to `required`.
