/**
 * OPENRLM-SDK (#9154) — desktop `RlmCorpusSource` over owner-local stores.
 *
 * Builds an authorized HistoryCorpus, maps it to the generic immutable
 * `RlmCorpusHandle`, and never trusts model-supplied visibility or redaction
 * policy. The host owns admission; the SDK owns digests and traversal.
 */

import { Context, Effect, Layer } from "effect"
import {
  buildHistoryCorpus,
  historyCorpusCoverageNote,
  type HistoryCorpusEntry,
  type HistoryCorpusPolicy,
  type HistoryCorpusScope,
  type NeutralThreadSnapshot,
} from "@openagentsinc/history-corpus"
import type { HarnessEventLogStore } from "@openagentsinc/agent-harness-contract"
import {
  computeContentDigest,
  computeManifestDigest,
  makeInlineCorpusHandle,
  RlmCorpusError,
  RlmCorpusSource,
  type RlmCorpusEntry,
  type RlmCorpusHandle,
  type RlmCorpusInput,
  type RlmCorpusManifest,
  type RlmCorpusSourceShape,
  type RlmRedactionClass,
} from "@openagentsinc/rlm"

/**
 * Map history redaction classes onto the generic RLM vocabulary. History keeps
 * display-class names; RLM stores a coarser engine-safe class.
 */
export const mapHistoryRedactionToRlm = (
  redactionClass: HistoryCorpusEntry["redactionClass"],
): RlmRedactionClass => {
  switch (redactionClass) {
    case "public_ref":
      return "none"
    case "redacted_summary":
    case "operator_summary":
      return "redacted"
    case "private_ref":
      return "private_ref"
    default: {
      const _exhaustive: never = redactionClass
      return _exhaustive
    }
  }
}

/** Cursor address scheme for history citations round-tripping to the event log. */
export const DESKTOP_HISTORY_ADDRESS_SCHEMA_ID =
  "openagents.desktop.history_cursor.v1" as const

/** Strategy profile pin for the first desktop RLM train (no artifact sink). */
export const DESKTOP_RLM_STRATEGY_REF =
  "openagents.desktop.rlm.history.v1" as const

/**
 * Trusted per-call clamps for the initial desktop tool. Host clamps model
 * requests downward; no artifact sink is configured.
 */
export const desktopRlmRootLimits = {
  maxProgramNodesPerIteration: 32,
  maxProgramNodes: 128,
  maxFanOut: 16,
  maxFanIn: 16,
  maxConcurrentCalls: 4,
  maxValues: 64,
  maxItemsPerValue: 256,
  maxEnvironmentBytes: 1_048_576,
  maxInlineOutputBytes: 16_384,
} as const

/** Owner-local policy: admit private/operator/public history for local recall. */
export const desktopHistoryCorpusPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public", "operator", "private"],
  includeRedactionClasses: [
    "public_ref",
    "redacted_summary",
    "operator_summary",
    "private_ref",
  ],
}

export interface DesktopHistoryCorpusSourceInput {
  readonly eventLog: HarnessEventLogStore
  readonly turnIdsForThread: (
    threadId: string,
  ) => ReadonlyArray<string> | Promise<ReadonlyArray<string>>
  readonly threadSnapshot?: (
    threadId: string,
  ) => NeutralThreadSnapshot | null | Promise<NeutralThreadSnapshot | null>
  readonly builtAt?: () => string
  /**
   * Prove the current session may read the named thread. Fail closed when
   * omitted for non-test hosts — tests may pass `() => true`.
   */
  readonly authorizeThread: (threadId: string) => boolean | Promise<boolean>
  /**
   * Optional run membership resolver. When absent, `Run` scopes are refused.
   */
  readonly threadIdsForRun?: (
    runRef: string,
  ) => ReadonlyArray<string> | Promise<ReadonlyArray<string>>
  readonly policy?: HistoryCorpusPolicy
}

const defaultBuiltAt = (): string => new Date().toISOString()

const scopeThreadIds = (scope: HistoryCorpusScope): ReadonlyArray<string> => {
  switch (scope._tag) {
    case "Thread":
      return [scope.threadId]
    case "Run":
      return scope.threadIds
    case "ThreadSet":
      return scope.threadIds
    default: {
      const _exhaustive: never = scope
      return _exhaustive
    }
  }
}

