import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  TASSADAR_RUN_SUMMARY_ENDPOINT,
  TASSADAR_RUN_TAG,
  dataStateForSummary,
  nextTassadarLocalAvatarPosition,
  proofLinkForSelection,
  pylonAttentionForAvatar,
  sanitizeTassadarChatBody,
  tassadarRunView,
  tassadarSupportHudItems,
} from './tassadarRunElement'
import {
  TASSADAR_SPACETIME_DATABASE_ATTRIBUTE,
  TASSADAR_SPACETIME_WORLD_URL_ATTRIBUTE,
} from './tassadarSpacetimeWorld'

// Page-wiring test for the live Tassadar run scene element (#5118). We stub
// `fetch` to exercise the three honest states the page must handle — populated,
// idle/empty, and a non-200 fetch error — and assert the element drives the
// adapter + renderer accordingly. The pure snapshot adapter is already covered
// by tassadarRunSnapshot.test.ts; here we only test the fetch → state →
// options-handoff wiring, NOT the WebGL scene. To keep happy-dom from booting a
// real WebGL mount, we replace the heavy three-effect renderer with an inert
// stub element that simply records the `visualization` property it receives.

const STUB_TAG = 'oa-training-run-stub'

const { recordedVisualizations } = vi.hoisted(() => ({
  recordedVisualizations: [] as Array<unknown>,
}))

vi.mock('@openagentsinc/three-effect/foldkit', () => {
  const tag = 'oa-training-run-stub'
  class StubRun extends HTMLElement {
    set visualization(value: unknown) {
      recordedVisualizations.push(value)
    }
  }
  if (
    typeof customElements !== 'undefined' &&
    customElements.get(tag) === undefined
  ) {
    customElements.define(tag, StubRun)
  }
  return {
    trainingRunTagName: tag,
    registerTrainingRunElement: () => {},
  }
})

vi.mock('./spacetimeWorldBindings', () => ({
  DbConnection: {
    builder: () => {
      let onConnectError: ((ctx: unknown, error: Error) => void) | undefined
      const builder = {
        build: () => {
          queueMicrotask(() => {
            onConnectError?.({}, new Error('spacetime unavailable'))
          })
          return { disconnect: () => {} }
        },
        onConnect: () => builder,
        onConnectError: (callback: (ctx: unknown, error: Error) => void) => {
          onConnectError = callback
          return builder
        },
        onDisconnect: () => builder,
        withCompression: () => builder,
        withDatabaseName: () => builder,
        withUri: () => builder,
      }
      return builder
    },
  },
}))

const populated = {
  generatedAt: '2026-06-17T16:39:20.270Z',
  runRef: 'run.tassadar.executor.20260615',
  runLabel: 'Tassadar executor run',
  runState: 'active',
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
  },
  emptyState: { idle: false },
  metrics: {
    assignedContributorCount: { value: 0 },
    activeWindowCount: { value: 2 },
    verifiedWorkCount: { value: 9 },
    providerConfirmedSettledPayoutSats: { value: 2100 },
    qualifiedContributorCount: { value: 1 },
  },
  corpus: {
    acceptedTraceCount: 1,
    traceRefs: ['trace.tassadar.accepted.1'],
    verdictRefs: ['verdict.tassadar.replay.1'],
  },
  realGradient: {
    leaderboardRows: [
      {
        pylonRef: 'pylon.worker.one',
        rank: 1,
        settledPayoutSats: 0,
        sourceRefs: ['training.lease.worker.one'],
        verifiedWindowCount: 1,
      },
    ],
    verifiedReplayPairs: [
      {
        challengeRef: 'challenge.tassadar.replay.1',
        validatorRef: 'validator.tassadar.1',
        verdictRefs: ['verdict.tassadar.replay.1'],
        workerRef: 'contribution.tassadar.worker.1',
      },
    ],
    rejectedReplayPairs: [
      {
        challengeRef: 'challenge.tassadar.replay.rejected.1',
        failureCodes: ['DigestMismatch'],
        validatorRef: 'validator.tassadar.rejected.1',
        verdictRefs: ['verdict.tassadar.replay.rejected.1'],
        workerRef: 'contribution.tassadar.worker.rejected.1',
      },
    ],
  },
  receiptRefs: [
    'receipt.nexus.tassadar_run_settlement.public_summary_test',
    'receipt.forum.1',
  ],
  settlementRows: [
    {
      amountSats: 21,
      apiUrl:
        '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
      contributorRef: 'pylon.worker.one',
      movementMode: 'simulation',
      realBitcoinMoved: false,
      receiptKind: 'settlement_recorded',
      receiptPageUrl:
        '/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
      receiptRef: 'receipt.nexus.tassadar_run_settlement.public_summary_test',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.public_summary_test',
        'pylon.worker.one',
        'challenge.tassadar.replay.1',
      ],
      state: 'settled',
      trainingRunRef: 'run.tassadar.executor.20260615',
      verificationChallengeRef: 'challenge.tassadar.replay.1',
    },
  ],
  windows: [{ windowRef: 'training.window.tassadar.executor.20260615.w1' }],
}

