import type { DesktopPlanEntry } from "./plan-card.tsx";

/**
 * Fixture set for `DesktopPlanCard` (issue 8870, epic 8857 T13 gallery lane).
 *
 * Covers the three plan-carrying sources unified by T8 (#8865): a structured
 * checklist mid-flight (`turn/plan/updated`), a fully completed checklist
 * (history), and free-form prose alone (the `plan` ThreadItem's
 * collaboration-mode write-up, which may carry no structured `entries` at
 * all).
 */
export type DesktopPlanCardFixture = Readonly<{
  name: string;
  itemKey: string;
  entries: ReadonlyArray<DesktopPlanEntry>;
  prose?: string;
}>;

export const desktopPlanCardFixtures: ReadonlyArray<DesktopPlanCardFixture> = [
  {
    name: "in progress (streaming update — one step active)",
    itemKey: "fixture-plan-streaming",
    entries: [
      { step: "Read the epic 8857 component audit", status: "completed" },
      { step: "Wire the reasoning delta into the timeline", status: "completed" },
      { step: "Add the context meter to the header", status: "in_progress" },
      { step: "Render every workbench variant in /components", status: "pending" },
      { step: "Write the completeness gate test", status: "pending" },
    ],
  },
  {
    name: "complete (all steps done)",
    itemKey: "fixture-plan-complete",
    entries: [
      { step: "Split desktop-workbench.tsx into per-component modules", status: "completed" },
      { step: "Restyle the shared CSS onto the Autopilot grammar", status: "completed" },
      { step: "Wire one component per wire type", status: "completed" },
    ],
  },
  {
    name: "prose only (collaboration-mode write-up, no checklist yet)",
    itemKey: "fixture-plan-prose",
    entries: [],
    prose:
      "I'll start by auditing which Wave-2 lanes already exported fixtures, then extend the family with the remaining component states before wiring the completeness gate.",
  },
];
