import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const QwenRemotePylonFineTuneGateSchemaVersion =
  'omega.qwen_remote_pylon_finetune_gate.v1'

export const QwenRemotePylonFineTuneDecision = S.Literals(['blocked', 'ready'])
export type QwenRemotePylonFineTuneDecision =
  typeof QwenRemotePylonFineTuneDecision.Type

export const QwenRemotePylonFineTuneDeviceScope = S.Literals([
  'local_loopback',
  'remote_pylon',
])
export type QwenRemotePylonFineTuneDeviceScope =
  typeof QwenRemotePylonFineTuneDeviceScope.Type

export const QwenRemotePylonFineTuneTrainingMode = S.Literals([
  'full_transformer_backprop',
  'sampled_projection_lora',
])
export type QwenRemotePylonFineTuneTrainingMode =
  typeof QwenRemotePylonFineTuneTrainingMode.Type

export const QwenRemotePylonFineTunePaymentState = S.Literals([
  'payable_pending_settlement',
  'settled_bitcoin',
  'unpaid_smoke',
])
export type QwenRemotePylonFineTunePaymentState =
  typeof QwenRemotePylonFineTunePaymentState.Type

export const QwenRemotePylonFineTuneHarveyScope = S.Literals([
  'private_benchmark',
  'public_replay',
])
export type QwenRemotePylonFineTuneHarveyScope =
  typeof QwenRemotePylonFineTuneHarveyScope.Type

export class QwenRemotePylonFineTuneWorkerReceipt extends S.Class<QwenRemotePylonFineTuneWorkerReceipt>(
  'QwenRemotePylonFineTuneWorkerReceipt',
)({
  artifactRefs: S.Array(S.String),
  deviceScope: QwenRemotePylonFineTuneDeviceScope,
  quarantineRefs: S.Array(S.String),
  shardReceiptRefs: S.Array(S.String),
  signedWorkerReceiptRefs: S.Array(S.String),
  workerRef: S.String,
}) {}

export class QwenRemotePylonFineTuneGateInput extends S.Class<QwenRemotePylonFineTuneGateInput>(
  'QwenRemotePylonFineTuneGateInput',
)({
  adapterAdmissionRefs: S.Array(S.String),
  evalReceiptRefs: S.Array(S.String),
  harveyScope: QwenRemotePylonFineTuneHarveyScope,
  mergeReceiptRefs: S.Array(S.String),
  modelRef: S.String,
  paymentReceiptRefs: S.Array(S.String),
  paymentState: QwenRemotePylonFineTunePaymentState,
  publicProjectionRefs: S.Array(S.String),
  requiredShardCount: S.Number,
  runRef: S.String,
  settlementReceiptRefs: S.Array(S.String),
  trainingMode: QwenRemotePylonFineTuneTrainingMode,
  workerReceipts: S.Array(QwenRemotePylonFineTuneWorkerReceipt),
}) {}

export class QwenRemotePylonFineTuneGateProjection extends S.Class<QwenRemotePylonFineTuneGateProjection>(
  'QwenRemotePylonFineTuneGateProjection',
)({
  adapterAdmissionRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  decision: QwenRemotePylonFineTuneDecision,
  evalReceiptRefs: S.Array(S.String),
  fullQwenBackpropClaimAllowed: S.Boolean,
  harveyPrivateBenchmarkClaimAllowed: S.Boolean,
  harveyScope: QwenRemotePylonFineTuneHarveyScope,
  mergeReceiptRefs: S.Array(S.String),
  modelRef: S.String,
  paymentReceiptRefs: S.Array(S.String),
  paymentState: QwenRemotePylonFineTunePaymentState,
  publicProjectionRefs: S.Array(S.String),
  qwenRemoteBoundedTrainingClaimAllowed: S.Boolean,
  qwenRemoteFineTuneClaimAllowed: S.Boolean,
  quarantinedShardRefs: S.Array(S.String),
  remoteDeviceClaimAllowed: S.Boolean,
  remoteWorkerRefs: S.Array(S.String),
  requiredShardCount: S.Number,
  runRef: S.String,
  schemaVersion: S.Literal(QwenRemotePylonFineTuneGateSchemaVersion),
  scopeLanguage: S.String,
  settledBitcoinClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  shardReceiptRefs: S.Array(S.String),
  signedWorkerReceiptRefs: S.Array(S.String),
  trainingMode: QwenRemotePylonFineTuneTrainingMode,
  workerRefs: S.Array(S.String),
}) {}

export class QwenRemotePylonFineTuneGateUnsafe extends S.TaggedErrorClass<QwenRemotePylonFineTuneGateUnsafe>()(
  'QwenRemotePylonFineTuneGateUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,280}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.(raw|private)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(
    ref =>
      !safeRefPattern.test(ref) ||
      containsProviderSecretMaterial(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new QwenRemotePylonFineTuneGateUnsafe({
      reason: `${label} contains private datasets, raw model weights, raw runner logs, provider secrets, wallet/payment material, private repos, local paths, or raw timestamps.`,
    })
  }

  return normalized
}