const idle = {
  runRef: 'run.tassadar.executor.20260615',
  emptyState: { idle: true, reason: 'no verified work yet' },
}

const jsonResponse = (body: unknown, status = 200): Response =>
  ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  }) as unknown as Response

// Mount the element into the document and wait for its async fetch to settle by
// polling the data-state off the loading state.
const mountAndSettle = async (
  configure?: (element: HTMLElement) => void,
): Promise<HTMLElement> => {
  // Force registration via the view helper, then create the element directly.
  tassadarRunView()
  const el = document.createElement(TASSADAR_RUN_TAG)
  configure?.(el)
  document.body.append(el)
  await waitForSettled(el)
  return el
}

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

const waitForSpacetimeState = async (
  el: HTMLElement,
  state: string,
): Promise<void> => {
  for (
    let i = 0;
    i < 50 && el.getAttribute('data-spacetime-state') !== state;
    i += 1
  ) {
    await Promise.resolve()
    await new Promise(resolve => setTimeout(resolve, 0))
  }
}

describe('tassadarRunView page wiring', () => {
  beforeEach(() => {
    recordedVisualizations.length = 0
  })

  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('fetches the public summary endpoint on connect', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(jsonResponse(populated))
    await mountAndSettle()
    expect(fetchSpy).toHaveBeenCalledTimes(1)
    expect(fetchSpy.mock.calls[0]?.[0]).toBe(TASSADAR_RUN_SUMMARY_ENDPOINT)
  })

  it('(a) populated summary → ok state, mounts renderer with produced options', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))
    const el = await mountAndSettle()
    expect(el.getAttribute('data-state')).toBe('ok')
    // The renderer received options derived from the summary (not faked, not empty).
    expect(recordedVisualizations).toHaveLength(1)
    const options = recordedVisualizations[0] as Record<string, unknown>
    expect(options).toBeTruthy()
    expect(typeof options).toBe('object')
    expect(options.cameraMode).toBe('perspective_walk')
    expect(options.controller).toBe('wasd_mouselook')
    expect(options.walkController).toMatchObject({
      bounds: { minX: -160, maxX: 160, minZ: -160, maxZ: 160 },
      eyeHeight: 1.65,
      initialPosition: [0, 1.65, 5.6],
      movementSpeed: 4.5,
      sprintMultiplier: 1.8,
    })
    expect(options.walkController).not.toHaveProperty('lockSelector')
    expect(
      el.shadowRoot?.querySelector('[data-tassadar-enter-world]'),
    ).toBeNull()
    const walkController = options.walkController as {
      debug?: (snapshot: {
        applied: boolean
        event: 'mousemove'
        locked: boolean
        movementX: number
        movementY: number
        pitch: number
        source: 'controller'
        yaw: number
      }) => void
      onLockChange?: (locked: boolean) => void
    }
    expect(typeof walkController.debug).toBe('function')
    walkController.onLockChange?.(true)
    expect(el.getAttribute('data-pointer-lock')).toBe('locked')
    walkController.debug?.({
      applied: true,
      event: 'mousemove',
      locked: true,
      movementX: 11,
      movementY: -3,
      pitch: 0.25,
      source: 'controller',
      yaw: -0.5,
    })
    expect(el.getAttribute('data-mouselook-count')).toBe('1')
    expect(el.getAttribute('data-mouselook-event')).toBe('mousemove')
    expect(el.getAttribute('data-mouselook-locked')).toBe('true')
    expect(el.getAttribute('data-mouselook-applied')).toBe('true')
    expect(el.getAttribute('data-mouselook-delta')).toBe('11,-3')
    expect(el.getAttribute('data-mouselook-last-nonzero')).toBe('11,-3')
    walkController.onLockChange?.(false)
    expect(el.getAttribute('data-pointer-lock')).toBe('released')
    // The underlying renderer element was mounted.
    expect(el.shadowRoot?.querySelector(STUB_TAG)).not.toBeNull()
  })

  it('keeps the Worker-summary scene when SpacetimeDB is disabled', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))
    const el = await mountAndSettle()

    expect(el.getAttribute('data-state')).toBe('ok')
    expect(el.getAttribute('data-spacetime-state')).toBeNull()
    expect(recordedVisualizations).toHaveLength(1)
    expect(el.shadowRoot?.querySelector(STUB_TAG)).not.toBeNull()
  })

  it('keeps the Worker-summary scene when the enabled SpacetimeDB subscription is unreachable', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))

    const el = await mountAndSettle(element => {
      element.setAttribute(
        TASSADAR_SPACETIME_WORLD_URL_ATTRIBUTE,
        'https://spacetime.invalid',
      )
      element.setAttribute(
        TASSADAR_SPACETIME_DATABASE_ATTRIBUTE,
        'openagents-world',
      )
    })
    await waitForSpacetimeState(el, 'error')

    expect(el.getAttribute('data-state')).toBe('ok')
    expect(el.getAttribute('data-spacetime-state')).toBe('error')
    expect(recordedVisualizations).toHaveLength(1)
    expect(el.shadowRoot?.querySelector(STUB_TAG)).not.toBeNull()
  })

  it('renders live snapshot metadata without manual refresh chrome', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))

    const el = await mountAndSettle()
    const status = el.shadowRoot?.querySelector('.status')
    expect(status?.textContent ?? '').toContain(
      'run.tassadar.executor.20260615',
    )
    expect(status?.textContent ?? '').toContain('active')
    expect(status?.textContent ?? '').toContain('2026-06-17T16:39:20.270Z')
    expect(status?.querySelector('.legend')).not.toBeNull()
    expect(
      status
        ?.querySelector('[data-tassadar-replay-link="first-real-settlement"]')
        ?.getAttribute('href'),
    ).toBe('/tassadar/replay/first-real-settlement')
    expect(status?.textContent ?? '').toContain('registered')
    expect(status?.textContent ?? '').toContain('world rows')
    expect(status?.textContent ?? '').not.toContain('Refresh snapshot')
    expect(el.shadowRoot?.querySelector('.status button')).toBeNull()

    expect(el.shadowRoot?.querySelector('.promise-gate')).toBeNull()
  })

  it('does not render product-promise gates or fleet stats in the main scene chrome', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))

    const el = await mountAndSettle()
    expect(el.shadowRoot?.querySelector('.promise-gate')).toBeNull()
    expect(el.shadowRoot?.textContent ?? '').not.toContain('Promise gates')
    expect(el.shadowRoot?.textContent ?? '').not.toContain('Fleet pylon stats')
  })

  it('(b) idle summary → empty state, still renders the honest (zeroed) scene — never faked', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(idle))
    const el = await mountAndSettle()
    expect(el.getAttribute('data-state')).toBe('empty')
    // Honest empty: a real scene is still mounted from real (zeroed) options.
    expect(recordedVisualizations).toHaveLength(1)
    expect(el.shadowRoot?.querySelector(STUB_TAG)).not.toBeNull()
    // No error overlay.
    expect(el.shadowRoot?.querySelector('.overlay')).toBeNull()
  })

  it('(c) non-200 response → error state, no scene, honest error message', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      jsonResponse({ error: 'boom' }, 503),
    )
    const el = await mountAndSettle()
    expect(el.getAttribute('data-state')).toBe('error')
    // No renderer mounted, no fabricated metrics.
    expect(recordedVisualizations).toHaveLength(0)
    expect(el.shadowRoot?.querySelector(STUB_TAG)).toBeNull()
    const overlay = el.shadowRoot?.querySelector('.overlay')
    expect(overlay).not.toBeNull()
    expect(overlay?.textContent ?? '').toContain('503')
  })

  it('resolves node-selected events to an in-page public proof drawer without opening a tab', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))
    const openSpy = vi.spyOn(window, 'open').mockImplementation(() => null)
    const el = await mountAndSettle()
    const run = el.shadowRoot?.querySelector(STUB_TAG)
    expect(run).not.toBeNull()

    run?.dispatchEvent(
      new CustomEvent('node-selected', {
        bubbles: true,
        composed: true,
        detail: {
          detail: 'verified',
          id: 'contribution.tassadar.worker.1',
          label: 'W1',
          role: 'run',
          status: 'verified',
        },
      }),
    )

    expect(openSpy).not.toHaveBeenCalled()
    const selection = el.shadowRoot?.querySelector('.selection')
    expect(selection?.getAttribute('data-proof-state')).toBe('linked')
    expect(selection?.textContent ?? '').toContain('Verified replay challenge')
    expect(selection?.textContent ?? '').toContain(
      '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=challenge.tassadar.replay.1',
    )
    expect(selection?.textContent ?? '').toContain('Open proof')
  })

  it('routes Nexus/Pylon settlement receipts to the public receipt API with simulation caveats', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))
    const openSpy = vi.spyOn(window, 'open').mockImplementation(() => null)
    const el = await mountAndSettle()
    const run = el.shadowRoot?.querySelector(STUB_TAG)

    run?.dispatchEvent(
      new CustomEvent('node-selected', {
        bubbles: true,
        composed: true,
        detail: {
          detail: 'settlement',
          id: 'settlement',
          label: 'settlement',
          role: 'rung',
          status: 'settled',
        },
      }),
    )

    expect(openSpy).not.toHaveBeenCalled()
    const selection = el.shadowRoot?.querySelector('.selection')
    expect(selection?.getAttribute('data-proof-state')).toBe('linked')
    expect(selection?.textContent ?? '').toContain('settlement_recorded')
    expect(selection?.textContent ?? '').toContain(
      '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
    )
    expect(selection?.textContent ?? '').toContain(
      'Simulation-backed settlement record',
    )
    expect(selection?.textContent ?? '').toContain('real bitcoin moved: no')
  })

  it('leaves an unlinked selection panel when no public proof ref exists', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(populated))
    const openSpy = vi.spyOn(window, 'open').mockImplementation(() => null)
    const el = await mountAndSettle()
    const run = el.shadowRoot?.querySelector(STUB_TAG)

    run?.dispatchEvent(
      new CustomEvent('node-selected', {
        bubbles: true,
        composed: true,
        detail: {
          detail: 'stale <= 5',
          id: 'state_synced',
          label: 'state synced',
          role: 'lifecycle',
          status: 'sync',
        },
      }),
    )

    expect(openSpy).not.toHaveBeenCalled()
    const selection = el.shadowRoot?.querySelector('.selection')
    expect(selection?.getAttribute('data-proof-state')).toBe('unlinked')
    expect(selection?.textContent ?? '').toContain(
      'No public proof ref is linked yet',
    )
  })

  it('network rejection → error state, no scene', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('offline'))
    const el = await mountAndSettle()
    expect(el.getAttribute('data-state')).toBe('error')
    expect(recordedVisualizations).toHaveLength(0)
    expect(el.shadowRoot?.querySelector('.overlay')).not.toBeNull()
  })
})

