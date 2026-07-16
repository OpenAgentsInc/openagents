/**
 * `DesktopReasoningDisclosure` (#8863, epic #8857 Wave 2 / T6: streaming
 * reasoning presentation).
 *
 * Renders a `reasoning` `WorkbenchItem` at the Autopilot ghost-text luminance
 * level (design spec `docs/fable/autopilot-ui-design-spec.md` §2.2
 * `text-ghost`/`text-faint`, folded into the mounted Khala theme via the
 * existing `--en-color-textFaint`/`--en-color-textMuted` roles per the
 * `openagents_desktop.design.khala_autopilot_foldin.v1` contract — this
 * component never imports the raw `autopilotTheme` palette or hardcodes an
 * Autopilot hex value).
 *
 * The bounded summary is the entire visible presentation. The host may pass
 * its safe Markdown projection as `children`; component-gallery callers fall
 * back to literal summary text. There is deliberately no title, preview,
 * disclosure control, pulse, or status commentary around that content.
 *
 * Honest-absence rule (design spec + component audit §5): redacted reasoning
 * NEVER reaches this component. The upstream typed projection
 * (`workbenchItemFromThreadItem` in `workbench-item-contract.ts`, consumed
 * by both the live Codex app-server turn client and `codex-history.ts`)
 * returns no item at all when the bounded summary is empty, and
 * `react-timeline.tsx` additionally drops `kind === "reasoning" && redacted`
 * rows before they are ever handed to `dispatchWorkbenchItem`. So this
 * component never renders a false "reasoning unavailable" card — for a
 * redacted item, it simply never mounts.
 */
import type { ReactElement, ReactNode } from "react"

export type DesktopReasoningStatus = "in_progress" | "completed" | "failed" | "declined"

export const DesktopReasoningDisclosure = ({
  children,
  itemKey,
  status,
  summary,
}: Readonly<{
  children?: ReactNode
  itemKey: string
  status: DesktopReasoningStatus
  summary: string
}>): ReactElement =>
  <div
    className="oa-react-reasoning-disclosure"
    data-status={status === "in_progress" ? "running" : "completed"}
    data-timeline-key={itemKey}
    role="listitem"
  >
    {children ?? summary}
  </div>

/**
 * Representative fixture data for every reasoning presentation state — the
 * #8870 `/components` workbench-family lane wires these into the gallery.
 * There is deliberately no "redacted" fixture here: a redacted reasoning
 * item never becomes a `WorkbenchReasoningDispatchItem` in the first place
 * (see the honest-absence note above), so the honest fixture for that state
 * is simply rendering nothing.
 */
export const desktopReasoningDisclosureFixtures = {
  streaming: {
    itemKey: "fixture-reasoning-streaming",
    status: "in_progress" as const,
    summary: "Checking whether the cached session token is still valid before making another network call",
  },
  completed: {
    itemKey: "fixture-reasoning-completed",
    status: "completed" as const,
    summary: "Confirmed the token had expired, so a fresh device-auth flow was required before continuing.",
  },
} satisfies Record<string, Readonly<{ itemKey: string; status: DesktopReasoningStatus; summary: string }>>
