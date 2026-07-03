import { describe, expect, test } from "bun:test";

import {
  QS7_EXECUTOR_RUN_REF,
  assertQs7ExternalPrPacketPublicSafe,
  renderQs7ExternalPrBody,
  rhysExecutorTargetAdapter,
  type Qs7ExternalPrPacket,
} from "./qs7-rhys-sales-motion";
import { targetFromAdapter } from "./target-adapter";

const packet = (): Qs7ExternalPrPacket => ({
  runRef: QS7_EXECUTOR_RUN_REF,
  verdict: "CONFIRMED",
  shareUrl: `https://openagents.com/qa/${QS7_EXECUTOR_RUN_REF}`,
  browserRecording: "video.qa_swarm.executor.qs7_public_home.browser",
  terminalRecording: "video.qa_swarm.executor.qs7_public_home.terminal",
  distilledTest: "test.qa_swarm.executor.public_home.e2e",
  trace: "trace.qa_swarm.executor.qs7_public_home",
  chillEvalComparison: "trace.compare.qa_swarm.executor.qs7_public_home",
  verificationCommand:
    "bun run --cwd apps/qa-runner evals -- --scenario executor-public-home --url https://executor.sh --name executor-public-prod --id qs7-public-home --reps 1",
  evalRows: [
    {
      variant: "baseline",
      axis: "executor-public:baseline",
      verdict: "CONFIRMED",
      runtime: "measured",
      notes: "public landing page copy rendered",
    },
    {
      variant: "candidate",
      axis: "executor-public:impossible-copy",
      verdict: "REFUTED",
      runtime: "measured",
      notes: "false copy assertion failed honestly",
    },
  ],
});

describe("QS7 executor sales-motion packet", () => {
  test("models executor as external production read-only", () => {
    const adapter = rhysExecutorTargetAdapter();
    const target = targetFromAdapter(adapter);

    expect(adapter.target.owner).toBe("external");
    expect(adapter.target.environment).toBe("prod");
    expect(target.restrictions.has("read-only")).toBe(true);
    expect(adapter.prodReadOnly.blockedStepKinds).toEqual(["click", "type"]);
  });

  test("renders a public-safe external PR body from concrete receipts", () => {
    const body = renderQs7ExternalPrBody(packet());

    expect(body).toContain("## QA Swarm audit");
    expect(body).toContain(`Share URL: https://openagents.com/qa/${QS7_EXECUTOR_RUN_REF}`);
    expect(body).toContain("executor-public:baseline");
    expect(body).not.toContain("<");
    expect(body).not.toContain("/Users/");
    expect(body).not.toContain("TODO");
  });

  test("refuses placeholders and local paths", () => {
    expect(() =>
      assertQs7ExternalPrPacketPublicSafe({
        ...packet(),
        browserRecording: "/Users/local/run/session.webm",
      }),
    ).toThrow(/public-safe/);
    expect(() =>
      assertQs7ExternalPrPacketPublicSafe({
        ...packet(),
        shareUrl: "<openagents.com/qa/...>",
      }),
    ).toThrow(/public-safe/);
  });
});
