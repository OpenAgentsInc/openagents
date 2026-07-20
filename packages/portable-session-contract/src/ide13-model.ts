export const IDE_PORTABLE_MODEL_ACTIONS = [
  "request_move", "quiesce", "verify_checkpoint", "stage_destination",
  "revoke_source", "attach_destination", "failback", "cancel",
  "crash_restart", "lost_ack", "replay", "stop",
] as const

export type IdePortableModelAction =
  | (typeof IDE_PORTABLE_MODEL_ACTIONS)[number]
  | `write_generation_${number}`

export type IdePortableModelState = Readonly<{
  phase: "source_attached" | "move_requested" | "source_quiesced" | "checkpoint_verified" |
    "destination_staged" | "source_revoked" | "destination_attached" | "failed_back" | "stopped"
  sourceGeneration: number
  destinationGeneration: number | null
  latestGeneration: number
  sourceAccepting: boolean
  destinationAccepting: boolean
  checkpointVerified: boolean
  sourceRevoked: boolean
  destinationRevoked: boolean
  completedMove: boolean
}>

export type IdePortableModelDecision = Readonly<{
  state: IdePortableModelState
  accepted: boolean
  reason: string
}>

export type IdePortableModelCounterexample = Readonly<{
  invariant: "exclusive_attachment" | "monotonic_generation" | "checkpoint_before_attach" |
    "source_revoked_before_attach" | "stale_writer_rejected" | "terminal_has_no_writer"
  trace: ReadonlyArray<IdePortableModelAction>
  state: IdePortableModelState
}>

export type IdePortableModelReceipt = Readonly<{
  schemaVersion: "openagents.desktop.ide-portable-model.v1"
  maximumDepth: number
  exploredStates: number
  exploredTransitions: number
  refusedTransitions: number
  replayTransitions: number
  crashTransitions: number
  staleWriteAttempts: number
  counterexamples: ReadonlyArray<IdePortableModelCounterexample>
  passed: boolean
}>

export const initialIdePortableModelState = (): IdePortableModelState => ({
  phase: "source_attached",
  sourceGeneration: 1,
  destinationGeneration: null,
  latestGeneration: 1,
  sourceAccepting: true,
  destinationAccepting: false,
  checkpointVerified: false,
  sourceRevoked: false,
  destinationRevoked: false,
  completedMove: false,
})

const refuse = (state: IdePortableModelState, reason: string): IdePortableModelDecision => ({ state, accepted: false, reason })
const accept = (state: IdePortableModelState, reason: string): IdePortableModelDecision => ({ state, accepted: true, reason })

export const transitionIdePortableModel = (
  state: IdePortableModelState,
  action: IdePortableModelAction,
): IdePortableModelDecision => {
  if (action.startsWith("write_generation_")) {
    const generation = Number(action.slice("write_generation_".length))
    const authoritative = generation === state.latestGeneration &&
      ((state.sourceAccepting && generation === state.sourceGeneration) ||
        (state.destinationAccepting && generation === state.destinationGeneration))
    return authoritative ? accept(state, "current generation accepted") : refuse(state, "stale generation fenced")
  }
  if (action === "crash_restart" || action === "lost_ack" || action === "replay") return accept(state, "durable idempotent replay")
  if (action === "stop") return accept({ ...state, phase: "stopped", sourceAccepting: false, destinationAccepting: false }, "stopped")
  if (action === "request_move" && state.phase === "source_attached") return accept({ ...state, phase: "move_requested" }, "move requested")
  if (action === "quiesce" && state.phase === "move_requested") return accept({ ...state, phase: "source_quiesced", sourceAccepting: false }, "source quiesced")
  if (action === "verify_checkpoint" && state.phase === "source_quiesced") return accept({ ...state, phase: "checkpoint_verified", checkpointVerified: true }, "checkpoint verified")
  if (action === "stage_destination" && state.phase === "checkpoint_verified") return accept({
    ...state,
    phase: "destination_staged",
    destinationGeneration: state.latestGeneration + 1,
  }, "destination staged without writer authority")
  if (action === "revoke_source" && state.phase === "destination_staged") return accept({ ...state, phase: "source_revoked", sourceRevoked: true }, "source revoked")
  if (action === "attach_destination" && state.phase === "source_revoked" && state.destinationGeneration !== null) return accept({
    ...state,
    phase: "destination_attached",
    latestGeneration: state.destinationGeneration,
    destinationAccepting: true,
    completedMove: true,
  }, "destination attached")
  if (action === "failback" && state.phase === "destination_attached") {
    const generation = state.latestGeneration + 1
    return accept({
      ...state,
      phase: "failed_back",
      sourceGeneration: generation,
      latestGeneration: generation,
      sourceAccepting: true,
      destinationAccepting: false,
      destinationRevoked: true,
    }, "failback attached a newer source generation")
  }
  if (action === "cancel" && ["move_requested", "source_quiesced", "checkpoint_verified", "destination_staged"].includes(state.phase)) return accept({
    ...state,
    phase: "source_attached",
    sourceAccepting: true,
    destinationGeneration: null,
    destinationAccepting: false,
    checkpointVerified: false,
  }, "pre-revocation move cancelled")
  return refuse(state, "illegal transition")
}