const missingRefBlocker = (label: string): string =>
  `blocker.public.qwen_remote_finetune.${label}_missing`

const baseCaveatRefs = [
  'caveat.public.qwen_remote_finetune.local_loopback_is_not_remote_device',
  'caveat.public.qwen_remote_finetune.sampled_projection_lora_is_not_full_backprop',
  'caveat.public.qwen_remote_finetune.public_harvey_is_not_private_benchmark',
  'caveat.public.qwen_remote_finetune.payable_is_not_settled_bitcoin',
]

const scopeLanguageFor = (
  input: QwenRemotePylonFineTuneGateInput,
  ready: boolean,
): string => {
  if (!ready) {
    return 'Qwen 3.6 remote Pylon training claim blocked; public copy may cite only local, loopback, or incomplete rehearsal evidence with the listed blockers.'
  }

  if (input.trainingMode === 'sampled_projection_lora') {
    return 'Remote Pylon Qwen 3.6 sampled-projection LoRA run is receipt-backed, evaluated, admitted, and settled; this is a bounded LoRA/adaptation report, not a full Qwen 3.6 transformer backprop fine-tune or private benchmark performance claim.'
  }

  return 'Remote Pylon Qwen 3.6 full-transformer fine-tune evidence is receipt-backed, evaluated, admitted, and settled; public copy must cite the listed worker, artifact, eval, payment, and settlement refs.'
}

