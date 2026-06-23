import { describe, expect, test } from "bun:test";

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";
import {
  buildMetricTable,
  costPerAcceptedOutcomeCell,
  dollarsCell,
  emitMetricTableFromManifest,
  inWorldVsGatewayCell,
  renderMetricTableMarkdown,
  tokensCell,
  wallClockCell,
} from "./emit-metric-table.mjs";

const recordedPath = new URL(
  "../../docs/inference/fixtures/khala-head-to-head-recorded-run.v1.json",
  import.meta.url,
);
const dryRunPath = new URL(
  "../../docs/inference/fixtures/khala-head-to-head-dry-run.v1.json",
  import.meta.url,
);

function recordedManifest() {
  return structuredClone(loadManifest(recordedPath));
}

describe("emit-metric-table honest unmeasured cells", () => {
  test("tokensCell downgrades an unmeasured zero to not_measured", () => {
    expect(
      tokensCell({ tokens: 0, blockerRefs: ["blocker.khala_demo.tokens_not_measured"] }),
    ).toBe("not_measured");
  });

  test("tokensCell keeps a real measured token count", () => {
    expect(tokensCell({ tokens: 89600, blockerRefs: [] })).toBe(89600);
  });

  test("tokensCell keeps a real zero with no not-measured blocker", () => {
    expect(tokensCell({ tokens: 0, blockerRefs: [] })).toBe(0);
  });

  test("dollarsCell downgrades an unmeasured zero to not_measured", () => {
    expect(
      dollarsCell({ dollars: 0, blockerRefs: ["blocker.khala_demo.cost_usd_not_measured"] }),
    ).toBe("not_measured");
  });

  test("dollarsCell keeps a real measured cost", () => {
    expect(dollarsCell({ dollars: 7.32, blockerRefs: [] })).toBe(7.32);
  });

  test("costPerAcceptedOutcomeCell preserves not_applicable", () => {
    expect(
      costPerAcceptedOutcomeCell({ costPerAcceptedOutcomeUsd: "not_applicable", blockerRefs: [] }),
    ).toBe("not_applicable");
  });

  test("costPerAcceptedOutcomeCell downgrades an unmeasured zero cost", () => {
    expect(
      costPerAcceptedOutcomeCell({
        costPerAcceptedOutcomeUsd: 0,
        blockerRefs: ["blocker.khala_demo.cost_usd_not_measured"],
      }),
    ).toBe("not_measured");
  });

  test("costPerAcceptedOutcomeCell keeps a real measured cost", () => {
    expect(
      costPerAcceptedOutcomeCell({ costPerAcceptedOutcomeUsd: 7.32, blockerRefs: [] }),
    ).toBe(7.32);
  });
});

describe("wallClockCell formatting", () => {
  test("formats seconds-only durations", () => {
    expect(wallClockCell({ wallClockMs: 106000 })).toBe("1m 46s");
  });

  test("formats sub-minute durations", () => {
    expect(wallClockCell({ wallClockMs: 45000 })).toBe("45s");
  });

  test("reports not_measured for a non-positive duration", () => {
    expect(wallClockCell({ wallClockMs: 0 })).toBe("not_measured");
  });
});

describe("inWorldVsGatewayCell", () => {
  test("reports not_measured when split is not measured", () => {
    expect(
      inWorldVsGatewayCell({ inWorldVsGatewaySplit: { status: "not_measured" } }),
    ).toBe("not_measured");
  });

  test("formats a measured split", () => {
    expect(
      inWorldVsGatewayCell({
        inWorldVsGatewaySplit: {
          status: "measured_from_manifest_units",
          inWorldShare: 0.6,
          gatewayShare: 0.4,
        },
      }),
    ).toBe("60% in-world / 40% gateway");
  });
});

describe("recorded run table — EXECUTED acceptance verdicts (honest red + genuine green)", () => {
  // Two honest data points now live in the recorded manifest:
  //   1. The bare-north-star run: running the preserved artifact through the real
  //      executed acceptance suite (scripts/khala-demo/run-executed-acceptance.mjs)
  //      FAILED 6/6 (localStorage-on-load crash; no state-contract hooks exposed).
  //   2. The contract-augmented run on 2026-06-23: the same headless suite ran the
  //      game and PASSED 6/6 (verified:true, scalarReward 1). This is the genuine
  //      executed north-star pass. (Gateway prod returned verification:failed on the
  //      same stream only because its cheap pre-screen rejects CDN-loaded three.js;
  //      the standalone Playwright runner has no such gate and is authoritative.)
  // So verified-rate is now 1/2 = 0.5 across the two runs. A real `failed` and a real
  // `passed` both beat any fake.
  test("two runs: verified-rate is 0.5 (one executed-fail, one executed-pass)", () => {
    const table = emitMetricTableFromManifest(recordedManifest(), { json: true });
    const parsed = JSON.parse(table);
    expect(parsed.verifiedRate).toBe(0.5);
    expect(parsed.rows).toHaveLength(2);
    const failRow = parsed.rows.find((r) => r.verificationClass === "failed");
    const passRow = parsed.rows.find((r) => r.verificationClass === "test_passed");
    expect(failRow).toBeDefined();
    expect(failRow.lane).toBe("khala");
    expect(failRow.wallClock).toBe("1m 46s");
    expect(failRow.accepted).toBe(false);
    expect(passRow).toBeDefined();
    expect(passRow.lane).toBe("khala");
    expect(passRow.accepted).toBe(true);
  });

  test("tokens and $ are honestly not_measured on the recorded runs", () => {
    const parsed = JSON.parse(emitMetricTableFromManifest(recordedManifest(), { json: true }));
    for (const khala of parsed.rows) {
      expect(khala.tokens).toBe("not_measured");
      expect(khala.dollars).toBe("not_measured");
      expect(khala.acceptedOutcomesPerKwh).toBe("not_measured");
      expect(khala.inWorldVsGatewaySplit).toBe("not_measured");
      expect(khala.settled).toBe(false);
    }
  });

  test("renders a Markdown table with the honest verified-rate 0.5 and no bare $0.00", () => {
    const md = renderMetricTableMarkdown(
      buildMetricTable(reduceKhalaHeadToHeadManifest(recordedManifest())),
    );
    expect(md).toContain("Verified-rate: 0.5");
    expect(md).toContain("1m 46s");
    expect(md).toContain("not_measured");
    expect(md).not.toContain("$0.00");
  });
});

describe("fixture dry-run table preserves measured numbers", () => {
  test("dry-run khala lane keeps its measured tokens and cost", () => {
    const parsed = JSON.parse(emitMetricTableFromManifest(structuredClone(loadManifest(dryRunPath)), { json: true }));
    const khala = parsed.rows.find((r) => r.lane === "khala");
    expect(khala.tokens).toBe(89600);
    expect(khala.dollars).toBe(7.32);
    expect(khala.costPerAcceptedOutcomeUsd).toBe(7.32);
  });
});