export const auditIdePortableModelState = (
  state: IdePortableModelState,
  trace: ReadonlyArray<IdePortableModelAction>,
): ReadonlyArray<IdePortableModelCounterexample> => {
  const failures: IdePortableModelCounterexample[] = []
  const add = (invariant: IdePortableModelCounterexample["invariant"]): void => {
    failures.push({ invariant, trace, state })
  }
  if (Number(state.sourceAccepting) + Number(state.destinationAccepting) > 1) add("exclusive_attachment")
  if (state.destinationGeneration !== null && state.destinationGeneration <= state.sourceGeneration && !state.destinationRevoked) add("monotonic_generation")
  if (state.destinationAccepting && !state.checkpointVerified) add("checkpoint_before_attach")
  if (state.destinationAccepting && !state.sourceRevoked) add("source_revoked_before_attach")
  if (state.phase === "stopped" && (state.sourceAccepting || state.destinationAccepting)) add("terminal_has_no_writer")
  return failures
}

const key = (state: IdePortableModelState): string => JSON.stringify(state)

export const checkIdePortableModel = (options: Readonly<{
  maximumDepth?: number
  transition?: typeof transitionIdePortableModel
}> = {}): IdePortableModelReceipt => {
  const maximumDepth = options.maximumDepth ?? 12
  const transition = options.transition ?? transitionIdePortableModel
  const actions: ReadonlyArray<IdePortableModelAction> = [
    ...IDE_PORTABLE_MODEL_ACTIONS,
    ...Array.from({ length: 6 }, (_, generation) => `write_generation_${generation}` as const),
  ]
  const queue: Array<{ state: IdePortableModelState; trace: IdePortableModelAction[] }> = [{ state: initialIdePortableModelState(), trace: [] }]
  const visited = new Set<string>()
  const counterexamples: IdePortableModelCounterexample[] = []
  let exploredTransitions = 0
  let refusedTransitions = 0
  let replayTransitions = 0
  let crashTransitions = 0
  let staleWriteAttempts = 0
  while (queue.length > 0) {
    const current = queue.shift()!
    const stateKey = key(current.state)
    if (visited.has(stateKey)) continue
    visited.add(stateKey)
    counterexamples.push(...auditIdePortableModelState(current.state, current.trace))
    if (current.trace.length >= maximumDepth) continue
    for (const action of actions) {
      exploredTransitions += 1
      if (action === "replay" || action === "lost_ack") replayTransitions += 1
      if (action === "crash_restart") crashTransitions += 1
      const decision = transition(current.state, action)
      if (!decision.accepted) refusedTransitions += 1
      if (action.startsWith("write_generation_")) {
        const generation = Number(action.slice("write_generation_".length))
        const currentWriter = generation === current.state.latestGeneration &&
          (current.state.sourceAccepting || current.state.destinationAccepting)
        if (!currentWriter) {
          staleWriteAttempts += 1
          if (decision.accepted) counterexamples.push({ invariant: "stale_writer_rejected", trace: [...current.trace, action], state: decision.state })
        }
      }
      if (decision.accepted) queue.push({ state: decision.state, trace: [...current.trace, action] })
    }
  }
  return {
    schemaVersion: "openagents.desktop.ide-portable-model.v1",
    maximumDepth,
    exploredStates: visited.size,
    exploredTransitions,
    refusedTransitions,
    replayTransitions,
    crashTransitions,
    staleWriteAttempts,
    counterexamples,
    passed: counterexamples.length === 0,
  }
}
