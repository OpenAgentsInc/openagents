import { Effect, Schema as S } from "effect"

export const KhalaFleetDelegateSignature = "khala.fleet.delegate" as const
export const KhalaFleetDelegationParameterSetSchemaVersion =
  "openagents.khala.fleet_delegation.parameters.v0" as const
export const KhalaFleetDelegationAdmittedParametersEnv =
  "OPENAGENTS_KHALA_FLEET_DELEGATION_ADMITTED_PARAMETERS_JSON" as const

export const KhalaFleetDelegateModuleName = S.Literals([
  "ensure_pylon",
  "advertise_capacity",
  "select_account",
  "prepare_work",
  "dispatch",
  "verify_closeout",
])
export type KhalaFleetDelegateModuleName = typeof KhalaFleetDelegateModuleName.Type

export const KhalaFleetDelegatePrecondition = S.Literals([
  "pylon_online",
  "advertised_codex_capacity",
  "advertised_claude_capacity",
  "ready_account_free_slot",
  "work_prepared",
  "dispatch_accepted",
  "closeout_verified",
])
export type KhalaFleetDelegatePrecondition = typeof KhalaFleetDelegatePrecondition.Type

export const KhalaFleetDelegateBlockerCode = S.Literals([
  "capacity_probe_failed",
  "connect_account_required",
  "credentials_missing",
  "dispatch_failed",
  "duplicate_active_assignment",
  "load_gated",
  "no_available_codex_capacity",
  "no_available_claude_capacity",
  "pylon_unavailable",
  "revoked",
  "stale_heartbeat",
  "verify_failed",
])
export type KhalaFleetDelegateBlockerCode = typeof KhalaFleetDelegateBlockerCode.Type

export const KhalaFleetDelegateStepStatus = S.Literals(["blocked", "recovered", "satisfied"])
export type KhalaFleetDelegateStepStatus = typeof KhalaFleetDelegateStepStatus.Type

export const KhalaFleetDelegationParameterSource = S.Literals(["admitted_candidate", "default"])
export type KhalaFleetDelegationParameterSource = typeof KhalaFleetDelegationParameterSource.Type

export const KhalaFleetDelegationAccountRankingHeuristic = S.Literals([
  "default_ready_highest_slots",
  "lexicographic_ready",
  "named_ready_highest_slots",
])
export type KhalaFleetDelegationAccountRankingHeuristic =
  typeof KhalaFleetDelegationAccountRankingHeuristic.Type

export const KhalaFleetDelegateConcreteWorkerKind = S.Literals(["claude", "codex"])
export type KhalaFleetDelegateConcreteWorkerKind =
  typeof KhalaFleetDelegateConcreteWorkerKind.Type

export const KhalaFleetDelegateWorkerKind = S.Literals(["auto", "claude", "codex"])
export type KhalaFleetDelegateWorkerKind = typeof KhalaFleetDelegateWorkerKind.Type

export const KhalaFleetDelegationWorkflowClass = S.Literals([
  "claude_agent_task",
  "cloud_coding_session",
  "codex_agent_task",
  "none",
])
export type KhalaFleetDelegationWorkflowClass =
  typeof KhalaFleetDelegationWorkflowClass.Type

export const KhalaFleetDelegationWorkflowClassification = S.Struct({
  confidence: S.Number,
  evidenceRefs: S.Array(S.String),
  workflowClass: KhalaFleetDelegationWorkflowClass,
})
export type KhalaFleetDelegationWorkflowClassification =
  typeof KhalaFleetDelegationWorkflowClassification.Type

const KhalaFleetDelegationAdvertiseCapacityPolicy = S.Struct({
  maxRequestedSlots: S.optionalKey(S.Number),
  perAccountConcurrency: S.optionalKey(S.Number),
})

const KhalaFleetDelegationAutoRoutingPolicy = S.Struct({
  classifierMinimumConfidence: S.optionalKey(S.Number),
  classifierPreferenceBonusSlots: S.optionalKey(S.Number),
  tieBreakWorkerKind: S.optionalKey(KhalaFleetDelegateConcreteWorkerKind),
})

const KhalaFleetDelegationTargetPolicy = S.Struct({
  autoRouting: S.optionalKey(KhalaFleetDelegationAutoRoutingPolicy),
  workerKind: S.optionalKey(KhalaFleetDelegateWorkerKind),
})

const KhalaFleetDelegationAccountRankingPolicy = S.Struct({
  heuristic: S.optionalKey(KhalaFleetDelegationAccountRankingHeuristic),
})

const KhalaFleetDelegationRetryBackoffPolicy = S.Struct({
  dispatchAttempts: S.optionalKey(S.Number),
  duplicateActiveAssignmentBackoffMs: S.optionalKey(S.Number),
})

const KhalaFleetDelegationVerifyCriteria = S.Struct({
  defaultVerify: S.optionalKey(S.String),
  requireVerifierForRepoWork: S.optionalKey(S.Boolean),
})

export class KhalaFleetDelegationParameterSet extends S.Class<KhalaFleetDelegationParameterSet>(
  "KhalaFleetDelegationParameterSet",
)({
  accountRanking: S.optionalKey(KhalaFleetDelegationAccountRankingPolicy),
  actionSubmissionRef: S.optionalKey(S.String),
  advertiseCapacity: S.optionalKey(KhalaFleetDelegationAdvertiseCapacityPolicy),
  candidateRef: S.optionalKey(S.String),
  delegationTarget: S.optionalKey(KhalaFleetDelegationTargetPolicy),
  objectiveTemplate: S.optionalKey(S.String),
  parameterSetRef: S.String,
  retryBackoff: S.optionalKey(KhalaFleetDelegationRetryBackoffPolicy),
  schemaVersion: S.Literal(KhalaFleetDelegationParameterSetSchemaVersion),
  source: KhalaFleetDelegationParameterSource,
  verifyCriteria: S.optionalKey(KhalaFleetDelegationVerifyCriteria),
}) {}

