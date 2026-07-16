import type { DesktopApprovalDecision } from "./approval-card.tsx";

/**
 * Fixture set for `DesktopApprovalCard` (issue 8870, epic 8857 T13 gallery
 * lane).
 *
 * Three of the four acceptance states are plain data (no callback needed):
 * `approved`, `denied`, and a read-only historical `pending` row — the exact
 * shape `dispatchWorkbenchItem`'s "approval" branch renders for a history
 * record that has no live answer channel (see `dispatch.tsx`'s
 * `toApprovalDecision`; it never passes `onDecision`, so the card falls
 * through to its own neutral "Pending" indicator).
 *
 * The fourth state, "pending-interactive", is deliberately NOT data-only:
 * a real Approve/Deny decision needs a closure over page-local state, so the
 * `/components` gallery instantiates that one directly with `useState` and
 * `onDecision`, reusing only the description/resource/title fields below.
 */
export type DesktopApprovalCardStaticFixture = Readonly<{
  name: string;
  itemKey: string;
  decision: DesktopApprovalDecision;
  decisionLabel?: string;
  description: string;
  resource: string;
  title: string;
}>;

export const desktopApprovalCardStaticFixtures: ReadonlyArray<DesktopApprovalCardStaticFixture> = [
  {
    name: "approved",
    itemKey: "fixture-approval-approved",
    decision: "approved",
    description: "Recorded decision.",
    resource: "apply_patch: packages/ui/src/workbench/plan-card.tsx",
    title: "Apply file change",
  },
  {
    name: "denied",
    itemKey: "fixture-approval-denied",
    decision: "denied",
    description: "Recorded decision.",
    resource: "shell: rm -rf node_modules/.cache",
    title: "Run command",
  },
  {
    name: "read-only historical (pending, no interactive channel)",
    itemKey: "fixture-approval-readonly-pending",
    decision: "pending",
    description: "This history record has no live answer channel; it can only be inspected.",
    resource: "shell: curl https://internal.example/deploy-hook",
    title: "Run command",
  },
];

/** Base fields the gallery wires up interactively for the pending-interactive story. */
export const desktopApprovalCardInteractiveFixture = {
  itemKey: "fixture-approval-pending-interactive",
  description: "Awaiting a decision.",
  resource: "shell: git push --force origin release/2026-07",
  title: "Run command",
} as const;
