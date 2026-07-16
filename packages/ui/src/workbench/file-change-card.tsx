import { FileText } from "lucide-react"
import type { ReactElement } from "react"

import type { DesktopActivityStatus } from "./activity-status.tsx"
import { DesktopProtocolCard } from "./protocol-card.tsx"

export type DesktopFileChange = Readonly<{
  additions?: number
  deletions?: number
  diff?: string
  diffCapReached?: boolean
  kind: "add" | "delete" | "update" | "added" | "deleted" | "modified"
  path: string
}>

const normalizedKind = (kind: DesktopFileChange["kind"]): "add" | "delete" | "update" =>
  kind === "added" ? "add" : kind === "deleted" ? "delete" : kind === "modified" ? "update" : kind

const diffTone = (line: string): "add" | "remove" | "meta" | "context" =>
  line.startsWith("+") && !line.startsWith("+++") ? "add"
    : line.startsWith("-") && !line.startsWith("---") ? "remove"
      : line.startsWith("@@") || line.startsWith("diff ") || line.startsWith("*** ") ? "meta"
        : "context"

const diffView = (diff: string): ReactElement => {
  const lines = diff.split("\n")
  return <pre><code>{lines.map((line, index) =>
    <span data-diff-line={diffTone(line)} key={index}>{line}</span>)}</code></pre>
}

export const DesktopFileChangeCard = ({ changes, defaultOpen, itemKey, scope = "item", status }: Readonly<{
  changes: ReadonlyArray<DesktopFileChange>
  defaultOpen?: boolean | undefined
  itemKey: string
  scope?: "item" | "turn" | undefined
  status: DesktopActivityStatus
}>): ReactElement => <DesktopProtocolCard
  body={<ul className="oa-react-file-list">{changes.map((change, index) => {
    const kind = normalizedKind(change.kind)
    const tag = kind === "add" ? "ADD" : kind === "delete" ? "DEL" : "MOD"
    return <li data-change-kind={kind} key={`${change.path}:${index}`}>
      <details className="oa-react-file-change" open={defaultOpen === true && changes.length === 1}>
        <summary aria-label={`${tag} ${change.path}`}>
          <span>[{tag}]</span>
          <code title={change.path}>{change.path}</code>
          <small><i>+{change.additions ?? 0}</i><b>−{change.deletions ?? 0}</b></small>
        </summary>
        {change.diff === undefined ? <p>No unified diff recorded.</p> : diffView(change.diff)}
        {change.diffCapReached === true ? <small className="oa-react-diff-cap" role="status">Diff truncated at the bounded display limit</small> : null}
      </details>
    </li>
  })}</ul>}
  defaultOpen={defaultOpen ?? status === "running"}
  icon={FileText}
  itemKey={itemKey}
  meta={`PATCH: ${status.toUpperCase()} · ${changes.length} ${changes.length === 1 ? "FILE" : "FILES"}`}
  status={status}
  summary={scope === "turn" ? "Aggregate turn diff" : "Patch updated"}
  title={scope === "turn" ? "Turn diff" : "File changes"}
  variant="fileChange"
/>