export class KhalaFleetDelegateModuleError extends S.TaggedErrorClass<KhalaFleetDelegateModuleError>()(
  "KhalaFleetDelegateModuleError",
  {
    blockerCode: KhalaFleetDelegateBlockerCode,
    module: KhalaFleetDelegateModuleName,
    message: S.String,
    refs: S.Array(S.String),
  },
) {}

export type KhalaFleetDelegateAccount = Readonly<{
  accountRef: string
  availableSlots?: number | undefined
  blockerRefs?: ReadonlyArray<string> | undefined
  isDefault?: boolean | undefined
  readiness: "available" | "credentials_missing" | "ready" | "revoked" | "unknown"
  workerKind?: KhalaFleetDelegateConcreteWorkerKind | undefined
}>

export type KhalaFleetDelegateCapacity = Readonly<{
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>
  available: number
  loadGated?: boolean | undefined
  max: number
}>

export type KhalaFleetDelegateWork =
  | Readonly<{
    fixture: true
    kind: "fixture"
  }>
  | Readonly<{
    branch: string
    claimRef: string
    commit: string
    issue?: number | undefined
    kind: "repo"
    repo: string
    verify: string
  }>

export type KhalaFleetDelegateInput = Readonly<{
  accountRef?: string | undefined
  branch?: string | undefined
  claimRef?: string | undefined
  commit?: string | undefined
  fixture?: boolean | undefined
  issue?: number | undefined
  objective: string
  repo?: string | undefined
  verify?: string | undefined
  workflowClassification?: KhalaFleetDelegationWorkflowClassification | undefined
}>

export type KhalaFleetDelegateProgramOptions = Readonly<{
  parameters?: KhalaFleetDelegationParameterSet | undefined
}>

export type KhalaFleetDelegateEnsureResult = Readonly<{
  pylonRef?: string | undefined
  started?: boolean | undefined
}>

export type KhalaFleetDelegateAdvertiseResult = Readonly<{
  capacity: KhalaFleetDelegateCapacity
  heartbeatRef?: string | undefined
}>

export type KhalaFleetDelegateDispatchResult = Readonly<{
  assignmentRef?: string | undefined
  blockerCode?: KhalaFleetDelegateBlockerCode | undefined
  message?: string | undefined
  ok: boolean
  refs?: ReadonlyArray<string> | undefined
}>

export type KhalaFleetDelegateVerifyResult = Readonly<{
  blockerRefs?: ReadonlyArray<string> | undefined
  message?: string | undefined
  ok: boolean
}>

export type KhalaFleetDelegateModules = Readonly<{
  advertiseCapacity: (input: Readonly<{
    pylonRef: string | undefined
    reason: KhalaFleetDelegateAdvertiseReason
  }>) => Effect.Effect<KhalaFleetDelegateAdvertiseResult, KhalaFleetDelegateModuleError>
  backoff?: (input: Readonly<{
    attempt: number
    reason: "duplicate_active_assignment"
  }>) => Effect.Effect<void, KhalaFleetDelegateModuleError>
  dispatch: (input: Readonly<{
    account: KhalaFleetDelegateAccount
    attempt: number
    capacity: KhalaFleetDelegateCapacity
    pylonRef: string | undefined
    work: KhalaFleetDelegateWork
    workerKind: KhalaFleetDelegateConcreteWorkerKind
  }>) => Effect.Effect<KhalaFleetDelegateDispatchResult, KhalaFleetDelegateModuleError>
  ensurePylon: (input: KhalaFleetDelegateInput) => Effect.Effect<KhalaFleetDelegateEnsureResult, KhalaFleetDelegateModuleError>
  verifyCloseout: (input: Readonly<{
    account: KhalaFleetDelegateAccount
    assignmentRef: string
    pylonRef: string | undefined
    workerKind: KhalaFleetDelegateConcreteWorkerKind
  }>) => Effect.Effect<KhalaFleetDelegateVerifyResult, KhalaFleetDelegateModuleError>
}>

export type KhalaFleetDelegateStep = Readonly<{
  blockerCode?: KhalaFleetDelegateBlockerCode | undefined
  fallbackModule?: KhalaFleetDelegateModuleName | undefined
  message: string
  module: KhalaFleetDelegateModuleName
  precondition: KhalaFleetDelegatePrecondition
  refs: ReadonlyArray<string>
  status: KhalaFleetDelegateStepStatus
}>

export type KhalaFleetDelegateCompletedResult = Readonly<{
    account: KhalaFleetDelegateAccount
    assignmentRef: string
    pylonRef: string | undefined
    signature: typeof KhalaFleetDelegateSignature
    status: "completed"
    trace: ReadonlyArray<KhalaFleetDelegateStep>
    workerKind: KhalaFleetDelegateConcreteWorkerKind
    work: KhalaFleetDelegateWork
  }>

export type KhalaFleetDelegateBlockedResult = Readonly<{
  blockerCode: KhalaFleetDelegateBlockerCode
  blockerRefs: ReadonlyArray<string>
  message: string
  pylonRef: string | undefined
  signature: typeof KhalaFleetDelegateSignature
  status: "blocked"
  trace: ReadonlyArray<KhalaFleetDelegateStep>
}>

export type KhalaFleetDelegateProgramResult =
  | KhalaFleetDelegateBlockedResult
  | KhalaFleetDelegateCompletedResult

export type KhalaFleetDelegateAdvertiseReason =
  | "initial"
  | "no_available_claude_capacity"
  | "no_available_codex_capacity"
  | "stale_heartbeat"

const DEFAULT_DISPATCH_ATTEMPTS = 4
const DEFAULT_DUPLICATE_ACTIVE_ASSIGNMENT_BACKOFF_MS = 1_000
const DEFAULT_CLASSIFIER_MINIMUM_CONFIDENCE = 0.75
const DEFAULT_CLASSIFIER_PREFERENCE_BONUS_SLOTS = 2
const DEFAULT_PARAMETER_SET_REF = "parameter_set.khala_fleet_delegation.default.v1"
const publicSafeWorkflowEvidenceRefPattern = /^[A-Za-z0-9_.:-]+$/
const unsafePolicyTextPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|credential|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|fixture|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|trace|traces)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i