export const projectQwenRemotePylonFineTuneGate = (
  input: QwenRemotePylonFineTuneGateInput,
): QwenRemotePylonFineTuneGateProjection => {
  const normalized = S.decodeUnknownSync(QwenRemotePylonFineTuneGateInput)(
    input,
  )

  if (
    !Number.isInteger(normalized.requiredShardCount) ||
    normalized.requiredShardCount <= 0
  ) {
    throw new QwenRemotePylonFineTuneGateUnsafe({
      reason:
        'Qwen remote fine-tune requiredShardCount must be a positive integer.',
    })
  }

  const workerReceipts = normalized.workerReceipts.map(
    receipt =>
      new QwenRemotePylonFineTuneWorkerReceipt({
        ...receipt,
        artifactRefs: uniqueRefs(receipt.artifactRefs),
        quarantineRefs: uniqueRefs(receipt.quarantineRefs),
        shardReceiptRefs: uniqueRefs(receipt.shardReceiptRefs),
        signedWorkerReceiptRefs: uniqueRefs(receipt.signedWorkerReceiptRefs),
      }),
  )
  const workerRefs = assertSafeRefs(
    'Qwen remote fine-tune worker refs',
    workerReceipts.map(receipt => receipt.workerRef),
  )
  const shardReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune shard receipt refs',
    workerReceipts.flatMap(receipt => receipt.shardReceiptRefs),
  )
  const quarantinedShardRefs = assertSafeRefs(
    'Qwen remote fine-tune quarantined shard refs',
    workerReceipts.flatMap(receipt => receipt.quarantineRefs),
  )
  const artifactRefs = assertSafeRefs(
    'Qwen remote fine-tune artifact refs',
    workerReceipts.flatMap(receipt => receipt.artifactRefs),
  )
  const signedWorkerReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune signed worker receipt refs',
    workerReceipts.flatMap(receipt => receipt.signedWorkerReceiptRefs),
  )
  const mergeReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune merge receipt refs',
    normalized.mergeReceiptRefs,
  )
  const evalReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune eval receipt refs',
    normalized.evalReceiptRefs,
  )
  const adapterAdmissionRefs = assertSafeRefs(
    'Qwen remote fine-tune adapter admission refs',
    normalized.adapterAdmissionRefs,
  )
  const paymentReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune payment receipt refs',
    normalized.paymentReceiptRefs,
  )
  const settlementReceiptRefs = assertSafeRefs(
    'Qwen remote fine-tune settlement receipt refs',
    normalized.settlementReceiptRefs,
  )
  const publicProjectionRefs = assertSafeRefs(
    'Qwen remote fine-tune public projection refs',
    normalized.publicProjectionRefs,
  )
  const identityRefs = assertSafeRefs('Qwen remote fine-tune identity refs', [
    normalized.harveyScope,
    normalized.modelRef,
    normalized.paymentState,
    normalized.runRef,
    normalized.trainingMode,
  ])
  const modelRef =
    identityRefs.find(ref => ref === normalized.modelRef) ??
    'model.public.qwen3_6.redacted'
  const runRef =
    identityRefs.find(ref => ref === normalized.runRef) ??
    'training_run.public.qwen3_6.redacted'

  const remoteWorkerRefs = uniqueRefs(
    workerReceipts
      .filter(receipt => receipt.deviceScope === 'remote_pylon')
      .map(receipt => receipt.workerRef),
  )
  const remoteDeviceClaimAllowed = remoteWorkerRefs.length >= 2
  const allWorkersSigned = workerReceipts.every(receipt =>
    hasRefs(receipt.signedWorkerReceiptRefs),
  )
  const everyWorkerHasShardRefs = workerReceipts.every(receipt =>
    hasRefs(receipt.shardReceiptRefs),
  )
  const everyWorkerHasArtifactRefs = workerReceipts.every(receipt =>
    hasRefs(receipt.artifactRefs),
  )
  const settledBitcoinClaimAllowed =
    normalized.paymentState === 'settled_bitcoin' &&
    hasRefs(paymentReceiptRefs) &&
    hasRefs(settlementReceiptRefs)
  const blockerRefs = uniqueRefs([
    ...(remoteDeviceClaimAllowed
      ? []
      : ['blocker.public.qwen_remote_finetune.remote_workers_missing']),
    ...(allWorkersSigned
      ? []
      : ['blocker.public.qwen_remote_finetune.signed_worker_receipts_missing']),
    ...(everyWorkerHasShardRefs &&
    shardReceiptRefs.length >= normalized.requiredShardCount
      ? []
      : [
          `blocker.public.qwen_remote_finetune.shard_receipts_missing.required_${publicRefSegment(
            String(normalized.requiredShardCount),
            'qwen',
          )}`,
        ]),
    ...(everyWorkerHasArtifactRefs && hasRefs(artifactRefs)
      ? []
      : [missingRefBlocker('artifact_refs')]),
    ...(hasRefs(quarantinedShardRefs)
      ? ['blocker.public.qwen_remote_finetune.quarantined_shards_present']
      : []),
    ...(hasRefs(mergeReceiptRefs) ? [] : [missingRefBlocker('merge_receipts')]),
    ...(hasRefs(evalReceiptRefs) ? [] : [missingRefBlocker('eval_receipts')]),
    ...(hasRefs(adapterAdmissionRefs)
      ? []
      : [missingRefBlocker('adapter_admission_refs')]),
    ...(hasRefs(publicProjectionRefs)
      ? []
      : [missingRefBlocker('public_projection_refs')]),
    ...(hasRefs(paymentReceiptRefs)
      ? []
      : [missingRefBlocker('payment_receipts')]),
    ...(normalized.paymentState === 'unpaid_smoke'
      ? [
          'blocker.public.qwen_remote_finetune.unpaid_smoke_has_no_payment_claim',
        ]
      : []),
    ...(normalized.paymentState === 'payable_pending_settlement'
      ? ['blocker.public.qwen_remote_finetune.payable_is_not_settled_bitcoin']
      : []),
    ...(settledBitcoinClaimAllowed
      ? []
      : [missingRefBlocker('settlement_receipts')]),
  ])
  const ready = blockerRefs.length === 0
  const fullQwenBackpropClaimAllowed =
    ready && normalized.trainingMode === 'full_transformer_backprop'
  const qwenRemoteBoundedTrainingClaimAllowed = ready
  const harveyPrivateBenchmarkClaimAllowed =
    ready && normalized.harveyScope === 'private_benchmark'

  return new QwenRemotePylonFineTuneGateProjection({
    adapterAdmissionRefs,
    artifactRefs,
    blockerRefs,
    caveatRefs: baseCaveatRefs,
    decision: ready ? 'ready' : 'blocked',
    evalReceiptRefs,
    fullQwenBackpropClaimAllowed,
    harveyPrivateBenchmarkClaimAllowed,
    harveyScope: normalized.harveyScope,
    mergeReceiptRefs,
    modelRef,
    paymentReceiptRefs,
    paymentState: normalized.paymentState,
    publicProjectionRefs,
    qwenRemoteBoundedTrainingClaimAllowed,
    qwenRemoteFineTuneClaimAllowed: fullQwenBackpropClaimAllowed,
    quarantinedShardRefs,
    remoteDeviceClaimAllowed,
    remoteWorkerRefs,
    requiredShardCount: normalized.requiredShardCount,
    runRef,
    schemaVersion: QwenRemotePylonFineTuneGateSchemaVersion,
    scopeLanguage: scopeLanguageFor(normalized, ready),
    settledBitcoinClaimAllowed,
    settlementReceiptRefs,
    shardReceiptRefs,
    signedWorkerReceiptRefs,
    trainingMode: normalized.trainingMode,
    workerRefs,
  })
}

export const qwenRemotePylonFineTuneGateHasPrivateMaterial = (
  projection: QwenRemotePylonFineTuneGateProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return (
    unsafeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized) ||
    containsProviderSecretMaterial(serialized)
  )
}
