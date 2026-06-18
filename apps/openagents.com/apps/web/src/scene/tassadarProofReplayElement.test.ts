import { TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT } from '@openagentsinc/proof-replay'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  TASSADAR_PROOF_REPLAY_TAG,
  TASSADAR_REPLAY_ORIGIN_DATA_KEY,
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
} as const

const recognitionReplayBundle = {
  ...replayBundle,
  actors: [
    {
      actorRef: 'actor.launch_recognition_terminal',
      avatarRole: 'recognition_terminal',
      displayName: 'Launch recognition terminal',
      fallbackAssetId: 'procedural.recognition_terminal.v1',
    },
    {
      actorRef: 'actor.orrery',
      avatarRole: 'recipient',
      displayName: 'Orrery',
      fallbackAssetId: 'procedural.pylon_avatar.recipient.orrery.v1',
    },
  ],
  bundleRef: 'proof_replay_bundle.launch_recognition.test',
  cameraCues: [
    {
      cueRef: 'camera.recognition',
      durationSecond: 18,
      focusRefs: ['stage.launch_recognition.orrery_overpayment'],
      mode: 'zap_focus',
      sourceRefs: ['doc.launch_recognition.test'],
      startSecond: 0,
    },
  ],
  captions: [
    {
      captionRef: 'caption.recognition',
      sequenceIndex: 0,
      sourceRefs: ['doc.launch_recognition.test'],
      text: 'Orrery overpayment is an exception lane.',
      timelineSecond: 12,
    },
  ],
  events: [
    {
      actorRefs: ['actor.orrery'],
      amountSats: 50_000,
      caveat: 'Intended recognition amount.',
      displayText: 'Orrery intended 50,000-sat recognition reward.',
      eventRef: 'event.recognition.intent',
      kind: 'recognition_reward_recorded',
      rail: 'split_lightning_address',
      sequenceIndex: 0,
      sourceRefs: ['doc.launch_recognition.test'],
      targetRefs: ['stage.launch_recognition.orrery'],
      timelineSecond: 2,
    },
    {
      actorRefs: ['actor.launch_recognition_terminal', 'actor.orrery'],
      amountSats: 50_000,
      displayText: 'Orrery recognition amount is covered by settled sends.',
      eventRef: 'event.recognition.zap',
      kind: 'payment_zap_confirmed',
      rail: 'split_lightning_address',
      sequenceIndex: 1,
      sourceRefs: ['recipient_confirmation.launch_recognition.orrery.test'],
      targetRefs: ['actor.orrery', 'stage.launch_recognition.orrery'],
      timelineSecond: 8,
    },
    {
      actorRefs: ['actor.launch_recognition_terminal', 'actor.orrery'],
      amountSats: 109_239,
      caveat: 'Overage documented as hazard pay.',
      displayText: 'Orrery overpayment detected.',
      eventRef: 'event.recognition.overpayment',
      kind: 'overpayment_detected',
      rail: 'split_lightning_address',
      sequenceIndex: 2,
      sourceRefs: ['recognition_ledger.launch_recognition.orrery.hazard_pay'],
      targetRefs: ['stage.launch_recognition.orrery_overpayment'],
      timelineSecond: 12,
    },
  ],
  flows: [],
  gaps: [],
  socialDisplayTime: 'June 17 closeout',
  sourceRefs: [
    { kind: 'doc', ref: 'doc.launch_recognition.test' },
    {
      kind: 'recipient_confirmation',
      ref: 'recipient_confirmation.launch_recognition.orrery.test',
    },
    {
      kind: 'payment_authority',
      ref: 'recognition_ledger.launch_recognition.orrery.hazard_pay',
    },
  ],
  stages: [
    {
      label: 'Launch recognition ledger',
      sourceRefs: ['doc.launch_recognition.test'],
      stageKind: 'recognition_terminal',
      stageRef: 'stage.launch_recognition.terminal',
    },
    {
      label: 'Orrery lane',
      sourceRefs: ['recipient_confirmation.launch_recognition.orrery.test'],
      stageKind: 'recognition_lane',
      stageRef: 'stage.launch_recognition.orrery',
    },
    {
      label: 'Orrery overpayment branch',
      sourceRefs: ['recognition_ledger.launch_recognition.orrery.hazard_pay'],
      stageKind: 'overpayment_branch',
      stageRef: 'stage.launch_recognition.orrery_overpayment',
    },
  ],
  title: 'Launch Recognition Payments: Trigger, Whitefang, Orrery',
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

const mountAndSettle = async (
  path = '/tassadar/replay/first-real-settlement',
  replaySlug?: string,
  replayOrigin?: string,
): Promise<HTMLElement> => {
  tassadarProofReplayView()
  window.history.replaceState({}, '', path)
  const el = document.createElement(TASSADAR_PROOF_REPLAY_TAG)
  if (replaySlug !== undefined) {
    el.setAttribute('data-replay-slug', replaySlug)
  }
  if (replayOrigin !== undefined) {
    el.setAttribute(`data-${TASSADAR_REPLAY_ORIGIN_DATA_KEY}`, replayOrigin)
  }
  document.body.append(el)
  await waitForSettled(el)
  return el
}

describe('tassadar proof replay element', () => {
  afterEach(() => {
    document.body.replaceChildren()
    window.history.replaceState({}, '', '/')
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
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="play"]'),
    ).not.toBeNull()
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="scrub"]'),
    ).not.toBeNull()
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="camera"]'),
    ).not.toBeNull()
    expect(el.shadowRoot?.textContent ?? '').toContain('Tassadar')
    expect(el.shadowRoot?.textContent ?? '').toContain(
      'The first real Tassadar settlement replay begins.',
    )
    expect(
      el.shadowRoot?.querySelector('canvas[data-proof-replay-webgl]'),
    ).not.toBeNull()
  })

  it('pauses from press events while the replay timer is running', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle()

    const play = el.shadowRoot?.querySelector(
      '[data-replay-control="play"]',
    ) as HTMLButtonElement | null
    expect(play).not.toBeNull()
    play!.dispatchEvent(
      new Event('pointerdown', {
        bubbles: true,
        cancelable: true,
      }),
    )

    const pause = el.shadowRoot?.querySelector(
      '[data-replay-control="play"]',
    ) as HTMLButtonElement | null
    expect(pause?.textContent).toBe('Pause')
    pause!.dispatchEvent(
      new Event('pointerdown', {
        bubbles: true,
        cancelable: true,
      }),
    )

    const resumed = el.shadowRoot?.querySelector(
      '[data-replay-control="play"]',
    ) as HTMLButtonElement | null
    expect(resumed?.textContent).toBe('Play')
  })

  it('keeps replay controls in a pointer-enabled layer above the stage', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle()

    const styleText = el.shadowRoot?.querySelector('style')?.textContent ?? ''
    expect(styleText).toContain('.top{position:relative;z-index:8;')
    expect(styleText).toContain('touch-action:manipulation')
    expect(styleText).toContain('.bottom{position:relative;z-index:6;')
    expect(styleText).toContain(
      '.controls,.events,.inspector{position:relative;z-index:7;pointer-events:auto}',
    )
    expect(styleText).toContain('touch-action:manipulation')
  })

  it('navigates to the live Tassadar route from the top action', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const assignSpy = vi
      .spyOn(window.location, 'assign')
      .mockImplementation(() => undefined)
    const el = await mountAndSettle()

    const live = el.shadowRoot?.querySelector(
      '[data-replay-control="live-tassadar"]',
    ) as HTMLAnchorElement | null
    expect(live).not.toBeNull()
    expect(live?.getAttribute('href')).toBe('/tassadar')
    live!.dispatchEvent(
      new Event('pointerdown', {
        bubbles: true,
        cancelable: true,
      }),
    )

    expect(assignSpy).toHaveBeenCalledWith('/tassadar')
  })

  it('renders a provided bundle without a browser fetch', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockRejectedValue(new Error('must not fetch'))
    tassadarProofReplayView([], replayBundle)
    const el = document.createElement(
      TASSADAR_PROOF_REPLAY_TAG,
    ) as HTMLElement & {
      bundle?: unknown
    }
    el.bundle = replayBundle
    document.body.append(el)
    await waitForSettled(el)

    expect(el.getAttribute('data-state')).toBe('ok')
    expect(el.shadowRoot?.textContent ?? '').toContain('Tassadar Run 1')
    expect(el.shadowRoot?.textContent ?? '').toContain('1,000 sats')
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  it('can load replay bundles from an explicit public origin for desktop embeds', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(jsonResponse(replayBundle))

    await mountAndSettle(
      '/desktop/autopilot',
      undefined,
      'https://openagents.com',
    )

    expect(fetchSpy).toHaveBeenCalledWith(
      `https://openagents.com${TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT}`,
      expect.objectContaining({
        headers: { accept: 'application/json' },
      }),
    )
  })

  it('loads non-first replay slugs from the generic proof replay resolver', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(jsonResponse(replayBundle))

    await mountAndSettle(
      '/tassadar/replay/launch-recognition-payments',
      'launch-recognition-payments',
    )

    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/public/proof-replays?ref=launch-recognition-payments',
      expect.objectContaining({
        headers: { accept: 'application/json' },
      }),
    )
  })

  it('drives recognition and overpayment moments through the WebGL replay frame', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      jsonResponse(recognitionReplayBundle),
    )
    const el = await mountAndSettle(
      '/tassadar/replay/launch-recognition-payments',
      'launch-recognition-payments',
    )

    const scrub = el.shadowRoot?.querySelector(
      '[data-replay-control="scrub"]',
    ) as HTMLInputElement | null
    expect(scrub).not.toBeNull()
    scrub!.value = '13'
    scrub!.dispatchEvent(new Event('input', { bubbles: true }))

    const world = el.shadowRoot?.querySelector('[data-replay-stage]')
    expect(world?.getAttribute('data-proof-replay-webgl')).toBe('unavailable')
    const canvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(canvas).not.toBeNull()
    expect(canvas?.getAttribute('data-proof-replay-second')).toBe('13.000')
    expect(canvas?.getAttribute('data-proof-replay-camera')).toBe('zap_focus')
    expect(el.shadowRoot?.textContent ?? '').toContain(
      'Orrery overpayment is an exception lane.',
    )
  })

  it('moves the WebGL replay frame to the confirmed real-sats moment', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle()

    const initialCanvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(initialCanvas?.getAttribute('data-proof-replay-second')).toBe(
      '0.000',
    )

    const scrub = el.shadowRoot?.querySelector(
      '[data-replay-control="scrub"]',
    ) as HTMLInputElement | null
    expect(scrub).not.toBeNull()
    scrub!.value = '39'
    scrub!.dispatchEvent(new Event('input', { bubbles: true }))

    const canvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(el.getAttribute('data-replay-second')).toBe('39.0')
    expect(canvas?.getAttribute('data-proof-replay-second')).toBe('39.000')
    expect(canvas?.getAttribute('data-proof-replay-camera')).toBe('zap_focus')
    expect(el.shadowRoot?.textContent ?? '').toContain(
      'The Spark treasury rail confirms real Bitcoin movement.',
    )
  })

  it('renders the deterministic social share cut chrome from query params', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle(
      '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
    )

    expect(el.getAttribute('data-replay-presentation')).toBe('social')
    expect(el.getAttribute('data-social-duration')).toBe('60')
    expect(
      el.shadowRoot?.querySelector('[data-social-hud="social"]'),
    ).not.toBeNull()
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="play"]'),
    ).toBeNull()
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="scrub"]'),
    ).toBeNull()

    const canvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(canvas).not.toBeNull()
    expect(canvas?.getAttribute('data-proof-replay-webgl')).toBe(
      'unavailable',
    )

    const text = el.shadowRoot?.textContent ?? ''
    expect(text).toContain('Tassadar Run 1: first real Bitcoin settlement')
    expect(text).toContain(
      'Verified work -> owner gate -> Spark zap -> public receipt',
    )
    expect(text).toContain('8:38pm, June 17')
    expect(text).not.toMatch(
      /spark1|bolt11|preimage|mnemonic|api[_ -]?key|service[_ -]?token|bearer|payment_hash|private log|raw prompt/i,
    )
    expect(
      el.shadowRoot?.querySelector('[data-proof-replay-webgl-mount="true"]'),
    ).not.toBeNull()
  })

  it('shows the confirmed zap and receipt-backed end card late in the social cut', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = await mountAndSettle(
      '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social&start=56',
    )

    const canvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(canvas?.getAttribute('data-proof-replay-second')).toBe('56.000')
    const endCard = el.shadowRoot?.querySelector(
      '[data-social-end-card="settled"]',
    )
    expect(endCard?.textContent ?? '').toContain('1,000 sats settled')
    expect(endCard?.textContent ?? '').toContain('realBitcoinMoved:true')
    expect(endCard?.textContent ?? '').toContain(
      'receipt.nexus.tassadar_run_settlement...v6.20260618',
    )
    expect(endCard?.querySelector('a')?.getAttribute('href')).toContain(
      '/api/public/nexus-pylon/receipts/',
    )
  })

  it('can seek the social replay through the programmatic frame driver', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(replayBundle))
    const el = (await mountAndSettle(
      '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
    )) as HTMLElement & {
      driveReplayFrame: (input: {
        second?: number
        cameraMode?: string
        cameraPose?: {
          fov?: number
          position?: { x: number; y: number; z: number }
          target?: { x: number; y: number; z: number }
        }
      }) => {
        fov?: number
        mode: string
        position: { x: number; y: number; z: number }
        second: number
      } | null
    }

    const pose = el.driveReplayFrame({
      cameraMode: 'zap_focus',
      cameraPose: {
        fov: 34,
        position: { x: 7, y: 4, z: 2 },
        target: { x: 0, y: 0, z: 0 },
      },
      second: 39,
    })

    expect(pose?.mode).toBe('zap_focus')
    expect(pose?.second).toBe(39)
    expect(pose?.fov).toBe(34)
    expect(pose?.position).toEqual({ x: 7, y: 4, z: 2 })
    expect(el.getAttribute('data-replay-second')).toBe('39.0')
    expect(el.getAttribute('data-replay-camera')).toBe('zap_focus')
    const world = el.shadowRoot?.querySelector('[data-replay-stage]')
    expect(world?.getAttribute('data-camera-fov')).toBe('34.00')
    expect(
      el.shadowRoot?.querySelector('[data-replay-control="scrub"]'),
    ).toBeNull()
    const canvas = el.shadowRoot?.querySelector(
      'canvas[data-proof-replay-webgl]',
    ) as HTMLCanvasElement | null
    expect(canvas?.getAttribute('data-proof-replay-second')).toBe('39.000')
    expect(canvas?.getAttribute('data-proof-replay-camera')).toBe('zap_focus')
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