export const DefaultKhalaFleetDelegationParameterSet = new KhalaFleetDelegationParameterSet({
  delegationTarget: { workerKind: "codex" },
  parameterSetRef: DEFAULT_PARAMETER_SET_REF,
  schemaVersion: KhalaFleetDelegationParameterSetSchemaVersion,
  source: "default",
})

export function decodeKhalaFleetDelegationParameterSet(
  input: unknown,
): KhalaFleetDelegationParameterSet {
  return normalizeKhalaFleetDelegationParameterSet(
    S.decodeUnknownSync(KhalaFleetDelegationParameterSet)(input),
  )
}

export function khalaFleetDelegationParametersFromEnv(
  env: Readonly<Record<string, string | undefined>>,
): KhalaFleetDelegationParameterSet {
  const raw = env[KhalaFleetDelegationAdmittedParametersEnv]?.trim()
  if (raw === undefined || raw.length === 0) {
    return DefaultKhalaFleetDelegationParameterSet
  }
  try {
    return decodeKhalaFleetDelegationParameterSet(JSON.parse(raw))
  } catch (error) {
    throw new Error(`Invalid ${KhalaFleetDelegationAdmittedParametersEnv}: ${error instanceof Error ? error.message : String(error)}`)
  }
}

export function resolveKhalaFleetDelegationParameters(
  parameters?: KhalaFleetDelegationParameterSet | null | undefined,
): KhalaFleetDelegationParameterSet {
  if (parameters === null || parameters === undefined) {
    return DefaultKhalaFleetDelegationParameterSet
  }
  return decodeKhalaFleetDelegationParameterSet(parameters)
}

export function khalaFleetDelegationPerAccountConcurrency(
  parameters: KhalaFleetDelegationParameterSet,
  fallback: number,
): number {
  return boundedPolicyInteger(
    parameters.advertiseCapacity?.perAccountConcurrency,
    fallback,
    1,
    16,
    "advertiseCapacity.perAccountConcurrency",
  )
}

export function khalaFleetDelegationMaxRequestedSlots(
  parameters: KhalaFleetDelegationParameterSet,
  fallback: number,
): number {
  return boundedPolicyInteger(
    parameters.advertiseCapacity?.maxRequestedSlots,
    fallback,
    1,
    64,
    "advertiseCapacity.maxRequestedSlots",
  )
}

export function khalaFleetDelegationDispatchAttempts(
  parameters: KhalaFleetDelegationParameterSet,
): number {
  return boundedPolicyInteger(
    parameters.retryBackoff?.dispatchAttempts,
    DEFAULT_DISPATCH_ATTEMPTS,
    1,
    8,
    "retryBackoff.dispatchAttempts",
  )
}

export function khalaFleetDelegationDuplicateBackoffMs(
  parameters: KhalaFleetDelegationParameterSet,
): number {
  return boundedPolicyInteger(
    parameters.retryBackoff?.duplicateActiveAssignmentBackoffMs,
    DEFAULT_DUPLICATE_ACTIVE_ASSIGNMENT_BACKOFF_MS,
    0,
    120_000,
    "retryBackoff.duplicateActiveAssignmentBackoffMs",
  )
}

export function khalaFleetDelegationClassifierMinimumConfidence(
  parameters: KhalaFleetDelegationParameterSet,
): number {
  return boundedPolicyNumber(
    parameters.delegationTarget?.autoRouting?.classifierMinimumConfidence,
    DEFAULT_CLASSIFIER_MINIMUM_CONFIDENCE,
    0,
    1,
    "delegationTarget.autoRouting.classifierMinimumConfidence",
  )
}

export function khalaFleetDelegationClassifierPreferenceBonusSlots(
  parameters: KhalaFleetDelegationParameterSet,
): number {
  return boundedPolicyInteger(
    parameters.delegationTarget?.autoRouting?.classifierPreferenceBonusSlots,
    DEFAULT_CLASSIFIER_PREFERENCE_BONUS_SLOTS,
    0,
    64,
    "delegationTarget.autoRouting.classifierPreferenceBonusSlots",
  )
}

export function khalaFleetDelegateWorkerKindForWorkflowClass(
  workflowClass: KhalaFleetDelegationWorkflowClass,
): KhalaFleetDelegateConcreteWorkerKind | undefined {
  if (workflowClass === "claude_agent_task") return "claude"
  if (workflowClass === "codex_agent_task" || workflowClass === "cloud_coding_session") return "codex"
  return undefined
}

export function renderKhalaFleetDelegationObjective(
  input: Readonly<{
    claimRef?: string | undefined
    branch?: string | undefined
    commit?: string | undefined
    issue?: number | null | undefined
    objective: string
    repo?: string | undefined
    verify?: string | undefined
  }>,
  parameters: KhalaFleetDelegationParameterSet = DefaultKhalaFleetDelegationParameterSet,
): string {
  const resolved = resolveKhalaFleetDelegationParameters(parameters)
  const objective = input.objective.trim()
  const template = resolved.objectiveTemplate?.trim()
  if (template === undefined || template.length === 0) {
    return renderDefaultKhalaFleetDelegationObjective(input)
  }
  const rendered = template
    .replaceAll("{objective}", objective)
    .replaceAll("{claim}", input.claimRef ?? "claim.unspecified")
    .replaceAll("{claimRef}", input.claimRef ?? "claim.unspecified")
    .replaceAll("{branch}", input.branch ?? "main")
    .replaceAll("{commit}", input.commit ?? "commit.unspecified")
    .replaceAll("{issue}", input.issue === null || input.issue === undefined ? "unassigned" : String(input.issue))
    .replaceAll("{repo}", input.repo ?? "repo.unspecified")
    .replaceAll("{verify}", input.verify ?? resolved.verifyCriteria?.defaultVerify ?? "verify.unspecified")
    .trim()

  const base = rendered.length === 0 ? objective : rendered
  if (input.claimRef === undefined) return base
  if (renderedContainsKhalaFleetDelegationDiscipline(base)) return base
  return `${base}\n\n${renderKhalaFleetDelegationDiscipline(input)}`
}

