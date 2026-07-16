import type { WorkbenchDispatchItem } from "./dispatch.tsx";

/**
 * Fixture set for `dispatchWorkbenchItem` (issue 8870, epic 8857 T13 gallery
 * lane). Two kinds of coverage live here rather than beside a standalone
 * component:
 *
 *  - the four long-tail ledger rows (T12 #8869) — `compaction`, `sleep`,
 *    `review` (entered/exited), and `hook` — which are rendered as inline
 *    honest mono rows directly inside `dispatchWorkbenchItem`'s switch, with
 *    no dedicated `Desktop*` component of their own to hang a fixture off of;
 *  - the `command` "declined" state, which is a `WorkbenchDispatchStatus`
 *    value with no matching `DesktopActivityStatus` (declined maps onto the
 *    same "failed" visual tone via `toActivityStatus`) — the honest way to
 *    demonstrate it is through the dispatch table, not a direct
 *    `DesktopCommandCard` prop that does not exist.
 */
export type DesktopDispatchFixture = Readonly<{
  name: string;
  itemKey: string;
  item: WorkbenchDispatchItem;
}>;

export const desktopDispatchLongTailFixtures: ReadonlyArray<DesktopDispatchFixture> = [
  {
    name: "context compacted",
    itemKey: "fixture-dispatch-compaction",
    item: { kind: "compaction", source: "codex" },
  },
  {
    name: "sleep",
    itemKey: "fixture-dispatch-sleep",
    item: { kind: "sleep", source: "codex", durationMs: 4_500 },
  },
  {
    name: "entered review mode",
    itemKey: "fixture-dispatch-review-entered",
    item: {
      kind: "review",
      source: "codex",
      phase: "entered",
      review: "Reviewing the workbench-family gallery changes before merge.",
    },
  },
  {
    name: "exited review mode",
    itemKey: "fixture-dispatch-review-exited",
    item: { kind: "review", source: "codex", phase: "exited", review: "" },
  },
  {
    name: "hook prompt",
    itemKey: "fixture-dispatch-hook",
    item: {
      kind: "hook",
      source: "codex",
      text: "pre-commit: running lint-staged on 4 changed files",
    },
  },
  {
    name: "command — declined",
    itemKey: "fixture-dispatch-command-declined",
    item: {
      kind: "command",
      source: "codex",
      command: "rm -rf /tmp/scratch",
      cwd: "/workspace/openagents",
      status: "declined",
    },
  },
];
