import type { DispatchSchedulerPreemptionEvidence } from './model-router'
import { currentEpochMillis } from '../runtime-primitives'

export type InternalStressPreemptionSnapshot = Readonly<{
  activeStressCount: number
}>

export type InternalStressPreemptionCoordinator = Readonly<{
  register: (
    input: Readonly<{
      abortController: AbortController
      nowMs: number
      requestId: string
    }>,
  ) => Promise<() => Promise<void>>
  preempt: (
    input: Readonly<{
      nowMs: number
      reason: string
    }>,
  ) => Promise<DispatchSchedulerPreemptionEvidence | undefined>
  snapshot: (
    input: Readonly<{
      nowMs: number
    }>,
  ) => Promise<InternalStressPreemptionSnapshot>
}>

export type InternalStressPreemptionRegistry = Readonly<{
  register: (
    input: Readonly<{
      abortController: AbortController
      requestId: string
    }>,
  ) => () => void
  preempt: (
    input: Readonly<{
      reason: string
    }>,
  ) => DispatchSchedulerPreemptionEvidence | undefined
}>

export const makeInternalStressPreemptionRegistry =
  (): InternalStressPreemptionRegistry => {
    const active = new Map<string, AbortController>()
    return {
      preempt: input => {
        const target = [...active.entries()].find(
          ([, controller]) => !controller.signal.aborted,
        )
        if (target === undefined) {
          return undefined
        }
        const [requestId, controller] = target
        active.delete(requestId)
        controller.abort(input.reason)
        return {
          evidenceRef: `scheduler.preemption.internal_stress.${requestId}`,
          reason: input.reason,
          targetDemandClass: 'internal_stress',
          targetOutcome: 'preempted_yielded',
        }
      },
      register: input => {
        if (input.abortController.signal.aborted) {
          return () => {}
        }
        active.set(input.requestId, input.abortController)
        let released = false
        return () => {
          if (released) {
            return
          }
          released = true
          if (active.get(input.requestId) === input.abortController) {
            active.delete(input.requestId)
          }
        }
      },
    }
  }

export interface InternalStressSchedulerStub {
  fetch(request: Request): Promise<globalThis.Response>
}

export interface InternalStressSchedulerNamespace {
  getByName(name: string): InternalStressSchedulerStub
}

const GLM_STRESS_SCHEDULER_OBJECT_NAME =
  'openagents.glm_52_reap_504b.internal_stress.v1'

const GLM_STRESS_SCHEDULER_ORIGIN = 'https://glm-stress-scheduler'

const GLM_STRESS_LEASE_TTL_MS = 5 * 60 * 1000
const GLM_STRESS_PREEMPTION_POLL_MS = 250

const schedulerUrl = (path: string): string =>
  `${GLM_STRESS_SCHEDULER_ORIGIN}${path}`

const jsonRequest = (path: string, body: unknown): Request =>
  new Request(schedulerUrl(path), {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

class InternalStressSchedulerHttpError extends Error {
  constructor(readonly status: number) {
    super(`internal stress scheduler returned HTTP ${status}`)
    this.name = 'InternalStressSchedulerHttpError'
  }
}

const readJson = async <T>(response: globalThis.Response): Promise<T> => {
  if (!response.ok) {
    throw new InternalStressSchedulerHttpError(response.status)
  }
  return (await response.json()) as T
}

const sleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

type PreemptedResponse = Readonly<{
  evidenceRef?: string
  preempted: boolean
  reason?: string
}>

export const makeInternalStressPreemptionCoordinatorDO = (
  namespace: InternalStressSchedulerNamespace,
): InternalStressPreemptionCoordinator => {
  const stub = namespace.getByName(GLM_STRESS_SCHEDULER_OBJECT_NAME)
  return {
    preempt: async input => {
      const response = await stub.fetch(
        jsonRequest('/v1/internal-stress/preempt', input),
      )
      const body = await readJson<
        | Readonly<{
            evidence: DispatchSchedulerPreemptionEvidence
            preempted: true
          }>
        | Readonly<{ evidence?: undefined; preempted: false }>
      >(response)
      return body.preempted ? body.evidence : undefined
    },
    register: async input => {
      await readJson<{ ok: true }>(
        await stub.fetch(
          jsonRequest('/v1/internal-stress/register', {
            expiresAtMs: input.nowMs + GLM_STRESS_LEASE_TTL_MS,
            nowMs: input.nowMs,
            requestId: input.requestId,
          }),
        ),
      )

      let released = false
      const poll = async (): Promise<void> => {
        while (!released && !input.abortController.signal.aborted) {
          await sleep(GLM_STRESS_PREEMPTION_POLL_MS)
          if (released || input.abortController.signal.aborted) {
            return
          }
          const url = new URL(schedulerUrl('/v1/internal-stress/preempted'))
          url.searchParams.set('requestId', input.requestId)
          const response = await stub.fetch(new Request(url, { method: 'GET' }))
          const body = await readJson<PreemptedResponse>(response).catch(
            () => undefined,
          )
          if (body?.preempted === true) {
            input.abortController.abort(
              body.reason ?? 'external_reserved_headroom_unavailable',
            )
            return
          }
        }
      }
      void poll()

      return async () => {
        if (released) {
          return
        }
        released = true
        await stub
          .fetch(
            jsonRequest('/v1/internal-stress/release', {
              nowMs: currentEpochMillis(),
              requestId: input.requestId,
            }),
          )
          .catch(() => undefined)
      }
    },
    snapshot: async input =>
      readJson<InternalStressPreemptionSnapshot>(
        await stub.fetch(
          jsonRequest('/v1/internal-stress/snapshot', {
            nowMs: input.nowMs,
          }),
        ),
      ),
  }
}
