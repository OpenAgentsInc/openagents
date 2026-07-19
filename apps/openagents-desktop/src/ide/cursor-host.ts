import { Context, Effect, Exit, Layer, Result, Scope } from "effect"

import {
  IdeCursorCommandResultSchema,
  IdeCursorSnapshotSchema,
  decodeIdeCursorCommand,
  emptyIdeCursorSnapshot,
  type IdeCursorCommand,
  type IdeCursorCommandResult,
  type IdeCursorSnapshot,
} from "./cursor-contract.ts"
import { IdeCursorProvider, type IdeCursorProviderShape } from "./cursor-provider.ts"
import {
  IdeCursorAuthorityFailure,
  IdeCursorDocumentAuthority,
  IdeCursorInvalidInput,
  IdeCursorProposalAuthority,
  IdeCursorService,
  IdeCursorStale,
  makeIdeCursorServiceLayer,
  type IdeCursorDocumentAuthorityShape,
  type IdeCursorProposalAuthorityShape,
  type IdeCursorServiceError,
  type IdeCursorServiceShape,
} from "./cursor-service.ts"

export type IdeCursorHost = Readonly<{
  snapshot: () => Promise<IdeCursorSnapshot>
  command: (value: unknown) => Promise<IdeCursorCommandResult>
  dispose: () => Promise<void>
}>

type RefusedReason = Extract<IdeCursorCommandResult, { readonly _tag: "Refused" }>["reason"]

const resultReason = (error: IdeCursorServiceError): RefusedReason => {
  if (error instanceof IdeCursorInvalidInput) return "invalid_input"
  if (error instanceof IdeCursorStale) {
    switch (error.reason) {
      case "sequence": return "stale_sequence"
      case "anchor": return "stale_anchor"
      case "identity": return "identity_mismatch"
      case "candidate": return "candidate_missing"
      case "stopped": return "stopped"
    }
  }
  if (error instanceof IdeCursorAuthorityFailure) {
    switch (error.reason) {
      case "stale": return "authority_stale"
      case "unavailable": return "authority_unavailable"
      case "conflict": return "conflict"
    }
  }
  return "unavailable"
}

const executeCommand = (
  service: IdeCursorServiceShape,
  command: IdeCursorCommand,
): Effect.Effect<IdeCursorSnapshot, IdeCursorServiceError> => {
  switch (command._tag) {
    case "Start": return service.start(command.input)
    case "Decide": return service.decide(command.decision)
    case "Stop": return service.stop(command.reason)
  }
}

export const openIdeCursorHost = async (
  provider: IdeCursorProviderShape,
  authority: IdeCursorDocumentAuthorityShape,
  options: Readonly<{
    now?: () => string
    initialSequence?: number
    proposalAuthority?: IdeCursorProposalAuthorityShape
  }> = {},
): Promise<IdeCursorHost> => {
  const scope = await Effect.runPromise(Scope.make())
  const baseDependencies = Layer.merge(
    Layer.succeed(IdeCursorProvider, provider),
    Layer.succeed(IdeCursorDocumentAuthority, authority),
  )
  const dependencies = options.proposalAuthority === undefined
    ? baseDependencies
    : Layer.merge(
        baseDependencies,
        Layer.succeed(IdeCursorProposalAuthority, options.proposalAuthority),
      )
  const layer = makeIdeCursorServiceLayer(options).pipe(Layer.provide(dependencies))
  const context = await Effect.runPromise(Layer.buildWithScope(layer, scope))
  const service = Context.get(context, IdeCursorService)
  let disposed = false
  let finalSnapshot = IdeCursorSnapshotSchema.make({
    ...emptyIdeCursorSnapshot(),
    state: "stopped",
  })

  const snapshot = async (): Promise<IdeCursorSnapshot> => {
    if (disposed) return finalSnapshot
    return Effect.runPromise(service.snapshot).catch(() => finalSnapshot)
  }

  const refused = async (reason: RefusedReason, message: string): Promise<IdeCursorCommandResult> =>
    IdeCursorCommandResultSchema.cases.Refused.make({
      reason,
      message: message.slice(0, 2_000),
      snapshot: await snapshot(),
    })

  const command = async (value: unknown): Promise<IdeCursorCommandResult> => {
    if (disposed) return refused("stopped", "The IDE cursor scope is closed.")
    const decoded = decodeIdeCursorCommand(value)
    if (decoded === null) return refused("invalid_input", "The IDE cursor command did not match the schema boundary.")
    try {
      const settled = await Effect.runPromise(Effect.result(executeCommand(service, decoded)))
      if (Result.isFailure(settled)) return refused(resultReason(settled.failure), settled.failure.detail)
      return IdeCursorCommandResultSchema.cases.Succeeded.make({ snapshot: settled.success })
    } catch {
      return refused("unavailable", "The IDE cursor command failed outside the typed service boundary.")
    }
  }

  const dispose = async (): Promise<void> => {
    if (disposed) return
    try {
      finalSnapshot = await Effect.runPromise(service.stop("host dispose"))
    } catch {
      finalSnapshot = IdeCursorSnapshotSchema.make({
        ...await snapshot(),
        activeRequestRef: null,
        activeAttemptRef: null,
        state: "stopped",
      })
    }
    disposed = true
    await Effect.runPromise(Scope.close(scope, Exit.void))
  }

  return { snapshot, command, dispose }
}
