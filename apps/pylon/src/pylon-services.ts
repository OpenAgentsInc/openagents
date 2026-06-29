import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import { Context, Data, Effect, Layer, Scope } from "effect"

import {
  executeCodexAgentAssignment,
  type CodexAgentExecutionOptions,
  type CodexAgentExecutionResult,
  type CodexAgentLease,
} from "./codex-agent-executor.js"
import {
  publishAssignmentPullRequest,
  type AssignmentPullRequestPublisher,
  type PublishAssignmentPullRequestInput,
  type PublishAssignmentPullRequestResult,
} from "./codex-pr-publisher.js"
import {
  materializeGitCheckoutWorkspace,
  removeMaterializedWorkspace,
  type GitCheckoutWorkspace,
  type MaterializedWorkspace,
  type WorkspaceCheckoutRunner,
} from "./workspace-materializer.js"
import {
  ensurePylonLocalState,
  resolveStatePaths,
  type PylonLocalState,
  type PylonPaths,
  type PylonPresenceState,
  type PylonRuntimeState,
} from "./state.js"
import type { BootstrapSummary } from "./bootstrap.js"

export type PylonServiceOperation =
  | "assignment.execute"
  | "config.env"
  | "pr.publish"
  | "state.ensure"
  | "state.read_presence"
  | "state.read_runtime"
  | "workspace.materialize"
  | "workspace.release"

export type PylonServiceErrorKind =
  | "not_found"
  | "malformed"
  | "storage_failed"
  | "adapter_failed"

function causeRefFor(error: unknown): string {
  const material =
    error instanceof Error
      ? `${error.name}:${error.message}`
      : typeof error === "string"
        ? error
        : JSON.stringify(error)
  return `cause.pylon.${createHash("sha256").update(material ?? "unknown").digest("hex").slice(0, 24)}`
}

export class PylonServiceError extends Data.TaggedError("PylonServiceError")<{
  readonly operation: PylonServiceOperation
  readonly kind: PylonServiceErrorKind
  readonly reasonRef: string
  readonly causeRef?: string
  readonly fallbackCloseoutUsed?: boolean
}> {}

function storageError(operation: PylonServiceOperation, error: unknown): PylonServiceError {
  return new PylonServiceError({
    operation,
    kind: "storage_failed",
    reasonRef: `reason.pylon.${operation}.storage_failed`,
    causeRef: causeRefFor(error),
  })
}

function adapterError(operation: PylonServiceOperation, error: unknown): PylonServiceError {
  return new PylonServiceError({
    operation,
    kind: "adapter_failed",
    reasonRef: `reason.pylon.${operation}.adapter_failed`,
    causeRef: causeRefFor(error),
  })
}

function malformedError(operation: PylonServiceOperation, reason: string): PylonServiceError {
  return new PylonServiceError({
    operation,
    kind: "malformed",
    reasonRef: `reason.pylon.${operation}.${reason}`,
  })
}

