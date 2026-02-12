import { describe, expect, it } from "vitest";

import {
  hasAnyHostedOpsPaneOpen,
  l402PaneRenderBranch,
  makeInitialL402PaneState,
  paneButtonVisualState,
  rejectL402PaneState,
  resolveL402PaneState,
  startL402PaneLoading,
} from "../../src/effuse-app/controllers/home/l402OpsPaneState";

describe("apps/web l402 hosted pane state helpers", () => {
  it("handles loading, empty, and data render branches", () => {
    const initial = makeInitialL402PaneState<{ id: string }>();
    expect(l402PaneRenderBranch(initial)).toBe("empty");

    const loading = startL402PaneLoading(initial);
    expect(loading.loadState).toBe("loading");
    expect(l402PaneRenderBranch(loading)).toBe("loading");

    const resolvedEmpty = resolveL402PaneState<{ id: string }>({
      rows: [],
      requestId: "req-empty",
      updatedAtMs: 10,
    });
    expect(l402PaneRenderBranch(resolvedEmpty)).toBe("empty");

    const resolvedData = resolveL402PaneState({
      rows: [{ id: "pw_1" }],
      requestId: "req-data",
      updatedAtMs: 20,
    });
    expect(l402PaneRenderBranch(resolvedData)).toBe("data");
  });

  it("preserves stale rows on failure for actionable non-silent errors", () => {
    const resolved = resolveL402PaneState({
      rows: [{ id: "set_1" }],
      requestId: "req-ok",
      updatedAtMs: 30,
    });

    const failed = rejectL402PaneState({
      previous: resolved,
      errorText: "forbidden",
      updatedAtMs: 40,
    });

    expect(failed.loadState).toBe("error");
    expect(failed.errorText).toBe("forbidden");
    expect(failed.rows).toHaveLength(1);
    expect(l402PaneRenderBranch(failed)).toBe("data");
  });

  it("derives pane button and hosted-open toggle state", () => {
    const closed = paneButtonVisualState(false);
    expect(closed.ariaPressed).toBe("false");
    expect(closed.opacity).toBe("0.82");

    const open = paneButtonVisualState(true);
    expect(open.ariaPressed).toBe("true");
    expect(open.opacity).toBe("1");

    expect(
      hasAnyHostedOpsPaneOpen({
        paywallsOpen: false,
        settlementsOpen: false,
        deploymentsOpen: false,
      }),
    ).toBe(false);

    expect(
      hasAnyHostedOpsPaneOpen({
        paywallsOpen: false,
        settlementsOpen: true,
        deploymentsOpen: false,
      }),
    ).toBe(true);
  });
});
