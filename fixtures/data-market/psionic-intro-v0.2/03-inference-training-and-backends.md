# Inference, Training, And Backends

Psionic currently spans three beginner-relevant areas.

## Inference

The inference docs define the minimum bar for an honest inference engine:

- model load and unload lifecycle
- request execution path
- token streaming or equivalent delivery
- KV-cache lifecycle
- deterministic execution metadata
- backend capability gating

For embeddings, the bar is similar:

- explicit request and response contract
- deterministic vector shape metadata
- stable model identity
- truthful capability reporting
- execution receipts tied to the served product

The short version is that "it ran a tensor op" is not enough. Psionic is only
inference-ready when it can expose a product-shaped serving path with truthful
metadata.

## Training

The train-system spec says Psionic train is not just one crate and not yet the
full distributed trainer-orchestrator runtime. What exists today is the lower
half of that stack plus a first real training core.

For a beginner, the useful takeaway is that Psionic already has real substrate
for:

- deterministic module and state-tree semantics
- checkpoint lineage
- optimizer and scheduler primitives
- dataset and checkpoint transport
- explicit trainer-step telemetry
- held-out eval and benchmark packaging
- bounded reference training lanes

That means the training story is early but real. It is no longer just a plan.

## Backends

Psionic keeps backend implementations explicit rather than hiding them behind
environment variables.

Current backend inventory in the docs:

- CPU
- Metal
- AMD KFD
- AMD userspace

The backend docs emphasize that backend claims must stay bounded by validation.
In practice that means backend selection and readiness should be explicit and
truthful rather than saying "GPU" while silently falling back to something
else.
