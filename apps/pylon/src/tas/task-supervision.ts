export type BackgroundRunState =
  | "queued"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"

export type BackgroundRun = {
  readonly runId: string
  readonly kind: string
  readonly state: BackgroundRunState
  readonly startedAt?: string
  readonly endedAt?: string
  readonly evidenceRefs?: ReadonlyArray<string>
}

export type BackgroundRunEvent =
  | {
      readonly type: "started"
      readonly at: string
      readonly evidenceRefs?: ReadonlyArray<string>
    }
  | {
      readonly type: "completed"
      readonly at: string
      readonly evidenceRefs?: ReadonlyArray<string>
    }
  | {
      readonly type: "failed"
      readonly at: string
      readonly evidenceRefs?: ReadonlyArray<string>
    }
  | {
      readonly type: "cancelled"
      readonly at: string
      readonly evidenceRefs?: ReadonlyArray<string>
    }
  | {
      readonly type: "evidence.recorded"
      readonly evidenceRefs: ReadonlyArray<string>
    }

export type SupervisionResult = {
  readonly run: BackgroundRun
  readonly emitReceipt: boolean
}

export class BackgroundRunTransitionError extends Error {
  constructor(
    readonly runId: string,
    readonly from: BackgroundRunState,
    readonly eventType: BackgroundRunEvent["type"],
  ) {
    super(`Illegal background run transition for ${runId}: ${from} + ${eventType}`)
    this.name = "BackgroundRunTransitionError"
  }
}

const terminalStates = new Set<BackgroundRunState>([
  "completed",
  "failed",
  "cancelled",
])

const eventTargetState = {
  started: "running",
  completed: "completed",
  failed: "failed",
  cancelled: "cancelled",
} as const satisfies Partial<
  Record<BackgroundRunEvent["type"], BackgroundRunState>
>

const legalTransitions = {
  queued: new Set<BackgroundRunState>(["running", "failed", "cancelled"]),
  running: new Set<BackgroundRunState>(["completed", "failed", "cancelled"]),
  completed: new Set<BackgroundRunState>(),
  failed: new Set<BackgroundRunState>(),
  cancelled: new Set<BackgroundRunState>(),
} as const satisfies Record<BackgroundRunState, ReadonlySet<BackgroundRunState>>

export function isTerminalRunState(state: BackgroundRunState): boolean {
  return terminalStates.has(state)
}

export function canTransitionRun(
  from: BackgroundRunState,
  to: BackgroundRunState,
): boolean {
  return legalTransitions[from].has(to)
}

export function supervise(
  run: BackgroundRun,
  event: BackgroundRunEvent,
): SupervisionResult {
  if (event.type === "evidence.recorded") {
    if (isTerminalRunState(run.state)) {
      throw new BackgroundRunTransitionError(run.runId, run.state, event.type)
    }

    return {
      run: {
        ...run,
        evidenceRefs: mergeEvidenceRefs(run.evidenceRefs, event.evidenceRefs),
      },
      emitReceipt: false,
    }
  }

  const nextState = eventTargetState[event.type]

  if (!canTransitionRun(run.state, nextState)) {
    throw new BackgroundRunTransitionError(run.runId, run.state, event.type)
  }

  const nextRun: BackgroundRun = {
    ...run,
    state: nextState,
    evidenceRefs: mergeEvidenceRefs(run.evidenceRefs, event.evidenceRefs),
    ...(event.type === "started" ? { startedAt: event.at } : {}),
    ...(isTerminalRunState(nextState) ? { endedAt: event.at } : {}),
  }

  return {
    run: nextRun,
    emitReceipt: isTerminalRunState(nextState),
  }
}

function mergeEvidenceRefs(
  current: ReadonlyArray<string> | undefined,
  incoming: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> | undefined {
  if (!incoming || incoming.length === 0) {
    return current
  }

  return [...new Set([...(current ?? []), ...incoming])]
}