describe('dataStateForSummary', () => {
  it('classifies idle as empty and non-idle as ok', () => {
    expect(dataStateForSummary(idle)).toBe('empty')
    expect(dataStateForSummary(populated)).toBe('ok')
    expect(dataStateForSummary({})).toBe('ok')
  })
})

describe('tassadarSupportHudItems', () => {
  it('keeps lifecycle/status context as compact support text, not map nodes', () => {
    expect(
      tassadarSupportHudItems({
        metrics: {
          activeWindowCount: { value: 2 },
          assignedContributorCount: { value: 4 },
          qualifiedContributorCount: { value: 1 },
          receiptRefCount: { value: 3 },
        },
        realGradient: {
          deviceRequirement: {
            observedDistinctContributorDevices: 6,
            requiredDistinctContributorDevices: 2,
          },
          externalAsk: { blockerRefs: ['blocker.one'] },
        },
        settlementRows: [
          {
            receiptRef: 'receipt.nexus.tassadar.example',
          },
        ],
        staleness: { maxStalenessSeconds: 5 },
        world: {
          avatarPositions: [{ avatarRef: 'avatar.one' } as never],
          pylonStations: [
            { pylonRef: 'pylon.one' } as never,
            { pylonRef: 'pylon.two' } as never,
          ],
        },
      }),
    ).toEqual([
      ['registered', '6 pylons'],
      ['qualified', '6/2 device gate'],
      ['state synced', '<= 5s'],
      ['active', '2 windows'],
      ['sync reentry', '1 blockers'],
      ['world rows', '2 stations / 1 avatars'],
      ['proof refs', '3 receipts'],
    ])
  })
})