export function renderDefaultKhalaFleetDelegationObjective(input: Readonly<{
  branch?: string | undefined
  claimRef?: string | undefined
  commit?: string | undefined
  issue?: number | null | undefined
  objective: string
  repo?: string | undefined
  verify?: string | undefined
}>): string {
  const objective = input.objective.trim()
  if (input.claimRef === undefined) return objective
  const lines = [
    objective,
    "",
    renderKhalaFleetDelegationDiscipline(input),
  ]
  return lines.join("\n")
}

function renderKhalaFleetDelegationDiscipline(input: Readonly<{
  branch?: string | undefined
  claimRef?: string | undefined
  commit?: string | undefined
  issue?: number | null | undefined
  repo?: string | undefined
  verify?: string | undefined
}>): string {
  const issueText = input.issue === null || input.issue === undefined ? "unassigned" : `#${input.issue}`
  const branch = input.branch ?? "main"
  return [
    `Public issue: ${issueText}.`,
    input.claimRef === undefined ? "Claim: claim.unspecified." : `Claim: ${input.claimRef}.`,
    input.repo === undefined ? "Repository: repo.unspecified." : `Repository: ${input.repo}.`,
    input.commit === undefined ? `Base branch: ${branch}.` : `Base branch: ${branch} at ${input.commit}.`,
    input.verify === undefined
      ? "Verification command ref: verify.unspecified."
      : `Verification command ref: ${input.verify}.`,
    input.issue === null || input.issue === undefined
      ? "Open a ready non-draft PR for this claim when the work is complete. Do not merge it."
      : `Open a ready non-draft PR for this claim, include "Closes #${input.issue}" in the PR body, and do not merge it.`,
    "Use a task branch name that clearly identifies the issue and claim.",
  ].join("\n")
}

function renderedContainsKhalaFleetDelegationDiscipline(value: string): boolean {
  return /\bClaim:\s+\S+/u.test(value) &&
    /\bBase branch:\s+\S+/u.test(value) &&
    /\bVerification command ref:\s+\S+/u.test(value)
}

export const runKhalaFleetDelegateProgram = (
  input: KhalaFleetDelegateInput,
  modules: KhalaFleetDelegateModules,
  options: KhalaFleetDelegateProgramOptions = {},
): Effect.Effect<KhalaFleetDelegateProgramResult, never> =>
  Effect.gen(function* () {
    const parameters = resolveKhalaFleetDelegationParameters(options.parameters)
    const trace: KhalaFleetDelegateStep[] = []
    const push = (step: KhalaFleetDelegateStep): void => {
      trace.push(step)
    }
    const block = (
      pylonRef: string | undefined,
      module: KhalaFleetDelegateModuleName,
      precondition: KhalaFleetDelegatePrecondition,
      blockerCode: KhalaFleetDelegateBlockerCode,
      message: string,
      refs: ReadonlyArray<string> = [khalaFleetDelegateBlockerRef(blockerCode)],
      fallbackModule?: KhalaFleetDelegateModuleName,
    ): KhalaFleetDelegateBlockedResult => {
      push({
        blockerCode,
        message,
        module,
        precondition,
        refs,
        status: "blocked",
        ...(fallbackModule === undefined ? {} : { fallbackModule }),
      })
      return {
        blockerCode,
        blockerRefs: refs,
        message,
        pylonRef,
        signature: KhalaFleetDelegateSignature,
        status: "blocked",
        trace: [...trace],
      }
    }

    const ensure = yield* modules.ensurePylon(input).pipe(
      Effect.catch(error =>
        Effect.succeed<KhalaFleetDelegateEnsureResult | KhalaFleetDelegateModuleError>(error),
      ),
    )
    if (ensure instanceof KhalaFleetDelegateModuleError) {
      return block(undefined, "ensure_pylon", "pylon_online", ensure.blockerCode, ensure.message, ensure.refs)
    }
    push({
      message: ensure.started === true ? "Pylon started or adopted." : "Pylon is online.",
      module: "ensure_pylon",
      precondition: "pylon_online",
      refs: ensure.pylonRef === undefined ? [] : [`pylon:${ensure.pylonRef}`],
      status: ensure.started === true ? "recovered" : "satisfied",
    })

    const advertised = yield* advertiseCapacity(modules, ensure.pylonRef, "initial")
    const workerKind = resolveKhalaFleetDelegateWorkerKind(
      advertised instanceof KhalaFleetDelegateModuleError ? [] : advertised.capacity.accounts,
      parameters,
      input.workflowClassification,
    )
    const autoRoutingRefs = khalaFleetDelegateAutoRoutingRefs(input.workflowClassification, parameters)
    if (advertised instanceof KhalaFleetDelegateModuleError) {
      return block(
        ensure.pylonRef,
        "advertise_capacity",
        khalaFleetDelegateAdvertisedCapacityPrecondition(workerKind),
        advertised.blockerCode,
        advertised.message,
        advertised.refs,
      )
    }
    push({
      message: `Advertised ${workerKind} capacity ${khalaFleetDelegateAvailableSlotsForKind(advertised.capacity.accounts, workerKind)}/${advertised.capacity.max}.`,
      module: "advertise_capacity",
      precondition: khalaFleetDelegateAdvertisedCapacityPrecondition(workerKind),
      refs: [
        ...(advertised.heartbeatRef === undefined ? [] : [advertised.heartbeatRef]),
        ...autoRoutingRefs,
      ],
      status: khalaFleetDelegateAvailableSlotsForKind(advertised.capacity.accounts, workerKind) > 0 ? "satisfied" : "blocked",
    })

    const selected = selectKhalaFleetDelegateAccount(input, advertised.capacity.accounts, parameters)
    if (selected.status === "blocked") {
      return block(
        ensure.pylonRef,
        "select_account",
        "ready_account_free_slot",
        selected.blockerCode,
        selected.message,
        selected.blockerRefs,
      )
    }
    push({
      message: `Selected ${selected.workerKind} account ${selected.account.accountRef}.`,
      module: "select_account",
      precondition: "ready_account_free_slot",
      refs: [`worker_kind:${selected.workerKind}`, `account:${selected.account.accountRef}`, ...autoRoutingRefs],
      status: "satisfied",
    })

    const prepared = prepareKhalaFleetDelegateWork(input, parameters)
    push({
      message: prepared.kind === "fixture" ? "Prepared fixture work." : `Prepared ${prepared.repo}@${prepared.commit}.`,
      module: "prepare_work",
      precondition: "work_prepared",
      refs: prepared.kind === "fixture" ? [`fixture:${selected.workerKind}_agent_task`] : [`repo:${prepared.repo}`, `commit:${prepared.commit}`],
      status: prepared.kind === "fixture" ? "recovered" : "satisfied",
    })

    const dispatch = yield* dispatchWithFallbacks({
      account: selected.account,
      block,
      capacity: advertised.capacity,
      modules,
      parameters,
      push,
      pylonRef: ensure.pylonRef,
      workerKind: selected.workerKind,
      work: prepared,
    })
    if (dispatch.status === "blocked") {
      return dispatch
    }

    const verified = yield* modules.verifyCloseout({
      account: selected.account,
      assignmentRef: dispatch.assignmentRef,
      pylonRef: ensure.pylonRef,
      workerKind: selected.workerKind,
    }).pipe(
      Effect.catch(error =>
        Effect.succeed<KhalaFleetDelegateVerifyResult | KhalaFleetDelegateModuleError>(error),
      ),
    )
    if (verified instanceof KhalaFleetDelegateModuleError) {
      return block(
        ensure.pylonRef,
        "verify_closeout",
        "closeout_verified",
        verified.blockerCode,
        verified.message,
        verified.refs,
      )
    }
    if (!verified.ok) {
      return block(
        ensure.pylonRef,
        "verify_closeout",
        "closeout_verified",
        "verify_failed",
        verified.message ?? "Closeout verification failed.",
        verified.blockerRefs ?? [khalaFleetDelegateBlockerRef("verify_failed")],
      )
    }
    push({
      message: verified.message ?? "Closeout verified.",
      module: "verify_closeout",
      precondition: "closeout_verified",
      refs: verified.blockerRefs ?? [],
      status: "satisfied",
    })

    return {
      account: selected.account,
      assignmentRef: dispatch.assignmentRef,
      pylonRef: ensure.pylonRef,
      signature: KhalaFleetDelegateSignature,
      status: "completed",
      trace: [...trace],
      workerKind: selected.workerKind,
      work: prepared,
    }
  })

