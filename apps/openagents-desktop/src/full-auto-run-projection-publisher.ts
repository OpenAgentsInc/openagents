import path from "node:path"

import type { FullAutoRunClientReceiptSummary, FullAutoRunClientRunProjection } from "@openagentsinc/khala-sync"
import { publishFullAutoRunClientProjection } from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import { FULL_AUTO_RUN_TERMINAL_STATES, type FullAutoRun, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import type { FullAutoRunReceipt } from "./full-auto-run-report.ts"
import type { DesktopSessionCredential } from "./desktop-session-vault.ts"

/**
 * MOB-FA-02 (#8994): field-for-field mapping from the private
 * `FullAutoRunReceipt` (`full-auto-run-report.ts`, FA-RUN-04 #8972 --
 * already the redaction-tested public-safe derivation of the private
 * `FullAutoRunReport`) to the shared, mirrored client schema
 * `FullAutoRunClientReceiptSummary`. Explicit field-by-field rather than a
 * spread so a schema literal mismatch or a future field added to one side
 * only (and not the other) is a compile error, never a silent drop or a
 * `schema` value that fails to decode on the client.
 */
export const toFullAutoRunClientReceiptSummary = (
  receipt: FullAutoRunReceipt,
): FullAutoRunClientReceiptSummary => ({
  schema: "full_auto_run.mobile_receipt.v1",
  runRef: receipt.runRef,
  ...(receipt.threadRef === undefined ? {} : { threadRef: receipt.threadRef }),
  objectiveDigest: receipt.objectiveDigest,
  doneConditionDigest: receipt.doneConditionDigest,
  workspaceRefDigest: receipt.workspaceRefDigest,
  state: receipt.state,
  ...(receipt.startedAt === undefined ? {} : { startedAt: receipt.startedAt }),
  ...(receipt.endedAt === undefined ? {} : { endedAt: receipt.endedAt }),
  turnCap: receipt.turnCap,
  successfulAttempts: receipt.successfulAttempts,
  failedAttempts: receipt.failedAttempts,
  providerIdentities: receipt.providerIdentities,
  providerTransitionCount: receipt.providerTransitionCount,
  providerTransitionDispositions: receipt.providerTransitionDispositions,
  livenessGapCount: receipt.livenessGapCount,
  recoveryActionsUsed: receipt.recoveryActionsUsed,
  verifiedRefCount: receipt.verifiedRefCount,
  claimedRefCount: receipt.claimedRefCount,
  progressDisposition: receipt.progressDisposition,
  usageKnown: receipt.usageKnown,
  reportRevision: receipt.reportRevision,
  createdAt: receipt.createdAt,
})

/**
 * FA-RUN-05 (#8981): publishes a public-safe, structured live projection of
 * the signed-in user's currently active `FullAutoRun` to the new
 * `/api/full-auto-runs` endpoint so mobile (#8982) can fetch "my active run"
 * cross-device -- the exact gap #8980 identified ("Nothing today carries run
 * identity or lifecycle state into the synced scope or any endpoint mobile
 * can reach").
 *
 * Deliberately ADDITIVE and side-channel: this module never mutates
 * `full-auto-run-registry.ts` or `full-auto-reconcile.ts` (both hot files
 * shared with concurrent #8971/#8975 work). Instead
 * `wrapFullAutoRunRegistryWithProjectionPublish` decorates the ALREADY
 * hardened registry object at its `main.ts` construction site, publishing a
 * fire-and-forget projection after every state-changing call the registry
 * already exposes (`start`, `startNew`, `rerun`, `transition`). A publish
 * failure NEVER blocks, delays, or retries into the local turn/dispatch
 * path -- same discipline as `desktop-codex-usage-reporter.ts`'s "Telemetry
 * admission can never block the local turn."
 *
 * Public-safe boundary: `workspaceLabel` derives a short basename from the
 * local `workspaceRef` path (never the raw path itself); `objective` and
 * `doneCondition` are the user's own task/goal text (the same fields already
 * shown in Desktop's own UI), never raw prompts, tool output, or
 * credentials.
 */

const sanitizeWorkspaceLabel = (workspaceRef: string | undefined): string | null => {
  if (workspaceRef === undefined) return null
  const label = path.basename(workspaceRef).slice(0, 200)
  if (label.length === 0 || /[/\\]/u.test(label)) return null
  return label
}

/**
 * Maps a durable `FullAutoRun` record to its public-safe client projection.
 * Returns `null` only for the defensive edge case of a run with no recorded
 * transition yet (a bare, never-started Draft) -- every mutating registry
 * method this module wraps (`start`/`startNew`/`rerun`/`transition`)
 * guarantees at least one transition on its result.
 *
 * MOB-FA-02 (#8994): `extra` carries the two pieces of state this module has
 * no way to derive from a bare `FullAutoRun` alone -- the typed same-pass
 * rotation count (lives on the thread-level `FullAutoRecord`, not the run)
 * and the bounded terminal receipt summary (lives in the private
 * `FullAutoRunReportStore`). Both default to the honest empty value
 * (`rotationCount: 0`, `receiptSummary: null`) so every existing call site
 * keeps working unchanged.
 */
export const toFullAutoRunClientProjection = (
  run: FullAutoRun,
  extra: Readonly<{ rotationCount?: number; receiptSummary?: FullAutoRunClientReceiptSummary | null }> = {},
): FullAutoRunClientRunProjection | null => {
  const lastTransition = run.transitions.at(-1)
  if (lastTransition === undefined) return null
  return {
    runRef: run.runRef,
    threadRef: run.threadRef ?? null,
    objective: run.objective,
    doneCondition: run.doneCondition,
    lifecycleState: run.state,
    workspaceLabel: sanitizeWorkspaceLabel(run.workspaceRef),
    startedAt: run.startedAt ?? null,
    updatedAt: lastTransition.at,
    lastTransition: { actor: lastTransition.actor, at: lastTransition.at },
    laneRef: run.profile?.lane ?? null,
    accountRef: run.profile?.accountRef ?? null,
    turnCap: run.turnCap,
    successfulAttempts: run.successfulAttempts,
    failedAttempts: run.failedAttempts,
    rotationCount: extra.rotationCount ?? 0,
    receiptSummary: FULL_AUTO_RUN_TERMINAL_STATES.has(run.state) ? (extra.receiptSummary ?? null) : null,
  }
}

export type FullAutoRunProjectionPublisher = Readonly<{
  /** Fire-and-forget: publishes the projection for `run` (or clears the
   * projection when `run` is `null`). Never throws, never blocks the
   * caller -- always resolves. */
  publish: (run: FullAutoRun | null) => Effect.Effect<void>
}>

export const makeFullAutoRunProjectionPublisher = (input: Readonly<{
  sessionReady: () => boolean
  credential: () => DesktopSessionCredential | null
  baseUrl: string
  fetchImpl?: typeof fetch
  /** MOB-FA-02 (#8994): sourced from `main.ts`, which alone has access to
   * both the thread-level rotation history and the private report store.
   * Absent (the pre-#8994 default) means every projection publishes
   * `rotationCount: 0`/`receiptSummary: null`, exactly the prior behavior. */
  deriveExtra?: (run: FullAutoRun) => Readonly<{ rotationCount: number; receiptSummary: FullAutoRunClientReceiptSummary | null }>
}>): FullAutoRunProjectionPublisher => {
  const publish = Effect.fn("FullAutoRunProjectionPublisher.publish")(function* (run: FullAutoRun | null) {
    if (!input.sessionReady()) return
    const credential = input.credential()
    if (credential === null) return
    const extra = run === null ? {} : (input.deriveExtra?.(run) ?? {})
    const projection = run === null ? null : toFullAutoRunClientProjection(run, extra)
    if (run !== null && projection === null) return
    yield* Effect.promise(() =>
      publishFullAutoRunClientProjection({
        baseUrl: input.baseUrl,
        accessToken: credential.accessToken,
        run: projection,
        ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
      }),
    )
  })

  return {
    publish: run => publish(run).pipe(Effect.catch(() => Effect.void)),
  }
}

/**
 * Decorates an already-open `FullAutoRunRegistry` with a fire-and-forget
 * projection publish after every state-changing call that returns an
 * updated run. Never touches the registry's internal storage, transition
 * legality, or concurrency invariants -- every wrapped method delegates to
 * the original registry FIRST and only observes its (already-committed)
 * result.
 */
export const wrapFullAutoRunRegistryWithProjectionPublish = (
  registry: FullAutoRunRegistry,
  publisher: FullAutoRunProjectionPublisher,
): FullAutoRunRegistry => {
  const publishFireAndForget = (run: FullAutoRun): void => {
    Effect.runFork(publisher.publish(run))
  }

  return {
    ...registry,
    start: (runRef, options) => {
      const result = registry.start(runRef, options)
      if (result.ok) publishFireAndForget(result.run)
      return result
    },
    startNew: input => {
      const result = registry.startNew(input)
      if (result.ok) publishFireAndForget(result.run)
      return result
    },
    rerun: (fromRunRef, input) => {
      const result = registry.rerun(fromRunRef, input)
      if (result.ok) publishFireAndForget(result.run)
      return result
    },
    transition: (runRef, input) => {
      const result = registry.transition(runRef, input)
      if (result.ok) publishFireAndForget(result.run)
      return result
    },
  }
}
