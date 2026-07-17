import { createHash } from "node:crypto";

import { describe, expect, test, vi } from "vite-plus/test";

import { compileThreadExportArtifact } from "./thread-export-artifact.js";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.1",
  idempotencyKey: "idempotency.export.1",
  threadRef: "thread.desktop.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 3 },
  createdAt: "2026-07-17T14:30:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const accepted = (eventRef: string, observedAt: string) => ({
  schema: "openagents.thread_event_authority.v1" as const,
  relationRef: `relation.accepted.${eventRef}`,
  threadRef: intent.threadRef,
  eventRef,
  observedAt,
  kind: "accepted" as const,
});

const sha256 = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

describe("openagents.thread_export_artifact.v1", () => {
  test("compiles deterministic real bytes with explicit accepted and superseded authority", () => {
    const digest = vi.fn(sha256);
    const input = {
      intent,
      events: [
        {
          eventRef: "event.2",
          threadRef: intent.threadRef,
          sequence: 2,
          data: { text: "replacement", nested: { z: 1, a: true } },
        },
        {
          eventRef: "event.1",
          threadRef: intent.threadRef,
          sequence: 1,
          data: { text: "original" },
        },
      ],
      relations: [
        accepted("event.2", "2026-07-17T14:30:02.000Z"),
        {
          ...accepted("event.1", "2026-07-17T14:30:00.000Z"),
          relationRef: "relation.superseded.event.1",
          observedAt: "2026-07-17T14:30:01.000Z",
          kind: "superseded" as const,
          supersededByEventRef: "event.2",
        },
        accepted("event.1", "2026-07-17T14:30:00.000Z"),
      ],
      sha256: digest,
    };

    const first = compileThreadExportArtifact(input);
    const second = compileThreadExportArtifact({
      ...input,
      events: [...input.events].reverse(),
      relations: [...input.relations].reverse(),
    });

    expect(first.encoded).toBe(second.encoded);
    expect(first.artifactSha256).toBe(second.artifactSha256);
    expect(first.artifact.events.map(({ eventRef }) => eventRef)).toEqual(["event.1", "event.2"]);
    expect(first.artifact.events[0]?.authority).toMatchObject({
      state: "superseded",
      supersededByEventRef: "event.2",
    });
    expect(first.artifact.events[1]?.data).toEqual({
      nested: { a: true, z: 1 },
      text: "replacement",
    });
    expect(new TextDecoder().decode(first.bytes)).toBe(first.encoded);
    expect(digest).toHaveBeenCalledWith(first.bytes);
    expect(first.artifactSha256).toBe(sha256(first.bytes));
  });

  test("preserves explicit reverted authority in the artifact", () => {
    const result = compileThreadExportArtifact({
      intent,
      events: [
        { eventRef: "event.2", threadRef: intent.threadRef, sequence: 1, data: { ok: true } },
      ],
      relations: [
        accepted("event.2", "2026-07-17T14:30:00.000Z"),
        {
          ...accepted("event.2", "2026-07-17T14:30:01.000Z"),
          relationRef: "relation.reverted.event.2",
          kind: "reverted" as const,
          revertedByEventRef: "event.revert.1",
          restoredEventRef: "event.1",
        },
      ],
      sha256,
    });
    expect(result.artifact.events[0]?.authority).toEqual({
      state: "reverted",
      relationRefs: ["relation.accepted.event.2", "relation.reverted.event.2"],
      revertedByEventRef: "event.revert.1",
      restoredEventRef: "event.1",
    });
  });

  test("fails closed for broader audiences, other formats, and non-export intents", () => {
    const compile = (candidate: unknown) =>
      compileThreadExportArtifact({ intent: candidate, events: [], relations: [], sha256 });
    expect(() => compile({ ...intent, artifactAudience: { kind: "internet_readable" } })).toThrow(
      /owner_only/,
    );
    expect(() => compile({ ...intent, format: "json" })).toThrow(/canonical_event_bundle/);
    expect(() =>
      compile({
        ...intent,
        kind: "thread.visibility.set",
        target: { audience: { kind: "owner_only" }, administratorAccess: { kind: "none" } },
      }),
    ).toThrow(/thread.export.create/);
  });

  test("fails closed for cross-thread, duplicate, missing, and conflicting event authority", () => {
    const event = { eventRef: "event.1", threadRef: intent.threadRef, sequence: 1, data: null };
    const compile = (events: ReadonlyArray<unknown>, relations: ReadonlyArray<unknown>) =>
      compileThreadExportArtifact({ intent, events, relations, sha256 });

    expect(() =>
      compile(
        [{ ...event, threadRef: "thread.other.1" }],
        [accepted("event.1", "2026-07-17T14:30:00.000Z")],
      ),
    ).toThrow(/intent thread/);
    expect(() =>
      compile(
        [event, { ...event, sequence: 2 }],
        [accepted("event.1", "2026-07-17T14:30:00.000Z")],
      ),
    ).toThrow(/duplicated/);
    expect(() =>
      compile(
        [event, { ...event, eventRef: "event.2" }],
        [accepted("event.1", "2026-07-17T14:30:00.000Z")],
      ),
    ).toThrow(/sequence is duplicated/);
    expect(() => compile([event], [])).toThrow(/authority is missing/);
    expect(() =>
      compile(
        [event],
        [{ ...accepted("event.1", "2026-07-17T14:30:00.000Z"), threadRef: "thread.other.1" }],
      ),
    ).toThrow(/cross_thread/);
  });

  test("rejects non-JSON data and invalid digest output", () => {
    const base = {
      intent,
      events: [
        { eventRef: "event.1", threadRef: intent.threadRef, sequence: 1, data: { ok: true } },
      ],
      relations: [accepted("event.1", "2026-07-17T14:30:00.000Z")],
    };
    expect(() =>
      compileThreadExportArtifact({
        ...base,
        events: [{ ...base.events[0], data: { bad: undefined } }],
        sha256,
      }),
    ).toThrow();
    expect(() => compileThreadExportArtifact({ ...base, sha256: () => "not-a-digest" })).toThrow(
      /SHA-256/,
    );
  });
});