function notFoundError(operation: PylonServiceOperation): PylonServiceError {
  return new PylonServiceError({
    operation,
    kind: "not_found",
    reasonRef: `reason.pylon.${operation}.not_found`,
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function decodeRuntimeState(value: unknown): PylonRuntimeState | null {
  if (!isRecord(value)) return null
  if (!["offline", "online", "paused", "degraded", "assignment-ready"].includes(String(value.lifecycle))) {
    return null
  }
  if (!Array.isArray(value.capabilityRefs) || !Array.isArray(value.blockerRefs)) return null
  if (typeof value.resourceMode !== "string" || typeof value.updatedAt !== "string") return null
  if (value.displayName !== null && typeof value.displayName !== "string") return null
  if (!value.capabilityRefs.every((ref) => typeof ref === "string")) return null
  if (!value.blockerRefs.every((ref) => typeof ref === "string")) return null
  return value as PylonRuntimeState
}

function decodePresenceState(value: unknown): PylonPresenceState | null {
  if (!isRecord(value)) return null
  if (
    typeof value.registered !== "boolean" ||
    typeof value.linked !== "boolean" ||
    typeof value.stale !== "boolean" ||
    typeof value.pylonRef !== "string" ||
    typeof value.heartbeatSequence !== "number" ||
    !Array.isArray(value.blockerRefs) ||
    typeof value.updatedAt !== "string"
  ) {
    return null
  }
  if (value.registrationRef !== null && typeof value.registrationRef !== "string") return null
  if (value.linkRef !== null && typeof value.linkRef !== "string") return null
  if (value.lastHeartbeatAt !== null && typeof value.lastHeartbeatAt !== "string") return null
  if (value.sparkPayoutTargetRef !== null && typeof value.sparkPayoutTargetRef !== "string") return null
  if (!value.blockerRefs.every((ref) => typeof ref === "string")) return null
  return value as PylonPresenceState
}

function readJsonFileEffect(path: string, operation: PylonServiceOperation): Effect.Effect<unknown, PylonServiceError> {
  return Effect.tryPromise({
    try: async () => {
      if (!existsSync(path)) throw notFoundError(operation)
      try {
        return JSON.parse(await readFile(path, "utf8")) as unknown
      } catch (error) {
        if (error instanceof SyntaxError) throw malformedError(operation, "json_malformed")
        throw error
      }
    },
    catch: (error) =>
      error instanceof PylonServiceError ? error : storageError(operation, error),
  })
}

export type PylonRuntimeConfigShape = {
  readonly getEnv: (name: string) => Effect.Effect<string | undefined, PylonServiceError>
  readonly requireEnv: (name: string) => Effect.Effect<string, PylonServiceError>
  readonly redactedEnvSnapshot: (names: ReadonlyArray<string>) => Effect.Effect<Record<string, string | null>>
}

export class PylonRuntimeConfig extends Context.Service<
  PylonRuntimeConfig,
  PylonRuntimeConfigShape
>()("PylonRuntimeConfig") {}

export function makePylonRuntimeConfigLayer(env: Record<string, string | undefined> = Bun.env) {
  return Layer.succeed(PylonRuntimeConfig, {
    getEnv: (name) =>
      Effect.sync(() => env[name]),
    requireEnv: (name) =>
      Effect.flatMap(Effect.sync(() => env[name]), (value) =>
        value === undefined || value.trim().length === 0
          ? Effect.fail(malformedError("config.env", "missing_required_env"))
          : Effect.succeed(value),
      ),
    redactedEnvSnapshot: (names) =>
      Effect.sync(() =>
        Object.fromEntries(
          names.map((name) => [name, env[name] === undefined ? null : `redacted.${causeRefFor(`${name}:${env[name]}`)}`]),
        ),
      ),
  })
}

export const PylonRuntimeConfigLive = makePylonRuntimeConfigLayer()

export type PylonLocalStateStoreShape = {
  readonly ensure: (summary: Pick<BootstrapSummary, "bootstrap" | "paths">) => Effect.Effect<PylonLocalState, PylonServiceError>
  readonly readRuntime: (paths: PylonPaths) => Effect.Effect<PylonRuntimeState, PylonServiceError>
  readonly readPresence: (paths: PylonPaths) => Effect.Effect<PylonPresenceState, PylonServiceError>
  readonly resolvePaths: (paths: BootstrapSummary["paths"]) => Effect.Effect<PylonPaths>
}

export class PylonLocalStateStore extends Context.Service<
  PylonLocalStateStore,
  PylonLocalStateStoreShape
>()("PylonLocalStateStore") {}

export const PylonLocalStateStoreLive = Layer.succeed(PylonLocalStateStore, {
  ensure: (summary) =>
    Effect.tryPromise({
      try: () => ensurePylonLocalState(summary),
      catch: (error) => storageError("state.ensure", error),
    }),
  readRuntime: (paths) =>
    Effect.flatMap(readJsonFileEffect(paths.runtimeState, "state.read_runtime"), (value) => {
      const decoded = decodeRuntimeState(value)
      return decoded === null ? Effect.fail(malformedError("state.read_runtime", "record_malformed")) : Effect.succeed(decoded)
    }),
  readPresence: (paths) =>
    Effect.flatMap(readJsonFileEffect(paths.presenceState, "state.read_presence"), (value) => {
      const decoded = decodePresenceState(value)
      return decoded === null
        ? Effect.fail(malformedError("state.read_presence", "record_malformed"))
        : Effect.succeed(decoded)
    }),
  resolvePaths: (paths) => Effect.succeed(resolveStatePaths(paths)),
})

export type PylonWorkspaceMaterializerShape = {
  readonly materializeGitCheckout: (input: {
    cacheRoot: string
    checkout: GitCheckoutWorkspace
    checkoutRunner?: WorkspaceCheckoutRunner
    leaseRef: string
    refPrefix: string
  }) => Effect.Effect<MaterializedWorkspace, PylonServiceError>
  readonly releaseMaterialized: (input: {
    cacheRoot: string
    workingDirectory: string
  }) => Effect.Effect<void, PylonServiceError>
  readonly scopedGitCheckout: (input: {
    cacheRoot: string
    checkout: GitCheckoutWorkspace
    checkoutRunner?: WorkspaceCheckoutRunner
    leaseRef: string
    refPrefix: string
  }) => Effect.Effect<MaterializedWorkspace, PylonServiceError, Scope.Scope>
}

export class PylonWorkspaceMaterializer extends Context.Service<
  PylonWorkspaceMaterializer,
  PylonWorkspaceMaterializerShape
>()("PylonWorkspaceMaterializer") {}

const liveWorkspaceMaterializer = {
  materializeGitCheckout: (input) =>
    Effect.tryPromise({
      try: () => materializeGitCheckoutWorkspace(input),
      catch: (error) => adapterError("workspace.materialize", error),
    }),
  releaseMaterialized: (input) =>
    Effect.tryPromise({
      try: () => removeMaterializedWorkspace(input),
      catch: (error) => adapterError("workspace.release", error),
    }),
  scopedGitCheckout: (input) =>
    Effect.acquireRelease(
      Effect.tryPromise({
        try: () => materializeGitCheckoutWorkspace(input),
        catch: (error) => adapterError("workspace.materialize", error),
      }),
      (workspace) =>
        Effect.orDie(
          Effect.tryPromise({
            try: () =>
              removeMaterializedWorkspace({
                cacheRoot: input.cacheRoot,
                workingDirectory: workspace.workingDirectory,
              }),
            catch: (error) => adapterError("workspace.release", error),
          }),
        ),
    ),
} satisfies PylonWorkspaceMaterializerShape

export function makePylonWorkspaceMaterializerTestLayer(input: {
  materializeGitCheckout?: PylonWorkspaceMaterializerShape["materializeGitCheckout"]
  releaseMaterialized?: PylonWorkspaceMaterializerShape["releaseMaterialized"]
}) {
  return Layer.succeed(PylonWorkspaceMaterializer, {
    materializeGitCheckout: input.materializeGitCheckout ?? liveWorkspaceMaterializer.materializeGitCheckout,
    releaseMaterialized: input.releaseMaterialized ?? liveWorkspaceMaterializer.releaseMaterialized,
    scopedGitCheckout: (workspaceInput) =>
      Effect.acquireRelease(
        (input.materializeGitCheckout ?? liveWorkspaceMaterializer.materializeGitCheckout)(workspaceInput),
        (workspace) =>
          Effect.orDie(
            (input.releaseMaterialized ?? liveWorkspaceMaterializer.releaseMaterialized)({
              cacheRoot: workspaceInput.cacheRoot,
              workingDirectory: workspace.workingDirectory,
            }),
          ),
      ),
  })
}

export const PylonWorkspaceMaterializerLive = Layer.succeed(PylonWorkspaceMaterializer, liveWorkspaceMaterializer)

export type PylonAssignmentExecutorShape = {
  readonly executeCodex: (
    state: PylonLocalState,
    lease: CodexAgentLease,
    now: Date,
    options?: CodexAgentExecutionOptions,
  ) => Effect.Effect<CodexAgentExecutionResult, PylonServiceError>
}

export class PylonAssignmentExecutor extends Context.Service<
  PylonAssignmentExecutor,
  PylonAssignmentExecutorShape
>()("PylonAssignmentExecutor") {}

export const PylonAssignmentExecutorLive = Layer.succeed(PylonAssignmentExecutor, {
  executeCodex: (state, lease, now, options = {}) =>
    Effect.tryPromise({
      try: () => executeCodexAgentAssignment(state, lease, now, options),
      catch: (error) =>
        new PylonServiceError({
          operation: "assignment.execute",
          kind: "adapter_failed",
          reasonRef: "reason.pylon.assignment.execute.adapter_failed",
          causeRef: causeRefFor(error),
          fallbackCloseoutUsed: false,
        }),
    }),
})

export type PylonPullRequestCloseoutShape = {
  readonly publish: AssignmentPullRequestPublisherEffect
}

export class PylonPullRequestCloseout extends Context.Service<
  PylonPullRequestCloseout,
  PylonPullRequestCloseoutShape
>()("PylonPullRequestCloseout") {}

export type AssignmentPullRequestPublisherEffect = (
  input: PublishAssignmentPullRequestInput,
) => Effect.Effect<PublishAssignmentPullRequestResult, PylonServiceError>

export const PylonPullRequestCloseoutLive = Layer.succeed(PylonPullRequestCloseout, {
  publish: (input) =>
    Effect.tryPromise({
      try: () => publishAssignmentPullRequest(input),
      catch: (error) => adapterError("pr.publish", error),
    }),
})

export function makePylonPullRequestCloseoutTestLayer(publisher: AssignmentPullRequestPublisher) {
  return Layer.succeed(PylonPullRequestCloseout, {
    publish: (input) =>
      Effect.tryPromise({
        try: () => publisher(input),
        catch: (error) => adapterError("pr.publish", error),
      }),
  })
}