const scopeRefOf = (scope: HistoryCorpusScope): string => {
  switch (scope._tag) {
    case "Thread":
      return `thread:${scope.threadId}`
    case "Run":
      return `run:${scope.runRef}`
    case "ThreadSet":
      return `threads:${[...scope.threadIds].sort().join(",")}`
    default: {
      const _exhaustive: never = scope
      return _exhaustive
    }
  }
}

/** Encode a durable history cursor as a generic source address. */
export const encodeHistoryCursorAddress = (input: {
  readonly turnId: string
  readonly sequence: number
}): { readonly addressSchemaId: string; readonly encodedAddress: string } => ({
  addressSchemaId: DESKTOP_HISTORY_ADDRESS_SCHEMA_ID,
  encodedAddress: `${input.turnId}\u0000${String(input.sequence)}`,
})

/** Decode a history cursor address; returns null when the schema does not match. */
export const decodeHistoryCursorAddress = (
  address: {
    readonly addressSchemaId: string
    readonly encodedAddress: string
  },
): { readonly turnId: string; readonly sequence: number } | null => {
  if (address.addressSchemaId !== DESKTOP_HISTORY_ADDRESS_SCHEMA_ID) return null
  const sep = address.encodedAddress.indexOf("\u0000")
  if (sep <= 0) return null
  const turnId = address.encodedAddress.slice(0, sep)
  const sequence = Number(address.encodedAddress.slice(sep + 1))
  if (!Number.isFinite(sequence) || sequence < 0) return null
  return { turnId, sequence }
}

/**
 * Map a history corpus entry onto the generic RLM entry shape with stable
 * ordinals and digest-relevant fields. Observed timestamps stay optional on
 * the RLM entry so content digests remain builtAt-stable.
 */
export const historyEntriesToRlmEntries = (
  entries: ReadonlyArray<HistoryCorpusEntry>,
): ReadonlyArray<RlmCorpusEntry> =>
  entries.map((entry, ordinal) => {
    const address = encodeHistoryCursorAddress({
      turnId: entry.turnId,
      sequence: entry.sequence,
    })
    const base: RlmCorpusEntry = {
      ordinal,
      entryRef: `${entry.turnId}#${entry.sequence}`,
      scopeRef: entry.scopeRef,
      sourceKind: entry.kind,
      sourceAddress: address,
      visibility: entry.visibility,
      redactionClass: mapHistoryRedactionToRlm(entry.redactionClass),
      observedAt: entry.observedAt,
    }
    return entry.text === undefined ? base : { ...base, text: entry.text }
  })

export const buildRlmInlineCorpusFromHistory = (input: {
  readonly scope: HistoryCorpusScope
  readonly entries: ReadonlyArray<HistoryCorpusEntry>
  readonly exclusions?: ReadonlyArray<{ readonly reason: string; readonly count: number }>
  readonly builtAt: string
  readonly coverageNote?: string
}): {
  readonly manifest: RlmCorpusManifest
  readonly entries: ReadonlyArray<RlmCorpusEntry>
  readonly corpusInput: Extract<RlmCorpusInput, { readonly _tag: "Inline" }>
} => {
  const rlmEntries = historyEntriesToRlmEntries(input.entries)
  const scopeRef = scopeRefOf(input.scope)
  const ordering = {
    rule: "chronological" as const,
    note: "HistoryCorpus chronological order mapped to contiguous ordinals.",
  }
  const encodedBytes = rlmEntries.reduce(
    (sum, e) => sum + (e.text?.length ?? 0) + e.entryRef.length + 64,
    0,
  )
  const contentDigest = computeContentDigest({
    scopeRef,
    ordering,
    entries: rlmEntries,
  })
  const coverage = {
    note: input.coverageNote ?? historyCorpusCoverageNote,
    entryCount: rlmEntries.length,
    encodedBytes,
    exclusions: input.exclusions ?? [],
  }
  const manifestDigest = computeManifestDigest({
    contentDigest,
    coverage,
    scopeRef,
    ordering,
  })
  const manifest: RlmCorpusManifest = {
    schemaId: "openagents.ai.rlm_corpus.v1",
    corpusRef: `desktop.history:${scopeRef}:${contentDigest.slice(0, 16)}`,
    contentDigest,
    manifestDigest,
    ordering,
    coverage,
    scopeRef,
    builtAt: input.builtAt,
  }
  const corpusInput = {
    _tag: "Inline" as const,
    manifest,
    entries: rlmEntries,
  }
  return { manifest, entries: rlmEntries, corpusInput }
}