describe('MVP avatar movement and pylon attention mapping', () => {
  it('integrates local WASD movement inside the server-enforced run bounds', () => {
    const next = nextTassadarLocalAvatarPosition(
      {
        movementMode: 'idle',
        pitch: 0,
        positionX: 159.9,
        positionY: 0,
        positionZ: -159.9,
        yaw: Math.PI / 2,
      },
      {
        backward: false,
        forward: true,
        left: false,
        right: true,
        sprint: true,
      },
      1_000,
    )

    expect(next.movementMode).toBe('running')
    expect(next.positionX).toBeLessThanOrEqual(160)
    expect(next.positionZ).toBeGreaterThanOrEqual(-160)
  })

  it('maps nearby, looking, and selected inspection states to reducer-safe attention rows', () => {
    const summary = {
      world: {
        pylonStations: [
          {
            interactionRadiusMeters: 2.4,
            label: 'P1',
            position: { x: 0, y: 0, z: 0 },
            pylonRef: 'pylon.worker.one',
            regionRef: 'region.run.tassadar.executor.20260615.main',
            sourceUrl:
              '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.worker.one',
          },
        ],
      },
    }

    expect(
      pylonAttentionForAvatar(
        summary,
        {
          movementMode: 'idle',
          pitch: 0,
          positionX: 0,
          positionY: 0,
          positionZ: 1,
          yaw: 0,
        },
        null,
      ),
    ).toMatchObject({
      attentionKind: 'looking',
      distanceMeters: 1,
      pylonRef: 'pylon.worker.one',
    })

    expect(
      pylonAttentionForAvatar(
        summary,
        {
          movementMode: 'idle',
          pitch: 0,
          positionX: 1,
          positionY: 0,
          positionZ: 0,
          yaw: 0,
        },
        null,
      ),
    ).toMatchObject({
      attentionKind: 'nearby',
      pylonRef: 'pylon.worker.one',
    })

    expect(
      pylonAttentionForAvatar(
        summary,
        {
          movementMode: 'idle',
          pitch: 0,
          positionX: 6,
          positionY: 0,
          positionZ: 6,
          yaw: 0,
        },
        'pylon.worker.one',
      ),
    ).toMatchObject({
      attentionKind: 'inspecting',
      pylonRef: 'pylon.worker.one',
      sourceEntityRef: 'pylon.worker.one',
    })
  })
})