export function prepareKhalaFleetDelegateWork(
  input: KhalaFleetDelegateInput,
  parameters: KhalaFleetDelegationParameterSet = DefaultKhalaFleetDelegationParameterSet,
): KhalaFleetDelegateWork {
  if (input.fixture === true || (input.repo === undefined && input.commit === undefined && input.verify === undefined)) {
    return { fixture: true, kind: "fixture" }
  }
  const repo = input.repo
  const commit = input.commit
  const claimRef = input.claimRef
  const resolvedParameters = resolveKhalaFleetDelegationParameters(parameters)
  const verify = input.verify ?? resolvedParameters.verifyCriteria?.defaultVerify
  const missing = [
    repo === undefined ? "repo" : null,
    commit === undefined ? "commit" : null,
    verify === undefined ? "verify" : null,
    claimRef === undefined ? "claimRef" : null,
  ].filter((value): value is string => value !== null)
  if (repo === undefined || commit === undefined || verify === undefined || claimRef === undefined) {
    throw new Error(`khala.fleet.delegate requires fixture or complete work pins; missing ${missing.join(", ")}`)
  }
  return {
    branch: input.branch ?? "main",
    claimRef,
    commit,
    ...(input.issue === undefined ? {} : { issue: input.issue }),
    kind: "repo",
    repo,
    verify,
  }
}

export function selectKhalaFleetDelegateAccount(
  input: Pick<KhalaFleetDelegateInput, "accountRef" | "workflowClassification">,
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>,
  parameters: KhalaFleetDelegationParameterSet = DefaultKhalaFleetDelegationParameterSet,
):
  | Readonly<{
    account: KhalaFleetDelegateAccount
    status: "selected"
    workerKind: KhalaFleetDelegateConcreteWorkerKind
  }>
  | Readonly<{
    blockerCode: KhalaFleetDelegateBlockerCode
    blockerRefs: ReadonlyArray<string>
    message: string
    status: "blocked"
  }> {
  const workerKind = resolveKhalaFleetDelegateWorkerKind(accounts, parameters, input.workflowClassification)
  const requested = input.accountRef?.trim()
  const candidates = requested === undefined || requested.length === 0
    ? accounts.filter(account => khalaFleetDelegateAccountWorkerKind(account) === workerKind)
    : accounts.filter(account =>
      account.accountRef === requested &&
      khalaFleetDelegateAccountWorkerKind(account) === workerKind
    )
  if (requested !== undefined && requested.length > 0 && candidates.length === 0) {
    return accountSelectionBlocker(
      "connect_account_required",
      `Requested ${workerKind} account ${requested} is not connected.`,
    )
  }
  const sorted = [...candidates].sort((left, right) =>
    compareKhalaFleetDelegateAccounts(left, right, parameters),
  )
  const selected = sorted.find(account =>
    (account.readiness === "ready" || account.readiness === "available") &&
    (account.availableSlots === undefined || account.availableSlots > 0),
  )
  if (selected !== undefined) {
    return { account: selected, status: "selected", workerKind }
  }
  const first = sorted[0]
  if (first?.readiness === "credentials_missing") {
    return accountSelectionBlocker("credentials_missing", `${workerKind} account ${first.accountRef} is missing credentials.`)
  }
  if (first?.readiness === "revoked") {
    return accountSelectionBlocker("revoked", `${workerKind} account ${first.accountRef} is revoked.`)
  }
  if (sorted.some(account => account.readiness === "ready" || account.readiness === "available")) {
    return accountSelectionBlocker(
      khalaFleetDelegateNoAvailableCapacityBlocker(workerKind),
      `Ready ${workerKind} accounts have no advertised free slot.`,
    )
  }
  return accountSelectionBlocker("connect_account_required", `No ready ${workerKind} account is connected.`)
}

