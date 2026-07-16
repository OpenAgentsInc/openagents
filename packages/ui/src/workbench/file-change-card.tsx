import { FileText } from "lucide-react"
import type { ReactElement } from "react"

import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopProtocolCard } from "./protocol-card.tsx"

export type DesktopFileChange = Readonly<{
  additions?: number
  deletions?: number
  kind: "added" | "deleted" | "modified"
  path: string
}>

export const DesktopFileChangeCard = ({ changes, defaultOpen, itemKey, status }: Readonly<{
  changes: ReadonlyArray<DesktopFileChange>
  defaultOpen?: boolean | undefined
  itemKey: string
  status: DesktopActivityStatus
}>): ReactElement => <DesktopProtocolCard
  body={<ul className="oa-react-file-list">{changes.map(change => <li data-change-kind={change.kind} key={change.path}>
    <span>{change.kind === "added" ? "A" : change.kind === "deleted" ? "D" : "M"}</span>
    <code>{change.path}</code>
    <small><i>+{change.additions ?? 0}</i><b>-{change.deletions ?? 0}</b></small>
  </li>)}</ul>}
  defaultOpen={defaultOpen}
  icon={FileText}
  itemKey={itemKey}
  meta={`${changes.length} ${changes.length === 1 ? "file" : "files"}`}
  status={status}
  summary="Patch updated"
  title="File changes"
  variant="fileChange"
/>
