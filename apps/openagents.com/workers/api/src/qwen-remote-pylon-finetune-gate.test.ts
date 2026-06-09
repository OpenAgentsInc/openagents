import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  QwenRemotePylonFineTuneGateInput,
  QwenRemotePylonFineTuneGateProjection,
  QwenRemotePylonFineTuneGateUnsafe,
  QwenRemotePylonFineTuneWorkerReceipt,
  projectQwenRemotePylonFineTuneGate,
  qwenRemotePylonFineTuneGateHasPrivateMaterial,
} from './qwen-remote-pylon-finetune-gate'

const shardRefs = (prefix: string): ReadonlyArray<string> =>
  Array.from(
    { length: 15 },
    (_, index) => `shard_receipt.public.qwen3_6.${prefix}.${index + 1}`,
  )

const worker = (
  suffix: string,
  overrides: Partial<QwenRemotePylonFineTuneWorkerReceipt> = {},
): QwenRemotePylonFineTuneWorkerReceipt =>
  new QwenRemotePylonFineTuneWorkerReceipt({
    artifactRefs: [`artifact.public.qwen3_6.remote_lora.${suffix}`],
    deviceScope: 'remote_pylon',
    quarantineRefs: [],
    shardReceiptRefs: shardRefs(suffix),
    signedWorkerReceiptRefs: [
      `worker_receipt.public.qwen3_6.remote_lora.${suffix}.signed`,
    ],
    workerRef: `pylon.public.remote_qwen_worker.${suffix}`,
    ...overrides,
  })

const gateInput = (
  overrides: Partial<QwenRemotePylonFineTuneGateInput> = {},
): QwenRemotePylonFineTuneGateInput =>
  new QwenRemotePylonFineTuneGateInput({
    adapterAdmissionRefs: ['admission.public.qwen3_6.remote_lora.accepted.1'],
    evalReceiptRefs: ['eval_receipt.public.qwen3_6.remote_lora.harvey.1'],
    harveyScope: 'public_replay',
    mergeReceiptRefs: ['merge_receipt.public.qwen3_6.remote_lora.1'],
    modelRef: 'model.public.qwen3_6_27b.remote_finetune',
    paymentReceiptRefs: ['payment_receipt.public.qwen3_6.remote_lora.1'],
    paymentState: 'settled_bitcoin',
    publicProjectionRefs: ['projection.public.qwen3_6.remote_lora.report.1'],
    requiredShardCount: 15,
    runRef: 'training_run.public.qwen3_6.remote_pylon.lora.1',
    settlementReceiptRefs: [
      'settlement_receipt.public.qwen3_6.remote_lora.bitcoin.1',
    ],
    trainingMode: 'sampled_projection_lora',
    workerReceipts: [worker('alpha'), worker('beta')],
    ...overrides,
  })

