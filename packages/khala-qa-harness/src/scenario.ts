import { Schema as S } from "effect"

export const KhalaCodeQaDriverMode = S.Literals(["rpc", "dom", "vision", "headless"])
export type KhalaCodeQaDriverMode = "rpc" | "dom" | "vision" | "headless"

export const KhalaCodeQaBackendTier = S.Literals(["fixture", "live_codex", "live_fleet"])
export type KhalaCodeQaBackendTier = "fixture" | "live_codex" | "live_fleet"

export const KhalaCodeQaVerdict = S.Literals(["CONFIRMED", "REFUTED", "INCONCLUSIVE"])
export type KhalaCodeQaVerdict = "CONFIRMED" | "REFUTED" | "INCONCLUSIVE"

export const KhalaCodeQaBootAction = S.Struct({
  backend: S.optional(KhalaCodeQaBackendTier),
  headless: S.optional(S.Boolean),
  kind: S.Literal("boot"),
})

export const KhalaCodeQaRpcCallAction = S.Struct({
  args: S.optional(S.Array(S.Unknown)),
  kind: S.Literal("rpc_call"),
  method: S.String,
})

export const KhalaCodeQaReadAction = S.Struct({
  kind: S.Literal("read"),
  query: S.String,
})

export const KhalaCodeQaUiAction = S.Struct({
  kind: S.Literals([
    "click",
    "type",
    "hotbar",
    "slash_command",
    "approve",
    "thread_select",
    "submit_composer",
    "wait_for",
  ]),
  target: S.optional(S.String),
  text: S.optional(S.String),
  value: S.optional(S.String),
})

export const KhalaCodeQaAction = S.Union(
  [
    KhalaCodeQaBootAction,
    KhalaCodeQaRpcCallAction,
    KhalaCodeQaReadAction,
    KhalaCodeQaUiAction,
  ],
)
export type KhalaCodeQaAction =
  | {
      readonly backend?: KhalaCodeQaBackendTier
      readonly headless?: boolean
      readonly kind: "boot"
    }
  | {
      readonly args?: ReadonlyArray<unknown>
      readonly kind: "rpc_call"
      readonly method: string
    }
  | {
      readonly kind: "read"
      readonly query: string
    }
  | {
      readonly kind:
        | "click"
        | "type"
        | "hotbar"
        | "slash_command"
        | "approve"
        | "thread_select"
        | "submit_composer"
        | "wait_for"
      readonly target?: string
      readonly text?: string
      readonly value?: string
    }

export const KhalaCodeQaOracleExpectation = S.Struct({
  budget: S.optional(S.Number),
  decode: S.optional(S.String),
  id: S.optional(S.String),
  left: S.optional(S.String),
  match: S.optional(S.String),
  metric: S.optional(S.String),
  oracle: S.Literals([
    "schema",
    "consistency",
    "invariant",
    "public_safe",
    "public_safe_dom",
    "visual",
    "perf",
    "a11y",
    "event",
    "crash",
  ]),
  query: S.optional(S.String),
  right: S.optional(S.String),
  within_ms: S.optional(S.Number),
})
export type KhalaCodeQaOracleExpectation = {
  readonly budget?: number
  readonly decode?: string
  readonly id?: string
  readonly left?: string
  readonly match?: string
  readonly metric?: string
  readonly oracle:
    | "schema"
    | "consistency"
    | "invariant"
    | "public_safe"
    | "public_safe_dom"
    | "visual"
    | "perf"
    | "a11y"
    | "event"
    | "crash"
  readonly query?: string
  readonly right?: string
  readonly within_ms?: number
}

export const KhalaCodeQaCommitment = S.Union(
  [
    S.Struct({
      claim: S.String,
      evidence: S.Literal("phase-oracle"),
      id: S.String,
      match: S.String,
    }),
    S.Struct({
      claim: S.String,
      evidence: S.Literal("run-pass"),
      id: S.String,
    }),
  ],
)
export type KhalaCodeQaCommitment =
  | {
      readonly claim: string
      readonly evidence: "phase-oracle"
      readonly id: string
      readonly match: string
    }
  | {
      readonly claim: string
      readonly evidence: "run-pass"
      readonly id: string
    }

export const KhalaCodeQaScenarioPhase = S.Struct({
  act: S.Array(KhalaCodeQaAction),
  expect: S.Array(KhalaCodeQaOracleExpectation),
  name: S.String,
})
export type KhalaCodeQaScenarioPhase = {
  readonly act: ReadonlyArray<KhalaCodeQaAction>
  readonly expect: ReadonlyArray<KhalaCodeQaOracleExpectation>
  readonly name: string
}

export const KhalaCodeQaScenario = S.Struct({
  backend: KhalaCodeQaBackendTier,
  commitments: S.Array(KhalaCodeQaCommitment),
  id: S.String,
  modes: S.Array(KhalaCodeQaDriverMode),
  phases: S.Array(KhalaCodeQaScenarioPhase),
})
export type KhalaCodeQaScenario = {
  readonly backend: KhalaCodeQaBackendTier
  readonly commitments: ReadonlyArray<KhalaCodeQaCommitment>
  readonly id: string
  readonly modes: ReadonlyArray<KhalaCodeQaDriverMode>
  readonly phases: ReadonlyArray<KhalaCodeQaScenarioPhase>
}

export type KhalaCodeQaScenarioLoadFailure = {
  readonly _tag: "KhalaCodeQaScenarioLoadFailure"
  readonly message: string
  readonly phaseName?: string
  readonly cause?: unknown
}

const parseErrorMessage = (cause: unknown): string =>
  cause instanceof Error ? cause.message : String(cause)

const validateScenario = (
  scenario: KhalaCodeQaScenario,
): KhalaCodeQaScenario | KhalaCodeQaScenarioLoadFailure => {
  if (scenario.modes.length === 0) {
    return {
      _tag: "KhalaCodeQaScenarioLoadFailure",
      message: "Scenario has no driver modes",
    }
  }
  if (scenario.phases.length === 0) {
    return {
      _tag: "KhalaCodeQaScenarioLoadFailure",
      message: "Scenario has no phases",
    }
  }
  for (const phase of scenario.phases) {
    if (phase.expect.length === 0) {
      return {
        _tag: "KhalaCodeQaScenarioLoadFailure",
        message: `Scenario phase "${phase.name}" has no oracle expectations`,
        phaseName: phase.name,
      }
    }
  }
  return scenario
}

export const decodeKhalaCodeQaScenario = (
  input: unknown,
): KhalaCodeQaScenario | KhalaCodeQaScenarioLoadFailure => {
  try {
    return validateScenario(S.decodeUnknownSync(KhalaCodeQaScenario)(input) as KhalaCodeQaScenario)
  } catch (cause) {
    return {
      _tag: "KhalaCodeQaScenarioLoadFailure",
      message: parseErrorMessage(cause),
      cause,
    }
  }
}

export const loadKhalaCodeQaScenario = (input: unknown): KhalaCodeQaScenario => {
  const decoded = decodeKhalaCodeQaScenario(input)
  if ("_tag" in decoded) throw new Error(decoded.message)
  return decoded
}
