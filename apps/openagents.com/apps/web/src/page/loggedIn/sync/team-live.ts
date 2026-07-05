import {
  KHALA_CODE_TEAM_CHAT_MESSAGE_ENTITY_TYPE,
  KHALA_CODE_THREAD_FILE_ENTITY_TYPE,
  decodeKhalaCodeTeamChatMessageEntity,
  decodeKhalaCodeThreadFileEntity,
  decodeLiveFrame,
  teamScope as khalaTeamScope,
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
 * KS-6.11a client repoint (#8423): team chat and thread files both now ALSO
 * connect to the khala-sync engine (`WS /api/sync/connect?scope=
 * scope.team.<teamId>`), mirroring the KS-6.6 agent-run pattern
 * (`./agent-run-live.ts`) and the KS-6.4 settled-feed pattern
 * (`page/loggedOut/settled-feed.ts`): a pure adapter translates each
 * khala-sync `ChangelogEntry` into the exact legacy `SyncPatch` shape
 * `../chatState.ts`'s `applyTeamSyncCollections` (via
 * `teamChatMessagesFromSyncCollections` / `threadFilesFromSyncCollections`)
 * already understands, so that reducer needs ZERO changes for this cutover.
 *
 * UNLIKE agent-run/settled-feed, this cutover does NOT remove the legacy
 * `team:<teamId>` scope from `subscriptions.ts`'s `syncScopesForModel` list.
 * That legacy multiplexed socket carries a THIRD collection this repoint
 * does not touch: `missions` (team-owned agent-run sidebar cards, appended
 * by `omni-runs.ts`'s `agentRunSyncChanges` — a completely different
 * producer from `publishTeamChatMessageSync`/`publishTeamThreadFileSync`).
 * `missions` has no khala-sync entity/producer at all (out of scope for
 * KS-6.11a; it is an agent-run sidebar concern, not a team-chat/thread-file
 * one). Dropping the legacy `team:<teamId>` subscription entirely would
 * silently stop live-updating team-owned mission cards in the sidebar — a
 * real regression this repoint must not cause. So BOTH sockets stay open
 * for a team route: the legacy one keeps delivering `missions` (and, until
 * the legacy producer calls below are deleted, redundant-but-idempotent
 * `team_chat_messages`/`thread_files` puts that this adapter's puts race
 * harmlessly against), and this dedicated one delivers
 * `team_chat_messages`/`thread_files` from the new engine. Once
 * `publishTeamChatMessageSync`/`publishTeamThreadFileSync` (and their
 * `notifySyncScopes` calls, 3 sites in `index.ts` plus
 * `thread-file-routes.ts`) are deleted, the legacy socket simply stops
 * producing those two collections — `missions` delivery is untouched
 * because it is written by an unrelated producer.
 *
 * `subscriptions.ts` still keys these patches under the SAME legacy scope
 * string (`team:<teamId>`, from `syncTeamScope` in `../model`) so every
 * existing `patch.scope`-matching consumer (`applyTeamSyncCollections`,
 * `isSidebarMissionScope`, `sidebarWithMissionPatch`) keeps working
 * unchanged — the wire transport is new, the local scope key is not.
 *
 * Both entity kinds ride the exact same `scope.team.<teamId>` wire scope
 * (`khala-code-product-state-projection.ts`'s `scopesForRow`), so they are
 * handled by one adapter/one socket, not two.
 */

export const TEAM_LIVE_CHAT_MESSAGES_COLLECTION = 'team_chat_messages'
export const TEAM_LIVE_THREAD_FILES_COLLECTION = 'thread_files'

/** The wire scope for `WS /api/sync/connect?scope=…` (new engine taxonomy). */
export const teamLiveWireScope = (teamId: string): string =>
  khalaTeamScope(teamId)

/**
 * Historical/backfilled `team_chat_message` rows may predate KS-6.11
 * (#8422)'s author-hydration JOIN and carry a `null` `authorName` — never
 * blank the author entirely (guardrail: "blank authors ... would be a real,
 * visible regression"). This mirrors how the run timeline already falls
 * back for an unknown/removed actor.
 */
const FALLBACK_AUTHOR_NAME = 'A teammate'

const teamChatMessagePatchFromRecord = (
  record: Record<string, unknown>,
): Readonly<{ id: string; value: Record<string, unknown> }> | undefined => {
  let entity: ReturnType<typeof decodeKhalaCodeTeamChatMessageEntity>
  try {
    entity = decodeKhalaCodeTeamChatMessageEntity(record)
  } catch {
    return undefined
  }

  return {
    id: entity.messageId,
    value: {
      id: entity.messageId,
      teamId: entity.teamId,
      projectId: entity.projectId,
      kind: entity.kind,
      body: entity.body,
      autopilotThreadId: entity.autopilotThreadId,
      agentRunId: entity.agentRunId,
      createdAt: entity.createdAt,
      // `runSummary`/`launchError` are NOT reconstructible here: the entity
      // deliberately excludes `metadata_json` (khala-code.ts's module doc),
      // so a message updated only through
      // `updateTeamChatMessageRunSummary` never carries them over this
      // path. `run-timeline/projection.ts`'s `teamAutopilotRunCardParts`
      // already degrades gracefully for both being `undefined` (falls back
      // to "Waiting for run details." and the live `chatRun`/`agentRunId`
      // link), so this is a bounded, non-crashing, non-blank-message
      // cosmetic gap — flagged in the RUNBOOK, not a blocker for this
      // repoint.
      author: {
        avatarUrl: entity.authorAvatarUrl,
        githubUsername: entity.authorGithubUsername,
        name: entity.authorName ?? FALLBACK_AUTHOR_NAME,
        userId: entity.authorUserId,
      },
    },
  }
}

/**
 * `thread_file`'s `downloadUrl`/`detailUrl` are structurally absent from the
 * entity by design (`khala-code.ts`: "clients fetch bytes through the
 * authorized download route, never from a synced storage pointer") — both
 * are pure functions of already-present fields, matching
 * `thread-files.ts`'s `publicThreadFile` exactly. `teamRouteRefValue` is the
 * CURRENT team's route ref (slug-or-id), resolved the same way every other
 * team-scoped consumer resolves it (`teamRouteRef` in `../model`); every
 * `thread_file` arriving on this team's `scope.team.<teamId>` socket
 * necessarily belongs to that same team (`scopesForRow` only fans a
 * `thread_file` row to `teamScope(teamId)` when it has one), so a single
 * resolved ref is correct for every entry in the frame.
 */
const threadFileRecordFromEntity = (
  record: Record<string, unknown>,
  teamRouteRefValue: string,
): Readonly<{ id: string; value: Record<string, unknown> }> | undefined => {
  let entity: ReturnType<typeof decodeKhalaCodeThreadFileEntity>
  try {
    entity = decodeKhalaCodeThreadFileEntity(record)
  } catch {
    return undefined
  }

  const detailUrl =
    entity.fileScope === 'team' && entity.teamId !== null
      ? `/teams/${encodeURIComponent(teamRouteRefValue)}/files/${encodeURIComponent(entity.fileId)}`
      : `/files/${encodeURIComponent(entity.fileId)}`

  return {
    id: entity.fileId,
    value: {
      contentType: entity.contentType,
      createdAt: entity.createdAt,
      detailUrl,
      downloadEnabled: entity.downloadEnabled,
      downloadUrl: `/api/thread-files/${encodeURIComponent(entity.fileId)}/download`,
      filename: entity.filename,
      id: entity.fileId,
      ownerUserId: entity.ownerUserId,
      scope: entity.fileScope,
      sizeBytes: entity.sizeBytes,
      teamId: entity.teamId,
      threadId: entity.threadId,
    },
  }
}

/**
 * Adapt one khala-sync `ChangelogEntry` for `scope.team.<teamId>` (either
 * entity type it carries) into the legacy `SyncPatch` shape, addressed at
 * `legacyScope` (the client's local `team:<teamId>` key — see the module
 * doc). Returns `undefined` for an unrecognized entity type, an unparseable
 * upsert post-image, a post-image that fails its entity contract decode
 * (defensive; the server never emits either in practice), or a message/file
 * that has been soft-deleted or archived (`deletedAt`/`archivedAt` set) —
 * every current writer leaves both `null` (there is no delete/archive path
 * today), but a `SyncPatch` for a row like that would resurrect state the
 * legacy REST reads (`WHERE deleted_at IS NULL`) never show, so this
 * degrades to "message/file disappears" (an implicit `delete`) rather than
 * "stale ghost row survives" if a future writer ever sets them.
 */
export const teamLivePatchFromChangelogEntry = (
  entry: ChangelogEntry,
  legacyScope: string,
  teamRouteRefValue: string,
): SyncPatch | undefined => {
  const collection =
    entry.entityType === KHALA_CODE_TEAM_CHAT_MESSAGE_ENTITY_TYPE
      ? TEAM_LIVE_CHAT_MESSAGES_COLLECTION
      : entry.entityType === KHALA_CODE_THREAD_FILE_ENTITY_TYPE
        ? TEAM_LIVE_THREAD_FILES_COLLECTION
        : undefined

  if (collection === undefined) {
    return undefined
  }

  const deletePatch = (id: string): SyncPatch =>
    new SyncPatch({
      collection: CollectionName.make(collection),
      id: EntityId.make(id),
      op: 'delete',
      scope: SyncScope.make(legacyScope),
      seq: SyncSequence.make(entry.version),
      serverTime: IsoTimestamp.make(entry.committedAt),
    })

  if (entry.op === 'delete') {
    return deletePatch(entry.entityId)
  }

  if (entry.postImageJson === undefined) {
    return undefined
  }

  const record = parseJsonRecord(entry.postImageJson)
  if (record === undefined) {
    return undefined
  }

  if (collection === TEAM_LIVE_CHAT_MESSAGES_COLLECTION) {
    let entity: ReturnType<typeof decodeKhalaCodeTeamChatMessageEntity>
    try {
      entity = decodeKhalaCodeTeamChatMessageEntity(record)
    } catch {
      return undefined
    }

    if (entity.deletedAt !== null || entity.archivedAt !== null) {
      return deletePatch(entity.messageId)
    }

    const adapted = teamChatMessagePatchFromRecord(record)

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

  let fileEntity: ReturnType<typeof decodeKhalaCodeThreadFileEntity>
  try {
    fileEntity = decodeKhalaCodeThreadFileEntity(record)
  } catch {
    return undefined
  }

  if (fileEntity.deletedAt !== null) {
    return deletePatch(fileEntity.fileId)
  }

  const adapted = threadFileRecordFromEntity(record, teamRouteRefValue)

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
 * `agentRunLiveMessagesFromLiveFramePayload` (KS-6.6) and
 * `settledFeedMessagesFromLiveFramePayload` (KS-6.4): a `DeltaFrame` may
 * batch several changed entities (chat messages AND thread files) into one
 * frame, so this can fan out into several `ReceivedSyncPatch` messages from
 * a single socket message. `MustRefetchFrame` degrades to `FailedSyncStream`
 * (no first-class "clear scope-local state and re-bootstrap" reconnect loop
 * exists here either); a fresh `LoadSyncSnapshot` on the next route entry
 * self-heals from it, same as the agent-run/settled-feed adapters.
 */
export const teamLiveMessagesFromLiveFramePayload = (
  payload: string,
  legacyScope: string,
  teamRouteRefValue: string,
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
        error: 'Team live-tail message could not be decoded.',
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
        error: `Team live-tail requested a refetch (${frame.reason}).`,
        scope: legacyScope,
      }),
    ]
  }

  // DeltaFrame: one message per changed entity, in server order.
  return frame.entries.flatMap(entry => {
    const patch = teamLivePatchFromChangelogEntry(
      entry,
      legacyScope,
      teamRouteRefValue,
    )

    return patch === undefined ? [] : [ReceivedSyncPatch({ patch })]
  })
}