describe('local chat input constraints', () => {
  it('keeps chat plain, non-empty, and bounded before reducer submission', () => {
    expect(sanitizeTassadarChatBody('  hello\n\t nearby   agents  ')).toBe(
      'hello nearby agents',
    )
    expect(sanitizeTassadarChatBody('   ')).toBeNull()
    expect(sanitizeTassadarChatBody('x'.repeat(320))).toHaveLength(280)
  })
})

describe('proofLinkForSelection', () => {
  it('maps verification and receipt selections to public-safe proof URLs', () => {
    const populatedWithWorld = {
      ...populated,
      world: {
        agentAvatars: [
          {
            actorKind: 'pylon_agent',
            avatarRef: 'avatar.pylon_agent.pylon.worker.one',
            displayName: 'P1 agent',
            homePylonRef: 'pylon.worker.one',
          },
        ],
        pylonStations: [
          {
            interactionRadiusMeters: 2.4,
            label: 'P1',
            position: { x: -2.35, y: 0, z: 1.5 },
            pylonRef: 'pylon.worker.one',
            regionRef: 'region.run.tassadar.executor.20260615.main',
            sourceUrl:
              '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.worker.one',
          },
        ],
      },
    }

    expect(
      proofLinkForSelection(populated, {
        detail: 'verified',
        id: 'validator.tassadar.1',
        label: 'V1',
        role: 'run',
        status: 'verified',
      }),
    ).toEqual({
      caveats: [],
      href: '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=challenge.tassadar.replay.1',
      kind: 'training_ref',
      label: 'Verified replay challenge',
      ref: 'challenge.tassadar.replay.1',
      sourceRefs: [],
      state: 'linked',
    })

    expect(
      proofLinkForSelection(populated, {
        detail: '20 receipts',
        id: 'receipt',
        label: 'receipt',
        role: 'receipt',
        status: 'verified',
      }),
    ).toEqual({
      caveats: [
        'Amount: 21 sats',
        'Simulation-backed settlement record; this does not prove real Bitcoin moved.',
      ],
      href: '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
      kind: 'settlement_recorded',
      label: 'Settlement receipt',
      ref: 'receipt.nexus.tassadar_run_settlement.public_summary_test',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.public_summary_test',
        'pylon.worker.one',
        'challenge.tassadar.replay.1',
      ],
      state: 'settled; simulation; real bitcoin moved: no',
    })

    expect(
      proofLinkForSelection(populatedWithWorld, {
        detail: 'station',
        id: 'station.pylon.worker.one',
        label: 'P1 hub',
        role: 'run',
        status: 'active',
      }),
    ).toEqual({
      caveats: [
        'Amount: 21 sats',
        'Simulation-backed settlement record; this does not prove real Bitcoin moved.',
      ],
      href: '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
      kind: 'settlement_recorded',
      label: 'Settlement receipt',
      ref: 'receipt.nexus.tassadar_run_settlement.public_summary_test',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.public_summary_test',
        'pylon.worker.one',
        'challenge.tassadar.replay.1',
      ],
      state: 'settled; simulation; real bitcoin moved: no',
    })

    expect(
      proofLinkForSelection(populatedWithWorld, {
        detail: 'avatar',
        id: 'avatar.pylon_agent.pylon.worker.one',
        label: 'P1 agent',
        role: 'run',
        status: 'active',
      }),
    ).toEqual({
      caveats: [
        'Amount: 21 sats',
        'Simulation-backed settlement record; this does not prove real Bitcoin moved.',
      ],
      href: '/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.public_summary_test',
      kind: 'settlement_recorded',
      label: 'Settlement receipt',
      ref: 'receipt.nexus.tassadar_run_settlement.public_summary_test',
      sourceRefs: [
        'receipt.nexus.tassadar_run_settlement.public_summary_test',
        'pylon.worker.one',
        'challenge.tassadar.replay.1',
      ],
      state: 'settled; simulation; real bitcoin moved: no',
    })

    expect(
      proofLinkForSelection(populated, {
        detail: 'rejected',
        id: 'contribution.tassadar.worker.rejected.1',
        label: 'RW1',
        role: 'run',
        status: 'active',
      }),
    ).toEqual({
      caveats: [],
      href: '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=challenge.tassadar.replay.rejected.1',
      kind: 'training_ref',
      label: 'Rejected replay challenge',
      ref: 'challenge.tassadar.replay.rejected.1',
      sourceRefs: [],
      state: 'linked',
    })

    expect(
      proofLinkForSelection(populated, {
        detail: 'accepted_trace',
        id: 'trace.tassadar.accepted.1',
        label: 'T1',
        role: 'run',
        status: 'active',
      }),
    ).toEqual({
      caveats: [],
      href: '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=trace.tassadar.accepted.1',
      kind: 'training_ref',
      label: 'Accepted trace corpus ref',
      ref: 'trace.tassadar.accepted.1',
      sourceRefs: [],
      state: 'linked',
    })
  })
})
