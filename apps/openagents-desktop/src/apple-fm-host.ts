/**
 * Apple Foundation Models supervisor state machine (AFM-6, #9075).
 *
 * A PURE, single-owned-session supervisor with a generation counter, modelled
 * on `voice-host.ts`. It consumes an injected `AppleFmLauncher` boundary (the
 * concrete packaged launcher lives in `apple-fm-native-helper.ts`) so the whole
 * lifecycle is testable with fakes and no real Apple Silicon device. Readiness
 * is TRUE only after a live `/health` probe through the in-process Pylon FM
 * client reports ready. A crash resolves to a typed failure; `dispose()` is
 * idempotent and stops only a bridge THIS host launched (an adopted operator
 * bridge is never stopped).
 *
 * The host emits the public-safe `AppleFmStatus` projection verbatim, so the
 * IPC layer forwards it without re-deriving any private state.
 */
import type {
  AppleFmHostStateValue,
  AppleFmModeValue,
  AppleFmReadinessValue,
  AppleFmStatus,
  AppleFmTurnResult,
  AppleFmUsageTruthValue,
} from "./apple-fm-contract.ts"
import { APPLE_FM_STATUS_SCHEMA_ID, APPLE_FM_TURN_SCHEMA_ID } from "./apple-fm-contract.ts"

// ---------------------------------------------------------------------------
// Injected launcher boundary — the ONLY impure surface.
// ---------------------------------------------------------------------------

/** A live readiness probe, health-derived, bounded and public-safe. */
export type AppleFmProbe = Readonly<{
  status: AppleFmReadinessValue
  ready: boolean
  model?: string
  profileId?: string
  usageTruth?: AppleFmUsageTruthValue
  unavailableReason?: string
}>

/** One bounded read-only turn outcome from the launcher's Pylon client. */
export type AppleFmLauncherTurn = Readonly<{
  outcome: "completed" | "failed"
  text?: string
  usageTruth: AppleFmUsageTruthValue
  promptTokens?: number
  completionTokens?: number
  totalTokens?: number
  failureClass?: string
}>

/** A launched or adopted sidecar session. `stop()` never stops an adopted bridge. */
export type AppleFmLauncherSession = Readonly<{
  mode: "launched" | "adopted"
  probe: () => Promise<AppleFmProbe>
  complete: (prompt: string) => Promise<AppleFmLauncherTurn>
  stop: () => void
}>

export type AppleFmLaunchOutcome =
  | Readonly<{ kind: "session"; session: AppleFmLauncherSession }>
  | Readonly<{ kind: "helper_missing"; blockerRef: string }>
  | Readonly<{ kind: "failed"; blockerRef: string; failureClass: string }>

export type AppleFmLauncher = Readonly<{
  /** macOS Apple Silicon gate. On any other platform the host reports `not_supported`. */
  supported: () => boolean
  /** Adopt an existing healthy bridge, else verify + spawn + poll one. */
  launch: (input: Readonly<{ onCrash: (failureClass: string) => void }>) => Promise<AppleFmLaunchOutcome>
}>

export type AppleFmHost = Readonly<{
  status: () => AppleFmStatus
  /** Idempotent: start (or re-probe) the owned session and return the projection. */
  ensureStarted: () => Promise<AppleFmStatus>
  /** Re-probe live readiness of the current session. */
  refresh: () => Promise<AppleFmStatus>
  /** Run one bounded read-only turn; refuses unless live-ready. */
  runTurn: (prompt: string) => Promise<AppleFmTurnResult>
  /** Stop an owned session (never an adopted bridge) and return the projection. */
  stop: () => AppleFmStatus
  dispose: () => void
}>

const blockerForReason = (reason: string | undefined): ReadonlyArray<string> =>
  reason === undefined ? [] : [`blocker.apple_fm.${reason}`]

