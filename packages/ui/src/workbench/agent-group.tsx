import { Bot, GitBranch, Network } from "lucide-react"
import { useState, type ReactElement } from "react"

import { activityStatusIcon, activityStatusLabel } from "./activity-status.tsx"

export type DesktopAgentStatus = "completed" | "failed" | "running" | "waiting"

/** subAgentActivity.kind — a ping distinct from the row's own lifecycle status. */
export type DesktopAgentActivityKind = "started" | "interacted" | "interrupted"

export type DesktopAgentActivity = Readonly<{
  agentKey: string
  depth?: number
  detail: string
  name: string
  parent?: string
  /** Nickname/path (subAgentActivity.agentPath, or any caller-resolved friendly label). */
  path?: string
  role: string
  status: DesktopAgentStatus
  /** Exact wire status text override (e.g. "PENDING INIT", "NOT FOUND") — falls back to the coarse `status` label. */
  statusLabel?: string
  /** subAgentActivity.kind, when this row represents an activity ping rather than a lifecycle transition. */
  activityKind?: DesktopAgentActivityKind
  transcript?: ReadonlyArray<Readonly<{ label: string; text: string }>>
  /** True only when a running agent can currently be interrupted (caller decides eligibility). */
  interruptable?: boolean
  /** Present together with `interruptable` to offer the Interrupt control (desktop-only; never assumed by web hosts). */
  onInterrupt?: () => void
}>

const DesktopAgentRow = ({ agent }: Readonly<{ agent: DesktopAgentActivity }>): ReactElement => {
  const [open, setOpen] = useState((agent.transcript?.length ?? 0) > 0)
  const statusLabel = agent.statusLabel ?? activityStatusLabel(agent.status)
  return <details
    className="oa-react-agent-card"
    data-depth={agent.depth ?? 0}
    data-status={agent.status}
    onToggle={event => setOpen(event.currentTarget.open)}
    open={open}
    role="listitem"
  >
    <summary>
      <span className="oa-react-agent-avatar"><Bot aria-hidden="true" /></span>
      <span className="oa-react-agent-heading">
        <strong>{agent.name}</strong>
        <small>{agent.role}</small>
        {agent.path === undefined || agent.path === agent.name ? null
          : <small className="oa-react-agent-path">{agent.path}</small>}
      </span>
      <span className="oa-react-agent-task">
        {agent.detail}
        {agent.activityKind === undefined ? null
          : <span className="oa-react-agent-activity-tag" data-activity={agent.activityKind}>{agent.activityKind}</span>}
      </span>
      <span className="oa-react-event-status" data-status={agent.status}>{activityStatusIcon(agent.status)}{statusLabel}</span>
      {agent.interruptable === true && agent.onInterrupt !== undefined
        ? <button
            className="oa-react-agent-interrupt"
            type="button"
            aria-label={`Interrupt ${agent.name}`}
            onClick={event => { event.preventDefault(); event.stopPropagation(); agent.onInterrupt!() }}
          >Interrupt</button>
        : null}
    </summary>
    {agent.transcript === undefined || agent.transcript.length === 0 ? null : <div className="oa-react-agent-transcript">
      {agent.parent === undefined ? null : <p className="oa-react-agent-parent"><GitBranch aria-hidden="true" />spawned by {agent.parent}</p>}
      {agent.transcript.map((line, index) => <p key={`${agent.agentKey}:line:${index}`}><strong>{line.label}</strong><span>{line.text}</span></p>)}
    </div>}
  </details>
}

export const DesktopAgentGroup = ({ agents, itemKey, operation, prompt, title = "Delegated agents" }: Readonly<{
  agents: ReadonlyArray<DesktopAgentActivity>
  itemKey: string
  /** Short operation verb (already lower/mixed-case; rendered as `[SPAWN]`/`[SEND]`/etc — collabAgentToolCall.tool). */
  operation?: string | undefined
  /** Bounded operation prompt line (collabAgentToolCall.prompt), shown once for the whole group. */
  prompt?: string | undefined
  title?: string | undefined
}>): ReactElement => {
  const completed = agents.filter(agent => agent.status === "completed").length
  const running = agents.filter(agent => agent.status === "running").length
  return <section className="oa-react-agent-group" data-kind="collabAgentToolCall" data-timeline-key={itemKey} role="listitem">
    <header>
      <span className="oa-react-event-title"><Network aria-hidden="true" /><strong>{title}</strong></span>
      {operation === undefined ? null : <span className="oa-react-agent-operation" data-operation={operation}>{operation}</span>}
      <span>{completed} done{running > 0 ? ` · ${running} running` : ""}</span>
    </header>
    {prompt === undefined || prompt === "" ? null : <p className="oa-react-agent-prompt"><span>Prompt</span>{prompt}</p>}
    <div className="oa-react-agent-list" role="list">
      {agents.map(agent => <DesktopAgentRow agent={agent} key={agent.agentKey} />)}
    </div>
  </section>
}
