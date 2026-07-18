import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  RUNTIME_INTERACTION_ENTITY_TYPE,
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncVersion,
  SyncVersionWatermark,
  agentRunScope,
  threadScope,
  canonicalJson,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  encodeAgentRunEntity,
  encodeAgentRunEventEntity,
  decodeRuntimeInteractionEntity,
  encodeRuntimeInteractionEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import { createKhalaSyncAgentTimeline } from "./agent-timeline.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const NOW = "2026-07-10T20:00:00.000Z"
const RUN = "run.timeline.1"
const scope = agentRunScope(RUN)
const roots: Array<string> = []

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const entry = (
  version: number,
  entityType: string,
  entityId: string,
  postImageJson: string,
) => new ChangelogEntry({
  scope,
  version: SyncVersion.make(version),
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImageJson,
  mutationRef: `mutation.timeline.${version}`,
  committedAt: NOW,
})

const runEntry = entry(
  1,
  AGENT_RUN_ENTITY_TYPE,
  RUN,
  canonicalJson(encodeAgentRunEntity(decodeAgentRunEntity({
    runId: RUN,
    routeId: "thread.timeline.1",
    userId: "owner.private",
    teamId: null,
    projectId: null,
    runtime: "codex",
    backend: "gcloud_vm",
    status: "running",
    goalId: null,
    goal: "private objective omitted by the client projection",
    repository: { provider: "github", owner: "private-owner", repo: "private-repo", ref: "main" },
    createdAt: NOW,
    updatedAt: NOW,
    startedAt: NOW,
    completedAt: null,
    failedAt: null,
    canceledAt: null,
  }))),
)

const eventEntry = (input: Readonly<{
  version: number
  id: string
  sequence: number
  summary: string
}>) => entry(
  input.version,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  input.id,
  canonicalJson(encodeAgentRunEventEntity(decodeAgentRunEventEntity({
    id: input.id,
    runId: RUN,
    sequence: input.sequence,
    type: "runtime.activity",
    summary: input.summary,
    status: "running",
    source: "provider-private-source",
    payloadJson: JSON.stringify({ rawProviderCallback: "must-not-project" }),
    artifactRefs: [`artifact.${input.sequence}`],
    externalEventId: `external-private-${input.sequence}`,
    createdAt: NOW,
  }))),
)

const session = (
  phase: ScopeSyncState = { phase: "live", cursor: SyncVersionWatermark.make(4) },
  opened: Array<string> = [],
): KhalaSyncSession => ({
  state: () => phase,
  pending: () => [{ mutationId: 1 }],
  subscribe: (requested: SyncScope) => Effect.sync(() => { opened.push(String(requested)) }),
}) as unknown as KhalaSyncSession

