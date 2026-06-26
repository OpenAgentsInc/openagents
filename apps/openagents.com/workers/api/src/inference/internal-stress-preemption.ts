import type { DispatchSchedulerPreemptionEvidence } from './model-router'

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
