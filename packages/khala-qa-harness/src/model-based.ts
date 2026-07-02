import { Effect, Schema as S } from "effect"

import type { KhalaCodeQaObservation } from "./driver.js"
import type { KhalaCodeRpcQaDriver } from "./rpc-driver.js"

type DeepMutable<T> = T extends readonly (infer U)[]
  ? DeepMutable<U>[]
  : T extends object
    ? { -readonly [P in keyof T]: DeepMutable<T[P]> }
    : T

export const KhalaCodeQaThreadModel = S.Struct({
  archived: S.Boolean,
  deleted: S.Boolean,
  forked: S.Boolean,
  state: S.Literals([
    "none",
    "starting",
    "ready",
    "turn_active",
    "interrupted",
    "completed",
    "resumed",
    "forked",
    "archived",
    "deleted",
  ]),
  threadId: S.NullOr(S.String),
  title: S.String,
})
export type KhalaCodeQaThreadModel = DeepMutable<typeof KhalaCodeQaThreadModel.Type>

export const KhalaCodeQaApprovalModel = S.Struct({
  decision: S.NullOr(S.Literals(["accept", "reject"])),
  method: S.Literals([
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
  ]),
  requestId: S.NullOr(S.String),
  state: S.Literals(["requested", "answered", "superseded", "turn_interrupted"]),
})
export type KhalaCodeQaApprovalModel = DeepMutable<typeof KhalaCodeQaApprovalModel.Type>

export const KhalaCodeQaFleetDelegateModule = S.Literals([
  "ensure_pylon",
  "advertise_capacity",
  "select_account",
  "prepare_work",
  "dispatch",
  "verify_closeout",
])
export type KhalaCodeQaFleetDelegateModule = typeof KhalaCodeQaFleetDelegateModule.Type

export const KhalaCodeQaFleetDelegateModuleState = S.Struct({
  module: KhalaCodeQaFleetDelegateModule,
  status: S.Literals(["pending", "satisfied", "recovered", "blocked"]),
})
export type KhalaCodeQaFleetDelegateModuleState =
  DeepMutable<typeof KhalaCodeQaFleetDelegateModuleState.Type>

export const KhalaCodeQaFleetDelegateProgramModel = S.Struct({
  acceptedCount: S.Number,
  delegateStatus: S.Literals(["idle", "completed", "blocked"]),
  modules: S.Array(KhalaCodeQaFleetDelegateModuleState),
  requestedCount: S.Number,
})
export type KhalaCodeQaFleetDelegateProgramModel =
  DeepMutable<typeof KhalaCodeQaFleetDelegateProgramModel.Type>

export const KhalaCodeQaAppServerSupervisorModel = S.Struct({
  initialized: S.Boolean,
  state: S.Literals(["idle", "starting", "ready", "restarting", "disposed", "errored"]),
})
export type KhalaCodeQaAppServerSupervisorModel =
  DeepMutable<typeof KhalaCodeQaAppServerSupervisorModel.Type>

export const KhalaCodeQaModelState = S.Struct({
  approval: KhalaCodeQaApprovalModel,
  delegateProgram: KhalaCodeQaFleetDelegateProgramModel,
  supervisor: KhalaCodeQaAppServerSupervisorModel,
  thread: KhalaCodeQaThreadModel,
})
export type KhalaCodeQaModelState = DeepMutable<typeof KhalaCodeQaModelState.Type>

export const KhalaCodeQaModelDivergence = S.Struct({
  command: S.String,
  expected: S.Unknown,
  lifecycle: S.Literals(["thread", "approval", "delegate_program", "app_server_supervisor"]),
  observed: S.Unknown,
  summary: S.String,
})
export type KhalaCodeQaModelDivergence = DeepMutable<typeof KhalaCodeQaModelDivergence.Type>

export const KhalaCodeQaModelRunReport = S.Struct({
  commands: S.Array(S.String),
  divergences: S.Array(KhalaCodeQaModelDivergence),
  schema: S.Literal("khala_code_qa_model_based_report.v1"),
})
export type KhalaCodeQaModelRunReport = DeepMutable<typeof KhalaCodeQaModelRunReport.Type>

