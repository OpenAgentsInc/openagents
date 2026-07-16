import type { ComponentProps } from "react";

import type { DesktopFileChangeCard } from "./file-change-card.tsx";

export type DesktopFileChangeCardFixture = Readonly<{
  name: string;
  props: ComponentProps<typeof DesktopFileChangeCard>;
}>;

/** Shared file-change lifecycle fixtures used by the web gallery and QA-3 pixels. */
export const desktopFileChangeCardFixtures: ReadonlyArray<DesktopFileChangeCardFixture> = [
  {
    name: "running turn diff",
    props: {
      itemKey: "fixture-file-turn-running",
      scope: "turn",
      status: "running",
      defaultOpen: true,
      changes: [
        {
          path: "src/workbench.tsx",
          kind: "update",
          additions: 7,
          deletions: 2,
          diff: "@@ -18,2 +18,7 @@\n-old surface\n+const surface = khalaTheme\n+render(surface)",
        },
        {
          path: "src/file-card.test.tsx",
          kind: "add",
          additions: 4,
          deletions: 0,
          diff: "--- /dev/null\n+++ b/src/file-card.test.tsx\n+test('renders diffs', () => {\n+  expect(card).toBeVisible()\n+})",
        },
      ],
    },
  },
  {
    name: "applied patch",
    props: {
      itemKey: "fixture-file-completed",
      scope: "item",
      status: "completed",
      defaultOpen: true,
      changes: [
        {
          path: "docs/design-contract.md",
          kind: "update",
          additions: 2,
          deletions: 1,
          diff: "@@ -4,1 +4,2 @@\n-Autopilot palette\n+Khala is the sole mounted theme.\n+Autopilot is donor grammar only.",
        },
      ],
    },
  },
  {
    name: "failed patch",
    props: {
      itemKey: "fixture-file-failed",
      scope: "item",
      status: "failed",
      defaultOpen: true,
      changes: [
        {
          path: "src/conflicted.ts",
          kind: "delete",
          additions: 0,
          deletions: 2,
          diff: "@@ -1,2 +0,0 @@\n-export const stale = true\n-export const duplicate = true",
        },
      ],
    },
  },
  {
    name: "bounded diff",
    props: {
      itemKey: "fixture-file-capped",
      scope: "item",
      status: "completed",
      defaultOpen: true,
      changes: [
        {
          path: "src/generated.ts",
          kind: "update",
          additions: 48,
          deletions: 12,
          diff: "@@ -400,2 +400,3 @@\n-old generated tail\n+new generated tail\n+…",
          diffCapReached: true,
        },
      ],
    },
  },
];
