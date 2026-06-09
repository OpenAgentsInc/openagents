# Qwen 3.6 Remote Pylon Fine-Tune Gate

Issue: [#568](https://github.com/OpenAgentsInc/openagents/issues/568)

The Qwen 3.6 public fine-tune claim is blocked until remote Pylon workers
produce a receipt-backed training run report. The implementation lives in
`workers/api/src/qwen-remote-pylon-finetune-gate.ts`.

The gate intentionally distinguishes a bounded remote Qwen training/adaptation
report from a full-transformer Qwen fine-tune claim. A sampled-projection LoRA
run can clear the bounded remote training claim only with exact scope language;
it still cannot become full Qwen 3.6 transformer backprop copy.

## Required Evidence

The gate requires:

- at least two distinct `remote_pylon` worker refs;
- signed worker receipt refs for every worker;
- Qwen shard receipt refs meeting the required shard count;
- no quarantined shard refs;
- artifact refs;
- merge receipt refs;
- eval receipt refs;
- adapter admission refs;
- public projection refs;
- payment receipt refs;
- settlement receipt refs when claiming settled bitcoin.

## Claim Boundaries

- Local loopback workers do not satisfy the remote-device claim.
- `sampled_projection_lora` may describe a remote LoRA rehearsal, but not a full
  Qwen 3.6 transformer backprop fine-tune.
- Public Harvey replay evidence does not become private benchmark performance.
- `payable_pending_settlement` is not settled bitcoin.
- Bad or quarantined shards block the public claim until a clean replacement run
  passes the required shard, merge, eval, and admission checks.

## Projection Fields

The public projection exposes separate booleans:

- `qwenRemoteBoundedTrainingClaimAllowed`: the remote worker, shard,
  artifact, merge, eval, admission, payment, settlement, and projection refs
  are present with no quarantine blockers.
- `qwenRemoteFineTuneClaimAllowed`: the bounded gate is ready and the run is
  explicitly `full_transformer_backprop`.
- `fullQwenBackpropClaimAllowed`: same authority boundary as the full
  fine-tune claim.
- `harveyPrivateBenchmarkClaimAllowed`: always false in this public-safe gate.
  Private benchmark performance requires a separate private benchmark authority
  path and must not be inferred from public Harvey replay refs.
- `settledBitcoinClaimAllowed`: true only for `settled_bitcoin` with both
  payment and settlement receipt refs.

## Public Report Scope

The projection returns `scopeLanguage` that product copy can use directly. It
must name the exact scope: blocked rehearsal evidence, receipt-backed bounded
sampled-projection LoRA/adaptation evidence, or full-transformer remote
fine-tune evidence with worker, artifact, eval, payment, and settlement refs.

## Verification

Regression coverage lives in
`workers/api/src/qwen-remote-pylon-finetune-gate.test.ts`.
