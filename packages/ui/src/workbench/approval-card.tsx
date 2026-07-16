import { Check, Shield, XCircle } from "lucide-react"
import type { ReactElement } from "react"

export type DesktopApprovalDecision = "approved" | "denied" | "pending"

/**
 * One custom action button for a pending decision that offers more than the
 * default binary Approve/Deny choice (e.g. a Codex plan review's Accept /
 * Request changes / Replan). When `actions` is supplied on the card it
 * REPLACES the default two-button pair; `onDecision` stays the simple binary
 * path used by ordinary tool/file approvals.
 */
export type DesktopApprovalAction = Readonly<{
  key: string
  label: string
  primary?: boolean
  onSelect: () => void
}>

/**
 * The trailing action/decision slot, computed with an explicit priority
 * chain rather than a nested ternary so a truly read-only pending card
 * (no `actions`, no `onDecision` — e.g. the answer bridge is unavailable)
 * renders its own neutral "Pending" indicator instead of falling through to
 * the resolved badge, which only knows "Approved"/"Denied" wording and would
 * otherwise mislabel an unresolved card as denied.
 */
const approvalFooter = (props: Readonly<{
  actions: ReadonlyArray<DesktopApprovalAction> | undefined
  decision: DesktopApprovalDecision
  decisionLabel: string | undefined
  onDecision: ((decision: Exclude<DesktopApprovalDecision, "pending">) => void) | undefined
}>): ReactElement => {
  if (props.decision === "pending" && props.actions !== undefined) {
    return <div className="oa-react-approval-actions">
      {props.actions.map(action => <button
        data-primary={action.primary === true ? "true" : undefined}
        key={action.key}
        onClick={action.onSelect}
        type="button"
      >{action.label}</button>)}
    </div>
  }
  if (props.decision === "pending" && props.onDecision !== undefined) {
    const onDecision = props.onDecision
    return <div className="oa-react-approval-actions">
      <button onClick={() => onDecision("denied")} type="button">Deny</button>
      <button data-primary="true" onClick={() => onDecision("approved")} type="button">Approve</button>
    </div>
  }
  if (props.decision === "pending") {
    return <span className="oa-react-approval-decision" data-decision="pending">Pending</span>
  }
  return <span className="oa-react-approval-decision" data-decision={props.decision}>
    {props.decision === "approved" ? <Check aria-hidden="true" /> : <XCircle aria-hidden="true" />}
    {props.decisionLabel ?? (props.decision === "approved" ? "Approved" : "Denied")}
  </span>
}

export const DesktopApprovalCard = ({
  actions,
  decision,
  decisionLabel,
  description,
  itemKey,
  onDecision,
  resource,
  title,
}: Readonly<{
  /**
   * Custom action buttons for a pending decision with more than two outcomes
   * (e.g. plan review). When present, these render INSTEAD of the default
   * Approve/Deny pair; `onDecision` is ignored while `actions` is supplied.
   */
  actions?: ReadonlyArray<DesktopApprovalAction>
  decision: DesktopApprovalDecision
  /** Resolved-state label override; defaults to "Approved"/"Denied". Lets a
   * non-binary outcome (e.g. "Changes requested") report its real outcome
   * instead of being forced into the binary wording. */
  decisionLabel?: string
  description: string
  itemKey: string
  onDecision?: (decision: Exclude<DesktopApprovalDecision, "pending">) => void
  resource: string
  title: string
}>): ReactElement => <article className="oa-react-approval-card" data-decision={decision} data-kind="approval" data-timeline-key={itemKey} role="listitem">
  <span className="oa-react-event-icon"><Shield aria-hidden="true" /></span>
  <div><strong>{title}</strong><p>{description}</p>{resource === "" ? null : <code>{resource}</code>}</div>
  {approvalFooter({ actions, decision, decisionLabel, onDecision })}
</article>
