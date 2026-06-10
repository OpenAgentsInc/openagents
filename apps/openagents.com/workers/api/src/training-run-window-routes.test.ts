import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type TrainingAuthorityStore,
  type TrainingRunRecord,
  type TrainingWindowEventRecord,
  type TrainingWindowLeaseRecord,
  type TrainingWindowRecord,
} from './training-run-window-authority'
import { makeTrainingRunWindowRoutes } from './training-run-window-routes'

const jsonRequest = (
  path: string,
  body: Record<string, unknown>,
  init: RequestInit = {},
): Request =>
  new Request(`https://openagents.test${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json', ...(init.headers ?? {}) },
    method: 'POST',
    ...init,
  })

const runRoute = async (
  route: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  expect(route).toBeDefined()

  return Effect.runPromise(route!)
}

const makeMemoryStore = (): TrainingAuthorityStore => {
  const runs = new Map<string, TrainingRunRecord>()
  const windows = new Map<string, TrainingWindowRecord>()
  const leases = new Map<string, TrainingWindowLeaseRecord>()
  const events: Array<TrainingWindowEventRecord> = []

  return {
    claimLease: async lease => {
      leases.set(lease.leaseRef, lease)

      return lease
    },
    listClaimableWindows: async nowIso =>
      [...windows.values()].filter(
        window =>
          window.state === 'active' &&
          ![...leases.values()].some(
            lease =>
              lease.windowRef === window.windowRef &&
              lease.state === 'active' &&
              Date.parse(lease.leaseExpiresAt) > Date.parse(nowIso),
          ),
      ),
    planRun: async run => {
      runs.set(run.trainingRunRef, run)

      return run
    },
    planWindow: async window => {
      windows.set(window.windowRef, window)

      return window
    },
    readRun: async trainingRunRef => runs.get(trainingRunRef),
    readWindow: async windowRef => windows.get(windowRef),
    transitionWindow: async (window, event) => {
      windows.set(window.windowRef, window)
      events.push(event)

      return window
    },
  }
}

describe('training run window routes', () => {
  it('plans, activates, seals, reconciles, reads, and claims training windows', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const plannedRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4673',
          sourceRefs: ['issue.github.openagents.4673'],
          trainingRunRef: 'training.run.4673',
        }),
        {},
      ),
    )
    expect(plannedRun.status).toBe(200)

    const plannedWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/windows/plan', {
          datasetRefs: ['dataset.cs336.homework.1'],
          homeworkKind: 'admin_dispatched_homework',
          trainingRunRef: 'training.run.4673',
          windowRef: 'training.window.4673',
        }),
        {},
      ),
    )
    expect(plannedWindow.status).toBe(200)

    for (const action of ['activate', 'seal', 'reconcile']) {
      const response = await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/training.window.4673/${action}`, {
            receiptRef: `receipt.training.${action}`,
          }),
          {},
        ),
      )
      expect(response.status).toBe(200)
    }

    const readRun = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/runs/training.run.4673',
        ),
        {},
      ),
    )
    expect(readRun.status).toBe(200)
    await expect(readRun.json()).resolves.toMatchObject({
      run: { state: 'planned', trainingRunRef: 'training.run.4673' },
    })

    const readWindow = await runRoute(
      routes.routeTrainingRunWindowRequest(
        new Request(
          'https://openagents.test/api/training/windows/training.window.4673',
        ),
        {},
      ),
    )
    expect(readWindow.status).toBe(200)
    await expect(readWindow.json()).resolves.toMatchObject({
      window: { state: 'reconciled', windowRef: 'training.window.4673' },
    })

    const leaseResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/leases/claim', {
          pylonRef: 'pylon.training.1',
          receiptRefs: ['receipt.training.lease'],
        }),
        {},
      ),
    )
    expect(leaseResponse.status).toBe(404)
  })

  it('claims the admin-dispatched active window before the starter window', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingRunWindowRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/runs', {
          promiseRef: 'promise.training.4673',
          trainingRunRef: 'training.run.4673',
        }),
        {},
      ),
    )

    for (const [windowRef, homeworkKind] of [
      ['training.window.starter', 'auto_starter'],
      ['training.window.admin', 'admin_dispatched_homework'],
    ] as const) {
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest('/api/training/windows/plan', {
            homeworkKind,
            trainingRunRef: 'training.run.4673',
            windowRef,
          }),
          {},
        ),
      )
      await runRoute(
        routes.routeTrainingRunWindowRequest(
          jsonRequest(`/api/training/windows/${windowRef}/activate`, {
            receiptRef: `receipt.${windowRef}.activate`,
          }),
          {},
        ),
      )
    }

    const leaseResponse = await runRoute(
      routes.routeTrainingRunWindowRequest(
        jsonRequest('/api/training/leases/claim', {
          pylonRef: 'pylon.training.1',
        }),
        {},
      ),
    )
    const body = await leaseResponse.json()

    expect(leaseResponse.status).toBe(200)
    expect(body).toMatchObject({
      lease: { windowRef: 'training.window.admin' },
    })
  })
})
