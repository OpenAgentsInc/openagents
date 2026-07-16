/**
 * Provider-keyed Full Auto policy (L6 #8901).
 *
 * The durable loop stores the SPI lane ref, not a provider-specific enum. The
 * two built-in policies live here so prompt framing and background-question
 * behavior cannot drift between main's reconciliation path and lane adapters.
 * Future admitted ACP lanes add one policy entry after their peer profile has
 * proven the same background-question invariant.
 */
export const FULL_AUTO_DEFAULT_LANE = "codex-local" as const

export type FullAutoLanePolicy = Readonly<{
  instruction: string
  /** A background turn has no renderer to answer questions. Eligibility is
   * fail-closed unless the lane can settle them without parking forever. */
  autoResolveQuestions: boolean
}>

const SHARED_INSTRUCTION =
  "Full Auto is on for this turn. Look at this repository's own README, docs " +
  "folder (if any), open issues, and specs/** ProductSpec/AssuranceSpec " +
  "obligations surfaced in the bounded spec context. Treat unmet obligations " +
  "as candidate work, never as provider-owned verdicts. Pick ONE concrete, real, useful next " +
  "thing to do here, and do it now. Do not ask clarifying questions -- make " +
  "a reasonable judgment call and proceed. Stop once this one thing is done; " +
  "you will be asked to continue with the next one."

export const FULL_AUTO_LANE_POLICIES: Readonly<Record<string, FullAutoLanePolicy>> = {
  "codex-local": {
    instruction: `${SHARED_INSTRUCTION} Use the repository's own agent instructions and documentation as authority.`,
    autoResolveQuestions: true,
  },
  "fable-local": {
    instruction: `${SHARED_INSTRUCTION} Use available Claude Code skills only when they are already enabled for this workspace.`,
    autoResolveQuestions: true,
  },
}

export const fullAutoLanePolicy = (laneRef: string): FullAutoLanePolicy | null =>
  FULL_AUTO_LANE_POLICIES[laneRef] ?? null

export const fullAutoPrompt = (laneRef: string, message: string): string => {
  const policy = fullAutoLanePolicy(laneRef)
  return policy === null ? message : `${policy.instruction}\n\n${message}`
}