export function khalaFleetDelegateBlockerRef(code: KhalaFleetDelegateBlockerCode): string {
  if (code === "duplicate_active_assignment") {
    return "blocker.public.pylon_dispatch.duplicate_active_assignment"
  }
  if (code === "stale_heartbeat") {
    return "blocker.public.pylon_dispatch.stale_heartbeat"
  }
  if (code === "no_available_claude_capacity" || code === "no_available_codex_capacity") {
    return `blocker.public.pylon_dispatch.${code}`
  }
  return `blocker.public.khala_fleet_delegate.${code}`
}

function advertiseCapacity(
  modules: KhalaFleetDelegateModules,
  pylonRef: string | undefined,
  reason: KhalaFleetDelegateAdvertiseReason,
): Effect.Effect<KhalaFleetDelegateAdvertiseResult | KhalaFleetDelegateModuleError, never> {
  return modules.advertiseCapacity({ pylonRef, reason }).pipe(
    Effect.catch(error =>
      Effect.succeed<KhalaFleetDelegateAdvertiseResult | KhalaFleetDelegateModuleError>(error),
    ),
  )
}

function accountSelectionBlocker(
  blockerCode: KhalaFleetDelegateBlockerCode,
  message: string,
): Readonly<{
  blockerCode: KhalaFleetDelegateBlockerCode
  blockerRefs: ReadonlyArray<string>
  message: string
  status: "blocked"
}> {
  return {
    blockerCode,
    blockerRefs: [khalaFleetDelegateBlockerRef(blockerCode)],
    message,
    status: "blocked",
  }
}

function normalizeKhalaFleetDelegationParameterSet(
  parameters: KhalaFleetDelegationParameterSet,
): KhalaFleetDelegationParameterSet {
  assertPublicSafePolicyText("parameterSetRef", parameters.parameterSetRef)
  if (parameters.actionSubmissionRef !== undefined) {
    assertPublicSafePolicyText("actionSubmissionRef", parameters.actionSubmissionRef)
  }
  if (parameters.candidateRef !== undefined) {
    assertPublicSafePolicyText("candidateRef", parameters.candidateRef)
  }
  if (parameters.objectiveTemplate !== undefined) {
    assertPublicSafePolicyText("objectiveTemplate", parameters.objectiveTemplate)
  }
  if (parameters.verifyCriteria?.defaultVerify !== undefined) {
    assertPublicSafePolicyText("verifyCriteria.defaultVerify", parameters.verifyCriteria.defaultVerify)
  }
  khalaFleetDelegationPerAccountConcurrency(parameters, 1)
  khalaFleetDelegationMaxRequestedSlots(parameters, 1)
  khalaFleetDelegationDispatchAttempts(parameters)
  khalaFleetDelegationDuplicateBackoffMs(parameters)
  khalaFleetDelegationClassifierMinimumConfidence(parameters)
  khalaFleetDelegationClassifierPreferenceBonusSlots(parameters)
  return parameters
}

export function resolveKhalaFleetDelegateWorkerKind(
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>,
  parameters: KhalaFleetDelegationParameterSet = DefaultKhalaFleetDelegationParameterSet,
  workflowClassification?: KhalaFleetDelegationWorkflowClassification | undefined,
): KhalaFleetDelegateConcreteWorkerKind {
  const resolved = resolveKhalaFleetDelegationParameters(parameters)
  const requested = resolved.delegationTarget?.workerKind ?? "codex"
  if (requested !== "auto") return requested
  return scoreKhalaFleetDelegateAutoWorkerKind(accounts, resolved, workflowClassification)
}

function khalaFleetDelegateAccountWorkerKind(
  account: KhalaFleetDelegateAccount,
): KhalaFleetDelegateConcreteWorkerKind {
  return account.workerKind ?? "codex"
}

function khalaFleetDelegateAvailableSlotsForKind(
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>,
  workerKind: KhalaFleetDelegateConcreteWorkerKind,
): number {
  return accounts
    .filter(account =>
      khalaFleetDelegateAccountWorkerKind(account) === workerKind &&
      (account.readiness === "ready" || account.readiness === "available")
    )
    .reduce((total, account) => total + Math.max(0, account.availableSlots ?? 1), 0)
}

function scoreKhalaFleetDelegateAutoWorkerKind(
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>,
  parameters: KhalaFleetDelegationParameterSet,
  workflowClassification?: KhalaFleetDelegationWorkflowClassification | undefined,
): KhalaFleetDelegateConcreteWorkerKind {
  const codexSlots = khalaFleetDelegateAvailableSlotsForKind(accounts, "codex")
  const claudeSlots = khalaFleetDelegateAvailableSlotsForKind(accounts, "claude")
  const classifiedKind = khalaFleetDelegateClassifiedWorkerKind(parameters, workflowClassification)
  const classifierBonus = khalaFleetDelegationClassifierPreferenceBonusSlots(parameters)
  const codexScore = codexSlots + (classifiedKind === "codex" && codexSlots > 0 ? classifierBonus : 0)
  const claudeScore = claudeSlots + (classifiedKind === "claude" && claudeSlots > 0 ? classifierBonus : 0)
  if (claudeScore !== codexScore) return claudeScore > codexScore ? "claude" : "codex"

  const tieBreak = parameters.delegationTarget?.autoRouting?.tieBreakWorkerKind ?? "codex"
  const tieBreakSlots = tieBreak === "claude" ? claudeSlots : codexSlots
  const other = tieBreak === "claude" ? "codex" : "claude"
  const otherSlots = other === "claude" ? claudeSlots : codexSlots
  if (tieBreakSlots > 0 || otherSlots === 0) return tieBreak
  return other
}