/**
 * Parse a Source corpus ref produced by this host. Model-supplied free-form
 * refs that do not match are rejected before any store read.
 */
export const parseDesktopHistorySourceRef = (
  encodedAddress: string,
): HistoryCorpusScope | null => {
  try {
    const parsed = JSON.parse(encodedAddress) as unknown
    if (typeof parsed !== "object" || parsed === null) return null
    const tag = (parsed as { _tag?: unknown })._tag
    if (tag === "Thread" && typeof (parsed as { threadId?: unknown }).threadId === "string") {
      return { _tag: "Thread", threadId: (parsed as { threadId: string }).threadId }
    }
    if (
      tag === "Run" &&
      typeof (parsed as { runRef?: unknown }).runRef === "string" &&
      Array.isArray((parsed as { threadIds?: unknown }).threadIds)
    ) {
      const threadIds = (parsed as { threadIds: unknown[] }).threadIds.filter(
        (id): id is string => typeof id === "string",
      )
      return {
        _tag: "Run",
        runRef: (parsed as { runRef: string }).runRef,
        threadIds,
      }
    }
    if (tag === "ThreadSet" && Array.isArray((parsed as { threadIds?: unknown }).threadIds)) {
      const threadIds = (parsed as { threadIds: unknown[] }).threadIds.filter(
        (id): id is string => typeof id === "string",
      )
      return { _tag: "ThreadSet", threadIds }
    }
    return null
  } catch {
    return null
  }
}

export const encodeDesktopHistorySourceRef = (scope: HistoryCorpusScope): string =>
  JSON.stringify(scope)

/**
 * Build one authorized RLM corpus handle from host stores for a logical scope.
 * Authorization runs before any event-log read.
 */
export const resolveDesktopHistoryCorpus = (
  sources: DesktopHistoryCorpusSourceInput,
  scope: HistoryCorpusScope,
): Effect.Effect<RlmCorpusHandle, RlmCorpusError> =>
  Effect.gen(function* () {
    let resolvedScope = scope

    if (scope._tag === "Run") {
      if (sources.threadIdsForRun === undefined) {
        return yield* new RlmCorpusError({
          reason: "unavailable",
          detailSafe: "run scope requires host run membership resolver",
        })
      }
      const membership = yield* Effect.tryPromise({
        try: async () => await sources.threadIdsForRun!(scope.runRef),
        catch: (cause) => cause,
      }).pipe(
        Effect.mapError(
          () =>
            new RlmCorpusError({
              reason: "unavailable",
              detailSafe: "run membership lookup failed",
            }),
        ),
      )
      // Host membership is authoritative; model-supplied threadIds are ignored.
      resolvedScope = {
        _tag: "Run",
        runRef: scope.runRef,
        threadIds: [...membership],
      }
    }

    const threadIds = scopeThreadIds(resolvedScope)
    if (threadIds.length === 0) {
      return yield* new RlmCorpusError({
        reason: "unavailable",
        detailSafe: "scope resolved to zero threads",
      })
    }

    for (const threadId of threadIds) {
      const allowed = yield* Effect.tryPromise({
        try: async () => await sources.authorizeThread(threadId),
        catch: () => false,
      }).pipe(Effect.orElseSucceed(() => false))
      if (!allowed) {
        return yield* new RlmCorpusError({
          reason: "unavailable",
          detailSafe: "thread not authorized for current session",
        })
      }
    }

    const turnIds: Array<string> = []
    const threads: Array<NeutralThreadSnapshot> = []
    for (const threadId of threadIds) {
      const ids = yield* Effect.tryPromise({
        try: async () => await sources.turnIdsForThread(threadId),
        catch: () => [] as ReadonlyArray<string>,
      }).pipe(Effect.orElseSucceed(() => [] as ReadonlyArray<string>))
      turnIds.push(...ids)
      if (sources.threadSnapshot !== undefined) {
        const snap = yield* Effect.tryPromise({
          try: async () => await sources.threadSnapshot!(threadId),
          catch: () => null,
        }).pipe(Effect.orElseSucceed(() => null))
        if (snap !== null) threads.push(snap)
      }
    }

    const builtAt = (sources.builtAt ?? defaultBuiltAt)()
    const built = yield* buildHistoryCorpus({
      scope: resolvedScope,
      eventLog: sources.eventLog,
      turnIds,
      threads,
      policy: sources.policy ?? desktopHistoryCorpusPolicy,
      builtAt,
    }).pipe(
      Effect.mapError(
        (err) =>
          new RlmCorpusError({
            reason: "unavailable",
            detailSafe:
              typeof err === "object" &&
              err !== null &&
              "message" in err &&
              typeof (err as { message: unknown }).message === "string"
                ? String((err as { message: string }).message).slice(0, 200)
                : "history corpus build failed",
          }),
      ),
    )

    const exclusionCounts: Array<{ readonly reason: string; readonly count: number }> =
      []
    if (built.manifest.exclusions.excludedByVisibility > 0) {
      exclusionCounts.push({
        reason: "excluded_by_visibility",
        count: built.manifest.exclusions.excludedByVisibility,
      })
    }
    if (built.manifest.exclusions.excludedByRedaction > 0) {
      exclusionCounts.push({
        reason: "excluded_by_redaction",
        count: built.manifest.exclusions.excludedByRedaction,
      })
    }

    const { corpusInput } = buildRlmInlineCorpusFromHistory({
      scope: resolvedScope,
      entries: built.entries,
      exclusions: exclusionCounts,
      builtAt,
      coverageNote: built.manifest.coverage.note,
    })

    return yield* makeInlineCorpusHandle(corpusInput)
  })

