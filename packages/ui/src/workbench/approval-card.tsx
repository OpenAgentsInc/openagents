import { Check, Shield, XCircle } from "lucide-react"
import type { ReactElement } from "react"

export type DesktopApprovalDecision = "approved" | "denied" | "pending"

export const DesktopApprovalCard = ({ decision, description, itemKey, onDecision, resource, title }: Readonly<{
  decision: DesktopApprovalDecision
  description: string
  itemKey: string
  onDecision?: (decision: Exclude<DesktopApprovalDecision, "pending">) => void
  resource: string
  title: string
}>): ReactElement => <article className="oa-react-approval-card" data-decision={decision} data-kind="approval" data-timeline-key={itemKey} role="listitem">
  <span className="oa-react-event-icon"><Shield aria-hidden="true" /></span>
  <div><strong>{title}</strong><p>{description}</p><code>{resource}</code></div>
  {decision === "pending" && onDecision !== undefined ? <div className="oa-react-approval-actions">
    <button onClick={() => onDecision("denied")} type="button">Deny</button>
    <button data-primary="true" onClick={() => onDecision("approved")} type="button">Approve</button>
  </div> : <span className="oa-react-approval-decision" data-decision={decision}>{decision === "approved" ? <Check aria-hidden="true" /> : <XCircle aria-hidden="true" />}{decision === "approved" ? "Approved" : "Denied"}</span>}
</article>
