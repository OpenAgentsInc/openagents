import { Exit, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { IdeBaselineReceiptSchema } from "./baseline-contract.ts";

describe("IDE baseline receipt contract", () => {
  test("requires p50/p95/p99 and explicit gaps", () => {
    const receipt = {
      schemaVersion: "openagents.desktop.ide-baseline.v1",
      environment: {
        capturedAt: "2026-07-19T00:00:00.000Z",
        commitSha: "a".repeat(40),
        platform: "darwin",
        architecture: "arm64",
        nodeVersion: "v24.0.0",
        electronVersion: "43.1.0",
        fixtureFiles: 100,
        repetitions: 9,
        mode: "public-safe deterministic local fixture",
      },
      metrics: [
        {
          metric: "workspace.tree.cached-first-page",
          category: "latency",
          unit: "milliseconds",
          repetitions: 9,
          p50: 1,
          p95: 2,
          p99: 3,
          minimum: 0.5,
          maximum: 4,
          sourceRef: "apps/openagents-desktop/src/workspace-service.ts",
          noise: "fixture",
        },
      ],
      gaps: [
        {
          probe: "finder.cold-open",
          status: "unmeasured",
          reason: "requires packaged Finder automation",
          plannedPacket: "IDE-07",
        },
      ],
      rawResultRefs: ["apps/openagents-desktop/benchmarks/ide/startup.json"],
      assertions: ["No private paths or repository content are emitted."],
    };
    expect(Exit.isSuccess(Schema.decodeUnknownExit(IdeBaselineReceiptSchema)(receipt))).toBe(true);
    expect(
      Exit.isFailure(
        Schema.decodeUnknownExit(IdeBaselineReceiptSchema)({
          ...receipt,
          metrics: [{ ...receipt.metrics[0], p99: undefined }],
        }),
      ),
    ).toBe(true);
  });
});