export const KHALA_CODE_QA_DELEGATE_MODULES: readonly KhalaCodeQaFleetDelegateModule[] = [
  "ensure_pylon",
  "advertise_capacity",
  "select_account",
  "prepare_work",
  "dispatch",
  "verify_closeout",
]

export type KhalaCodeQaModelRuntime = {
  readonly driver: KhalaCodeRpcQaDriver
  readonly report: KhalaCodeQaModelRunReport
}

export class KhalaCodeQaModelDivergenceError extends Error {
  readonly report: KhalaCodeQaModelRunReport

  constructor(report: KhalaCodeQaModelRunReport) {
    super(`Khala Code QA model divergence: ${JSON.stringify(report.divergences[0] ?? report)}`)
    this.name = "KhalaCodeQaModelDivergenceError"
    this.report = report
  }
}

export const initialKhalaCodeQaModelState = (): KhalaCodeQaModelState => ({
  approval: {
    decision: null,
    method: "item/commandExecution/requestApproval",
    requestId: "approval-fixture",
    state: "requested",
  },
  delegateProgram: {
    acceptedCount: 0,
    delegateStatus: "idle",
    modules: KHALA_CODE_QA_DELEGATE_MODULES.map((module) => ({ module, status: "pending" })),
    requestedCount: 0,
  },
  supervisor: {
    initialized: false,
    state: "idle",
  },
  thread: {
    archived: false,
    deleted: false,
    forked: false,
    state: "none",
    threadId: null,
    title: "Fixture thread",
  },
})

export const initialKhalaCodeQaModelReport = (): KhalaCodeQaModelRunReport => ({
  commands: [],
  divergences: [],
  schema: "khala_code_qa_model_based_report.v1",
})

export const decodeKhalaCodeQaModelState = (input: unknown): KhalaCodeQaModelState =>
  S.decodeUnknownSync(KhalaCodeQaModelState)(input) as KhalaCodeQaModelState

export const decodeKhalaCodeQaModelRunReport = (input: unknown): KhalaCodeQaModelRunReport =>
  S.decodeUnknownSync(KhalaCodeQaModelRunReport)(input) as KhalaCodeQaModelRunReport

export const recordKhalaCodeQaModelCommand = (
  report: KhalaCodeQaModelRunReport,
  command: string,
): void => {
  report.commands.push(command)
}

export const recordKhalaCodeQaModelDivergence = (
  report: KhalaCodeQaModelRunReport,
  divergence: KhalaCodeQaModelDivergence,
): never => {
  report.divergences.push(divergence)
  throw new KhalaCodeQaModelDivergenceError(report)
}

const observationValue = <T>(observation: KhalaCodeQaObservation): T => {
  if (!observation.ok) {
    throw new Error(`RPC observation failed: ${observation.label}: ${observation.error ?? "unknown error"}`)
  }
  const data = observation.data as { readonly value?: unknown } | undefined
  return data?.value as T
}

const rpcAct = async (
  runtime: KhalaCodeQaModelRuntime,
  method: string,
  args: ReadonlyArray<unknown> = [],
): Promise<KhalaCodeQaObservation> =>
  Effect.runPromise(runtime.driver.act({ args, kind: "rpc_call", method }))

const hasThread = (value: unknown, expectedId: string | null): boolean => {
  const record = value as {
    readonly data?: readonly { readonly id?: string }[]
    readonly items?: readonly { readonly id?: string }[]
    readonly threads?: readonly { readonly id?: string }[]
  }
  const threads = [
    ...(record.threads ?? []),
    ...(record.data ?? []),
    ...(record.items ?? []),
  ]
  return expectedId !== null && threads.some((thread) => thread.id === expectedId)
}

const threadField = (value: unknown, field: string): unknown => {
  const thread = (value as { readonly thread?: Record<string, unknown> }).thread
  return thread?.[field]
}

const assertThreadRead = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
  command: string,
): Promise<void> => {
  if (model.thread.threadId === null || model.thread.deleted) return
  const result = observationValue(await rpcAct(runtime, "codexThreadRead", [{
    includeTurns: true,
    threadId: model.thread.threadId,
  }]))
  const observedId = (result as { readonly threadId?: string }).threadId
  const observedArchived = threadField(result, "archived")
  if (observedId !== model.thread.threadId || observedArchived !== model.thread.archived) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { archived: model.thread.archived, threadId: model.thread.threadId },
      lifecycle: "thread",
      observed: result,
      summary: "thread read projection diverged from model",
    })
  }
}