describe("contract khala_sync.client.confirmed_agent_timeline.v1", () => {
  test("reconstructs ordered confirmed state after duplicate/out-of-order replay and restart", () => {
    const root = mkdtempSync(join(tmpdir(), "khala-agent-timeline-"))
    roots.push(root)
    const database = join(root, "timeline.sqlite")
    const initial = openKhalaSyncStore(database)
    const replay = [
      runEntry,
      eventEntry({ version: 2, id: "event.3", sequence: 3, summary: "Third" }),
      eventEntry({ version: 3, id: "event.1", sequence: 1, summary: "First" }),
      eventEntry({ version: 4, id: "event.2", sequence: 2, summary: "Second" }),
    ]
    Effect.runSync(initial.applyConfirmed(scope, replay, SyncVersion.make(4)))
    Effect.runSync(initial.applyConfirmed(scope, replay, SyncVersion.make(4)))
    Effect.runSync(initial.close())

    const restarted = openKhalaSyncStore(database)
    try {
      const opened: Array<string> = []
      const timeline = createKhalaSyncAgentTimeline({ store: restarted, session: session(undefined, opened) })
      Effect.runSync(timeline.open(RUN))
      const snapshot = Effect.runSync(timeline.snapshot(RUN))

      expect(opened).toEqual(["scope.agent_run.run.timeline.1"])
      expect(snapshot.status).toEqual({ phase: "live", cursor: 4, pendingMutationCount: 1 })
      expect(snapshot.run).toEqual({
        backend: "gcloud_vm",
        runRef: RUN,
        routeRef: "thread.timeline.1",
        runtime: "codex",
        status: "running",
        createdAt: NOW,
        updatedAt: NOW,
        startedAt: NOW,
        completedAt: null,
        failedAt: null,
        canceledAt: null,
        version: 1,
      })
      expect(snapshot.events.map(event => [event.eventRef, event.sequence, event.version])).toEqual([
        ["event.1", 1, 3],
        ["event.2", 2, 4],
        ["event.3", 3, 2],
      ])
      const serialized = JSON.stringify(snapshot)
      expect(serialized).not.toContain("owner.private")
      expect(serialized).not.toContain("private objective")
      expect(serialized).not.toContain("provider-private-source")
      expect(serialized).not.toContain("rawProviderCallback")
      expect(serialized).not.toContain("external-private")
    } finally {
      Effect.runSync(restarted.close())
    }
  })

  test("hides cached rows unless the exact run scope is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [runEntry], SyncVersion.make(1)))
      const timeline = createKhalaSyncAgentTimeline({
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
      })
      expect(Effect.runSync(timeline.snapshot(RUN))).toEqual({
        status: { phase: "must_refetch", cursor: null, pendingMutationCount: 1 },
        run: null,
        events: [],
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("projects authoritative provider and model identity from runtime events", () => {
    const store = openKhalaSyncStore(":memory:")
    const startedEvent = entry(
      2,
      AGENT_RUN_EVENT_ENTITY_TYPE,
      "event.gemma.started",
      canonicalJson(encodeAgentRunEventEntity(decodeAgentRunEventEntity({
        id: "event.gemma.started",
        runId: RUN,
        sequence: 0,
        type: "turn.started",
        summary: "Turn started",
        status: "running",
        source: "runtime.openagents_native",
        payloadJson: JSON.stringify({
          schema: "openagents.khala_runtime_event.v1",
          eventId: "event.gemma.started",
          turnId: RUN,
          threadId: "thread.timeline.1",
          sequence: 0,
          observedAt: NOW,
          source: {
            lane: "hosted_khala",
            adapterKind: "openagents_native",
            surface: "server",
            providerRef: "google-ai-studio",
            modelRef: "gemma-4-31b-it",
          },
          visibility: "private",
          redactionClass: "private_ref",
          causalityRefs: [],
          kind: "turn.started",
        }),
        artifactRefs: [],
        externalEventId: "event.gemma.started",
        createdAt: NOW,
      }))),
    )
    try {
      Effect.runSync(store.applyConfirmed(scope, [runEntry, startedEvent], SyncVersion.make(2)))
      const timeline = createKhalaSyncAgentTimeline({ store, session: session() })
      expect(Effect.runSync(timeline.snapshot(RUN)).events[0]).toMatchObject({
        item: { kind: "connected", lane: "hosted_khala" },
        source: {
          lane: "hosted_khala",
          adapterKind: "openagents_native",
          surface: "server",
          providerRef: "google-ai-studio",
          modelRef: "gemma-4-31b-it",
        },
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("discovers the confirmed latest run from the canonical thread route", () => {
    const store = openKhalaSyncStore(":memory:")
    const thread = "thread.timeline.1"
    const threadEntry = (value: ChangelogEntry) => new ChangelogEntry({
      ...value,
      scope: threadScope(thread),
    })
    try {
      const interaction = new ChangelogEntry({
        scope: threadScope(thread),
        version: SyncVersion.make(3),
        entityType: EntityType.make(RUNTIME_INTERACTION_ENTITY_TYPE),
        entityId: EntityId.make("interaction.thread.question.1"),
        op: "upsert",
        postImageJson: canonicalJson(encodeRuntimeInteractionEntity(
          decodeRuntimeInteractionEntity({
            interactionRef: "interaction.thread.question.1",
            threadId: thread,
            turnId: RUN,
            ownerUserId: "owner.private",
            kind: "provider_question",
            status: "pending",
            interaction: {
              schema: "openagents.runtime_interaction.v1",
              interactionRef: "interaction.thread.question.1",
              threadId: thread,
              turnId: RUN,
              requestedSequence: 2,
              requestedAt: NOW,
              expiresAt: "2026-07-10T20:05:00.000Z",
              source: { lane: "codex_app_server", surface: "server" },
              visibility: "private",
              redactionClass: "private_ref",
              causalityRefs: [],
              payload: {
                kind: "provider_question",
                displayTitle: "Choose verification",
                questions: [{
                  questionRef: "question.thread.1",
                  displayText: "Which verification should run?",
                  multiSelect: false,
                  options: [{ optionRef: "option.tests", label: "Tests" }],
                }],
              },
              lifecycle: { status: "pending" },
            },
            createdAt: NOW,
            updatedAt: NOW,
          }),
        )),
        mutationRef: "mutation.timeline.interaction.3",
        committedAt: NOW,
      })
      Effect.runSync(store.applyConfirmed(
        threadScope(thread),
        [threadEntry(runEntry), threadEntry(eventEntry({
          version: 2,
          id: "event.thread.1",
          sequence: 1,
          summary: "Connected",
        })), interaction],
        SyncVersion.make(3),
      ))
      const timeline = createKhalaSyncAgentTimeline({ store, session: session() })
      const snapshot = Effect.runSync(timeline.snapshotForThread(thread))
      expect(snapshot.run).toMatchObject({ runRef: RUN, routeRef: thread })
      expect(snapshot.events.map(event => event.eventRef)).toEqual([
        "event.thread.1",
        "interaction.thread.question.1",
      ])
      expect(snapshot.events[1]).toMatchObject({
        eventType: "runtime.interaction.provider_question",
        sequence: 2,
        status: "pending",
        item: {
          kind: "question",
          questionRef: "interaction.thread.question.1",
          status: "pending",
          questions: [{
            questionRef: "question.thread.1",
            options: [{ optionRef: "option.tests", label: "Tests" }],
          }],
        },
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("retains prior-run events when a thread starts a newer run", () => {
    const store = openKhalaSyncStore(":memory:")
    const thread = "thread.timeline.history"
    const later = "2026-07-10T20:01:00.000Z"
    const runRow = (runRef: string, version: number, createdAt: string) =>
      new ChangelogEntry({
        scope: threadScope(thread),
        version: SyncVersion.make(version),
        entityType: EntityType.make(AGENT_RUN_ENTITY_TYPE),
        entityId: EntityId.make(runRef),
        op: "upsert",
        postImageJson: canonicalJson(encodeAgentRunEntity(decodeAgentRunEntity({
          runId: runRef,
          routeId: thread,
          userId: "owner.private",
          teamId: null,
          projectId: null,
          runtime: "openagents_native",
          backend: "hosted",
          status: "running",
          goalId: null,
          goal: "private",
          repository: {
            provider: "github",
            owner: "private-owner",
            repo: "private-repo",
            ref: "main",
          },
          createdAt,
          updatedAt: createdAt,
          startedAt: createdAt,
          completedAt: null,
          failedAt: null,
          canceledAt: null,
        }))),
        mutationRef: `mutation.${runRef}`,
        committedAt: createdAt,
      })
    const eventRow = (runRef: string, eventRef: string, version: number, createdAt: string) =>
      new ChangelogEntry({
        scope: threadScope(thread),
        version: SyncVersion.make(version),
        entityType: EntityType.make(AGENT_RUN_EVENT_ENTITY_TYPE),
        entityId: EntityId.make(eventRef),
        op: "upsert",
        postImageJson: canonicalJson(encodeAgentRunEventEntity(decodeAgentRunEventEntity({
          id: eventRef,
          runId: runRef,
          sequence: 1,
          type: "runtime.activity",
          summary: eventRef,
          status: "running",
          source: "provider-private-source",
          payloadJson: null,
          artifactRefs: [],
          externalEventId: null,
          createdAt,
        }))),
        mutationRef: `mutation.${eventRef}`,
        committedAt: createdAt,
      })
    try {
      Effect.runSync(store.applyConfirmed(
        threadScope(thread),
        [
          runRow("run.timeline.history.1", 1, NOW),
          eventRow("run.timeline.history.1", "event.timeline.history.1", 2, NOW),
          runRow("run.timeline.history.2", 3, later),
          eventRow("run.timeline.history.2", "event.timeline.history.2", 4, later),
        ],
        SyncVersion.make(4),
      ))
      const timeline = createKhalaSyncAgentTimeline({ store, session: session() })
      const snapshot = Effect.runSync(timeline.snapshotForThread(thread))

      expect(snapshot.run?.runRef).toBe("run.timeline.history.2")
      expect(snapshot.events.map(event => event.eventRef)).toEqual([
        "event.timeline.history.1",
        "event.timeline.history.2",
      ])
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("bounds the public timeline to the newest 500 ordered events", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const events = Array.from({ length: 505 }, (_, index) =>
        eventEntry({
          version: index + 2,
          id: `event.${String(index + 1).padStart(3, "0")}`,
          sequence: index + 1,
          summary: `Event ${index + 1}`,
        }))
      Effect.runSync(store.applyConfirmed(scope, [runEntry, ...events], SyncVersion.make(506)))
      const timeline = createKhalaSyncAgentTimeline({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(506) }),
      })
      const snapshot = Effect.runSync(timeline.snapshot(RUN))
      expect(snapshot.events).toHaveLength(500)
      expect(snapshot.events[0]?.sequence).toBe(6)
      expect(snapshot.events.at(-1)?.sequence).toBe(505)
    } finally {
      Effect.runSync(store.close())
    }
  })
})
