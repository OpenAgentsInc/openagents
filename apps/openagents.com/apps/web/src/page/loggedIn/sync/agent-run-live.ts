import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  agentRunScope as khalaAgentRunScope,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  decodeLiveFrame,
  type ChangelogEntry,
  type LiveFrame,
} from '@openagentsinc/khala-sync'
import {
  CollectionName,
  EntityId,
  IsoTimestamp,
  SyncPatch,
  SyncScope,
  SyncSequence,
} from '@openagentsinc/sync-schema'

import { parseJsonRecord } from '../../../json-boundary'
import { FailedSyncStream, ReceivedSyncPatch, type Message } from '../message'

/**
 * KS-6.6 client repoint (#8416): the live agent-run status + transcript
 * stream now connects to the khala-sync engine
 * (`WS /api/sync/connect?scope=scope.agent_run.<runId>`) instead of the
 * legacy `agent-run:<runId>` DO-room socket
 * (`/api/sync/agent-run/<runId>/stream`). This module is the pure adapter,
 * mirroring the KS-6.4 settled-feed pattern
 * (`page/loggedOut/settled-feed.ts`'s `settledFeedPatchFromChangelogEntry`):
 * translate one khala-sync `ChangelogEntry` into the exact legacy `SyncPatch`
 * shape `./transitions.ts`'s `updateSync` reducer and `./projection.ts`'s
 * `agentRunFromSyncRecord` / `agentRunEventFromSyncRecord` already
 * understand, so NEITHER file changes for this engine cutover.
 *
 * `subscriptions.ts` keeps emitting patches under the SAME legacy scope key
 * (`agent-run:<runId>`, from `syncAgentRunScope` in `../model`) it always
 * used for this scope, even though the wire transport now speaks the new
 * engine's dotted scope taxonomy — the local model doesn't need those two to
 * match, and reusing the legacy key is what keeps every existing
 * `patch.scope`-matching consumer (`activeRunMatchesScope`, `syncScopeId`,
 * `isSidebarMissionScope`, …) working unchanged.
 *
 * Why `agent_run` uses `op: 'patch'`, not `put`: the new engine's
 * `AgentRunEntity` (packages/khala-sync/src/agent-run.ts) deliberately never
 * carries `runnerId` / `eventCursor` / `externalRunId` (they were never part
 * of any public-safe projection surface, unlike every other run field). The
 * legacy `agentRunFromSyncRecord` treats `runnerId` as REQUIRED — a `put`
 * (full replace) with it missing would make every run-status patch silently
 * fail to parse, and even where a field IS present, a blind full replace
 * would blow away already-known `eventCursor`/`externalRunId` values. A
 * `patch` (`{...previousRecord, ...patchRecord}` merge — see
 * `./projection.ts`'s `syncWithPatch`) applies only the fields this entity
 * actually carries, so those already-known fields survive untouched. They
 * are seeded by the still-legacy `LoadSyncSnapshot` REST fetch
 * (`/api/sync/agent-run/<runId>/snapshot`), which this cutover does NOT
 * touch: that endpoint replays `sync_changes` rows written by the always-on
 * `appendAgentRunSyncChanges` (`omni-runs.ts`), a completely different
 * mechanism from the `notifySyncScopes(env, syncScopeForAgentRun(...))`
 * broadcast calls this issue's final step deletes. Deleting those calls
 * only stops a live-room broadcast to a scope this client no longer
 * subscribes to; it does not affect what the snapshot endpoint reads.
 *
 * `agent_run_event` needs no such split: `AgentRunEventEntity`'s fields are
 * already named identically to what `agentRunEventFromSyncRecord` reads
 * (`runId`, `sequence`, `type`, `summary`, `status`, `source`,
 * `payloadJson`, `artifactRefs`, `externalEventId`, `createdAt`), so a
 * straight `put` (full replace, keyed by the event's own id) is correct and
 * simpler — each event is a complete, immutable post-image once written.
 */

export const AGENT_RUN_LIVE_RUNS_COLLECTION = 'agent_runs'
export const AGENT_RUN_LIVE_EVENTS_COLLECTION = 'agent_run_events'

/** The wire scope for `WS /api/sync/connect?scope=…` (new engine taxonomy). */
export const agentRunLiveWireScope = (runId: string): string =>
  khalaAgentRunScope(runId)

const agentRunPatchFromRunEntityRecord = (
  record: Record<string, unknown>,
): Readonly<{ id: string; patch: Record<string, unknown> }> | undefined => {
  let entity: ReturnType<typeof decodeAgentRunEntity>
  try {
    entity = decodeAgentRunEntity(record)
  } catch {
    return undefined
  }

  return {
    id: entity.runId,
    patch: {
      id: entity.runId,
      backend: entity.backend,
      createdAt: entity.createdAt,
      goal: entity.goal,
      projectId: entity.projectId,
      repository: {
        owner: entity.repository.owner,
        provider: entity.repository.provider,
        ref: entity.repository.ref,
        repo: entity.repository.repo,
      },
      runtime: entity.runtime,
      status: entity.status,
      teamId: entity.teamId,
      updatedAt: entity.updatedAt,
      userId: entity.userId,
    },
  }
}

