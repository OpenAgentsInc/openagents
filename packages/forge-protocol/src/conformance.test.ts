import { describe, expect, test } from "vite-plus/test";

import buzzFixtures from "../conformance/fixtures/buzz/5a3b8176aac5f4bced452ac8920477c5e059b828/events.json" with { type: "json" };
import gitworkshopFixtures from "../conformance/fixtures/gitworkshop/b049b163ba22774e918cdb7fffb3dc0662567238/events.json" with { type: "json" };
import ngitFixtures from "../conformance/fixtures/ngit/6d806d5e93babe01b2b0053821d15afbbff406f6/events.json" with { type: "json" };
import matrix from "../conformance/matrix.json" with { type: "json" };

import {
  compileForgeConformanceMatrix,
  encodeForgeInteropProjection,
  evaluateForgeProjectionGate,
  validateForgeInteropFixture,
  type ForgeInteropProjection,
} from "./conformance.js";

const validProjection = (value: unknown): ForgeInteropProjection => {
  const result = validateForgeInteropFixture(value);
  expect(result.state).toBe("FixtureValid");
  if (result.state === "FixtureInvalid") throw new Error(result.diagnostics.join("\n"));
  return result.projection;
};

describe("Forge NIP-34 conformance fixtures", () => {
  test("reads all pinned fixture profiles", () => {
    const fixtures = [
      ...ngitFixtures.fixtures,
      ...gitworkshopFixtures.fixtures,
      ...buzzFixtures.fixtures,
    ];
    expect(fixtures).toHaveLength(9);
    for (const fixture of fixtures)
      expect(validateForgeInteropFixture(fixture).state).toBe("FixtureValid");
  });

  test("retains canonical multi-value clones and unknown tags exactly", () => {
    const projection = validProjection(ngitFixtures.fixtures[0]);
    expect(projection.cloneUrls).toEqual([
      "https://openagents.com/git/openagents/openagents.git",
      "https://mirror.example/openagents.git",
    ]);
    expect(projection.unknownTags).toEqual([
      ["future-capability", "v1", "preserve-me"],
      ["future-capability", "v2"],
    ]);
    expect(encodeForgeInteropProjection(projection)).toEqual(ngitFixtures.fixtures[0]?.event);
  });

  test("holds repository state until every named object exists", () => {
    const projection = validProjection(ngitFixtures.fixtures[1]);
    expect(evaluateForgeProjectionGate(projection, new Set())).toEqual({
      state: "ProjectionBlocked",
      reason: "object_unavailable",
      missingObjectIds: [
        "2222222222222222222222222222222222222222",
        "3333333333333333333333333333333333333333",
        "4444444444444444444444444444444444444444",
      ],
    });
    const available = new Set(projection.requiredObjectIds);
    expect(evaluateForgeProjectionGate(projection, available).state).toBe("ProjectionReady");
  });

  test("models both event-first and object-first pointer races", () => {
    const projection = validProjection(ngitFixtures.fixtures[3]);
    expect(evaluateForgeProjectionGate(projection, new Set()).state).toBe("ProjectionBlocked");
    const objectFirst = new Set(["6666666666666666666666666666666666666666"]);
    expect(evaluateForgeProjectionGate(projection, objectFirst).state).toBe("ProjectionReady");
  });

  test("rejects a refs/nostr pointer that does not bind the event to its tip", () => {
    const source = ngitFixtures.fixtures[3];
    expect(source).toBeDefined();
    if (source === undefined) return;
    const mismatched = {
      ...source,
      objectRefs: [
        {
          ref: `refs/nostr/${source.event.id}`,
          oid: "ffffffffffffffffffffffffffffffffffffffff",
        },
      ],
    };
    expect(validateForgeInteropFixture(mismatched)).toEqual({
      state: "FixtureInvalid",
      diagnostics: ["pointer ref does not resolve to the event commit"],
    });
  });

  test("keeps the pinned Buzz SDK and Desktop emitters distinct", () => {
    expect(buzzFixtures.fixtures.map((fixture) => fixture.profile)).toEqual([
      "buzz-sdk-pointer-pr-1618",
      "buzz-sdk-pointer-update-1619",
      "buzz-desktop-target-branch-1618",
    ]);
  });

  test("compiles fixture claims but no peer compatibility claim", () => {
    const result = compileForgeConformanceMatrix(matrix);
    expect(result.state).toBe("ConformanceReady");
    if (result.state === "ConformanceUnavailable") return;
    expect(result.fixtureClaims.length).toBeGreaterThan(0);
    expect(result.liveClaims).toEqual([]);
    expect(result.blockedRows).toEqual([
      "ngit-live-clone-fetch",
      "gitworkshop-live-discovery-read",
      "owned-service-object-projection-race",
    ]);
  });

  test("keeps the restore receipt as partial evidence without a live ngit claim", () => {
    const row = matrix.rows.find((candidate) => candidate.id === "ngit-live-clone-fetch");
    expect(row).toMatchObject({
      blockerRefs: ["github:OpenAgentsInc/openagents#9244"],
      claim: "none",
      result: "blocked",
    });
    expect(row?.evidence).toEqual([
      {
        path: "docs/forge/receipts/2026-07-26-forge-git-live-disk-restore.json",
        sha256: "a1be07fff0e872febd5715af02787be9875a9a72e3d68cba8212a409e0c966ed",
      },
    ]);
    expect(
      matrix.rows.find(
        (candidate) => candidate.id === "gitworkshop-live-discovery-read",
      )?.blockerRefs,
    ).toEqual(["github:OpenAgentsInc/openagents#9244"]);
    expect(
      matrix.rows.find(
        (candidate) => candidate.id === "owned-service-object-projection-race",
      )?.blockerRefs,
    ).toEqual(["github:OpenAgentsInc/openagents#9249"]);
  });

  test("fails closed when a blocked row claims peer compatibility", () => {
    const invalid = structuredClone(matrix);
    const row = invalid.rows.find((candidate) => candidate.id === "ngit-live-clone-fetch");
    expect(row).toBeDefined();
    if (row === undefined) return;
    row.claim = "works-with-peer";
    const result = compileForgeConformanceMatrix(invalid);
    expect(result.state).toBe("ConformanceUnavailable");
  });

  test("fails closed when a live claim cites only fixture evidence", () => {
    const invalid = structuredClone(matrix);
    const row = invalid.rows.find((candidate) => candidate.id === "ngit-live-clone-fetch");
    expect(row).toBeDefined();
    if (row === undefined) return;
    row.result = "live-pass";
    row.claim = "works-with-peer";
    row.evidence = [structuredClone(matrix.rows[0]!.evidence[0]!)];
    row.blockerRefs = [];
    const result = compileForgeConformanceMatrix(invalid);
    expect(result.state).toBe("ConformanceUnavailable");
    if (result.state === "ConformanceUnavailable")
      expect(result.diagnostics).toContain(
        "row ngit-live-clone-fetch has a live pass without a live receipt",
      );
  });
});
