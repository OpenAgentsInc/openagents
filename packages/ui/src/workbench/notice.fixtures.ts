import type { DesktopTimelineNoticeSeverity } from "./notice.tsx";

/**
 * Fixture set for `DesktopTimelineNotice` (issue 8870, epic 8857 T13 gallery
 * lane). Covers the three typed severities plus the legacy `danger` boolean
 * shape that pre-#8869 history-projected error/lifecycle rows still pass.
 */
export type DesktopTimelineNoticeFixture = Readonly<{
  name: string;
  itemKey: string;
  label: string;
  body: string;
  severity?: DesktopTimelineNoticeSeverity;
  danger?: boolean;
}>;

export const desktopTimelineNoticeFixtures: ReadonlyArray<DesktopTimelineNoticeFixture> = [
  {
    name: "info",
    itemKey: "fixture-notice-info",
    label: "Model rerouted",
    body: "Rerouted from gpt-5.1-codex to gpt-5.1-codex-mini for the remainder of this turn.",
    severity: "info",
  },
  {
    name: "warning",
    itemKey: "fixture-notice-warning",
    label: "Deprecation notice",
    body: "The `unified_exec` startup path will require an explicit shell in a future release.",
    severity: "warning",
  },
  {
    name: "error",
    itemKey: "fixture-notice-error",
    label: "Turn error",
    body: "The app-server process exited unexpectedly before the turn completed.",
    severity: "error",
  },
  {
    name: "legacy danger=true (pre-severity history row)",
    itemKey: "fixture-notice-legacy-danger",
    label: "Error",
    body: "Guardian review rejected this action before it could run.",
    danger: true,
  },
];
