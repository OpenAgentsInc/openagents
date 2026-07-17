import { MutatorName, type MutationId, type SyncScope } from "@openagentsinc/khala-sync"
import {
  PORTABLE_ATTACHMENT_ENTITY_TYPE,
  PORTABLE_COMMAND_ENTITY_TYPE,
  PORTABLE_SESSION_ENTITY_TYPE,
  PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE,
  PortableAttachmentSchema,
  PortableCodingSessionSchema,
  PortableCommandProjectionSchema,
  PortableTargetDirectoryProjectionSchema,
  type PortableAttachment,
  type PortableCodingSession,
  type PortableCommandProjection,
  type PortableSessionCommand,
  type PortableTargetDirectoryProjection,
} from "@openagentsinc/portable-session-contract"
import { Effect, Schema } from "effect"

import type { ClientMutator, OverlayError } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type { KhalaSyncClientStoreError, KhalaSyncLocalStore } from "./store.js"

export const PORTABLE_REQUEST_COMMAND_MUTATOR_NAME = "portable.requestCommand" as const
export const MAX_CONFIRMED_PORTABLE_SESSIONS = 512
export const MAX_CONFIRMED_PORTABLE_ATTACHMENTS = 2_048
export const MAX_CONFIRMED_PORTABLE_COMMANDS = 2_048

export const createPortableRequestCommandMutator = (): ClientMutator<PortableSessionCommand> => ({
  // Command acceptance and outcomes are server authority. A queued command
  // deliberately creates no optimistic entity.
  apply: () => [],
  name: MutatorName.make(PORTABLE_REQUEST_COMMAND_MUTATOR_NAME),
})

export type PortableProjectionIssue = Readonly<{
  code: "malformed" | "entity_ref_mismatch" | "owner_scope_mismatch" | "orphaned"
  affectedRef: string
}>

export type ConfirmedPortableSessionSnapshot = Readonly<{
  status: Readonly<{
    phase: ScopeSyncState["phase"]
    cursor: number | null
    pendingCommandCount: number
  }>
  sessions: ReadonlyArray<PortableCodingSession>
  targetDirectories: ReadonlyArray<PortableTargetDirectoryProjection>
  attachments: ReadonlyArray<PortableAttachment>
  commands: ReadonlyArray<PortableCommandProjection>
  issues: ReadonlyArray<PortableProjectionIssue>
}>

export type KhalaSyncPortableSessions = Readonly<{
  snapshot: () => Effect.Effect<ConfirmedPortableSessionSnapshot, KhalaSyncClientStoreError>
  request: (command: PortableSessionCommand) => Effect.Effect<MutationId, OverlayError>
}>

const empty = (
  state: ScopeSyncState,
  pendingCommandCount: number,
): ConfirmedPortableSessionSnapshot => ({
  status: { phase: state.phase, cursor: null, pendingCommandCount },
  sessions: [],
  targetDirectories: [],
  attachments: [],
  commands: [],
  issues: [],
})

const decodeSession = Schema.decodeUnknownSync(PortableCodingSessionSchema)
const decodeTargetDirectory = Schema.decodeUnknownSync(PortableTargetDirectoryProjectionSchema)
const decodeAttachment = Schema.decodeUnknownSync(PortableAttachmentSchema)
const decodeCommandProjection = Schema.decodeUnknownSync(PortableCommandProjectionSchema)

const boundedLatest = <A>(
  values: ReadonlyArray<Readonly<{ version: number; value: A }>>,
  limit: number,
): ReadonlyArray<A> => [...values]
  .sort((left, right) => right.version - left.version)
  .slice(0, limit)
  .map(item => item.value)

