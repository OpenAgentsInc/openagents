export type DesktopKhalaDispatchPlanInput = {
  readonly accounts: readonly string[]
  readonly candidateRefs: readonly string[]
  readonly concurrency: number
  readonly priorityLane: string
  readonly repository: string
  readonly branch?: string
  readonly commit: string
  readonly verifier: string
  readonly targetPylonRef?: string
}

export type DesktopKhalaDispatchPlanResult =
  | {
      readonly ok: true
      readonly observedAt: string
      readonly plan: unknown
    }
  | {
      readonly ok: false
      readonly observedAt: string
      readonly error: string
    }