describe('Qwen remote Pylon fine-tune gate', () => {
  test('projects a settled remote bounded Qwen LoRA run without overclaiming full backprop', () => {
    const projection = projectQwenRemotePylonFineTuneGate(gateInput())

    expect(
      S.decodeUnknownSync(QwenRemotePylonFineTuneGateProjection)(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      decision: 'ready',
      fullQwenBackpropClaimAllowed: false,
      harveyPrivateBenchmarkClaimAllowed: false,
      qwenRemoteBoundedTrainingClaimAllowed: true,
      qwenRemoteFineTuneClaimAllowed: false,
      remoteDeviceClaimAllowed: true,
      settledBitcoinClaimAllowed: true,
      trainingMode: 'sampled_projection_lora',
    })
    expect(projection.remoteWorkerRefs).toEqual([
      'pylon.public.remote_qwen_worker.alpha',
      'pylon.public.remote_qwen_worker.beta',
    ])
    expect(projection.workerRefs).toEqual([
      'pylon.public.remote_qwen_worker.alpha',
      'pylon.public.remote_qwen_worker.beta',
    ])
    expect(projection.shardReceiptRefs).toHaveLength(30)
    expect(projection.signedWorkerReceiptRefs).toEqual([
      'worker_receipt.public.qwen3_6.remote_lora.alpha.signed',
      'worker_receipt.public.qwen3_6.remote_lora.beta.signed',
    ])
    expect(projection.scopeLanguage).toContain('bounded LoRA/adaptation report')
    expect(projection.caveatRefs).toContain(
      'caveat.public.qwen_remote_finetune.sampled_projection_lora_is_not_full_backprop',
    )
    expect(qwenRemotePylonFineTuneGateHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('allows full Qwen backprop copy only when the training mode and receipts say so', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        runRef: 'training_run.public.qwen3_6.remote_pylon.full_backprop.1',
        trainingMode: 'full_transformer_backprop',
      }),
    )

    expect(projection).toMatchObject({
      decision: 'ready',
      fullQwenBackpropClaimAllowed: true,
      harveyPrivateBenchmarkClaimAllowed: false,
      qwenRemoteBoundedTrainingClaimAllowed: true,
      qwenRemoteFineTuneClaimAllowed: true,
      remoteDeviceClaimAllowed: true,
      settledBitcoinClaimAllowed: true,
      trainingMode: 'full_transformer_backprop',
    })
    expect(projection.scopeLanguage).toContain(
      'Remote Pylon Qwen 3.6 full-transformer fine-tune evidence',
    )
  })

  test('blocks local loopback evidence from satisfying the remote-device claim', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        workerReceipts: [
          worker('local_a', {
            deviceScope: 'local_loopback',
            workerRef: 'pylon.public.loopback_qwen_worker.local_a',
          }),
          worker('local_b', {
            deviceScope: 'local_loopback',
            workerRef: 'pylon.public.loopback_qwen_worker.local_b',
          }),
        ],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.remoteDeviceClaimAllowed).toBe(false)
    expect(projection.qwenRemoteFineTuneClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.remote_workers_missing',
    )
  })

  test('keeps sampled-projection LoRA from becoming a full Qwen backprop claim', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        trainingMode: 'sampled_projection_lora',
      }),
    )

    expect(projection.decision).toBe('ready')
    expect(projection.remoteDeviceClaimAllowed).toBe(true)
    expect(projection.qwenRemoteBoundedTrainingClaimAllowed).toBe(true)
    expect(projection.fullQwenBackpropClaimAllowed).toBe(false)
    expect(projection.qwenRemoteFineTuneClaimAllowed).toBe(false)
    expect(projection.scopeLanguage).toContain(
      'not a full Qwen 3.6 transformer backprop fine-tune',
    )
  })

  test('keeps public Harvey replay evidence separate from private benchmark performance', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        harveyScope: 'public_replay',
      }),
    )

    expect(projection.decision).toBe('ready')
    expect(projection.harveyPrivateBenchmarkClaimAllowed).toBe(false)
    expect(projection.caveatRefs).toContain(
      'caveat.public.qwen_remote_finetune.public_harvey_is_not_private_benchmark',
    )
  })

  test('allows private Harvey benchmark copy only for private benchmark scope', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        harveyScope: 'private_benchmark',
      }),
    )

    expect(projection.decision).toBe('ready')
    expect(projection.harveyPrivateBenchmarkClaimAllowed).toBe(true)
  })

  test('blocks bad shard quarantine before merge or public launch copy', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        workerReceipts: [
          worker('alpha', {
            quarantineRefs: ['quarantine.public.qwen3_6.shard_7.bad_digest'],
          }),
          worker('beta'),
        ],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.quarantinedShardRefs).toEqual([
      'quarantine.public.qwen3_6.shard_7.bad_digest',
    ])
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.quarantined_shards_present',
    )
  })

  test('keeps payable deferred work separate from settled bitcoin', () => {
    const projection = projectQwenRemotePylonFineTuneGate(
      gateInput({
        paymentState: 'payable_pending_settlement',
        settlementReceiptRefs: [],
      }),
    )

    expect(projection.decision).toBe('blocked')
    expect(projection.settledBitcoinClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.payable_is_not_settled_bitcoin',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.settlement_receipts_missing',
    )
  })

  test('rejects unsafe private refs, raw model weights, and payment material', () => {
    expect(() =>
      projectQwenRemotePylonFineTuneGate(
        gateInput({
          paymentReceiptRefs: ['payment_preimage.private.qwen'],
        }),
      ),
    ).toThrow(QwenRemotePylonFineTuneGateUnsafe)

    expect(() =>
      projectQwenRemotePylonFineTuneGate(
        gateInput({
          workerReceipts: [
            worker('alpha', {
              shardReceiptRefs: ['weights.gguf.raw'],
            }),
            worker('beta'),
          ],
        }),
      ),
    ).toThrow(QwenRemotePylonFineTuneGateUnsafe)

    expect(() =>
      projectQwenRemotePylonFineTuneGate(
        gateInput({
          requiredShardCount: 0,
        }),
      ),
    ).toThrow(QwenRemotePylonFineTuneGateUnsafe)
  })
})