const assertThreadList = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
  command: string,
): Promise<void> => {
  const activeList = observationValue(await rpcAct(runtime, "codexThreadList", [{ archived: false }]))
  const archivedList = observationValue(await rpcAct(runtime, "codexThreadList", [{ archived: true }]))
  const shouldShowActive =
    model.thread.threadId !== null && !model.thread.deleted && !model.thread.archived
  const shouldShowArchived =
    model.thread.threadId !== null && !model.thread.deleted && model.thread.archived

  if (hasThread(activeList, model.thread.threadId) !== shouldShowActive) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { activeVisible: shouldShowActive, thread: model.thread },
      lifecycle: "thread",
      observed: activeList,
      summary: "thread active-list visibility diverged from model",
    })
  }
  if (hasThread(archivedList, model.thread.threadId) !== shouldShowArchived) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { archivedVisible: shouldShowArchived, thread: model.thread },
      lifecycle: "thread",
      observed: archivedList,
      summary: "thread archived-list visibility diverged from model",
    })
  }
}

export const startThreadModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.start"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.thread = { ...model.thread, archived: false, deleted: false, forked: false, state: "starting" }
  const result = observationValue<{ readonly threadId: string }>(
    await rpcAct(runtime, "codexThreadStart", [{ sessionId: "desktop-session-fixture" }]),
  )
  model.supervisor = { initialized: true, state: "ready" }
  model.thread = { ...model.thread, state: "ready", threadId: result.threadId }
  await assertThreadList(model, runtime, command)
  await assertThreadRead(model, runtime, command)
}

export const completeTurnModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.turn.complete"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  model.thread = { ...model.thread, state: "turn_active" }
  const result = observationValue<{ readonly backend?: { readonly threadId?: string } }>(
    await rpcAct(runtime, "codexTurnStart", [{
      messages: [{ body: "fixture model turn", id: "user-model", role: "user" }],
      sessionId: "desktop-session-fixture",
      threadId: model.thread.threadId,
    }]),
  )
  model.thread = { ...model.thread, state: "completed" }
  if (result.backend?.threadId !== model.thread.threadId) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { threadId: model.thread.threadId },
      lifecycle: "thread",
      observed: result,
      summary: "completed turn response was not bound to the modeled thread",
    })
  }
  await assertThreadRead(model, runtime, command)
}

export const interruptTurnModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.turn.interrupt"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  const previous = { ...model.thread }
  const result = observationValue<{ readonly error?: string; readonly ok: boolean; readonly threadId?: string }>(
    await rpcAct(runtime, "codexTurnInterrupt", [{ sessionId: "desktop-session-fixture", turnId: "turn-fixture" }]),
  )
  if (!result.ok) {
    model.thread = previous
    if (result.error === undefined) {
      recordKhalaCodeQaModelDivergence(runtime.report, {
        command,
        expected: { ok: false, error: "No active Codex turn is registered for this desktop session." },
        lifecycle: "thread",
        observed: result,
        summary: "turn interrupt no-active-turn response omitted the handler error",
      })
    }
    await assertThreadRead(model, runtime, command)
    return
  }
  model.thread = { ...model.thread, state: "interrupted" }
  if (result.threadId !== model.thread.threadId) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { threadId: model.thread.threadId },
      lifecycle: "thread",
      observed: result,
      summary: "turn interrupt response was not bound to the modeled thread",
    })
  }
  await assertThreadRead(model, runtime, command)
}

export const archiveThreadModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.archive"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  model.thread = { ...model.thread, archived: true, state: "archived" }
  await rpcAct(runtime, "codexThreadArchive", [{ threadId: model.thread.threadId }])
  await assertThreadList(model, runtime, command)
  await assertThreadRead(model, runtime, command)
}

