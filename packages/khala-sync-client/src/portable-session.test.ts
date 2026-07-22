import { describe, expect, test } from "vite-plus/test"
import {
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationId,
  SyncVersion,
  SyncVersionWatermark,
  personalScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  PORTABLE_ATTACHMENT_ENTITY_TYPE,
  PORTABLE_COMMAND_ENTITY_TYPE,
  PORTABLE_SESSION_ENTITY_TYPE,
  PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE,
  type PortableSessionCommand,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import {
  createKhalaSyncPortableSessions,
  PORTABLE_REQUEST_COMMAND_MUTATOR_NAME,
} from "./portable-session.js"
import type { ClientMutator } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const scope = personalScope("owner.1")
const at = "2026-07-17T12:00:00.000Z"
const sessionValue = {
  schema: "openagents.portable_session.v1" as const,
  sessionRef: "session.portable.1",
  ownerRef: "owner.1",
  identityBasis: "owner_minted" as const,
  workContextRef: "work-context.1",
  eventLogRef: "event-log.1",
  currentProjectionRef: "projection.1",
  commandScopeRef: "command-scope.1",
  graph: {
    rootAgentRef: "agent.root",
    nodes: [{
      agentRef: "agent.root",
      threadRef: "thread.root",
      transcriptRef: "transcript.root",
      activityCursor: 3,
      lifecycle: "running" as const,
      attachmentGeneration: 1,
    }],
  },
  adoptedFromLocalHistory: false,
}
const targets = {
  sessionRef: sessionValue.sessionRef,
  targets: [
    { targetRef: "target.local", targetClass: "owner_local" as const, adapterRef: "adapter.pylon", ownerRef: "owner.1", compatibilityRef: "catalog.1", isolation: "owner_host_process" as const, dataPosture: "owner_device_only" as const, health: "ready" as const },
    { targetRef: "target.managed", targetClass: "openagents_managed" as const, adapterRef: "adapter.agent-computer", ownerRef: "owner.1", compatibilityRef: "catalog.1", isolation: "dedicated_microvm" as const, dataPosture: "openagents_managed_region" as const, health: "ready" as const },
  ],
}
const attachment = {
  attachmentRef: "attachment.portable.1",
  sessionRef: sessionValue.sessionRef,
  targetRef: "target.local",
  generation: 1,
  state: "active" as const,
  descendantAgentRefs: ["agent.root"],
  capabilityLeaseRefs: ["lease.provider.1"],
  evidenceRefs: ["receipt.attach.1"],
}
const command: PortableSessionCommand = {
  schema: "openagents.portable_session_command.v1",
  commandRef: "command.move.1",
  idempotencyKey: "idempotency.move.1",
  ownerRef: "owner.1",
  sessionRef: sessionValue.sessionRef,
  kind: "move",
  expectedAttachmentRef: attachment.attachmentRef,
  expectedGeneration: 1,
  destinationTargetRef: "target.managed",
  checkpointRef: "checkpoint.move.1",
  expiresAt: "2026-07-17T13:00:00.000Z",
}

const entry = (version: number, entityType: string, entityId: string, value: unknown) => new ChangelogEntry({
  scope,
  version: SyncVersion.make(version),
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImageJson: JSON.stringify(value),
  mutationRef: `mutation.portable.${version}`,
  committedAt: at,
})

const session = (
  state: ScopeSyncState,
  onMutate?: (mutator: ClientMutator<PortableSessionCommand>, value: PortableSessionCommand) => void,
  pending: KhalaSyncSession["pending"] = () => [],
): KhalaSyncSession => ({
  state: (_scope: SyncScope) => state,
  pending,
  mutate: (mutator: ClientMutator<PortableSessionCommand>, value: PortableSessionCommand) => {
    onMutate?.(mutator, value)
    return Effect.succeed(MutationId.make(7))
  },
}) as unknown as KhalaSyncSession

describe("confirmed portable-session client", () => {
  test("reads only coherent confirmed authority and loss-accounts invalid rows", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, PORTABLE_SESSION_ENTITY_TYPE, sessionValue.sessionRef, sessionValue),
        entry(2, PORTABLE_TARGET_DIRECTORY_ENTITY_TYPE, sessionValue.sessionRef, targets),
        entry(3, PORTABLE_ATTACHMENT_ENTITY_TYPE, attachment.attachmentRef, attachment),
        entry(4, PORTABLE_COMMAND_ENTITY_TYPE, command.commandRef, { command, status: "accepted" }),
        entry(5, PORTABLE_ATTACHMENT_ENTITY_TYPE, "attachment.malformed", { token: "not-authority" }),
        entry(6, PORTABLE_SESSION_ENTITY_TYPE, "session.foreign", { ...sessionValue, sessionRef: "session.foreign", ownerRef: "other.owner" }),
        entry(7, PORTABLE_ATTACHMENT_ENTITY_TYPE, "attachment.orphan", { ...attachment, attachmentRef: "attachment.orphan", sessionRef: "session.missing" }),
        entry(8, PORTABLE_COMMAND_ENTITY_TYPE, "command.foreign", {
          command: { ...command, commandRef: "command.foreign", ownerRef: "other.owner" },
          status: "accepted",
        }),
      ], SyncVersion.make(8)))
      const snapshot = Effect.runSync(createKhalaSyncPortableSessions({
        ownerRef: "owner.1",
        ownerScope: scope,
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(8) }),
      }).snapshot())
      expect(snapshot.status).toEqual({ phase: "live", cursor: 8, pendingCommandCount: 0 })
      expect(snapshot.sessions.map(value => value.sessionRef)).toEqual([sessionValue.sessionRef])
      expect(snapshot.attachments.map(value => value.attachmentRef)).toEqual([attachment.attachmentRef])
      expect(snapshot.commands).toEqual([{ command, status: "accepted" }])
      expect(snapshot.issues.map(issue => issue.code).sort()).toEqual([
        "malformed",
        "orphaned",
        "owner_scope_mismatch",
        "owner_scope_mismatch",
      ])
      expect(JSON.stringify(snapshot)).not.toContain("not-authority")
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("withholds cached rows until the owner scope is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, PORTABLE_SESSION_ENTITY_TYPE, sessionValue.sessionRef, sessionValue),
      ], SyncVersion.make(1)))
      const snapshot = Effect.runSync(createKhalaSyncPortableSessions({
        ownerRef: "owner.1",
        ownerScope: scope,
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
      }).snapshot())
      expect(snapshot).toMatchObject({
        status: { phase: "must_refetch", cursor: null },
        sessions: [], targetDirectories: [], attachments: [], commands: [], issues: [],
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("queues exact command bytes without optimistic authority", () => {
    let captured: PortableSessionCommand | null = null
    const clientSession = session(
      { phase: "live", cursor: SyncVersionWatermark.make(0) },
      (mutator, value) => {
        expect(String(mutator.name)).toBe(PORTABLE_REQUEST_COMMAND_MUTATOR_NAME)
        expect(mutator.apply(value, {} as never)).toEqual([])
        captured = value
      },
    )
    const store = openKhalaSyncStore(":memory:")
    try {
      const client = createKhalaSyncPortableSessions({
        ownerRef: "owner.1", ownerScope: scope, store, session: clientSession,
      })
      expect(Number(Effect.runSync(client.request(command)))).toBe(7)
      expect(captured).toEqual(command)
    } finally {
      Effect.runSync(store.close())
    }
  })
})
