import { Pause } from "lucide-react"
import type { ReactElement } from "react"

export const DesktopQueuedFollowup = ({ itemKey, position, text }: Readonly<{
  itemKey: string
  position: number
  text: string
}>): ReactElement => <article className="oa-react-queue-card" data-kind="queue" data-timeline-key={itemKey} role="listitem">
  <Pause aria-hidden="true" /><strong>Queued follow-up (#{position})</strong><span>{text}</span><small>Runs when this turn completes</small>
</article>