export const createAppleFmHost = (launcher: AppleFmLauncher): AppleFmHost => {
  const supported = launcher.supported()
  let state: AppleFmHostStateValue = supported ? "candidate" : "not_supported"
  let readiness: AppleFmReadinessValue = supported ? "unreachable" : "unsupported"
  let ready = false
  let mode: AppleFmModeValue = "none"
  let model: string | null = null
  let profileId: string | null = null
  let usageTruth: AppleFmUsageTruthValue = "unknown"
  let unavailableReason: string | null = supported ? null : "unsupported_hardware"
  let blockerRefs: ReadonlyArray<string> = supported ? [] : ["blocker.apple_fm.unsupported_platform"]
  let session: AppleFmLauncherSession | null = null
  let generation = 0
  let starting: Promise<AppleFmStatus> | null = null
  let disposed = false

  const status = (): AppleFmStatus => ({
    schema: APPLE_FM_STATUS_SCHEMA_ID,
    supported,
    state,
    readiness,
    ready,
    mode,
    model,
    profileId,
    usageTruth,
    unavailableReason,
    blockerRefs: blockerRefs.slice(0, 8),
  })

  const applyProbe = (probe: AppleFmProbe): void => {
    readiness = probe.status
    model = probe.model ?? model
    profileId = probe.profileId ?? profileId
    usageTruth = probe.usageTruth ?? usageTruth
    if (probe.ready) {
      state = "ready"
      ready = true
      unavailableReason = null
      blockerRefs = []
    } else {
      state = "unavailable"
      ready = false
      unavailableReason = probe.unavailableReason ?? probe.status
      blockerRefs = blockerForReason(probe.unavailableReason ?? probe.status)
    }
  }

  const refreshSession = async (owned: AppleFmLauncherSession, ownedGeneration: number): Promise<void> => {
    let probe: AppleFmProbe
    try {
      probe = await owned.probe()
    } catch {
      probe = { status: "unreachable", ready: false, unavailableReason: "bridge_unreachable" }
    }
    if (generation !== ownedGeneration || session !== owned) return
    applyProbe(probe)
  }

  const start = async (): Promise<AppleFmStatus> => {
    if (disposed || !supported) return status()
    if (session !== null) {
      await refreshSession(session, generation)
      return status()
    }
    generation += 1
    const ownedGeneration = generation
    state = "launching"
    readiness = "unreachable"
    ready = false
    mode = "none"
    unavailableReason = null
    blockerRefs = []
    const outcome = await launcher.launch({
      onCrash: (failureClass) => {
        if (generation !== ownedGeneration) return
        session = null
        state = "failed"
        readiness = "unreachable"
        ready = false
        mode = "none"
        unavailableReason = "helper_crashed"
        blockerRefs = [`blocker.apple_fm.${failureClass}`]
      },
    })
    if (disposed || generation !== ownedGeneration) {
      if (outcome.kind === "session" && outcome.session.mode === "launched") outcome.session.stop()
      return status()
    }
    if (outcome.kind === "helper_missing") {
      state = "helper_missing"
      readiness = "unavailable"
      ready = false
      unavailableReason = "helper_missing"
      blockerRefs = [outcome.blockerRef]
      return status()
    }
    if (outcome.kind === "failed") {
      state = "failed"
      readiness = "unreachable"
      ready = false
      unavailableReason = outcome.failureClass
      blockerRefs = [outcome.blockerRef]
      return status()
    }
    session = outcome.session
    mode = outcome.session.mode === "adopted" ? "local_adopted" : "local_launched"
    state = outcome.session.mode === "adopted" ? "adopted" : "running"
    await refreshSession(outcome.session, ownedGeneration)
    return status()
  }

  const ensureStarted = (): Promise<AppleFmStatus> => {
    if (disposed || !supported) return Promise.resolve(status())
    if (starting !== null) return starting
    const pending = start().finally(() => {
      if (starting === pending) starting = null
    })
    starting = pending
    return pending
  }

  return {
    status,
    ensureStarted,
    refresh: async () => {
      if (disposed || !supported || session === null) return status()
      await refreshSession(session, generation)
      return status()
    },
    runTurn: async (prompt): Promise<AppleFmTurnResult> => {
      if (disposed || !supported) {
        return {
          schema: APPLE_FM_TURN_SCHEMA_ID,
          ok: false,
          outcome: "refused_unsupported",
          text: null,
          usageTruth: "unknown",
          promptTokens: null,
          completionTokens: null,
          totalTokens: null,
          failureClass: "unsupported_platform",
        }
      }
      if (session === null || state !== "ready" || !ready) {
        return {
          schema: APPLE_FM_TURN_SCHEMA_ID,
          ok: false,
          outcome: "refused_not_ready",
          text: null,
          usageTruth: "unknown",
          promptTokens: null,
          completionTokens: null,
          totalTokens: null,
          failureClass: "not_ready",
        }
      }
      const turn = await session.complete(prompt)
      return {
        schema: APPLE_FM_TURN_SCHEMA_ID,
        ok: turn.outcome === "completed",
        outcome: turn.outcome,
        text: turn.text ?? null,
        usageTruth: turn.usageTruth,
        promptTokens: turn.promptTokens ?? null,
        completionTokens: turn.completionTokens ?? null,
        totalTokens: turn.totalTokens ?? null,
        failureClass: turn.failureClass ?? null,
      }
    },
    stop: () => {
      if (!supported) return status()
      generation += 1
      const owned = session
      session = null
      if (owned !== null && owned.mode === "launched") owned.stop()
      state = "stopped"
      readiness = "unreachable"
      ready = false
      mode = "none"
      unavailableReason = null
      blockerRefs = []
      return status()
    },
    dispose: () => {
      if (disposed) return
      disposed = true
      generation += 1
      const owned = session
      session = null
      if (owned !== null && owned.mode === "launched") owned.stop()
      state = "stopped"
      ready = false
      mode = "none"
    },
  }
}