function khalaFleetDelegateClassifiedWorkerKind(
  parameters: KhalaFleetDelegationParameterSet,
  workflowClassification?: KhalaFleetDelegationWorkflowClassification | undefined,
): KhalaFleetDelegateConcreteWorkerKind | undefined {
  if (workflowClassification === undefined) return undefined
  if (!khalaFleetDelegationWorkflowClassificationIsPublicSafe(workflowClassification)) {
    return undefined
  }
  if (workflowClassification.confidence < khalaFleetDelegationClassifierMinimumConfidence(parameters)) {
    return undefined
  }
  return khalaFleetDelegateWorkerKindForWorkflowClass(workflowClassification.workflowClass)
}

function khalaFleetDelegateAutoRoutingRefs(
  workflowClassification: KhalaFleetDelegationWorkflowClassification | undefined,
  parameters: KhalaFleetDelegationParameterSet,
): ReadonlyArray<string> {
  if ((parameters.delegationTarget?.workerKind ?? "codex") !== "auto") {
    return []
  }
  const workerKind = khalaFleetDelegateClassifiedWorkerKind(parameters, workflowClassification)
  if (workerKind === undefined || workflowClassification === undefined) {
    return []
  }
  return [
    `auto_route.worker_kind.${workerKind}`,
    `workflow.public.khala_coding.${workflowClassification.workflowClass}`,
    ...workflowClassification.evidenceRefs,
  ]
}

function khalaFleetDelegationWorkflowClassificationIsPublicSafe(
  workflowClassification: KhalaFleetDelegationWorkflowClassification,
): boolean {
  return Number.isFinite(workflowClassification.confidence) &&
    workflowClassification.confidence >= 0 &&
    workflowClassification.confidence <= 1 &&
    workflowClassification.evidenceRefs.every(ref => publicSafeWorkflowEvidenceRefPattern.test(ref))
}

function khalaFleetDelegateAdvertisedCapacityPrecondition(
  workerKind: KhalaFleetDelegateConcreteWorkerKind,
): KhalaFleetDelegatePrecondition {
  return workerKind === "claude" ? "advertised_claude_capacity" : "advertised_codex_capacity"
}

function khalaFleetDelegateNoAvailableCapacityBlocker(
  workerKind: KhalaFleetDelegateConcreteWorkerKind,
): "no_available_claude_capacity" | "no_available_codex_capacity" {
  return workerKind === "claude" ? "no_available_claude_capacity" : "no_available_codex_capacity"
}

function assertPublicSafePolicyText(label: string, value: string): void {
  const trimmed = value.trim()
  if (trimmed.length === 0 || trimmed.length > 8_000 || unsafePolicyTextPattern.test(trimmed)) {
    throw new Error(`${label} must be bounded public-safe delegation policy text`)
  }
}

function boundedPolicyInteger(
  value: number | undefined,
  fallback: number,
  min: number,
  max: number,
  label: string,
): number {
  if (value === undefined) return fallback
  if (!Number.isSafeInteger(value) || value < min || value > max) {
    throw new Error(`${label} must be an integer between ${min} and ${max}`)
  }
  return value
}

function boundedPolicyNumber(
  value: number | undefined,
  fallback: number,
  min: number,
  max: number,
  label: string,
): number {
  if (value === undefined) return fallback
  if (!Number.isFinite(value) || value < min || value > max) {
    throw new Error(`${label} must be a number between ${min} and ${max}`)
  }
  return value
}

function compareKhalaFleetDelegateAccounts(
  left: KhalaFleetDelegateAccount,
  right: KhalaFleetDelegateAccount,
  parameters: KhalaFleetDelegationParameterSet,
): number {
  const heuristic =
    resolveKhalaFleetDelegationParameters(parameters).accountRanking?.heuristic ??
    "named_ready_highest_slots"
  const leftReady = left.readiness === "ready" || left.readiness === "available"
  const rightReady = right.readiness === "ready" || right.readiness === "available"
  if (leftReady !== rightReady) {
    return leftReady ? -1 : 1
  }
  if (heuristic === "lexicographic_ready") {
    return left.accountRef.localeCompare(right.accountRef)
  }
  const leftSlots = left.availableSlots ?? 1
  const rightSlots = right.availableSlots ?? 1
  if (leftSlots !== rightSlots) {
    return rightSlots - leftSlots
  }
  if ((left.isDefault ?? false) !== (right.isDefault ?? false)) {
    if (heuristic === "default_ready_highest_slots") {
      return left.isDefault === true ? -1 : 1
    }
    return left.isDefault === true ? 1 : -1
  }
  return left.accountRef.localeCompare(right.accountRef)
}

function dispatchWithFallbacks(input: Readonly<{
  account: KhalaFleetDelegateAccount
  block: (
    pylonRef: string | undefined,
    module: KhalaFleetDelegateModuleName,
    precondition: KhalaFleetDelegatePrecondition,
    blockerCode: KhalaFleetDelegateBlockerCode,
    message: string,
    refs?: ReadonlyArray<string>,
    fallbackModule?: KhalaFleetDelegateModuleName,
  ) => KhalaFleetDelegateBlockedResult
  capacity: KhalaFleetDelegateCapacity
  modules: KhalaFleetDelegateModules
  parameters: KhalaFleetDelegationParameterSet
  push: (step: KhalaFleetDelegateStep) => void
  pylonRef: string | undefined
  workerKind: KhalaFleetDelegateConcreteWorkerKind
  work: KhalaFleetDelegateWork
}>): Effect.Effect<
  | Readonly<{ assignmentRef: string; status: "accepted" }>
  | KhalaFleetDelegateBlockedResult,
  never
