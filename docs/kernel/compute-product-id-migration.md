# Compute Product ID Migration

This note defines the canonical compute-product namespace introduced for the
Psionic-owned compute family tree.

## Canonical Format

Compute product IDs now follow:

`psionic.<provisioning>.<family>.<implementation>.<topology>`

Rules:

- `provisioning` describes where the product is admitted from:
  - `local`
  - `remote_sandbox`
  - later: `cluster`
- `family` describes what is being sold:
  - `inference`
  - `embeddings`
  - `sandbox_execution`
  - later: `evaluation`, `training`, `adapter_hosting`
- `implementation` is the backend family for model-serving products or the
  execution class for sandbox products.
- `topology` names the truthful execution topology exposed in the capability
  envelope.

## Current Canonical IDs

| Old ID | Canonical ID |
| --- | --- |
| `ollama.text_generation` | `psionic.local.inference.gpt_oss.single_node` |
| `gpt_oss.text_generation` | `psionic.local.inference.gpt_oss.single_node` |
| `ollama.embeddings` | `psionic.local.embeddings.gpt_oss.single_node` |
| `gpt_oss.embeddings` | `psionic.local.embeddings.gpt_oss.single_node` |
| `apple_foundation_models.text_generation` | `psionic.local.inference.apple_foundation_models.single_node` |
| `sandbox.container.exec` | `psionic.remote_sandbox.sandbox_execution.container_exec.sandbox_isolated` |
| `sandbox.python.exec` | `psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated` |
| `sandbox.node.exec` | `psionic.remote_sandbox.sandbox_execution.node_exec.sandbox_isolated` |
| `sandbox.posix.exec` | `psionic.remote_sandbox.sandbox_execution.posix_exec.sandbox_isolated` |

## Compatibility

- Kernel-side canonicalization accepts both canonical IDs and the historical
  launch aliases above.
- Provider-substrate emits canonical IDs for newly-derived products, but still
  recognizes historical aliases when reading existing state or tests.
- Desktop launch bindings now emit canonical inference product IDs.

## Forward Examples

These are naming targets, not implemented product families:

- `psionic.cluster.inference.gpt_oss.tensor_sharded`
- `psionic.cluster.inference.gpt_oss.pipeline_sharded`
- `psionic.cluster.training.gpt_oss.training_elastic`
- `psionic.cluster.evaluation.apple_foundation_models.replicated`

The product ID should say what market object is being sold. Detailed runtime
facts still belong in the capability envelope, delivery proof, and evidence.
