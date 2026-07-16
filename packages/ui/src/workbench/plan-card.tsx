import { Check, CircleDot, Circle, GitBranch } from "lucide-react"
import type { ReactElement } from "react"

export type DesktopPlanEntry = Readonly<{
  step: string
  status: "completed" | "in_progress" | "pending"
}>

export const DesktopPlanCard = ({ entries, itemKey, title = "Plan" }: Readonly<{
  entries: ReadonlyArray<DesktopPlanEntry>
  itemKey: string
  title?: string | undefined
}>): ReactElement => {
  const completed = entries.filter(entry => entry.status === "completed").length
  return <article className="oa-react-plan oa-react-plan-card" data-kind="plan" data-timeline-key={itemKey} role="listitem">
    <header>
      <span className="oa-react-event-title"><GitBranch aria-hidden="true" /><strong>{title}</strong></span>
      <span>{completed} of {entries.length} done</span>
    </header>
    <ol className="oa-react-plan-list">
      {entries.map((entry, index) => <li data-status={entry.status} key={`${itemKey}:${index}`}>
        <span className="oa-react-plan-glyph">{entry.status === "completed" ? <Check aria-hidden="true" /> : entry.status === "in_progress" ? <CircleDot aria-hidden="true" /> : <Circle aria-hidden="true" />}</span>
        <span>{entry.step}</span>
        <small>{entry.status === "completed" ? "Done" : entry.status === "in_progress" ? "In progress" : "Pending"}</small>
      </li>)}
    </ol>
  </article>
}
