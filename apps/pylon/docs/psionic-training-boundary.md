# Psionic Training Boundary

Date: 2026-06-10

Issue: [#4669](https://github.com/OpenAgentsInc/openagents/issues/4669)

Pylon treats neural training through Psionic as a gated connector boundary, not
as built-in local training authority. GEPA text optimization and Qwen neural
training remain separate claims.

## OpenAgents-Owned Boundary

The Pylon side owns these public-safe checks:

- `openagents.psionic.training_release_manifest.v0.3` must verify as signed by
  an allowed Psionic signer ref before a training sidecar can be considered.
- Training artifacts must match their manifest SHA-256 digest before they can
  count as usable model/runtime inputs.
- The local sidecar lifecycle must reach `health_ready`; crashed, stopped, or
  merely started sidecars keep the boundary blocked.
- `openagents.psionic.training_worker_receipt.v0.3` receipts must verify as
  signed by an allowed Psionic signer ref before Pylon imports them into
  public-safe closeout evidence refs.
- `supportsTraining` stays false unless manifest verification, artifact digest
  verification, healthy sidecar state, and receipt import are all ready.

The public projection redacts content and exposes refs only. It must not expose
raw checkpoints, local paths, model weight filenames, prompts, wallet material,
provider credentials, or private process state.

## External Psionic Asks

Psionic remains the ML execution substrate. OpenAgents/Pylon needs these
external contracts before this boundary can clear on a real machine:

- `external.psionic.training_job_contract`: the stable training job request and
  response contract for Pylon assignments.
- `external.psionic.signed_release_manifest`: release manifests signed by the
  Psionic release authority, including sidecar protocol refs and expected
  artifact refs.
- `external.psionic.worker_receipt_format`: signed worker receipt format for
  run refs, checkpoint refs, metric refs, proof refs, and assignment closeout
  import.

Until those exist, Pylon can model and test the boundary but cannot honestly
claim live Qwen training on contributor devices.

## Verification

Run from `apps/pylon`:

```sh
bun test tests/psionic-training-boundary.test.ts tests/launch-gates.test.ts --max-concurrency=1
```
