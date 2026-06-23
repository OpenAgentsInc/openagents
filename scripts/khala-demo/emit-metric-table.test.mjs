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

describe("recorded run table — EXECUTED acceptance verdict (honest red)", () => {
  // The recorded north-star run's earlier `verified:true` came from the M2 STATIC
  // regex pre-screen. Running the preserved artifact through the real executed
  // acceptance suite (scripts/khala-demo/run-executed-acceptance.mjs) FAILED 6/6
  // checks, so the recorded manifest now carries the honest executed verdict:
  // accepted:false, verificationClass:"failed", verified-rate 0. A real `failed`
  // beats a fake `passed`.
  test("verified-rate is 0 (executed: failed) and wall-clock is the recorded 106s", () => {
    const table = emitMetricTableFromManifest(recordedManifest(), { json: true });
    const parsed = JSON.parse(table);
    expect(parsed.verifiedRate).toBe(0);
    expect(parsed.rows).toHaveLength(1);
    const khala = parsed.rows[0];
    expect(khala.lane).toBe("khala");
    expect(khala.wallClock).toBe("1m 46s");
    expect(khala.verificationClass).toBe("failed");
    expect(khala.accepted).toBe(false);
  });

  test("tokens and $ are honestly not_measured; cost/accepted-outcome is not_applicable (unaccepted)", () => {
    const parsed = JSON.parse(emitMetricTableFromManifest(recordedManifest(), { json: true }));
    const khala = parsed.rows[0];
    expect(khala.tokens).toBe("not_measured");
    expect(khala.dollars).toBe("not_measured");
    expect(khala.costPerAcceptedOutcomeUsd).toBe("not_applicable");
    expect(khala.acceptedOutcomesPerKwh).toBe("not_measured");
    expect(khala.inWorldVsGatewaySplit).toBe("not_measured");
    expect(khala.settled).toBe(false);
  });

  test("renders a Markdown table with the honest verified-rate 0 and no bare $0.00", () => {
    const md = renderMetricTableMarkdown(
      buildMetricTable(reduceKhalaHeadToHeadManifest(recordedManifest())),
    );
    expect(md).toContain("Verified-rate: 0");
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
