import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  TASSADAR_RUN_SUMMARY_ENDPOINT,
  TASSADAR_RUN_TAG,
  dataStateForSummary,
  proofLinkForSelection,
  tassadarRunView,
} from './tassadarRunElement'

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
const mountAndSettle = async (): Promise<HTMLElement> => {
  // Force registration via the view helper, then create the element directly.
  tassadarRunView()
  const el = document.createElement(TASSADAR_RUN_TAG)
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

describe('tassadarRunView page wiring', () => {
  beforeEach(() => {
    recordedVisualizations.length = 0
  })

  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
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
    // The underlying renderer element was mounted.
    expect(el.shadowRoot?.querySelector(STUB_TAG)).not.toBeNull()
  })

  it('renders the live snapshot metadata and manual refresh state from the public summary', async () => {
    const refreshed = {
      ...populated,
      generatedAt: '2026-06-17T16:40:20.270Z',
      runState: 'active',
    }
    let summaryCalls = 0
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async () =>
        jsonResponse(summaryCalls++ === 0 ? populated : refreshed),
      )

    const el = await mountAndSettle()
    const status = el.shadowRoot?.querySelector('.status')
    expect(status?.textContent ?? '').toContain(
      'run.tassadar.executor.20260615',
    )
    expect(status?.textContent ?? '').toContain('active')
    expect(status?.textContent ?? '').toContain('2026-06-17T16:39:20.270Z')
    expect(status?.textContent ?? '').toContain('Refresh snapshot')

    expect(el.shadowRoot?.querySelector('.promise-gate')).toBeNull()

    const refresh = el.shadowRoot?.querySelector('button')
    expect(refresh).not.toBeNull()
    refresh?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await waitForSettled(el)

    expect(fetchSpy).toHaveBeenCalledTimes(2)
    expect(
      el.shadowRoot?.querySelector('.status')?.textContent ?? '',
    ).toContain('2026-06-17T16:40:20.270Z')
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

describe('proofLinkForSelection', () => {
  it('maps verification and receipt selections to public-safe proof URLs', () => {
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
