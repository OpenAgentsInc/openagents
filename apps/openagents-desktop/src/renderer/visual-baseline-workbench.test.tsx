import {
  desktopAgentGroupFixtures,
  desktopApprovalCardInteractiveFixture,
  desktopApprovalCardStaticFixtures,
  desktopCommandCardFixtures,
  desktopContextMeterFixtures,
  desktopDispatchLongTailFixtures,
  desktopFileChangeCardFixtures,
  desktopPlanCardFixtures,
  desktopReasoningDisclosureFixtures,
  desktopTimelineMessageFixtures,
  desktopTimelineNoticeFixtures,
  desktopToolCallCardFixtures,
} from "@openagentsinc/ui/desktop-workbench";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vite-plus/test";

import { VISUAL_BASELINE_WORKBENCH_STATES } from "../visual-baseline-contract.ts";
import { visualBaselineWorkbenchContent } from "./visual-baseline-workbench.tsx";

describe("visual-baseline shared workbench catalog", () => {
  test("every catalog page renders through the real shared components", () => {
    for (const state of VISUAL_BASELINE_WORKBENCH_STATES) {
      const html = renderToStaticMarkup(visualBaselineWorkbenchContent(state));
      expect(html.length, state).toBeGreaterThan(100);
    }
  });

  test("the captured pages consume every shared #8870 fixture", () => {
    const html = VISUAL_BASELINE_WORKBENCH_STATES.map((state) =>
      renderToStaticMarkup(visualBaselineWorkbenchContent(state)),
    ).join("\n");
    const keys = [
      ...desktopTimelineMessageFixtures.map((fixture) => fixture.itemKey),
      desktopReasoningDisclosureFixtures.streaming.itemKey,
      desktopReasoningDisclosureFixtures.completed.itemKey,
      ...desktopCommandCardFixtures.map((fixture) => fixture.props.itemKey),
      ...desktopFileChangeCardFixtures.map((fixture) => fixture.props.itemKey),
      ...desktopToolCallCardFixtures.map((fixture) => fixture.props.itemKey),
      ...desktopPlanCardFixtures.map((fixture) => fixture.itemKey),
      ...desktopApprovalCardStaticFixtures.map((fixture) => fixture.itemKey),
      desktopApprovalCardInteractiveFixture.itemKey,
      ...desktopAgentGroupFixtures.map((fixture) => fixture.itemKey),
      ...desktopContextMeterFixtures.map((fixture) => fixture.props.itemKey),
      ...desktopTimelineNoticeFixtures.map((fixture) => fixture.itemKey),
      ...desktopDispatchLongTailFixtures.map((fixture) => fixture.itemKey),
    ];
    for (const key of keys) expect(html, key).toContain(key);
  });
});
