import { describe, expect, test } from "vite-plus/test";

import checkedReleaseMatrix from "../compatibility/release-matrix.json" with { type: "json" };

import { compileAcpReleaseEvidence } from "./release-evidence.ts";

const NOW = new Date("2026-07-17T13:00:00.000Z");

describe("checked ACP release evidence compiler", () => {
  test("compiles the complete checked matrix into peer admission and feature evidence", () => {
    const result = compileAcpReleaseEvidence(checkedReleaseMatrix, { now: NOW });
    expect(result._tag).toBe("ReleaseEvidenceReady");
    if (result._tag !== "ReleaseEvidenceReady") return;
    expect(result.evidence.grok.map((record) => record.suiteId)).toEqual([
      "acp-wire-v1-conformance",
      "acp-release-matrix-v1",
      "grok-authority-reverse",
      "grok-question-extensions",
    ]);
    expect(result.evidence.cursor.map((record) => record.suiteId)).toEqual([
      "acp-wire-v1-conformance",
      "cursor-t3-bde0a4c0",
      "acp-release-matrix-v1",
      "cursor-authority-reverse",
      "cursor-vendor-extensions",
      "cursor-model-discovery",
    ]);
    expect(result.evidence.cursor.at(2)).toMatchObject({
      peerVersion: "2026.6.24",
      executableSha256: "b7babf47d8b1eee28ac27a74affa02a559bb38103a6e71fbb1f120805d51fedf",
      installationClosureSha256: "69d078daa4db8cbb4163ce2f010207553efb06d652c1e1ea421d739795532faa",
      platform: { os: "darwin", arch: "arm64" },
    });
  });

  test("fails closed for stale, incomplete, or revision-substituted evidence", () => {
    const stale = compileAcpReleaseEvidence(checkedReleaseMatrix, {
      now: new Date("2026-09-01T00:00:00.000Z"),
    });
    expect(stale).toMatchObject({ _tag: "ReleaseEvidenceUnavailable" });

    const incomplete = structuredClone(checkedReleaseMatrix) as any;
    incomplete.peers[0].scenarios.find((scenario: any) => scenario.id === "initialize").result =
      "blocked";
    expect(compileAcpReleaseEvidence(incomplete, { now: NOW })).toMatchObject({
      _tag: "ReleaseEvidenceUnavailable",
    });

    expect(
      compileAcpReleaseEvidence(checkedReleaseMatrix, {
        now: NOW,
        expectedOpenAgentsRevision: "f".repeat(40),
      }),
    ).toEqual({
      _tag: "ReleaseEvidenceUnavailable",
      diagnostics: ["checked release evidence does not match the expected OpenAgents revision"],
    });
  });
});
