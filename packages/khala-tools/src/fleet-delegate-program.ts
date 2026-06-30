import { Effect, Schema as S } from "effect"

export const KhalaFleetDelegateSignature = "khala.fleet.delegate" as const

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
  "pylon_unavailable",
  "revoked",
  "stale_heartbeat",
  "verify_failed",
])
export type KhalaFleetDelegateBlockerCode = typeof KhalaFleetDelegateBlockerCode.Type

export const KhalaFleetDelegateStepStatus = S.Literals(["blocked", "recovered", "satisfied"])
export type KhalaFleetDelegateStepStatus = typeof KhalaFleetDelegateStepStatus.Type

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
    commit: string
    kind: "repo"
    repo: string
    verify: string
  }>

export type KhalaFleetDelegateInput = Readonly<{
  accountRef?: string | undefined
  branch?: string | undefined
  commit?: string | undefined
  fixture?: boolean | undefined
  objective: string
  repo?: string | undefined
  verify?: string | undefined
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
    reason: "initial" | "no_available_codex_capacity" | "stale_heartbeat"
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
  }>) => Effect.Effect<KhalaFleetDelegateDispatchResult, KhalaFleetDelegateModuleError>
  ensurePylon: (input: KhalaFleetDelegateInput) => Effect.Effect<KhalaFleetDelegateEnsureResult, KhalaFleetDelegateModuleError>
  verifyCloseout: (input: Readonly<{
    account: KhalaFleetDelegateAccount
    assignmentRef: string
    pylonRef: string | undefined
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

const DEFAULT_DISPATCH_ATTEMPTS = 4

export const runKhalaFleetDelegateProgram = (
  input: KhalaFleetDelegateInput,
  modules: KhalaFleetDelegateModules,
): Effect.Effect<KhalaFleetDelegateProgramResult, never> =>
  Effect.gen(function* () {
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
    if (advertised instanceof KhalaFleetDelegateModuleError) {
      return block(
        ensure.pylonRef,
        "advertise_capacity",
        "advertised_codex_capacity",
        advertised.blockerCode,
        advertised.message,
        advertised.refs,
      )
    }
    push({
      message: `Advertised Codex capacity ${advertised.capacity.available}/${advertised.capacity.max}.`,
      module: "advertise_capacity",
      precondition: "advertised_codex_capacity",
      refs: advertised.heartbeatRef === undefined ? [] : [advertised.heartbeatRef],
      status: advertised.capacity.available > 0 ? "satisfied" : "blocked",
    })

    const selected = selectKhalaFleetDelegateAccount(input, advertised.capacity.accounts)
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
      message: `Selected ${selected.account.accountRef}.`,
      module: "select_account",
      precondition: "ready_account_free_slot",
      refs: [`account:${selected.account.accountRef}`],
      status: "satisfied",
    })

    const prepared = prepareKhalaFleetDelegateWork(input)
    push({
      message: prepared.kind === "fixture" ? "Prepared fixture work." : `Prepared ${prepared.repo}@${prepared.commit}.`,
      module: "prepare_work",
      precondition: "work_prepared",
      refs: prepared.kind === "fixture" ? ["fixture:codex_agent_task"] : [`repo:${prepared.repo}`, `commit:${prepared.commit}`],
      status: prepared.kind === "fixture" ? "recovered" : "satisfied",
    })

    const dispatch = yield* dispatchWithFallbacks({
      account: selected.account,
      block,
      capacity: advertised.capacity,
      modules,
      push,
      pylonRef: ensure.pylonRef,
      work: prepared,
    })
    if (dispatch.status === "blocked") {
      return dispatch
    }

    const verified = yield* modules.verifyCloseout({
      account: selected.account,
      assignmentRef: dispatch.assignmentRef,
      pylonRef: ensure.pylonRef,
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
      work: prepared,
    }
  })

export function prepareKhalaFleetDelegateWork(input: KhalaFleetDelegateInput): KhalaFleetDelegateWork {
  if (input.fixture === true || (input.repo === undefined && input.commit === undefined && input.verify === undefined)) {
    return { fixture: true, kind: "fixture" }
  }
  const repo = input.repo
  const commit = input.commit
  const verify = input.verify
  const missing = [
    repo === undefined ? "repo" : null,
    commit === undefined ? "commit" : null,
    verify === undefined ? "verify" : null,
  ].filter((value): value is string => value !== null)
  if (repo === undefined || commit === undefined || verify === undefined) {
    throw new Error(`khala.fleet.delegate requires fixture or complete work pins; missing ${missing.join(", ")}`)
  }
  return {
    branch: input.branch ?? "main",
    commit,
    kind: "repo",
    repo,
    verify,
  }
}

export function selectKhalaFleetDelegateAccount(
  input: Pick<KhalaFleetDelegateInput, "accountRef">,
  accounts: ReadonlyArray<KhalaFleetDelegateAccount>,
):
  | Readonly<{ account: KhalaFleetDelegateAccount; status: "selected" }>
  | Readonly<{
    blockerCode: KhalaFleetDelegateBlockerCode
    blockerRefs: ReadonlyArray<string>
    message: string
    status: "blocked"
  }> {
  const requested = input.accountRef?.trim()
  const candidates = requested === undefined || requested.length === 0
    ? accounts
    : accounts.filter(account => account.accountRef === requested)
  if (requested !== undefined && requested.length > 0 && candidates.length === 0) {
    return accountSelectionBlocker(
      "connect_account_required",
      `Requested Codex account ${requested} is not connected.`,
    )
  }
  const sorted = [...candidates].sort(compareKhalaFleetDelegateAccounts)
  const selected = sorted.find(account =>
    (account.readiness === "ready" || account.readiness === "available") &&
    (account.availableSlots === undefined || account.availableSlots > 0),
  )
  if (selected !== undefined) {
    return { account: selected, status: "selected" }
  }
  const first = sorted[0]
  if (first?.readiness === "credentials_missing") {
    return accountSelectionBlocker("credentials_missing", `Codex account ${first.accountRef} is missing credentials.`)
  }
  if (first?.readiness === "revoked") {
    return accountSelectionBlocker("revoked", `Codex account ${first.accountRef} is revoked.`)
  }
  if (sorted.some(account => account.readiness === "ready" || account.readiness === "available")) {
    return accountSelectionBlocker("no_available_codex_capacity", "Ready Codex accounts have no advertised free slot.")
  }
  return accountSelectionBlocker("connect_account_required", "No ready Codex account is connected.")
}

export function khalaFleetDelegateBlockerRef(code: KhalaFleetDelegateBlockerCode): string {
  if (code === "duplicate_active_assignment") {
    return "blocker.public.pylon_dispatch.duplicate_active_assignment"
  }
  if (code === "stale_heartbeat") {
    return "blocker.public.pylon_dispatch.stale_heartbeat"
  }
  if (code === "no_available_codex_capacity") {
    return "blocker.public.pylon_dispatch.no_available_codex_capacity"
  }
  return `blocker.public.khala_fleet_delegate.${code}`
}

function advertiseCapacity(
  modules: KhalaFleetDelegateModules,
  pylonRef: string | undefined,
  reason: "initial" | "no_available_codex_capacity" | "stale_heartbeat",
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

function compareKhalaFleetDelegateAccounts(
  left: KhalaFleetDelegateAccount,
  right: KhalaFleetDelegateAccount,
): number {
  const leftReady = left.readiness === "ready" || left.readiness === "available"
  const rightReady = right.readiness === "ready" || right.readiness === "available"
  if (leftReady !== rightReady) {
    return leftReady ? -1 : 1
  }
  const leftSlots = left.availableSlots ?? 1
  const rightSlots = right.availableSlots ?? 1
  if (leftSlots !== rightSlots) {
    return rightSlots - leftSlots
  }
  if ((left.isDefault ?? false) !== (right.isDefault ?? false)) {
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
  push: (step: KhalaFleetDelegateStep) => void
  pylonRef: string | undefined
  work: KhalaFleetDelegateWork
}>): Effect.Effect<
  | Readonly<{ assignmentRef: string; status: "accepted" }>
  | KhalaFleetDelegateBlockedResult,
  never
> {
  return Effect.gen(function* () {
    let capacity = input.capacity
    for (let attempt = 1; attempt <= DEFAULT_DISPATCH_ATTEMPTS; attempt += 1) {
      if (capacity.loadGated === true) {
        return input.block(
          input.pylonRef,
          "dispatch",
          "dispatch_accepted",
          "load_gated",
          "Machine load is too high for another Codex dispatch.",
          [khalaFleetDelegateBlockerRef("load_gated")],
        )
      }
      const dispatched = yield* input.modules.dispatch({
        account: input.account,
        attempt,
        capacity,
        pylonRef: input.pylonRef,
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
      if (blockerCode === "stale_heartbeat" && attempt < DEFAULT_DISPATCH_ATTEMPTS) {
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
            "advertised_codex_capacity",
            refreshed.blockerCode,
            refreshed.message,
            refreshed.refs,
          )
        }
        capacity = refreshed.capacity
        input.push({
          message: `Refreshed Codex capacity ${capacity.available}/${capacity.max}.`,
          module: "advertise_capacity",
          precondition: "advertised_codex_capacity",
          refs: refreshed.heartbeatRef === undefined ? [] : [refreshed.heartbeatRef],
          status: "recovered",
        })
        continue
      }
      if (blockerCode === "no_available_codex_capacity" && attempt < DEFAULT_DISPATCH_ATTEMPTS) {
        input.push({
          blockerCode,
          fallbackModule: "advertise_capacity",
          message: result.message ?? "Dispatch found no Codex capacity; advertising capacity again.",
          module: "dispatch",
          precondition: "dispatch_accepted",
          refs,
          status: "blocked",
        })
        const refreshed = yield* advertiseCapacity(input.modules, input.pylonRef, "no_available_codex_capacity")
        if (refreshed instanceof KhalaFleetDelegateModuleError) {
          return input.block(
            input.pylonRef,
            "advertise_capacity",
            "advertised_codex_capacity",
            refreshed.blockerCode,
            refreshed.message,
            refreshed.refs,
          )
        }
        capacity = refreshed.capacity
        input.push({
          message: `Advertised Codex capacity ${capacity.available}/${capacity.max}.`,
          module: "advertise_capacity",
          precondition: "advertised_codex_capacity",
          refs: refreshed.heartbeatRef === undefined ? [] : [refreshed.heartbeatRef],
          status: "recovered",
        })
        continue
      }
      if (blockerCode === "duplicate_active_assignment" && attempt < DEFAULT_DISPATCH_ATTEMPTS) {
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
