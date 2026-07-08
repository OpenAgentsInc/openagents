/**
 * Harness conformance contract (MH-1, issue #8582).
 *
 * Steals effect-native's `componentTags` renderer-conformance trick and applies
 * it to coding harnesses: a suite driven by the harness-kind enum, so adding a
 * new harness kind REDS the sweep until that kind proves five capabilities:
 *
 *   (a) chat runtime      startThread/startTurn/interrupt/resume ->
 *                         `khala.chat_turn_event.v1`
 *   (b) worker executor   claim -> pinned worktree -> closeout with verify
 *   (c) capacity/readiness probe
 *   (d) metering honesty  exact fields when present; `not_measured` otherwise;
 *                         NEVER synthesized/invented tokens
 *   (e) typed failure     classes including account_exhausted,
 *                         account_rate_limited, account_quota_exhausted
 *
 * The mechanism has two teeth:
 *   1. Compile-time: `harnessKindClassification` is `satisfies Record<
 *      AgentDefinitionHarnessKind, ...>`, so a new literal in the enum breaks
 *      typecheck until it is classified. The fixture registry is likewise
 *      `satisfies Record<CodingWorkerHarnessKind, ...>`, so a coding harness
 *      cannot be forgotten.
 *   2. Run-time: proven kinds run the five-capability suite (green); pending
 *      kinds emit `test.todo` (visible red-by-design) and are checked against a
 *      known-pending allowlist, so a *new* pending coding kind reds the sweep.
 */
import type {
  AgentDefinitionHarnessKind,
  AgentRuntimeAdapterKind,
} from "@openagentsinc/agent-runtime-schema"
import type {
  FleetHarnessKind,
  MarginalCostClass,
} from "@openagentsinc/khala-fleet-intents"
import type { KhalaChatTurnEventV1 } from "@openagentsinc/agent-runtime-schema"

// --- Harness-kind classification (compile-time exhaustive) -----------------

export type HarnessConformanceClass =
  | "coding_worker_harness"
  | "not_a_coding_worker"

/**
 * Every `AgentDefinitionHarnessKind` literal must be classified. The three
 * concrete coding harnesses (codex, claude_code, grok_cli — mirroring
 * `FleetHarnessKind` codex/claude/grok) are coding-worker harnesses that owe
 * the full five-fixture proof. The rest are meta/host kinds with no local
 * worker runtime; if a future kind becomes a real coding worker, flip its
 * classification here and it immediately owes fixtures (reds until filled).
 */
export const harnessKindClassification = {
  codex: "coding_worker_harness",
  claude_code: "coding_worker_harness",
  grok_cli: "coding_worker_harness",
  khala: "not_a_coding_worker",
  opencode: "not_a_coding_worker",
  hermes: "not_a_coding_worker",
  openagents_native: "not_a_coding_worker",
  hosted_container: "not_a_coding_worker",
  custom: "not_a_coding_worker",
  test_fixture: "not_a_coding_worker",
} as const satisfies Record<AgentDefinitionHarnessKind, HarnessConformanceClass>

export type CodingWorkerHarnessKind = {
  [K in AgentDefinitionHarnessKind]: (typeof harnessKindClassification)[K] extends "coding_worker_harness"
    ? K
    : never
}[AgentDefinitionHarnessKind]

export const codingWorkerHarnessKinds: ReadonlyArray<CodingWorkerHarnessKind> = (
  Object.keys(harnessKindClassification) as ReadonlyArray<AgentDefinitionHarnessKind>
).filter(
  (kind): kind is CodingWorkerHarnessKind =>
    harnessKindClassification[kind] === "coding_worker_harness",
)

/** Coding-harness kind -> the concrete `FleetHarnessKind` it dispatches as. */
export const codingWorkerFleetKind = {
  codex: "codex",
  claude_code: "claude",
  grok_cli: "grok",
} as const satisfies Record<CodingWorkerHarnessKind, FleetHarnessKind>

/** Coding-harness kind -> its `AgentRuntimeAdapterKind` (both enums stay aligned). */
export const codingWorkerAdapterKind = {
  codex: "codex",
  claude_code: "claude_code",
  grok_cli: "grok_cli",
} as const satisfies Record<CodingWorkerHarnessKind, AgentRuntimeAdapterKind>

// --- The five capability shapes --------------------------------------------

export type MeteringLabel = "exact" | "not_measured"

export type AuthPlane = "cli_session" | "api_key" | "subscription" | "unknown"

