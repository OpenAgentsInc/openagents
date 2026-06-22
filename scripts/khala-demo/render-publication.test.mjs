import { describe, expect, test } from "bun:test";

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";
import { renderKhalaHeadToHeadPublication } from "./render-publication.mjs";

const fixturePath = new URL(
  "../../docs/inference/fixtures/khala-head-to-head-dry-run.v1.json",
  import.meta.url,
);

describe("Khala head-to-head publication renderer", () => {
  test("renders fixture caveats, scoreboard, external claims, and blockers", () => {
    const metrics = reduceKhalaHeadToHeadManifest(loadManifest(fixturePath));
    const markdown = renderKhalaHeadToHeadPublication(metrics);

    expect(markdown).toContain("FIXTURE SCAFFOLD - not product proof");
    expect(markdown).toContain("canClose: `false`");
    expect(markdown).toContain("| khala | openagents/khala | fixture_scaffold | 89,600 | $7.32 | 18m 12s | yes | test_passed | $7.32 | not_measured | 60% in-world / 40% gateway |");
    expect(markdown).toContain("These rows are not OpenAgents measurements.");
    expect(markdown).toContain("reported_without_primary_url");
    expect(markdown).toContain("blocker.khala_demo.fixture_scaffold_not_live");
    expect(markdown).toContain("Settlement claims require public worker and validator settlement refs.");
  });
});