/** Confirmed-only reader and command writer for one authenticated owner scope. */
export const createKhalaSyncPortableSessions = (input: Readonly<{
  ownerRef: string
  ownerScope: SyncScope
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
  /** Reuse the exact mutator instance registered in the session overlay. */
  mutator?: ClientMutator<PortableSessionCommand>
}>): KhalaSyncPortableSessions => {
  const mutator = input.mutator ?? createPortableRequestCommandMutator()
  const pendingCommandCount = (): number => input.session.pending().filter(
    mutation => String(mutation.name) === PORTABLE_REQUEST_COMMAND_MUTATOR_NAME,
  ).length
  return {
    snapshot: () => {
      const state = input.session.state(input.ownerScope)
      if (state.phase !== "live") return Effect.succeed(empty(state, pendingCommandCount()))
      return Effect.map(input.store.readEntities(input.ownerScope), rows => {
        const sessions: Array<{ value: PortableCodingSession; version: number }> = []
        const targetDirectories: Array<{ value: PortableTargetDirectoryProjection; version: number }> = []
        const attachments: Array<{ value: PortableAttachment; version: number }> = []
        const commands: Array<{ value: PortableCommandProjection; version: number }> = []
        const issues: PortableProjectionIssue[] = []
        for (const row of rows) {
          if (row.entityType !== PORTABLE_SESSION_ENTITY_TYPE &&
              row.entityType !== PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE &&
              row.entityType !== PORTABLE_ATTACHMENT_ENTITY_TYPE &&
              row.entityType !== PORTABLE_COMMAND_ENTITY_TYPE) continue
          try {
            const version = Number(row.version)
            if (row.entityType === PORTABLE_SESSION_ENTITY_TYPE) {
              const value = decodeSession(JSON.parse(row.postImageJson))
              if (value.sessionRef !== row.entityId) issues.push({ code: "entity_ref_mismatch", affectedRef: row.entityId })
              else if (value.ownerRef !== input.ownerRef) issues.push({ code: "owner_scope_mismatch", affectedRef: row.entityId })
              else sessions.push({ value, version })
            } else if (row.entityType === PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE) {
              const value = decodeTargetDirectory(JSON.parse(row.postImageJson))
              if (value.sessionRef !== row.entityId) issues.push({ code: "entity_ref_mismatch", affectedRef: row.entityId })
              else targetDirectories.push({ value, version })
            } else if (row.entityType === PORTABLE_ATTACHMENT_ENTITY_TYPE) {
              const value = decodeAttachment(JSON.parse(row.postImageJson))
              if (value.attachmentRef !== row.entityId) issues.push({ code: "entity_ref_mismatch", affectedRef: row.entityId })
              else attachments.push({ value, version })
            } else {
              const value = decodeCommandProjection(JSON.parse(row.postImageJson))
              if (value.command.commandRef !== row.entityId) issues.push({ code: "entity_ref_mismatch", affectedRef: row.entityId })
              else commands.push({ value, version })
            }
          } catch {
            issues.push({ code: "malformed", affectedRef: row.entityId })
          }
        }
        const confirmedSessions = boundedLatest(sessions, MAX_CONFIRMED_PORTABLE_SESSIONS)
        const sessionRefs = new Set(confirmedSessions.map(value => value.sessionRef))
        const confirmedTargets = boundedLatest(targetDirectories, MAX_CONFIRMED_PORTABLE_SESSIONS)
          .filter(value => sessionRefs.has(value.sessionRef) || (issues.push({ code: "orphaned", affectedRef: value.sessionRef }), false))
        const confirmedAttachments = boundedLatest(attachments, MAX_CONFIRMED_PORTABLE_ATTACHMENTS)
          .filter(value => sessionRefs.has(value.sessionRef) || (issues.push({ code: "orphaned", affectedRef: value.attachmentRef }), false))
        const confirmedCommands = boundedLatest(commands, MAX_CONFIRMED_PORTABLE_COMMANDS)
          .filter(value => sessionRefs.has(value.command.sessionRef) || (issues.push({ code: "orphaned", affectedRef: value.command.commandRef }), false))
        return {
          status: {
            phase: state.phase,
            cursor: Number(state.cursor),
            pendingCommandCount: pendingCommandCount(),
          },
          sessions: confirmedSessions,
          targetDirectories: confirmedTargets,
          attachments: confirmedAttachments,
          commands: confirmedCommands,
          issues,
        }
      })
    },
    request: command => input.session.mutate(mutator, command),
  }
}
