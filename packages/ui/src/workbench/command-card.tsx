import { Terminal } from "lucide-react"
import type { ReactElement } from "react"

import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopProtocolCard } from "./protocol-card.tsx"

export const DesktopCommandCard = ({
  command,
  commandSource,
  cwd,
  defaultOpen,
  durationMs,
  exitCode,
  itemKey,
  output = "",
  outputCapReached = false,
  status,
}: Readonly<{
  command: string
  commandSource?: "agent" | "userShell" | "unifiedExecStartup" | "unifiedExecInteraction" | undefined
  cwd?: string | undefined
  defaultOpen?: boolean | undefined
  durationMs?: number | undefined
  exitCode?: number | null | undefined
  itemKey: string
  output?: string | undefined
  outputCapReached?: boolean | undefined
  status: DesktopActivityStatus
}>): ReactElement => {
  const headerMeta = [
    ...(exitCode === undefined ? [] : [`EXIT: ${exitCode === null ? "—" : exitCode}`]),
    ...(durationMs === undefined ? [] : [`${Math.max(0, Math.round(durationMs))}MS`]),
  ].join(" · ")
  const source = commandSource === "userShell" ? "USER SHELL"
    : commandSource === "unifiedExecStartup" ? "UNIFIED EXEC STARTUP"
      : commandSource === "unifiedExecInteraction" ? "UNIFIED EXEC INTERACTION"
        : commandSource === "agent" ? "AGENT" : undefined
  const visibleOutput = output === ""
    ? status === "running" ? "Waiting for output…" : "No output recorded."
    : output
  return <DesktopProtocolCard
  body={<>
    <div className="oa-react-command-meta">
      <span>CWD</span><code>{cwd ?? "—"}</code>
      <span>SOURCE</span><code>{source ?? "—"}</code>
    </div>
    <pre className="oa-react-command-output" data-output-capped={outputCapReached ? "true" : "false"}><code>{visibleOutput}</code></pre>
    {outputCapReached ? <small className="oa-react-command-cap" role="status">Earlier output omitted · showing bounded tail</small> : null}
  </>}
  defaultOpen={defaultOpen ?? status === "running"}
  icon={Terminal}
  itemKey={itemKey}
  meta={headerMeta === "" ? undefined : headerMeta}
  status={status}
  summary={<code>{command}</code>}
  title="Command"
  variant="commandExecution"
/>
}
