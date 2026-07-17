import { describe, expect, test } from "vite-plus/test";

import {
  decodeThreadEventAuthorityRelation,
  projectThreadEventAuthority,
} from "./thread-event-authority.js";

const accepted = {
  schema: "openagents.thread_event_authority.v1" as const,
  relationRef: "relation.accepted.1",
  threadRef: "thread.desktop.1",
  eventRef: "event.original.1",
  observedAt: "2026-07-17T13:00:00.000Z",
  kind: "accepted" as const,
};

describe("openagents.thread_event_authority.v1", () => {
  test("decodes accepted, superseded, and reverted relations as ref-only evidence", () => {
    const relations = [
      accepted,
      {
        ...accepted,
        relationRef: "relation.superseded.1",
        kind: "superseded",
        supersededByEventRef: "event.replacement.1",
      },
      {
        ...accepted,
        relationRef: "relation.reverted.1",
        eventRef: "event.replacement.1",
        kind: "reverted",
        revertedByEventRef: "event.revert.1",
        restoredEventRef: "event.original.1",
      },
    ].map(decodeThreadEventAuthorityRelation);

    expect(relations.map(({ kind }) => kind)).toEqual(["accepted", "superseded", "reverted"]);
    expect(JSON.stringify(relations)).not.toContain('"body"');
    expect(JSON.stringify(relations)).not.toContain('"summary"');
  });

  test("rejects raw content, malformed refs/timestamps, and self-relations", () => {
    expect(() =>
      decodeThreadEventAuthorityRelation({ ...accepted, body: "private transcript" }),
    ).toThrow();
    expect(() =>
      decodeThreadEventAuthorityRelation({ ...accepted, eventRef: "../escape" }),
    ).toThrow();
    expect(() =>
      decodeThreadEventAuthorityRelation({ ...accepted, observedAt: "not-a-time" }),
    ).toThrow();
    expect(() =>
      decodeThreadEventAuthorityRelation({
        ...accepted,
        kind: "superseded",
        supersededByEventRef: accepted.eventRef,
      }),
    ).toThrow();
    expect(() =>
      decodeThreadEventAuthorityRelation({
        ...accepted,
        kind: "reverted",
        revertedByEventRef: accepted.eventRef,
        restoredEventRef: "event.original.0",
      }),
    ).toThrow();
    expect(() =>
      decodeThreadEventAuthorityRelation({
        ...accepted,
        kind: "reverted",
        revertedByEventRef: "event.revert.1",
        restoredEventRef: accepted.eventRef,
      }),
    ).toThrow();
  });

  test("projects accepted then superseded in observation order, not input order", () => {
    const superseded = {
      ...accepted,
      relationRef: "relation.superseded.1",
      observedAt: "2026-07-17T13:01:00.000Z",
      kind: "superseded" as const,
      supersededByEventRef: "event.replacement.1",
    };
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [superseded, accepted],
      }),
    ).toEqual({
      status: "resolved",
      threadRef: accepted.threadRef,
      eventRef: accepted.eventRef,
      state: "superseded",
      relationRefs: [accepted.relationRef, superseded.relationRef],
      supersededByEventRef: superseded.supersededByEventRef,
    });
  });

  test("projects an accepted event reverted by an exact event while naming the restored event", () => {
    const reverted = {
      ...accepted,
      relationRef: "relation.reverted.1",
      observedAt: "2026-07-17T13:01:00.000Z",
      kind: "reverted" as const,
      revertedByEventRef: "event.revert.1",
      restoredEventRef: "event.previous.1",
    };
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [accepted, reverted],
      }),
    ).toMatchObject({
      status: "resolved",
      state: "reverted",
      revertedByEventRef: "event.revert.1",
      restoredEventRef: "event.previous.1",
    });
  });

  test("returns explicit missing without promoting unrelated relation evidence", () => {
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: "event.missing.1",
        relations: [accepted],
      }),
    ).toEqual({
      status: "missing",
      threadRef: accepted.threadRef,
      eventRef: "event.missing.1",
    });
  });

  test("fails closed on cross-thread, duplicate, ambiguous, and invalid-transition evidence", () => {
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [{ ...accepted, threadRef: "thread.other.1" }],
      }),
    ).toMatchObject({ status: "conflict", reason: "cross_thread" });
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [accepted, accepted],
      }),
    ).toMatchObject({ status: "conflict", reason: "duplicate_relation" });
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [
          accepted,
          {
            ...accepted,
            relationRef: "relation.superseded.1",
            kind: "superseded",
            supersededByEventRef: "event.next.1",
          },
        ],
      }),
    ).toMatchObject({ status: "conflict", reason: "ambiguous_order" });
    expect(
      projectThreadEventAuthority({
        threadRef: accepted.threadRef,
        eventRef: accepted.eventRef,
        relations: [{ ...accepted, kind: "superseded", supersededByEventRef: "event.next.1" }],
      }),
    ).toMatchObject({ status: "conflict", reason: "invalid_transition" });
  });
});
