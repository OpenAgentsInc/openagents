import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT,
  TASSADAR_PROOF_REPLAY_TAG,
  tassadarProofReplayView,
} from './tassadarProofReplayElement'

const replayBundle = {
  actors: [
    {
      actorRef: 'actor.pylon.orrery',
      avatarRole: 'contributor',
      displayName: 'Orrery pylon',
      fallbackAssetId: 'procedural.pylon_avatar.contributor.v1',
      pylonRef: 'pylon.448ba824b5fc879f3a59',
    },
    {
      actorRef: 'actor.validator',
      avatarRole: 'validator',
      displayName: 'Independent validator',
      fallbackAssetId: 'procedural.pylon_avatar.validator.v1',
    },
    {
      actorRef: 'actor.spark_treasury_terminal',
      avatarRole: 'settlement_terminal',
      displayName: 'Spark treasury terminal',
      fallbackAssetId: 'procedural.settlement_terminal.spark.v1',
    },
  ],
  bundleRef: 'proof_replay.tassadar.first_real_settlement.test',
  cameraCues: [
    {
      cueRef: 'camera.overview',
      durationSecond: 30,
      focusRefs: ['stage.tassadar.run_core'],
      mode: 'overview',
      sourceRefs: ['run.tassadar.executor.20260615'],
      startSecond: 0,
    },
    {
      cueRef: 'camera.zap',
      durationSecond: 20,
      focusRefs: ['stage.tassadar.settlement_terminal'],
      mode: 'zap_focus',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
      ],
      startSecond: 32,
    },
  ],
  captions: [
    {
      captionRef: 'caption.open',
      sequenceIndex: 0,
      sourceRefs: ['run.tassadar.executor.20260615'],
      text: 'The first real Tassadar settlement replay begins.',
      timelineSecond: 0,
    },
    {
      captionRef: 'caption.zap',
      sequenceIndex: 1,
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
      ],
      text: 'The Spark treasury rail confirms real Bitcoin movement.',
      timelineSecond: 38,
    },
  ],
  claimScope: 'evidence_presentation_only',
  events: [
    {
      actorRefs: ['actor.pylon.orrery'],
      displayText: 'Orrery pylon enters the replay stage.',
      eventRef: 'event.enter',
      kind: 'actor_entered_region',
      sequenceIndex: 0,
      sourceRefs: ['run.tassadar.executor.20260615'],
      targetRefs: ['stage.tassadar.run_core'],
      timelineSecond: 0,
    },
    {
      actorRefs: ['actor.validator'],
      displayText: 'Independent replay verifies the challenge digest.',
      eventRef: 'event.verified',
      kind: 'proof_verified',
      sequenceIndex: 1,
      sourceRefs: [
        'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c',
      ],
      targetRefs: ['stage.tassadar.proof_gate'],
      timelineSecond: 10,
    },
    {
      actorRefs: ['actor.spark_treasury_terminal'],
      caveat: 'Failed closed before dispatch; 0 sats moved.',
      displayText: 'Treasury adapter unavailable, failed closed.',
      eventRef: 'event.blocked',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 2,
      sourceRefs: [
        'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa',
      ],
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 24,
    },
    {
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: 1_000,
      displayText: '1,000 sats zap to Orrery is receipt-backed.',
      eventRef: 'event.zap',
      kind: 'payment_zap_confirmed',
      rail: 'spark_treasury',
      sequenceIndex: 3,
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
        '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
      ],
      targetRefs: ['actor.pylon.orrery', 'stage.pylon.orrery.station'],
      timelineSecond: 38,
    },
  ],
  flows: [],
  gaps: [],
  generatedAt: '2026-06-18T02:00:00.000Z',
  privacyLevel: 'public_safe',
  schemaVersion: 'proof_replay_bundle.v1',
  socialDisplayTime: '8:38pm, June 17',
  sourceAuthority: 'worker_d1_public',
  sourceRefs: [
    { kind: 'run', ref: 'run.tassadar.executor.20260615' },
    {
      kind: 'verification_challenge',
      ref: 'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c',
    },
    {
      kind: 'receipt',
      ref: 'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
    },
    {
      kind: 'api',
      ref: '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
    },
    {
      kind: 'forum_post',
      ref: 'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa',
      url: 'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa',
    },
  ],
  stages: [
    {
      label: 'Tassadar',
      sourceRefs: ['run.tassadar.executor.20260615'],
      stageKind: 'run_core',
      stageRef: 'stage.tassadar.run_core',
    },
    {
      label: 'Orrery pylon station',
      sourceRefs: ['pylon.448ba824b5fc879f3a59'],
      stageKind: 'pylon_station',
      stageRef: 'stage.pylon.orrery.station',
    },
    {
      label: 'Exact replay proof gate',
      sourceRefs: [
        'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c',
      ],
      stageKind: 'proof_gate',
      stageRef: 'stage.tassadar.proof_gate',
    },
    {
      label: 'Spark settlement terminal',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
      ],
      stageKind: 'settlement_terminal',
      stageRef: 'stage.tassadar.settlement_terminal',
    },
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['training_verification_challenge_recorded'],
  },
  title: 'Tassadar Run 1: First Real Bitcoin Settlement',
}

