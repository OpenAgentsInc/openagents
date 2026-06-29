export type CloudDeployProgressEvent = {
  state: string
  at: string
}

export type CloudDeployProgress = {
  percent: number
  phase: string
  done: boolean
  failed: boolean
}

const progressByState = {
  queued: { percent: 10, done: false, failed: false },
  building: { percent: 60, done: false, failed: false },
  deployed: { percent: 100, done: true, failed: false },
  failed: { percent: 100, done: false, failed: true },
} as const

export function projectDeployProgress(
  events: { state: string, at: string }[],
): CloudDeployProgress {
  let progress: CloudDeployProgress = {
    percent: 0,
    phase: "",
    done: false,
    failed: false,
  }

  for (const event of events) {
    const state = event.state.trim().toLowerCase()
    if (state in progressByState) {
      progress = {
        ...progressByState[state as keyof typeof progressByState],
        phase: state,
      }
    }
  }

  return progress
}
