import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  QwenRemotePylonFineTuneGateInput,
  QwenRemotePylonFineTuneWorkerReceipt,
} from './qwen-remote-pylon-finetune-gate'
import {
  QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF,
  QWEN_REMOTE_PYLON_TRAINING_REQUIRED_CAPABILITY_REF,
  QwenRemotePylonLiveTrainingSmokeProjection,
  projectQwenRemotePylonLiveTrainingPreflight,
  projectQwenRemotePylonLiveTrainingSmoke,
} from './qwen-remote-pylon-live-training-smoke'

const shardRefs = (prefix: string): ReadonlyArray<string> =>
  Array.from(
    { length: 15 },
    (_, index) => `shard_receipt.public.qwen3_6.live.${prefix}.${index + 1}`,
  )

const worker = (
  suffix: string,
  overrides: Partial<QwenRemotePylonFineTuneWorkerReceipt> = {},
): QwenRemotePylonFineTuneWorkerReceipt =>
  new QwenRemotePylonFineTuneWorkerReceipt({
    artifactRefs: [`artifact.public.qwen3_6.live_lora.${suffix}`],
    deviceScope: 'remote_pylon',
    quarantineRefs: [],
    shardReceiptRefs: shardRefs(suffix),
    signedWorkerReceiptRefs: [
      `worker_receipt.public.qwen3_6.live_lora.${suffix}.signed`,
    ],
    workerRef: `pylon.public.qwen_training.${suffix}`,
    ...overrides,
  })

const gateInput = (
  overrides: Partial<QwenRemotePylonFineTuneGateInput> = {},
): QwenRemotePylonFineTuneGateInput =>
  new QwenRemotePylonFineTuneGateInput({
    adapterAdmissionRefs: ['admission.public.qwen3_6.live_lora.accepted'],
    evalReceiptRefs: ['eval_receipt.public.qwen3_6.live_lora.harvey'],
    harveyScope: 'public_replay',
    mergeReceiptRefs: ['merge_receipt.public.qwen3_6.live_lora'],
    modelRef: 'model.public.qwen3_6_27b.remote_lora',
    paymentReceiptRefs: ['payment_receipt.public.qwen3_6.live_lora'],
    paymentState: 'settled_bitcoin',
    publicProjectionRefs: ['projection.public.qwen3_6.live_lora.report'],
    requiredShardCount: 15,
    runRef: 'training_run.public.qwen3_6.live_pylon.lora',
    settlementReceiptRefs: ['settlement_receipt.public.qwen3_6.live_lora'],
    trainingMode: 'sampled_projection_lora',
    workerReceipts: [worker('alpha'), worker('beta')],
    ...overrides,
  })

const candidate = (
  suffix: string,
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  capabilityRefs: [
    QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF,
    QWEN_REMOTE_PYLON_TRAINING_REQUIRED_CAPABILITY_REF,
  ],
  displayName: `Qwen worker ${suffix}`,
  latestHeartbeatDisplay: 'Just now',
  latestHeartbeatStatus: 'online',
  pylonRef: `pylon.public.qwen_training.${suffix}`,
  status: 'active',
  walletReady: true,
  ...overrides,
})

describe('Qwen remote Pylon live training smoke', () => {
  test('blocks synthetic, stale, and capability-missing public Pylon candidates', () => {
    const preflight = projectQwenRemotePylonLiveTrainingPreflight({
      candidates: [
        candidate('alpha', {
          pylonRef: 'pylon.codex.live_smoke.20260611',
        }),
        candidate('beta', {
          capabilityRefs: [QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF],
        }),
        candidate('gamma', {
          latestHeartbeatDisplay: '20 minutes ago',
          latestHeartbeatStatus: 'offline',
        }),
      ],
    })

    expect(preflight.state).toBe('blocked')
    expect(preflight.candidatePylonRefs).toEqual([])
    expect(preflight.blockerRefs).toContain(
      'blocker.public.qwen_remote_training.live_training_pylons_missing',
    )
    expect(preflight.blockerRefs).toContain(
      'blocker.public.qwen_remote_training.synthetic_pylons_selected',
    )
    expect(preflight.blockerRefs).toContain(
      'blocker.public.qwen_remote_training.required_capability_missing',
    )
    expect(preflight.blockerRefs).toContain(
      'blocker.public.qwen_remote_training.pylons_not_fresh_wallet_ready',
    )
  })

  test('allows two fresh real training-capable Pylons through preflight', () => {
    const preflight = projectQwenRemotePylonLiveTrainingPreflight({
      candidates: [candidate('alpha'), candidate('beta')],
    })

    expect(preflight).toMatchObject({
      assignmentCapabilityRef: QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF,
      blockerRefs: [],
      candidatePylonRefs: [
        'pylon.public.qwen_training.alpha',
        'pylon.public.qwen_training.beta',
      ],
      requiredCapabilityRef: QWEN_REMOTE_PYLON_TRAINING_REQUIRED_CAPABILITY_REF,
      state: 'green',
    })
  })

  test('projects a green bounded live training smoke without full-transformer overclaim', () => {
    const projection = projectQwenRemotePylonLiveTrainingSmoke({
      gateInput: gateInput(),
      preflight: projectQwenRemotePylonLiveTrainingPreflight({
        candidates: [candidate('alpha'), candidate('beta')],
      }),
    })

    expect(
      S.decodeUnknownSync(QwenRemotePylonLiveTrainingSmokeProjection)(projection),
    ).toEqual(projection)
    expect(projection).toMatchObject({
      blockerRefs: [],
      state: 'green',
    })
    expect(projection.gate.qwenRemoteBoundedTrainingClaimAllowed).toBe(true)
    expect(projection.gate.qwenRemoteFineTuneClaimAllowed).toBe(false)
    expect(projection.gate.fullQwenBackpropClaimAllowed).toBe(false)
    expect(projection.gate.settledBitcoinClaimAllowed).toBe(true)
    expect(projection.gate.scopeLanguage).toContain('bounded LoRA/adaptation report')
  })

  test('blocks a ready gate bundle when live preflight cannot verify the worker refs', () => {
    const projection = projectQwenRemotePylonLiveTrainingSmoke({
      gateInput: gateInput(),
      preflight: projectQwenRemotePylonLiveTrainingPreflight({
        candidates: [candidate('gamma'), candidate('delta')],
      }),
    })

    expect(projection.state).toBe('blocked')
    expect(projection.gate.decision).toBe('ready')
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_training.worker_preflight_missing',
    )
  })

  test('keeps missing settlement receipts blocked even with a green live preflight', () => {
    const projection = projectQwenRemotePylonLiveTrainingSmoke({
      gateInput: gateInput({
        paymentState: 'payable_pending_settlement',
        settlementReceiptRefs: [],
      }),
      preflight: projectQwenRemotePylonLiveTrainingPreflight({
        candidates: [candidate('alpha'), candidate('beta')],
      }),
    })

    expect(projection.state).toBe('blocked')
    expect(projection.gate.settledBitcoinClaimAllowed).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.payable_is_not_settled_bitcoin',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.public.qwen_remote_finetune.settlement_receipts_missing',
    )
  })
})