const jsonResponse = (body: unknown, status = 200): Response =>
  ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  }) as unknown as Response

const waitForSettled = async (el: HTMLElement): Promise<void> => {
  for (
    let i = 0;
    i < 50 && el.getAttribute('data-state') === 'loading';
    i += 1
  ) {
    await Promise.resolve()
    await new Promise(resolve => setTimeout(resolve, 0))
  }
}

const mountAndSettle = async (): Promise<HTMLElement> => {
  tassadarProofReplayView()
  const el = document.createElement(TASSADAR_PROOF_REPLAY_TAG)
  document.body.append(el)
  await waitForSettled(el)
  return el
}

describe('tassadar proof replay element', () => {
  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('loads the first-settlement replay bundle and renders controls', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(jsonResponse(replayBundle))

    const el = await mountAndSettle()

    expect(fetchSpy).toHaveBeenCalledWith(
      TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT,
      expect.objectContaining({
        headers: { accept: 'application/json' },
      }),
    )
    expect(el.getAttribute('data-state')).toBe('ok')
    expect(el.shadowRoot?.querySelector('[data-replay-stage]')).not.toBeNull()
    expect(el.shadowRoot?.querySelector('[data-replay-control="play"]')).not.toBeNull()
    expect(el.shadowRoot?.querySelector('[data-replay-control="scrub"]')).not.toBeNull()
    expect(el.shadowRoot?.querySelector('[data-replay-control="camera"]')).not.toBeNull()
    expect(el.shadowRoot?.textContent ?? '').toContain('Tassadar')
    expect(el.shadowRoot?.textContent ?? '').toContain(
      'The first real Tassadar settlement replay begins.',
    )
    expect(el.shadowRoot?.querySelector('[data-replay-zap="confirmed"]')).toBeNull()
  })

  it('does not render the real sats zap until the confirmed payment event', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle()

    expect(el.shadowRoot?.querySelector('[data-replay-zap="confirmed"]')).toBeNull()

    const scrub = el.shadowRoot?.querySelector(
      '[data-replay-control="scrub"]',
    ) as HTMLInputElement | null
    expect(scrub).not.toBeNull()
    scrub!.value = '39'
    scrub!.dispatchEvent(new Event('input', { bubbles: true }))

    const zap = el.shadowRoot?.querySelector('[data-replay-zap="confirmed"]')
    expect(el.getAttribute('data-replay-second')).toBe('39.0')
    expect(zap).not.toBeNull()
    expect(zap?.textContent ?? '').toContain('1000 sats spark_treasury')
  })

  it('opens source refs in the inspector when an event is selected', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle()

    const zapEvent = el.shadowRoot?.querySelector(
      '[data-replay-event-ref="event.zap"]',
    ) as HTMLButtonElement | null
    expect(zapEvent).not.toBeNull()
    zapEvent!.click()

    const inspector = el.shadowRoot?.querySelector('.inspector')
    expect(inspector?.textContent ?? '').toContain(
      'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
    )
    expect(inspector?.querySelector('a')?.getAttribute('href')).toContain(
      '/api/public/nexus-pylon/receipts/',
    )
  })
})