export const makeDesktopHistoryCorpusSource = (
  sources: DesktopHistoryCorpusSourceInput,
): RlmCorpusSourceShape => ({
  resolve: (input) =>
    Effect.gen(function* () {
      if (input._tag === "Inline") {
        // Only hermetic tests may pass Inline; product host refuses model-shaped inline.
        return yield* makeInlineCorpusHandle(input)
      }
      if (input.sourceRef.addressSchemaId !== DESKTOP_HISTORY_ADDRESS_SCHEMA_ID) {
        return yield* new RlmCorpusError({
          reason: "unavailable",
          detailSafe: "unsupported corpus source address schema",
        })
      }
      const scope = parseDesktopHistorySourceRef(input.sourceRef.encodedAddress)
      if (scope === null) {
        return yield* new RlmCorpusError({
          reason: "invalid_inline",
          detailSafe: "corpus source address is not a host-authorized scope",
        })
      }
      return yield* resolveDesktopHistoryCorpus(sources, scope)
    }),
})

export class DesktopHistoryCorpusSource extends Context.Service<
  DesktopHistoryCorpusSource,
  RlmCorpusSourceShape
>()("@openagentsinc/desktop/DesktopHistoryCorpusSource") {}

export const desktopHistoryCorpusSourceLayer = (
  sources: DesktopHistoryCorpusSourceInput,
): Layer.Layer<RlmCorpusSource | DesktopHistoryCorpusSource> => {
  const shape = makeDesktopHistoryCorpusSource(sources)
  return Layer.mergeAll(
    Layer.succeed(RlmCorpusSource, RlmCorpusSource.of(shape)),
    Layer.succeed(DesktopHistoryCorpusSource, DesktopHistoryCorpusSource.of(shape)),
  )
}

/** Build a Source corpus input for a host-authorized scope (tool path). */
export const desktopHistoryCorpusInputForScope = (
  scope: HistoryCorpusScope,
): Extract<RlmCorpusInput, { readonly _tag: "Source" }> => ({
  _tag: "Source",
  sourceRef: {
    addressSchemaId: DESKTOP_HISTORY_ADDRESS_SCHEMA_ID,
    encodedAddress: encodeDesktopHistorySourceRef(scope),
  },
})
