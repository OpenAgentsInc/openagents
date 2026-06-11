# Qwen Remote Pylon Live Training Runbook

Date: 2026-06-11

Issue: #4670

This runbook is the operator-facing smoke path for the bounded Qwen remote
training claim. It verifies the public-safe evidence bundle accepted by
`qwen-remote-pylon-finetune-gate.ts` and checks that the named worker refs are
fresh, real, wallet-ready, assignment-ready Pylons that advertise the public
fine-tuning/training capability.

## Boundary

This smoke does not dispatch a training job, spend bitcoin, settle workers,
promote a model, or edit the product-promise registry. It only verifies public
refs after the external Lane B work exists.

The accepted bounded claim is exactly the gate scope language:

> Remote Pylon Qwen 3.6 sampled-projection LoRA run is receipt-backed,
> evaluated, admitted, and settled; this is a bounded LoRA/adaptation report,
> not a full Qwen 3.6 transformer backprop fine-tune or private benchmark
> performance claim.

`qwenRemoteFineTuneClaimAllowed` and `fullQwenBackpropClaimAllowed` must remain
false for a `sampled_projection_lora` run.

## Preflight

From `apps/openagents.com/workers/api`:

```bash
bun run smoke:qwen-remote-training -- --preflight
```

To constrain the check to the intended workers:

```bash
bun run smoke:qwen-remote-training -- --preflight \
  --pylon-ref pylon.public.qwen_training.alpha \
  --pylon-ref pylon.public.qwen_training.beta
```

The preflight is green only when at least two selected or discovered Pylons are:

- `status: active`;
- fresh and online in the public Pylon list;
- wallet-ready;
- advertising `capability.pylon.assignment_ready`;
- advertising `capability.public.pylon.fine_tuning_training`;
- not smoke, canary, demo, fixture, packaged-smoke, or loopback registrations.

## Bundle Shape

After the Lane B run completes, write a JSON bundle containing the
`QwenRemotePylonFineTuneGateInput` shape:

```json
{
  "adapterAdmissionRefs": ["admission.public.qwen3_6.live_lora.accepted"],
  "evalReceiptRefs": ["eval_receipt.public.qwen3_6.live_lora.harvey"],
  "harveyScope": "public_replay",
  "mergeReceiptRefs": ["merge_receipt.public.qwen3_6.live_lora"],
  "modelRef": "model.public.qwen3_6_27b.remote_lora",
  "paymentReceiptRefs": ["payment_receipt.public.qwen3_6.live_lora"],
  "paymentState": "settled_bitcoin",
  "publicProjectionRefs": ["projection.public.qwen3_6.live_lora.report"],
  "requiredShardCount": 15,
  "runRef": "training_run.public.qwen3_6.live_pylon.lora",
  "settlementReceiptRefs": ["settlement_receipt.public.qwen3_6.live_lora"],
  "trainingMode": "sampled_projection_lora",
  "workerReceipts": [
    {
      "artifactRefs": ["artifact.public.qwen3_6.live_lora.alpha"],
      "deviceScope": "remote_pylon",
      "quarantineRefs": [],
      "shardReceiptRefs": ["shard_receipt.public.qwen3_6.live.alpha.1"],
      "signedWorkerReceiptRefs": [
        "worker_receipt.public.qwen3_6.live_lora.alpha.signed"
      ],
      "workerRef": "pylon.public.qwen_training.alpha"
    },
    {
      "artifactRefs": ["artifact.public.qwen3_6.live_lora.beta"],
      "deviceScope": "remote_pylon",
      "quarantineRefs": [],
      "shardReceiptRefs": ["shard_receipt.public.qwen3_6.live.beta.1"],
      "signedWorkerReceiptRefs": [
        "worker_receipt.public.qwen3_6.live_lora.beta.signed"
      ],
      "workerRef": "pylon.public.qwen_training.beta"
    }
  ]
}
```

The real bundle must include enough shard receipt refs to satisfy
`requiredShardCount`, no quarantined shard refs, signed worker receipt refs for
every worker, payment receipt refs, settlement receipt refs, and public
projection refs.

Verify the bundle against the live preflight:

```bash
bun run smoke:qwen-remote-training -- --bundle qwen-training-bundle.json \
  --pylon-ref pylon.public.qwen_training.alpha \
  --pylon-ref pylon.public.qwen_training.beta
```

The process exits 0 only when the combined projection state is `green`.
Blocked output is structured and should be pasted into the issue without raw
payment, wallet, worker-home, runner-log, model-weight, or customer material.

## Current 2026-06-11 Blocker

The live public fleet has assignment-ready Pylons, but #4670 is still blocked:

- `pylon.first_real_model_training_run.v1` remains red with
  `remote_multi_device_training_missing` and
  `qwen_training_postponed_after_gepa`.
- `pylon.compute_revenue_modes.v1` remains red with
  `remote_qwen_training_missing`.
- No two real non-synthetic Pylons currently advertise
  `capability.public.pylon.fine_tuning_training`.
- No public signed worker, shard, merge, eval, adapter-admission, payment, or
  settlement refs exist for the Lane B bounded run.

Do not edit the product-promise registry until transition receipts exist before
the registry edit.
