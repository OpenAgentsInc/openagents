import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { decodeWorkIndexReadRequest } from "@openagentsinc/all-work-contract";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts";
import {
  AllWorkReadError,
  readFullAutoWorkIndex,
  readFullAutoWorkSnapshot,
} from "./all-work-projection.ts";

const temporaryRoots: Array<string> = [];
const makeRegistry = () => {
  const root = mkdtempSync(join(tmpdir(), "omega-effectd-all-work-"));
  temporaryRoots.push(root);
  return openFullAutoRunRegistry(join(root, "runs.json"), () => new Date("2026-08-02T12:00:00Z"));
};

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) rmSync(root, { force: true, recursive: true });
});

describe("omega-effectd All Work projection", () => {
  test("projects a durable Full Auto run without leaking its objective", () => {
    const registry = makeRegistry();
    const draft = registry.createDraft({
      title: "Review security evidence",
      objective: "private objective text",
      doneCondition: "private completion text",
      objectiveSource: "user",
      threadRef: "thread:test:1",
    });
    const started = registry.start(draft.runRef, {
      actor: "owner_ui",
      reason: "Start the admitted run",
    });
    expect(started.ok).toBe(true);

    const result = readFullAutoWorkIndex(
      registry.list(),
      decodeWorkIndexReadRequest({}),
      "2026-08-02T12:00:01Z",
    );
    expect(result.items).toHaveLength(1);
    expect(result.items[0]).toMatchObject({
      workRef: `work:${draft.runRef}`,
      state: "active",
      revision: 1,
      domain: "general",
      workClass: "run",
    });
    expect(JSON.stringify(result)).not.toContain("private objective text");
    expect(JSON.stringify(result)).not.toContain("private completion text");

    const snapshot = readFullAutoWorkSnapshot(
      registry.list(),
      `work:${draft.runRef}`,
      "2026-08-02T12:00:01Z",
    );
    expect(snapshot.snapshot.runRefs).toEqual([draft.runRef]);
    expect(snapshot.snapshot.threadRefs).toEqual(["thread:test:1"]);
  });

  test("binds cursors to the exact query and returns typed not-found errors", () => {
    const registry = makeRegistry();
    registry.createDraft({
      title: "First",
      objective: "First objective",
      doneCondition: "First done condition",
      objectiveSource: "user",
    });
    registry.createDraft({
      title: "Second",
      objective: "Second objective",
      doneCondition: "Second done condition",
      objectiveSource: "user",
    });

    const first = readFullAutoWorkIndex(
      registry.list(),
      decodeWorkIndexReadRequest({ limit: 1 }),
      "2026-08-02T12:00:01Z",
    );
    expect(first.nextCursor).not.toBeNull();
    const second = readFullAutoWorkIndex(
      registry.list(),
      decodeWorkIndexReadRequest({ limit: 1, cursor: first.nextCursor }),
      "2026-08-02T12:00:01Z",
    );
    expect(second.items).toHaveLength(1);
    expect(second.nextCursor).toBeNull();

    expect(() =>
      readFullAutoWorkIndex(
        registry.list(),
        decodeWorkIndexReadRequest({
          cursor: first.nextCursor,
          filter: { domains: ["security"], states: [] },
        }),
        "2026-08-02T12:00:01Z",
      ),
    ).toThrow(AllWorkReadError);
    expect(() =>
      readFullAutoWorkSnapshot(registry.list(), "work:missing:1", "2026-08-02T12:00:01Z"),
    ).toThrow(AllWorkReadError);
  });
});