export const unarchiveThreadModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.unarchive"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  model.thread = { ...model.thread, archived: false, state: "resumed" }
  await rpcAct(runtime, "codexThreadUnarchive", [{ threadId: model.thread.threadId }])
  await assertThreadList(model, runtime, command)
  await assertThreadRead(model, runtime, command)
}

export const forkThreadModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.fork"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  model.thread = { ...model.thread, forked: true, state: "forked" }
  const result = observationValue<{ readonly newThreadId?: string }>(
    await rpcAct(runtime, "codexThreadFork", [{
      sessionId: "desktop-session-fixture",
      threadId: model.thread.threadId,
    }]),
  )
  if (result.newThreadId === undefined || result.newThreadId === model.thread.threadId) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { newThreadId: "present-and-distinct", sourceThreadId: model.thread.threadId },
      lifecycle: "thread",
      observed: result,
      summary: "thread fork response did not expose a distinct new thread id",
    })
  }
  model.thread = {
    ...model.thread,
    archived: false,
    state: "forked",
    threadId: result.newThreadId ?? model.thread.threadId,
  }
  await assertThreadRead(model, runtime, command)
}

export const deleteThreadModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "thread.delete"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  if (model.thread.threadId === null || model.thread.deleted) return
  model.thread = { ...model.thread, deleted: true, state: "deleted" }
  await rpcAct(runtime, "codexThreadDelete", [{ threadId: model.thread.threadId }])
  await assertThreadList(model, runtime, command)
}

export const answerApprovalModelCommand = async (
  decision: "accept" | "reject",
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = `approval.${decision}`
  recordKhalaCodeQaModelCommand(runtime.report, command)
  const requestId = model.approval.requestId ?? "approval-fixture"
  model.approval = { ...model.approval, decision, requestId, state: "answered" }
  const result = observationValue<{ readonly ok: boolean; readonly requestId: string | number }>(
    await rpcAct(runtime, "codexApprovalRespond", [{
      action: decision === "accept" ? "accept" : "decline",
      method: model.approval.method,
      requestId,
    }]),
  )
  if (model.supervisor.state !== "ready") {
    if (result.ok) {
      recordKhalaCodeQaModelDivergence(runtime.report, {
        command,
        expected: { ok: false, state: model.supervisor.state },
        lifecycle: "approval",
        observed: result,
        summary: "approval response succeeded while app-server was not running",
      })
    }
    return
  }
  if (!result.ok || String(result.requestId) !== requestId) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { ok: true, requestId },
      lifecycle: "approval",
      observed: result,
      summary: "approval response did not answer the modeled request",
    })
  }
}

export const interruptApprovalModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "approval.turn_interrupted"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.approval = { ...model.approval, decision: null, state: "turn_interrupted" }
  const result = observationValue<{
    readonly error?: string
    readonly ok: boolean
  }>(await rpcAct(runtime, "codexTurnInterrupt", [{
    sessionId: "desktop-session-fixture",
    turnId: "approval-no-pending-turn",
  }]))
  if (result.ok || result.error === undefined) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { ok: false, error: "No active Codex turn is registered for this desktop session." },
      lifecycle: "approval",
      observed: result,
      summary: "turn interruption without a pending turn diverged from handler semantics",
    })
  }
}

export const supersedeApprovalModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "approval.supersede"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  const previousRequestId = model.approval.requestId ?? "approval-fixture"
  model.approval = {
    ...model.approval,
    decision: null,
    requestId: "approval-fixture-next",
    state: "superseded",
  }
  const result = observationValue<{
    readonly ok: boolean
    readonly requestId: string | number
  }>(await rpcAct(runtime, "codexApprovalRespond", [{
    action: "cancel",
    method: model.approval.method,
    requestId: previousRequestId,
  }]))
  if (model.supervisor.state !== "ready") {
    if (result.ok) {
      recordKhalaCodeQaModelDivergence(runtime.report, {
        command,
        expected: { ok: false, state: model.supervisor.state },
        lifecycle: "approval",
        observed: result,
        summary: "approval supersede succeeded while app-server was not running",
      })
    }
    return
  }
  if (!result.ok || String(result.requestId) !== previousRequestId) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: { ok: true, requestId: previousRequestId },
      lifecycle: "approval",
      observed: result,
      summary: "approval supersede did not surface the no-pending handler response",
    })
  }
}

