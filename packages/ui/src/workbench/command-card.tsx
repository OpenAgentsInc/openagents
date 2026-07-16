import { Terminal } from "lucide-react"
import type { ReactElement, ReactNode } from "react"

import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopProtocolCard } from "./protocol-card.tsx"

export const DesktopCommandCard = ({ command, cwd, defaultOpen, itemKey, output, status }: Readonly<{
  command: string
  cwd: string
  defaultOpen?: boolean | undefined
  itemKey: string
  output: ReactNode
  status: DesktopActivityStatus
}>): ReactElement => <DesktopProtocolCard
  body={<><div className="oa-react-command-meta"><span>cwd</span><code>{cwd}</code></div><pre><code>{output}</code></pre></>}
  defaultOpen={defaultOpen}
  icon={Terminal}
  itemKey={itemKey}
  status={status}
  summary={<code>{command}</code>}
  title="Command"
  variant="commandExecution"
/>
