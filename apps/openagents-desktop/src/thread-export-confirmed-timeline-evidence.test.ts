import {
  compileThreadExportArtifact,
  decodeThreadEventAuthorityRelation,
} from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { readDesktopThreadExportEvidenceFromConfirmedTimeline } from "./thread-export-confirmed-timeline-evidence.ts";

const THREAD = "thread.confirmed.export.1";
const RUN = "run.confirmed.export.1";

const event = (overrides: Record<string, unknown> = {}) => ({
  eventRef: "event.confirmed.export.1",
  runRef: RUN,
  sequence: 1,
  eventType: "text.delta",
  summary: "Confirmed answer",
  status: null,
  artifactRefs: ["artifact.confirmed.1"],
  item: { kind: "text", messageRef: "message.confirmed.1", text: "Confirmed answer" },
  createdAt: "2026-07-17T21:40:00Z",
  version: 3,
  ...overrides,
});

const snapshot = (overrides: Record<string, unknown> = {}) => ({
  status: { phase: "live", cursor: 8, pendingMutationCount: 0 },
  run: {
    runRef: RUN,
    routeRef: "route.confirmed.export.1",
    runtime: "openagents_native",
    backend: "hosted",
    status: "completed",
    createdAt: "2026-07-17T21:39:00Z",
    updatedAt: "2026-07-17T21:40:01Z",
    startedAt: "2026-07-17T21:39:01Z",
    completedAt: "2026-07-17T21:40:01Z",
    failedAt: null,
    canceledAt: null,
    version: 4,
  },
  events: [event()],
  ...overrides,
});

const run = (value: unknown, threadRef: unknown = THREAD) =>
  Effect.runPromise(
    readDesktopThreadExportEvidenceFromConfirmedTimeline(
      { snapshotForThread: () => value },
      threadRef,
    ),
  );

describe("Desktop confirmed-timeline canonical-export evidence", () => {
  test("projects exact confirmed events with deterministic accepted authority", async () => {
    let observedThreadRef = "";
    const result = await Effect.runPromise(
      readDesktopThreadExportEvidenceFromConfirmedTimeline(
        {
          snapshotForThread: (threadRef) => {
            observedThreadRef = threadRef;
            return snapshot();
          },
        },
        THREAD,
      ),
    );

    expect(observedThreadRef).toBe(THREAD);
    expect(result).toMatchObject({
      status: "available",
      threadRef: THREAD,
      events: [
        {
          eventRef: "event.confirmed.export.1",
          threadRef: THREAD,
          sequence: 1,
          data: {
            runRef: RUN,
            eventType: "text.delta",
            summary: "Confirmed answer",
            version: 3,
          },
        },
      ],
      relations: [
        {
          schema: "openagents.thread_event_authority.v1",
          threadRef: THREAD,
          eventRef: "event.confirmed.export.1",
          kind: "accepted",
        },
      ],
    });
    if (result.status !== "available") throw new Error("expected available evidence");
    const relation = decodeThreadEventAuthorityRelation(result.relations[0]);
    expect(relation.relationRef).toMatch(/^relation\.confirmed\.[a-f0-9]{64}$/);
    expect(relation).toEqual(result.relations[0]);
    expect(JSON.stringify(result.relations)).not.toMatch(/summary|text|artifact|path|credential/i);

    const repeated = await run(snapshot());
    expect(repeated).toEqual(result);
  });

  test("feeds the existing owner-only canonical export compiler", async () => {
    const evidence = await run(snapshot());
    if (evidence.status !== "available") throw new Error("expected available evidence");
    const compiled = compileThreadExportArtifact({
      intent: {
        schema: "openagents.thread_disclosure_intent.v1",
        intentRef: "intent.confirmed.export.1",
        idempotencyKey: "idempotency.confirmed.export.1",
        threadRef: THREAD,
        actorRef: "actor.owner.1",
        expectedVisibilityVersion: { state: "known", value: 8 },
        createdAt: "2026-07-17T21:41:00Z",
        kind: "thread.export.create",
        format: "canonical_event_bundle",
        artifactAudience: { kind: "owner_only" },
      },
      events: evidence.events,
      relations: evidence.relations,
      sha256: () => "a".repeat(64),
    });
    expect(compiled.artifact.events[0]?.authority.state).toBe("accepted");
    expect(compiled.artifact.events[0]?.data).toMatchObject({ runRef: RUN, version: 3 });
  });

  test("fails closed before lookup for a non-canonical thread ref", async () => {
    let reads = 0;
    const result = await Effect.runPromise(
      readDesktopThreadExportEvidenceFromConfirmedTimeline(
        {
          snapshotForThread: () => {
            reads += 1;
            return snapshot();
          },
        },
        "thread/unsafe",
      ),
    );
    expect(result).toEqual({ status: "unavailable" });
    expect(reads).toBe(0);
  });

  test("withholds non-live, cursorless, optimistic, or runless snapshots", async () => {
    for (const value of [
      snapshot({ status: { phase: "catching_up", cursor: 8, pendingMutationCount: 0 } }),
      snapshot({ status: { phase: "live", cursor: null, pendingMutationCount: 0 } }),
      snapshot({ status: { phase: "live", cursor: 8, pendingMutationCount: 1 } }),
      snapshot({ run: null }),
    ]) {
      await expect(run(value)).resolves.toEqual({ status: "unavailable" });
    }
  });

  test("withholds throwing, malformed, oversized, or conflicting event sources", async () => {
    await expect(
      Effect.runPromise(
        readDesktopThreadExportEvidenceFromConfirmedTimeline(
          {
            snapshotForThread: () => {
              throw new Error("private source detail");
            },
          },
          THREAD,
        ),
      ),
    ).resolves.toEqual({ status: "unavailable" });

    for (const value of [
      { raw: "source" },
      snapshot({ events: Array.from({ length: 501 }, (_, index) => event({ eventRef: `event.${index}`, sequence: index })) }),
      snapshot({ events: [event({ runRef: "run.other" })] }),
      snapshot({ events: [event(), event({ sequence: 2 })] }),
      snapshot({ events: [event(), event({ eventRef: "event.confirmed.export.2" })] }),
      snapshot({ events: [event({ eventRef: "event/unsafe" })] }),
      snapshot({ events: [event({ createdAt: "not-a-timestamp" })] }),
    ]) {
      await expect(run(value)).resolves.toEqual({ status: "unavailable" });
    }
  });

  test("does not invent supersession or reversion facts", async () => {
    const result = await run(
      snapshot({
        events: [
          event(),
          event({
            eventRef: "event.confirmed.export.2",
            sequence: 2,
            version: 4,
            createdAt: "2026-07-17T21:40:01Z",
          }),
        ],
      }),
    );
    if (result.status !== "available") throw new Error("expected available evidence");
    expect(
      result.relations.map((relation) => decodeThreadEventAuthorityRelation(relation).kind),
    ).toEqual(["accepted", "accepted"]);
    expect(JSON.stringify(result.relations)).not.toMatch(/superseded|reverted|restored/i);
  });
});