export const delegateProgramModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "fleet.delegate"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.delegateProgram = {
    acceptedCount: 1,
    delegateStatus: "completed",
    modules: KHALA_CODE_QA_DELEGATE_MODULES.map((module) => ({
      module,
      status: module === "prepare_work" ? "recovered" : "satisfied",
    })),
    requestedCount: 1,
  }
  const result = observationValue<{
    readonly acceptedCount: number
    readonly delegateStatus: "blocked" | "completed"
    readonly requestedCount: number
    readonly trace: readonly { readonly module: string; readonly status: string }[]
  }>(await rpcAct(runtime, "codexFleetDelegateRun", [{
    mode: "fixture",
    objective: "fixture model-based delegate run",
    requestedCount: 1,
  }]))
  const observedModules = result.trace.map((step) => step.module)
  const observedModuleStatuses = new Map(result.trace.map((step) => [step.module, step.status]))
  if (
    result.delegateStatus !== "completed" ||
    result.acceptedCount !== model.delegateProgram.acceptedCount ||
    result.requestedCount !== model.delegateProgram.requestedCount ||
    KHALA_CODE_QA_DELEGATE_MODULES.some((module) => !observedModules.includes(module)) ||
    model.delegateProgram.modules.some((step) => observedModuleStatuses.get(step.module) !== step.status)
  ) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: model.delegateProgram,
      lifecycle: "delegate_program",
      observed: result,
      summary: "fleet delegate six-module trace diverged from model",
    })
  }
}

const supervisorStatusToModelState = (state: string): KhalaCodeQaAppServerSupervisorModel["state"] => {
  switch (state) {
    case "running":
      return "ready"
    case "starting":
      return "starting"
    case "stopped":
      return "disposed"
    case "errored":
      return "errored"
    default:
      return "idle"
  }
}

export const appServerStartModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "app_server.start"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.supervisor = { initialized: true, state: "ready" }
  const result = observationValue<{ readonly status: { readonly initialized: boolean; readonly state: string } }>(
    await rpcAct(runtime, "codexAppServerStart"),
  )
  if (!result.status.initialized || supervisorStatusToModelState(result.status.state) !== model.supervisor.state) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: model.supervisor,
      lifecycle: "app_server_supervisor",
      observed: result,
      summary: "app-server start did not reach ready supervisor state",
    })
  }
}

export const appServerRestartModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "app_server.restart"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.supervisor = { initialized: true, state: "restarting" }
  const result = observationValue<{ readonly status: { readonly initialized: boolean; readonly state: string } }>(
    await rpcAct(runtime, "codexAppServerRestart"),
  )
  model.supervisor = { initialized: true, state: "ready" }
  model.thread = {
    archived: false,
    deleted: false,
    forked: false,
    state: "none",
    threadId: null,
    title: model.thread.title,
  }
  if (!result.status.initialized || supervisorStatusToModelState(result.status.state) !== model.supervisor.state) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: model.supervisor,
      lifecycle: "app_server_supervisor",
      observed: result,
      summary: "app-server restart did not settle back to ready",
    })
  }
}

export const appServerStopModelCommand = async (
  model: KhalaCodeQaModelState,
  runtime: KhalaCodeQaModelRuntime,
): Promise<void> => {
  const command = "app_server.stop"
  recordKhalaCodeQaModelCommand(runtime.report, command)
  model.supervisor = { initialized: false, state: "disposed" }
  const result = observationValue<{ readonly status: { readonly initialized: boolean; readonly state: string } }>(
    await rpcAct(runtime, "codexAppServerStop"),
  )
  model.thread = {
    archived: false,
    deleted: false,
    forked: false,
    state: "none",
    threadId: null,
    title: model.thread.title,
  }
  if (result.status.initialized || supervisorStatusToModelState(result.status.state) !== model.supervisor.state) {
    recordKhalaCodeQaModelDivergence(runtime.report, {
      command,
      expected: model.supervisor,
      lifecycle: "app_server_supervisor",
      observed: result,
      summary: "app-server stop did not dispose supervisor state",
    })
  }
}
