import { createHash } from "node:crypto"

export type DesktopSourceBinding = Readonly<{
  sessionRef: string
  attachmentRef: string
  grantRef: string
  generation: number
}>

export type DesktopSourceSubsystemResult =
  | Readonly<{ state: "quiesced" }>
  | Readonly<{ state: "unsupported"; detailRef: string }>

export type DesktopSourceSubsystem = Readonly<{
  subsystem: string
  quiesce: (binding: DesktopSourceBinding) => Promise<DesktopSourceSubsystemResult>
}>

export type DesktopSourceSubsystemOutcome = Readonly<{
  subsystem: string
  state: "quiesced" | "failed" | "timed_out" | "unsupported"
  evidenceRef: string
  detailRef: string | null
  executionClaim: "local_cleanup_only"
}>

export type DesktopSourceSafePointResult =
  | Readonly<{
      state: "refused"
      reason: "binding_unavailable" | "stale_binding" | "already_quiesced"
      outcomes: ReadonlyArray<DesktopSourceSubsystemOutcome>
      remoteExecution: "not_claimed"
    }>
  | Readonly<{
      state: "quiescent" | "not_quiescent"
      binding: DesktopSourceBinding
      outcomes: ReadonlyArray<DesktopSourceSubsystemOutcome>
      remoteExecution: "not_claimed"
    }>

export type DesktopSourceSafePoint = Readonly<{
  currentBinding: () => DesktopSourceBinding | null
  quiesce: (binding: DesktopSourceBinding) => Promise<DesktopSourceSafePointResult>
}>

export type DesktopSourceSafePointOptions = Readonly<{
  currentBinding: () => DesktopSourceBinding | null
  subsystems: ReadonlyArray<DesktopSourceSubsystem>
  timeoutMs?: number
}>

const sameBinding = (left: DesktopSourceBinding, right: DesktopSourceBinding): boolean =>
  left.sessionRef === right.sessionRef &&
  left.attachmentRef === right.attachmentRef &&
  left.grantRef === right.grantRef &&
  left.generation === right.generation

const boundedRef = (value: string): string => value.trim().slice(0, 512)

const evidenceRef = (
  binding: DesktopSourceBinding,
  subsystem: string,
  state: DesktopSourceSubsystemOutcome["state"],
  detailRef: string | null,
): string => {
  const digest = createHash("sha256").update([
    binding.sessionRef,
    binding.attachmentRef,
    binding.grantRef,
    String(binding.generation),
    subsystem,
    state,
    detailRef ?? "",
  ].join("\0")).digest("hex")
  return `ide.source-safe-point.${digest}`
}

const outcome = (
  binding: DesktopSourceBinding,
  subsystem: string,
  state: DesktopSourceSubsystemOutcome["state"],
  detailRef: string | null,
): DesktopSourceSubsystemOutcome => ({
  subsystem,
  state,
  evidenceRef: evidenceRef(binding, subsystem, state, detailRef),
  detailRef,
  executionClaim: "local_cleanup_only",
})

/**
 * Compose the main-process helper safe point. This service does not move an
 * attachment and does not revoke placement authority. It only closes the
 * helpers that the caller supplies for one exact source binding.
 */
export const makeDesktopSourceSafePoint = (
  options: DesktopSourceSafePointOptions,
): DesktopSourceSafePoint => {
  const timeoutMs = Math.max(10, Math.min(options.timeoutMs ?? 5_000, 30_000))
  let quiesced = false
  let active: Promise<DesktopSourceSafePointResult> | null = null

  const runSubsystem = (
    subsystem: DesktopSourceSubsystem,
    binding: DesktopSourceBinding,
  ): Promise<DesktopSourceSubsystemOutcome> => new Promise(resolve => {
    let settled = false
    const finish = (value: DesktopSourceSubsystemOutcome): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve(value)
    }
    const timer = setTimeout(() => finish(outcome(
      binding,
      subsystem.subsystem,
      "timed_out",
      "desktop.source-safe-point.deadline",
    )), timeoutMs)
    timer.unref?.()
    void subsystem.quiesce(binding).then(
      result => result.state === "quiesced"
        ? finish(outcome(binding, subsystem.subsystem, "quiesced", null))
        : finish(outcome(
            binding,
            subsystem.subsystem,
            "unsupported",
            boundedRef(result.detailRef),
          )),
      () => finish(outcome(
        binding,
        subsystem.subsystem,
        "failed",
        "desktop.source-safe-point.subsystem-failed",
      )),
    )
  })

  return {
    currentBinding: options.currentBinding,
    quiesce: binding => {
      const current = options.currentBinding()
      if (current === null) return Promise.resolve({
        state: "refused",
        reason: "binding_unavailable",
        outcomes: [],
        remoteExecution: "not_claimed",
      })
      if (!sameBinding(current, binding)) return Promise.resolve({
        state: "refused",
        reason: "stale_binding",
        outcomes: [],
        remoteExecution: "not_claimed",
      })
      if (active !== null) return active
      if (quiesced) return Promise.resolve({
        state: "refused",
        reason: "already_quiesced",
        outcomes: [],
        remoteExecution: "not_claimed",
      })
      quiesced = true
      const immutableBinding = Object.freeze({ ...binding })
      const operation = Promise.all(options.subsystems.map(subsystem =>
        runSubsystem(subsystem, immutableBinding))).then(outcomes => {
        const stillCurrent = options.currentBinding()
        const bindingStayedCurrent = stillCurrent !== null && sameBinding(stillCurrent, immutableBinding)
        return {
          state: bindingStayedCurrent && outcomes.every(value => value.state === "quiesced")
            ? "quiescent"
            : "not_quiescent",
          binding: immutableBinding,
          outcomes,
          remoteExecution: "not_claimed",
        } as const
      })
      active = operation
      void operation.finally(() => {
        if (active === operation) active = null
      })
      return operation
    },
  }
}