const agentRunEventValueFromRecord = (
  record: Record<string, unknown>,
): Readonly<{ id: string; value: Record<string, unknown> }> | undefined => {
  let entity: ReturnType<typeof decodeAgentRunEventEntity>
  try {
    entity = decodeAgentRunEventEntity(record)
  } catch {
    return undefined
  }

  return { id: entity.id, value: record }
}

/**
 * Adapt one khala-sync `ChangelogEntry` for `scope.agent_run.<runId>` (either
 * entity type it carries) into the legacy `SyncPatch` shape, addressed at
 * `legacyScope` (the client's local `agent-run:<runId>` key — see the module
 * doc). Returns `undefined` for an unrecognized entity type, an unparseable
 * upsert post-image, or a post-image that fails its entity contract decode
 * (defensive; the server never emits either in practice).
 */
export const agentRunLivePatchFromChangelogEntry = (
  entry: ChangelogEntry,
  legacyScope: string,
): SyncPatch | undefined => {
  const collection =
    entry.entityType === AGENT_RUN_ENTITY_TYPE
      ? AGENT_RUN_LIVE_RUNS_COLLECTION
      : entry.entityType === AGENT_RUN_EVENT_ENTITY_TYPE
        ? AGENT_RUN_LIVE_EVENTS_COLLECTION
        : undefined

  if (collection === undefined) {
    return undefined
  }

  if (entry.op === 'delete') {
    return new SyncPatch({
      collection: CollectionName.make(collection),
      id: EntityId.make(entry.entityId),
      op: 'delete',
      scope: SyncScope.make(legacyScope),
      seq: SyncSequence.make(entry.version),
      serverTime: IsoTimestamp.make(entry.committedAt),
    })
  }

  if (entry.postImageJson === undefined) {
    return undefined
  }

  const record = parseJsonRecord(entry.postImageJson)
  if (record === undefined) {
    return undefined
  }

  if (collection === AGENT_RUN_LIVE_RUNS_COLLECTION) {
    const adapted = agentRunPatchFromRunEntityRecord(record)
    if (adapted === undefined) {
      return undefined
    }

    return new SyncPatch({
      collection: CollectionName.make(collection),
      id: EntityId.make(adapted.id),
      op: 'patch',
      patch: adapted.patch,
      scope: SyncScope.make(legacyScope),
      seq: SyncSequence.make(entry.version),
      serverTime: IsoTimestamp.make(entry.committedAt),
    })
  }

  const adapted = agentRunEventValueFromRecord(record)
  if (adapted === undefined) {
    return undefined
  }

  return new SyncPatch({
    collection: CollectionName.make(collection),
    id: EntityId.make(adapted.id),
    op: 'put',
    scope: SyncScope.make(legacyScope),
    seq: SyncSequence.make(entry.version),
    serverTime: IsoTimestamp.make(entry.committedAt),
    value: adapted.value,
  })
}

/**
 * Decode one khala-sync `LiveFrame` WebSocket payload into zero or more
 * `page/loggedIn/message.ts` messages, addressed at `legacyScope`. Mirrors
 * `settledFeedMessagesFromLiveFramePayload` (KS-6.4): a `DeltaFrame` may
 * batch several changed entities into one frame, so this can fan out into
 * several `ReceivedSyncPatch` messages from a single socket message.
 * `MustRefetchFrame` degrades to `FailedSyncStream` (no first-class
 * "clear scope-local state and re-bootstrap" reconnect loop exists here
 * either — same honest degrade the settled feed uses); the still-active
 * `FetchAutopilotRun` 2s reconcile poll (while the run is busy) and a fresh
 * `LoadSyncSnapshot` on the next launch/route-entry both self-heal from it.
 */
export const agentRunLiveMessagesFromLiveFramePayload = (
  payload: string,
  legacyScope: string,
): ReadonlyArray<Message> => {
  let frame: LiveFrame
  try {
    const record = parseJsonRecord(payload)
    if (record === undefined) {
      throw new Error('not a JSON object')
    }
    frame = decodeLiveFrame(record)
  } catch {
    return [
      FailedSyncStream({
        error: 'Agent run live-tail message could not be decoded.',
        scope: legacyScope,
      }),
    ]
  }

  if (frame._tag === 'PingFrame' || frame._tag === 'MutationAckFrame') {
    return []
  }

  if (frame._tag === 'MustRefetchFrame') {
    return [
      FailedSyncStream({
        error: `Agent run live-tail requested a refetch (${frame.reason}).`,
        scope: legacyScope,
      }),
    ]
  }

  // DeltaFrame: one message per changed entity, in server order.
  return frame.entries.flatMap(entry => {
    const patch = agentRunLivePatchFromChangelogEntry(entry, legacyScope)

    return patch === undefined ? [] : [ReceivedSyncPatch({ patch })]
  })
}
