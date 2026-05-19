# Harvey Legal Qwen Training Objective

This is the current Nexus/Pylon foreground ML objective for legal benchmark
work:

```text
harvey_legal_qwen_finetune_v1
```

The CS336 and A1 lanes remain useful references and release proofs, but they
are no longer the foreground ML target for Pylon v0.2 work. The active target is
to use admitted Pylons to produce legal benchmark support data, validation
replays, evaluation results, and eventually Qwen legal adapter updates that can
improve Harvey benchmark completion.

## Control Plane Switch

Nexus reads the active objective from:

```bash
NEXUS_CONTROL_ACTIVE_TRAINING_OBJECTIVE_ID=harvey_legal_qwen_finetune_v1
```

If the variable is unset, Nexus defaults to the Harvey legal Qwen objective.
`/api/stats` exposes the active objective id and the canonical objective
registry so public/operator surfaces can distinguish:

- `harvey_legal_qwen_finetune_v1`: foreground legal benchmark work
- `stanford_cs336_assignment1_demo`: bounded reference/demo lane
- `tiny_transformer_next_token_prediction`: A1 minimal model-progress reference

## Runtime Contract

The legal objective uses the Psionic lane contract:

```text
lane_id: qwen_legal_adapter_sft_v1
environment_ref: psionic.environment.qwen_legal_adapter_sft.cuda.operator@v1
backend_family: cuda
topology: single_node_cuda_lora_sft
```

The smoke-scale base target is `model://qwen/Qwen3.5-4B`. The serious target is
`model://qwen/Qwen3.6-35B-A3B` once the runtime and data path are ready for the
larger model.

## Pylon Capability Labels

Pylon v0.2 publishes explicit training capability labels in the v2 capability
envelope and in TRN node records:

- `legal_dataset_extract`
- `legal_eval_case`
- `legal_validation_replay`
- `legal_judge_calibration`
- `qwen_legal_adapter_training`
- `qwen_legal_adapter_eval`
- `qwen_legal_checkpoint_validation`
- `artifact_integrity`

Weak or support-only Pylons may honestly publish legal support, eval, validation
replay, and artifact integrity labels. They must not publish
`qwen_legal_adapter_training` unless they are trainer-tier and can run the Qwen
legal adapter SFT lane through the Psionic training surface.

Nexus enforces the same boundary when matching leases. Harvey legal adapter
training requires `qwen_legal_adapter_training`; legal validation and support
work require `legal_validation_replay`; checkpoint promotion requires
`qwen_legal_checkpoint_validation`.

## Work Split

Use this split when launching or triaging work:

- Support/eval/integrity work can go to weaker admitted nodes when they have
  the legal support labels and accepted-work LDK settlement.
- Adapter training work goes only to trainer-tier CUDA Pylons that advertise
  the Qwen legal adapter SFT environment.
- Model-progress accounting is separate from participation-only support work.
  Accepted support can be paid, but it should not be misrepresented as a model
  improvement.
- CS336 remains a bounded proof lane for scheduler, settlement, and public
  earning behavior. It should not be treated as the objective to hill climb.

## Operator Checks

Before launching a legal Qwen run, verify:

```bash
cargo test -p psionic-train-contract qwen_legal
cargo test -p pylon legal_qwen_capability_labels
cargo test -p nexus-control harvey_legal_objective
```

Then check the Nexus stats surface for:

- `active_objective_id = harvey_legal_qwen_finetune_v1`
- a Harvey objective row with the Qwen legal capability labels
- admitted nodes whose TRN records include the required labels for their role

The first end-to-end live proof should close one support/eval/integrity smoke
assignment through the LDK payout path before opening larger adapter-training
windows.
