import path from "node:path"

import type { FullAutoRunClientRunProjection } from "@openagentsinc/khala-sync"
import { publishFullAutoRunClientProjection } from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import type { FullAutoRun, FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import type { DesktopSessionCredential } from "./desktop-session-vault.ts"

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

/** Maps a durable `FullAutoRun` record to its public-safe client projection.
 * Returns `null` only for the defensive edge case of a run with no recorded
 * transition yet (a bare, never-started Draft) -- every mutating registry
 * method this module wraps (`start`/`startNew`/`rerun`/`transition`)
 * guarantees at least one transition on its result. */
export const toFullAutoRunClientProjection = (run: FullAutoRun): FullAutoRunClientRunProjection | null => {
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
}>): FullAutoRunProjectionPublisher => {
  const publish = Effect.fn("FullAutoRunProjectionPublisher.publish")(function* (run: FullAutoRun | null) {
    if (!input.sessionReady()) return
    const credential = input.credential()
    if (credential === null) return
    const projection = run === null ? null : toFullAutoRunClientProjection(run)
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
