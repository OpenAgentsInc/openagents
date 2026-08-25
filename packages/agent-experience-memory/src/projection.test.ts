import { describe, expect, test } from "vite-plus/test";

import {
  buildEngramBody,
  buildEngramEvent,
  engramContentDigest,
  signSupersedingEngram,
  type EngramEvent,
} from "./engram.js";
import { project, projectedValue, projectionMatches } from "./projection.js";

const PUBKEY = "a".repeat(64);
const sign = (eventId: string): string => `sig-${eventId.slice(0, 16)}`;

const engram = (
  slug: string,
  value: string | null,
  createdAt: number,
  entityId = "entity-1",
  derivedFromSlugs: ReadonlyArray<string> = [],
): EngramEvent => {
  const body = buildEngramBody(slug, value, {
    admission: "admitted",
    entityId,
    contentDigest: engramContentDigest(value),
    sourceEventRefs: [],
    relations: [],
    derivedFromSlugs: [...derivedFromSlugs],
  });
  return buildEngramEvent(PUBKEY, createdAt, slug, JSON.stringify(body), sign);
};

describe("projecting an engram log", () => {
  test("derives the live value per slug", () => {
    const projection = project([
      engram("note/one", "first", 1_000),
      engram("note/two", "second", 1_001),
    ]);
    expect(projectedValue(projection, "note/one")).toBe("first");
    expect(projectedValue(projection, "note/two")).toBe("second");
    expect(projection.entries).toHaveLength(2);
  });

  test("resolves a supersession chain to its newest value", () => {
    const first = engram("note/one", "first", 1_000);
    const second = signSupersedingEngram(first, "corrected", 1_001, PUBKEY, sign);
    const third = signSupersedingEngram(second, "corrected twice", 1_002, PUBKEY, sign);

    const projection = project([first, second, third]);
    expect(projectedValue(projection, "note/one")).toBe("corrected twice");
    expect(projection.entries[0]?.revisions).toBe(3);
    expect(projection.entries[0]?.head).toBe(third.id);
  });

  test("names a tombstoned slug rather than dropping it silently", () => {
    const first = engram("note/one", "first", 1_000);
    const tombstone = signSupersedingEngram(first, null, 1_001, PUBKEY, sign);

    const projection = project([first, tombstone]);
    expect(projection.tombstoned).toEqual(["note/one"]);
    expect(projectedValue(projection, "note/one")).toBeUndefined();
  });

  test("is order-independent: shuffled arrival projects identically", () => {
    const first = engram("note/one", "first", 1_000);
    const second = signSupersedingEngram(first, "corrected", 1_001, PUBKEY, sign);
    const other = engram("note/two", "second", 1_002);

    const forwards = project([first, second, other]);
    const backwards = project([other, second, first]);
    expect(backwards.digest).toBe(forwards.digest);
  });

  test("is idempotent: a cold rebuild equals the warm one", () => {
    const events = [
      engram("note/one", "first", 1_000),
      engram("note/two", "second", 1_001, "entity-2"),
    ];
    expect(project(events).digest).toBe(project(events).digest);
    expect(projectionMatches(project(events), events)).toBe(true);
  });

  test("refuses a tampered event and counts it", () => {
    const authentic = engram("note/one", "authentic", 1_000);
    const tampered: EngramEvent = {
      ...authentic,
      content: authentic.content.replace("authentic", "forged"),
    };

    const projection = project([tampered]);
    expect(projection.entries).toHaveLength(0);
    expect(projection.rejected.unverified).toBe(1);
  });

  test("drops a forked chain whole rather than applying half a correction", () => {
    const first = engram("note/one", "first", 1_000);
    const branchA = signSupersedingEngram(first, "branch a", 1_001, PUBKEY, sign);
    const branchB = signSupersedingEngram(first, "branch b", 1_002, PUBKEY, sign);

    const projection = project([first, branchA, branchB]);
    expect(projectedValue(projection, "note/one")).toBeUndefined();
    expect(projection.rejected.unresolvedChain).toBe(3);
  });

  test("drops a chain whose prior link is missing", () => {
    const first = engram("note/one", "first", 1_000);
    const second = signSupersedingEngram(first, "corrected", 1_001, PUBKEY, sign);

    // The root never arrived.
    const projection = project([second]);
    expect(projectedValue(projection, "note/one")).toBeUndefined();
    expect(projection.rejected.unresolvedChain).toBe(1);
  });

  test("gathers slugs under the entity that names them, and derivation edges", () => {
    const projection = project([
      engram("fact/a", "a", 1_000, "entity-1"),
      engram("fact/b", "b", 1_001, "entity-1"),
      engram("heuristic/x", "distilled", 1_002, "entity-2", ["fact/a", "fact/b"]),
    ]);

    const entity = projection.entities.find((candidate) => candidate.entityId === "entity-1");
    expect(entity?.slugs).toEqual(["fact/a", "fact/b"]);
    expect(projection.relations).toEqual([
      { from: "heuristic/x", to: "fact/a", type: "derived_from" },
      { from: "heuristic/x", to: "fact/b", type: "derived_from" },
    ]);
  });

  test("notices when the log has moved past the projection", () => {
    const events = [engram("note/one", "first", 1_000)];
    const projection = project(events);
    const grown = [...events, engram("note/two", "second", 1_001)];
    expect(projectionMatches(projection, grown)).toBe(false);
  });
});