/**
 * Failure taxonomy shared across coding harnesses. The three account-capacity
 * classes are mandatory fixtures per the issue; the remainder are the real
 * classes emitted by the codex/claude session-error classifier and the grok
 * ACP classifier.
 */
export type HarnessFailureClass =
  | "account_exhausted"
  | "account_rate_limited"
  | "account_quota_exhausted"
  | "auth_required"
  | "verification_failed"
  | "workspace_materialization"
  | "cancelled"
  | "timeout"
  | "unknown"

export const requiredFailureClasses: ReadonlyArray<HarnessFailureClass> = [
  "account_exhausted",
  "account_rate_limited",
  "account_quota_exhausted",
]

/** (d) A single usage sample. `exact` carries real token fields; `not_measured` carries none. */
export type HarnessUsageSnapshot = {
  readonly metering: MeteringLabel
  readonly inputTokens?: number
  readonly outputTokens?: number
  readonly totalTokens?: number
  readonly reasoningTokens?: number
  readonly wallClockMs: number
  readonly model?: string
  readonly plane: AuthPlane
  readonly marginalCostClass: MarginalCostClass
}

/** (b) A pinned work claim: a claim ref bound to an exact repo/commit/branch worktree + verify. */
export type HarnessWorkerClaimPin = {
  readonly claimRef: string
  readonly workUnitRef: string
  readonly runRef: string
  readonly repo: string
  readonly commit: string
  readonly branch: string
  readonly verifyCommand: string
  readonly cwd: string
}

/** (b) The closeout of a claimed work unit, with the own-capacity no-spend settlement invariants. */
export type HarnessWorkerCloseout = {
  readonly ok: boolean
  readonly claimRef: string
  readonly stopReason: string
  readonly verifyPassed: boolean
  readonly paymentMode: "no-spend" | "metered"
  readonly settlementState: "not_applicable" | "settled" | "pending"
  readonly payoutClaimAllowed: boolean
  readonly resultRef?: string
  readonly usage: HarnessUsageSnapshot
  readonly failureClass?: HarnessFailureClass
}

/** (c) Capacity/readiness snapshot, mirroring the public Pylon capacity refs. */
export type HarnessReadiness = {
  readonly ready: boolean
  readonly harness: string
  readonly capacityAvailable: number
  readonly capacityReady: number
  readonly busy: number
  readonly queued: number
  readonly plane: AuthPlane
  readonly models: ReadonlyArray<string>
  readonly failureClass?: HarnessFailureClass
  readonly detail?: string
}

/** A single typed-failure sample: an operator-facing class plus a redacted digest ref. */
export type HarnessFailureSample = {
  readonly failureClass: HarnessFailureClass
  readonly errorDigestRef: string
  readonly detail?: string
}

// --- The fixture a harness must supply to go green -------------------------

export interface HarnessConformanceFixture {
  readonly harnessKind: CodingWorkerHarnessKind
  /** (a) chat runtime: control ops projected onto `khala.chat_turn_event.v1`. */
  readonly chatRuntime: {
    /** startThread + startTurn: thread_ready -> message lifecycle -> message_done. */
    readonly startThreadTurn: () => ReadonlyArray<KhalaChatTurnEventV1>
    /** interrupt an in-flight turn. */
    readonly interruptTurn: () => ReadonlyArray<KhalaChatTurnEventV1>
    /** resume a persisted thread (emits a fresh thread_ready binding the resumed thread). */
    readonly resumeThread: () => ReadonlyArray<KhalaChatTurnEventV1>
  }
  /** (b) worker executor. */
  readonly workerExecutor: {
    readonly claim: HarnessWorkerClaimPin
    readonly closeout: HarnessWorkerCloseout
  }
  /** (c) capacity/readiness probe. */
  readonly readinessProbe: () => HarnessReadiness
  /** (d) metering honesty samples: MUST include both an `exact` and a `not_measured` sample. */
  readonly meteringSamples: ReadonlyArray<HarnessUsageSnapshot>
  /** (e) typed failure classes: MUST include the three mandatory account-capacity classes. */
  readonly typedFailures: Partial<Record<HarnessFailureClass, () => HarnessFailureSample>>
}

// --- Registry entry: proven or pending -------------------------------------

export type HarnessConformanceEntry =
  | { readonly status: "proven"; readonly fixture: HarnessConformanceFixture }
  | {
      readonly status: "pending"
      /** Public-safe ref for WHY this kind has no fixtures yet. */
      readonly reasonRef: string
      /** The lane/agent that owns filling these fixtures. */
      readonly ownerLane: string
    }
