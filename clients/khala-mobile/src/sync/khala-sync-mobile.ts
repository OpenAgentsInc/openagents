import { createCollection } from "@tanstack/db"
import {
  decodeChatThreadEntity,
  decodeFleetRunEntity,
  fleetRunScope,
  personalScope,
  type ChatThreadEntity,
  type FleetRunEntity
} from "@openagentsinc/khala-sync"
import type {
  KhalaSyncOverlay,
  KhalaSyncSession
} from "@openagentsinc/khala-sync-client"
import {
  chatThreadKhalaSyncCollectionOptions,
  chatThreadsForSidebar,
  createKhalaSyncMutationTracker,
  fleetApprovalKhalaSyncCollectionOptions,
  fleetRunKhalaSyncCollectionOptions,
  fleetSteerKhalaSyncCollectionOptions,
  fleetWorkerKhalaSyncCollectionOptions
} from "@openagentsinc/khala-sync-db-collection"

export type KhalaMobileCollectionsInput = Readonly<{
  ownerUserId: string
  fleetRunId: string
  session: KhalaSyncSession
  overlay: KhalaSyncOverlay
}>

export const createKhalaMobileCollections = (
  input: KhalaMobileCollectionsInput,
) => {
  const mutationTracker = createKhalaSyncMutationTracker()
  const chatThreads = createCollection(
    chatThreadKhalaSyncCollectionOptions({
      awaitServerSync: false,
      id: "khala-mobile-chat-threads",
      mutationTracker,
      onError: () => undefined,
      overlay: input.overlay,
      ownerUserId: input.ownerUserId,
      scope: personalScope(input.ownerUserId),
      session: input.session,
      startSync: true
    })
  )
  const fleetScope = fleetRunScope(input.fleetRunId)
  const fleetRun = createCollection(
    fleetRunKhalaSyncCollectionOptions({
      id: "khala-mobile-fleet-run",
      mutationTracker,
      onError: () => undefined,
      overlay: input.overlay,
      scope: fleetScope,
      session: input.session,
      startSync: true
    })
  )
  // MH-6 (#8585): the per-harness worker cards, pending approvals, and steer
  // receipts the fleet peek observes. All read-only — the phone dispatches the
  // three typed steering intents via session.mutate, never a local write here.
  const fleetWorkers = createCollection(
    fleetWorkerKhalaSyncCollectionOptions({
      id: "khala-mobile-fleet-workers",
      mutationTracker,
      onError: () => undefined,
      overlay: input.overlay,
      scope: fleetScope,
      session: input.session,
      startSync: true
    })
  )
  const fleetApprovals = createCollection(
    fleetApprovalKhalaSyncCollectionOptions({
      id: "khala-mobile-fleet-approvals",
      mutationTracker,
      onError: () => undefined,
      overlay: input.overlay,
      scope: fleetScope,
      session: input.session,
      startSync: true
    })
  )
  const fleetSteers = createCollection(
    fleetSteerKhalaSyncCollectionOptions({
      id: "khala-mobile-fleet-steers",
      mutationTracker,
      onError: () => undefined,
      overlay: input.overlay,
      scope: fleetScope,
      session: input.session,
      startSync: true
    })
  )

  return {
    chatThreads,
    fleetApprovals,
    fleetRun,
    fleetSteers,
    fleetWorkers,
    mutationTracker
  } as const
}

export const mobileChatPreviewThreads = (): ReadonlyArray<ChatThreadEntity> =>
  chatThreadsForSidebar([
    decodeChatThreadEntity({
      createdAt: "2026-07-04T20:00:00.000Z",
      lastMessageAt: "2026-07-04T20:05:00.000Z",
      messageCount: 3,
      ownerUserId: "user.preview",
      status: "active",
      threadId: "thread.preview.operator",
      title: "Operator sync",
      updatedAt: "2026-07-04T20:05:00.000Z"
    }),
    decodeChatThreadEntity({
      createdAt: "2026-07-04T19:00:00.000Z",
      lastMessageAt: "2026-07-04T19:20:00.000Z",
      messageCount: 7,
      ownerUserId: "user.preview",
      status: "active",
      threadId: "thread.preview.fleet",
      title: "Fleet health",
      updatedAt: "2026-07-04T19:20:00.000Z"
    })
  ])

export const mobileFleetPreviewRun = (): FleetRunEntity =>
  decodeFleetRunEntity({
    counters: {
      activeAssignments: 2,
      blockedAssignments: 1,
      completedAssignments: 18,
      failedAssignments: 0,
      workUnitsTotal: 21
    },
    desiredSlots: 4,
    runId: "fleet.preview.ts8",
    startedAt: "2026-07-04T20:00:00.000Z",
    status: "running",
    updatedAt: "2026-07-04T20:10:00.000Z",
    workerKind: "codex"
  })

export const createMobileKhalaSyncPreviewState = () => ({
  chatThreads: mobileChatPreviewThreads(),
  fleetRun: mobileFleetPreviewRun()
})