> {
  return Effect.gen(function* () {
    let capacity = input.capacity
    const dispatchAttempts = khalaFleetDelegationDispatchAttempts(input.parameters)
    for (let attempt = 1; attempt <= dispatchAttempts; attempt += 1) {
      if (capacity.loadGated === true) {
        return input.block(
          input.pylonRef,
          "dispatch",
          "dispatch_accepted",
          "load_gated",
          `Machine load is too high for another ${input.workerKind} dispatch.`,
          [khalaFleetDelegateBlockerRef("load_gated")],
        )
      }
      const dispatched = yield* input.modules.dispatch({
        account: input.account,
        attempt,
        capacity,
        pylonRef: input.pylonRef,
        workerKind: input.workerKind,
        work: input.work,
      }).pipe(
        Effect.catch(error =>
          Effect.succeed<KhalaFleetDelegateDispatchResult | KhalaFleetDelegateModuleError>(error),
        ),
      )
      const result = dispatched instanceof KhalaFleetDelegateModuleError
        ? {
          blockerCode: dispatched.blockerCode,
          message: dispatched.message,
          ok: false,
          refs: dispatched.refs,
        } satisfies KhalaFleetDelegateDispatchResult
        : dispatched
      if (result.ok && result.assignmentRef !== undefined) {
        input.push({
          message: `Dispatch accepted ${result.assignmentRef}.`,
          module: "dispatch",
          precondition: "dispatch_accepted",
          refs: [result.assignmentRef],
          status: attempt > 1 ? "recovered" : "satisfied",
        })
        return { assignmentRef: result.assignmentRef, status: "accepted" }
      }
      const blockerCode = result.blockerCode ?? "dispatch_failed"
      const refs = result.refs ?? [khalaFleetDelegateBlockerRef(blockerCode)]
      if (blockerCode === "stale_heartbeat" && attempt < dispatchAttempts) {
        input.push({
          blockerCode,
          fallbackModule: "advertise_capacity",
          message: result.message ?? "Dispatch saw a stale heartbeat; refreshing capacity.",
          module: "dispatch",
          precondition: "dispatch_accepted",
          refs,
          status: "blocked",
        })
        const refreshed = yield* advertiseCapacity(input.modules, input.pylonRef, "stale_heartbeat")
        if (refreshed instanceof KhalaFleetDelegateModuleError) {
          return input.block(
            input.pylonRef,
            "advertise_capacity",
            khalaFleetDelegateAdvertisedCapacityPrecondition(input.workerKind),
            refreshed.blockerCode,
            refreshed.message,
            refreshed.refs,
          )
        }
        capacity = refreshed.capacity
        input.push({
          message: `Refreshed ${input.workerKind} capacity ${khalaFleetDelegateAvailableSlotsForKind(capacity.accounts, input.workerKind)}/${capacity.max}.`,
          module: "advertise_capacity",
          precondition: khalaFleetDelegateAdvertisedCapacityPrecondition(input.workerKind),
          refs: refreshed.heartbeatRef === undefined ? [] : [refreshed.heartbeatRef],
          status: "recovered",
        })
        continue
      }
      if (blockerCode === khalaFleetDelegateNoAvailableCapacityBlocker(input.workerKind) && attempt < dispatchAttempts) {
        input.push({
          blockerCode,
          fallbackModule: "advertise_capacity",
          message: result.message ?? `Dispatch found no ${input.workerKind} capacity; advertising capacity again.`,
          module: "dispatch",
          precondition: "dispatch_accepted",
          refs,
          status: "blocked",
        })
        const refreshed = yield* advertiseCapacity(
          input.modules,
          input.pylonRef,
          khalaFleetDelegateNoAvailableCapacityBlocker(input.workerKind),
        )
        if (refreshed instanceof KhalaFleetDelegateModuleError) {
          return input.block(
            input.pylonRef,
            "advertise_capacity",
            khalaFleetDelegateAdvertisedCapacityPrecondition(input.workerKind),
            refreshed.blockerCode,
            refreshed.message,
            refreshed.refs,
          )
        }
        capacity = refreshed.capacity
        input.push({
          message: `Advertised ${input.workerKind} capacity ${khalaFleetDelegateAvailableSlotsForKind(capacity.accounts, input.workerKind)}/${capacity.max}.`,
          module: "advertise_capacity",
          precondition: khalaFleetDelegateAdvertisedCapacityPrecondition(input.workerKind),
          refs: refreshed.heartbeatRef === undefined ? [] : [refreshed.heartbeatRef],
          status: "recovered",
        })
        continue
      }
      if (blockerCode === "duplicate_active_assignment" && attempt < dispatchAttempts) {
        input.push({
          blockerCode,
          fallbackModule: "dispatch",
          message: result.message ?? "Duplicate active assignment; backing off before retry.",
          module: "dispatch",
          precondition: "dispatch_accepted",
          refs,
          status: "blocked",
        })
        if (input.modules.backoff !== undefined) {
          const backedOff = yield* input.modules.backoff({ attempt, reason: "duplicate_active_assignment" }).pipe(
            Effect.as({ ok: true as const }),
            Effect.catch(error => Effect.succeed({ error, ok: false as const })),
          )
          if (!backedOff.ok) {
            return input.block(
              input.pylonRef,
              "dispatch",
              "dispatch_accepted",
              backedOff.error.blockerCode,
              backedOff.error.message,
              backedOff.error.refs,
            )
          }
        }
        continue
      }
      return input.block(
        input.pylonRef,
        "dispatch",
        "dispatch_accepted",
        blockerCode,
        result.message ?? "Dispatch failed.",
        refs,
      )
    }
    return input.block(
      input.pylonRef,
      "dispatch",
      "dispatch_accepted",
      "dispatch_failed",
      "Dispatch retry budget exhausted.",
      [khalaFleetDelegateBlockerRef("dispatch_failed")],
    )
  })
}
