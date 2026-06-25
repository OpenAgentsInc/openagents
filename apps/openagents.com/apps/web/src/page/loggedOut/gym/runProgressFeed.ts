import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Array as Arr, Option, Schema as S } from 'effect'

import {
  type GymRunProgressStreamModel,
  type PublicGymRunProgressModel,
  GymRunProgressStreamModel as GymRunProgressStreamModelSchema,
  LoadedPublicGymRunProgress,
} from '../model'
import { GymRunProgressPublicProjection } from './runProgress'

// Live Gym / Harbor "Follow an active Terminal-Bench run" panel feed (#6261).
//
// PUSH is the primary path: the panel seeds its run cards ONCE from the public
// sync-room snapshot on `/gym` entry, then upserts each run card the instant the
// operator ingest publishes a public-safe projected snapshot over the
// `public-gym-run-progress:network` sync scope. This module owns the pure
// reducers; update.ts wires them to messages and subscriptions.ts opens the
// socket. Mirrors `khala-tokens-served-feed.ts`.
//
// UPSERT-BY-runRef. Each streamed put carries the public-safe
// `openagents.gym.run_progress.v1` projection for ONE run, keyed by `runRef`. The
// reducer replaces that run's card and keeps the others, so the panel never shows
// stale or duplicate runs. The Worker is the authority; the client only mirrors.

export const GYM_RUN_PROGRESS_ID = 'network'
export const GYM_RUN_PROGRESS_SCOPE = `public-gym-run-progress:${GYM_RUN_PROGRESS_ID}`
export const GYM_RUN_PROGRESS_SYNC_COLLECTION = 'gym_run_progress'

// Bound the de-dupe ledger so a long-lived session can't grow it without limit.
// Far more than any plausible cursor-replay batch; older refs age out FIFO. With
// the upsert-by-runRef reducer this is belt-and-suspenders, but it keeps a
// replayed put from redundantly re-touching `appliedEventRefs`.
const MAX_APPLIED_EVENT_REFS = 256

const decodeProjection = (
  value: unknown,
): Option.Option<GymRunProgressPublicProjection> =>
  S.decodeUnknownOption(GymRunProgressPublicProjection)(value)

const withConnection = (
  model: GymRunProgressStreamModel,
  connection: GymRunProgressStreamModel['connection'],
): GymRunProgressStreamModel =>
  GymRunProgressStreamModelSchema({ ...model, connection })

export const gymRunProgressStreamConnecting = (
  model: GymRunProgressStreamModel,
): GymRunProgressStreamModel => withConnection(model, 'connecting')

export const gymRunProgressStreamOpen = (
  model: GymRunProgressStreamModel,
): GymRunProgressStreamModel => withConnection(model, 'open')

export const gymRunProgressStreamClosed = (
  model: GymRunProgressStreamModel,
): GymRunProgressStreamModel => withConnection(model, 'closed')

export const gymRunProgressStreamFailed = (
  model: GymRunProgressStreamModel,
): GymRunProgressStreamModel => withConnection(model, 'failed')

// Upsert one run's projection into the runs list by `runRef`: replace the run
// with a matching ref, keep every other run, or append when it is new. The Worker
// already ordered/redacted runs; the panel only mirrors the latest per-run state,
// so the simple replace-or-append keeps a single card per run with no duplicates.
const upsertRun = (
  runs: ReadonlyArray<GymRunProgressPublicProjection>,
  next: GymRunProgressPublicProjection,
): ReadonlyArray<GymRunProgressPublicProjection> => {
  const replaced = runs.map(run => (run.runRef === next.runRef ? next : run))

  return Arr.some(runs, run => run.runRef === next.runRef)
    ? replaced
    : [...runs, next]
}

// Seed the run cards + cursor from ONE public snapshot read of the gym
// run-progress sync scope. The snapshot collapses puts by `runRef`, so each
// snapshot entry is the latest per-run projection; subscribing strictly from the
// snapshot cursor means a put already baked into the seed is never replayed into a
// duplicate. The runs are always set to a Loaded state (possibly empty) so the
// honest empty state shows only when there are truly no runs.
export const gymRunProgressAfterSnapshot = (
  input: Readonly<{
    counter: PublicGymRunProgressModel
    cursor: number
    runs: ReadonlyArray<GymRunProgressPublicProjection>
    stream: GymRunProgressStreamModel
  }>,
): Readonly<{
  counter: PublicGymRunProgressModel
  stream: GymRunProgressStreamModel
}> => {
  const seededStream = GymRunProgressStreamModelSchema({
    ...input.stream,
    cursor: Math.max(input.stream.cursor, input.cursor),
  })

  return {
    counter: LoadedPublicGymRunProgress({ runs: input.runs }),
    stream: seededStream,
  }
}

// Apply one streamed patch: advance the stream cursor + record the applied event
// ref, and upsert the run's projection by `runRef`. A patch for an already-applied
// event ref, an undecodable value, a non-put op, or an unknown collection only
// advances the cursor. A patch that arrives before the panel is Loaded seeds a
// fresh Loaded list with that single run (the snapshot seed normally arrives
// first, but a push that races ahead must not be dropped).
export const applyGymRunProgressPatch = (
  input: Readonly<{
    counter: PublicGymRunProgressModel
    patch: SyncPatch
    stream: GymRunProgressStreamModel
  }>,
): Readonly<{
  counter: PublicGymRunProgressModel
  stream: GymRunProgressStreamModel
}> => {
  const cursor = Math.max(input.stream.cursor, input.patch.seq)
  const advancedStream = GymRunProgressStreamModelSchema({
    ...input.stream,
    cursor,
  })
  const unchanged = { counter: input.counter, stream: advancedStream }

  if (input.patch.op !== 'put' && input.patch.op !== 'patch') {
    return unchanged
  }

  if (input.patch.collection !== GYM_RUN_PROGRESS_SYNC_COLLECTION) {
    return unchanged
  }

  return Option.match(decodeProjection(input.patch.value), {
    onNone: () => unchanged,
    onSome: projection => {
      const eventRef = `${projection.runRef}@${input.patch.seq}`
      const alreadyApplied = Arr.contains(
        input.stream.appliedEventRefs,
        eventRef,
      )

      if (alreadyApplied) {
        return unchanged
      }

      const appliedEventRefs = [
        ...input.stream.appliedEventRefs,
        eventRef,
      ].slice(-MAX_APPLIED_EVENT_REFS)
      const streamWithRef = GymRunProgressStreamModelSchema({
        ...advancedStream,
        appliedEventRefs,
      })

      const runs =
        input.counter._tag === 'PublicGymRunProgressLoaded'
          ? upsertRun(input.counter.runs, projection)
          : [projection]

      return {
        counter: LoadedPublicGymRunProgress({ runs }),
        stream: streamWithRef,
      }
    },
  })
}

// A cursor gap means we may have missed puts; advance the cursor to the received
// seq so the next reconnect replays from there. The slow reconcile poll + the
// next ingested push are self-healing: each run's latest projection is republished
// in full, so a skipped intermediate put never leaves a stale card.
export const gymRunProgressStreamAfterCursorGap = (
  model: GymRunProgressStreamModel,
  gap: CursorGap,
): GymRunProgressStreamModel =>
  GymRunProgressStreamModelSchema({
    ...model,
    cursor: Math.max(model.cursor, gap.receivedSeq),
  })
