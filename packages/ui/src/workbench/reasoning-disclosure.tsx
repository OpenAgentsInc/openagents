/**
 * `DesktopReasoningDisclosure` (#8863, epic #8857 Wave 2 / T6: streaming
 * reasoning disclosure).
 *
 * Renders a `reasoning` `WorkbenchItem` at the Autopilot ghost-text luminance
 * level (design spec `docs/fable/autopilot-ui-design-spec.md` §2.2
 * `text-ghost`/`text-faint`, folded into the mounted Khala theme via the
 * existing `--en-color-textFaint`/`--en-color-textMuted` roles per the
 * `openagents_desktop.design.khala_autopilot_foldin.v1` contract — this
 * component never imports the raw `autopilotTheme` palette or hardcodes an
 * Autopilot hex value).
 *
 * Two presentation states:
 *   - `status: "in_progress"` — the model is still thinking. The card stays
 *     OPEN and the growing summary streams in as dim ghost text, chunk by
 *     chunk, with a small pulsing indicator. There is nothing bounded to
 *     collapse to yet.
 *   - anything else (`"completed"`/`"failed"`/`"declined"`, i.e. the item
 *     finished) — the card COLLAPSES to a single bounded summary line, the
 *     same fold every other "work" entry in the timeline uses
 *     (`DesktopWorkEntry`, `DesktopProtocolCard`). The full text stays
 *     reachable behind the disclosure triangle; it is never left permanently
 *     expanded as a wall of streamed reasoning.
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
import type { ReactElement } from "react"
import { useEffect, useState } from "react"

export type DesktopReasoningStatus = "in_progress" | "completed" | "failed" | "declined"

const firstNonEmptyLine = (value: string): string =>
  value.split("\n").find(line => line.trim() !== "")?.trim() ?? ""

export const DesktopReasoningDisclosure = ({
  itemKey,
  status,
  summary,
}: Readonly<{
  itemKey: string
  status: DesktopReasoningStatus
  summary: string
}>): ReactElement => {
  const running = status === "in_progress"
  const [open, setOpen] = useState(running)
  // Collapse-on-complete: the moment the item stops running, fold it down to
  // the bounded summary line regardless of whatever the user last toggled —
  // "streams while thinking, then collapses" is unconditional on completion.
  useEffect(() => {
    if (!running) setOpen(false)
  }, [running])
  const preview = running
    ? (firstNonEmptyLine(summary) || "Thinking…")
    : (firstNonEmptyLine(summary) || "No summary")
  const paragraphs = summary.split("\n").filter(line => line.trim() !== "")
  return <details
    className="oa-react-reasoning-disclosure"
    data-status={running ? "running" : "completed"}
    data-timeline-key={itemKey}
    onToggle={event => setOpen(event.currentTarget.open)}
    open={open}
    role="listitem"
  >
    <summary>
      <span className="oa-react-reasoning-label">Reasoning</span>
      <span className="oa-react-reasoning-preview">{preview}</span>
      {running ? <span aria-hidden="true" className="oa-react-reasoning-pulse" /> : null}
    </summary>
    <div className="oa-react-reasoning-body">
      {paragraphs.length === 0
        ? null
        : paragraphs.map((line, index) => <p key={index}>{line}</p>)}
    </div>
  </details>
}

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
