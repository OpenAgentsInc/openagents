import { Globe, Image, Network, Wrench, type LucideIcon } from "lucide-react"
import type { ReactElement, ReactNode } from "react"

import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopProtocolCard } from "./protocol-card.tsx"

export type DesktopToolKind = "dynamic" | "image" | "mcp" | "web"

const toolIcons: Readonly<Record<DesktopToolKind, LucideIcon>> = {
  dynamic: Wrench,
  image: Image,
  mcp: Network,
  web: Globe,
}

export const DesktopToolCallCard = ({ body, defaultOpen, itemKey, label, meta, status, summary, toolKind }: Readonly<{
  body: ReactNode
  defaultOpen?: boolean | undefined
  itemKey: string
  label: string
  meta?: string | undefined
  status: DesktopActivityStatus
  summary: string
  toolKind: DesktopToolKind
}>): ReactElement => <DesktopProtocolCard
  body={body}
  defaultOpen={defaultOpen}
  icon={toolIcons[toolKind]}
  itemKey={itemKey}
  meta={meta}
  status={status}
  summary={summary}
  title={label}
  variant={toolKind === "mcp" ? "mcpToolCall" : toolKind === "web" ? "webSearch" : toolKind === "image" ? "imageView" : "dynamicToolCall"}
/>
