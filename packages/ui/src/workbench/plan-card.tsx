import { Check, CircleDot, Circle, GitBranch } from "lucide-react"
import type { ReactElement } from "react"

export type DesktopPlanEntry = Readonly<{
  step: string
  status: "completed" | "in_progress" | "pending"
}>

/** Quantized block-progress strip segment count (design spec §5.2/§7, the
 * SAME motif as the "Working" indicator's `.oa-react-working` blocks —
 * discrete rectangles, sharp corners, accent fill vs muted unfilled — but a
 * static completed/total ratio here rather than a busy-pulse animation. */
const PLAN_PROGRESS_SEGMENTS = 10

const statusLabel = (status: DesktopPlanEntry["status"]): string =>
  status === "completed" ? "Done" : status === "in_progress" ? "In progress" : "Pending"

/**
 * The ONE plan renderer (T8 #8865, epic #8857 Wave 2): every plan source —
 * the live `turn/plan/updated` notification, the `plan` ThreadItem
 * (collaboration-mode prose write-ups, previously dropped), and history
 * `plan`/`todo_list` rows — projects into this same component through
 * `dispatchWorkbenchItem`'s "plan" branch. `entries` carries the structured
 * step checklist; `prose` carries free-form narrative; a plan item may carry
 * either or both.
 *
 * Event-ledger grammar (design spec): STEP rows with a right-aligned
 * `[STATUS]` tag in mono; the active/in_progress row is emphasized ONLY by
 * luminance (full-brightness text, no bold, no extra color) so completed
 * rows sit one step dimmer and pending rows dimmer still — a grey ladder, not
 * weight or hue. `itemKey` is the caller's STABLE per-plan key (one per turn
 * for live plans, one per history row) — reused unchanged across in-place
 * updates so React never remounts the card while new entries stream in.
 */
export const DesktopPlanCard = ({ entries, itemKey, prose, title = "Plan" }: Readonly<{
  entries: ReadonlyArray<DesktopPlanEntry>
  itemKey: string
  /** Free-form plan narrative (T8 #8865). Shown above the checklist, or
   * alone when there are no structured entries yet. */
  prose?: string | undefined
  title?: string | undefined
}>): ReactElement => {
  const completed = entries.filter(entry => entry.status === "completed").length
  const total = entries.length
  const filledSegments = total === 0 ? 0 : Math.round((completed / total) * PLAN_PROGRESS_SEGMENTS)
  return <article className="oa-react-plan oa-react-plan-card" data-kind="plan" data-timeline-key={itemKey} role="listitem">
    <header>
      <span className="oa-react-event-title"><GitBranch aria-hidden="true" /><strong>{title}</strong></span>
      {total > 0
        ? <span className="oa-react-plan-summary">
            <span>{completed} of {total} done</span>
            <span className="oa-react-plan-progress" aria-hidden="true">
              {Array.from({ length: PLAN_PROGRESS_SEGMENTS }, (_, index) =>
                <i data-filled={index < filledSegments} key={`${itemKey}-progress-${index}`} />)}
            </span>
          </span>
        : null}
    </header>
    {prose === undefined || prose === "" ? null : <p className="oa-react-plan-prose">{prose}</p>}
    {total === 0 ? null : <ol className="oa-react-plan-list">
      {entries.map((entry, index) => <li data-status={entry.status} key={`${itemKey}:${index}`}>
        <span className="oa-react-plan-glyph">{entry.status === "completed" ? <Check aria-hidden="true" /> : entry.status === "in_progress" ? <CircleDot aria-hidden="true" /> : <Circle aria-hidden="true" />}</span>
        <span>{entry.step}</span>
        <small>{statusLabel(entry.status)}</small>
      </li>)}
    </ol>}
  </article>
}
