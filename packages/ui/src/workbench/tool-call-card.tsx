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

export type DesktopToolCallArg = Readonly<{ key: string; value: string }>

/**
 * Mirrors `isOpaqueBlobValue` in
 * `apps/openagents-desktop/src/renderer/tool-cards.ts` (long, unbroken,
 * letter+digit base64-class runs never render as a table value). Duplicated
 * rather than imported: `@openagentsinc/ui` is a shared, host-agnostic
 * package and must not depend on the Desktop app package. Args/results
 * arriving here are already bounded by
 * `apps/openagents-desktop/src/workbench-item-contract.ts` (400/2000/400
 * char ceilings); this is a render-time honesty check on top of that bound,
 * not a substitute for it.
 */
const isOpaqueBlobValue = (value: string): boolean => {
  const text = value.trim()
  return text.length >= 64 && /^[A-Za-z0-9+/=_-]+$/.test(text) && /\d/.test(text) && /[A-Za-z]/.test(text)
}

const presentText = (value: string): string => isOpaqueBlobValue(value) ? "[blob omitted]" : value

const firstLine = (value: string): string => {
  const line = value.split("\n").find(part => part.trim() !== "") ?? value
  return line.length > 160 ? `${line.slice(0, 159)}…` : line
}

export type DesktopToolCallCardProps = Readonly<{
  itemKey: string
  toolKind: DesktopToolKind
  status: DesktopActivityStatus
  defaultOpen?: boolean | undefined

  /** Legacy passthrough — kept for existing consumers (e.g. the `/splash` demo fixture). */
  label?: string | undefined
  summary?: ReactNode | undefined
  meta?: string | undefined
  body?: ReactNode | undefined

  /**
   * Structured payload (#8864, epic #8857 T7). When any of these is present
   * the card computes its own title/summary/meta/body per `callKind` instead
   * of relying on the legacy fields above.
   */
  tool?: string | undefined
  server?: string | undefined
  namespace?: string | undefined
  args?: ReadonlyArray<DesktopToolCallArg> | undefined
  resultSnippet?: string | undefined
  errorMessage?: string | undefined
  durationMs?: number | undefined
  /** web: the search query. */
  query?: string | undefined
  /** web: how many structured results came back. */
  resultCount?: number | undefined
  /** image: the viewed/saved image path — an honest text row, never a real preview. */
  path?: string | undefined
  /** mcp: connector/app badge, when the call rode through an app-context plugin. */
  appContext?: string | undefined
  /** mcp: latest `McpToolCallProgress` tick while the call is still running. */
  progressMessage?: string | undefined
}>

const structuredTitle = (props: DesktopToolCallCardProps): string => {
  const { namespace, server, tool, toolKind } = props
  if (toolKind === "mcp") return server === undefined ? (tool ?? "MCP tool") : `${server} · ${tool ?? "tool"}`
  if (toolKind === "dynamic") return namespace === undefined ? (tool ?? "Dynamic tool") : `${namespace} · ${tool ?? "tool"}`
  if (toolKind === "web") return "Web search"
  return tool === "imageView" ? "Image view" : "Image generation"
}

const structuredSummary = (props: DesktopToolCallCardProps): ReactNode => {
  const { args, errorMessage, progressMessage, query, resultSnippet, status, toolKind } = props
  if (toolKind === "web") {
    if (query !== undefined) return <code>{presentText(query)}</code>
    return status === "running" ? "Searching…" : "Web search"
  }
  if (toolKind === "image") {
    if (resultSnippet !== undefined) return firstLine(presentText(resultSnippet))
    return status === "running" ? "Generating…" : "Image"
  }
  // mcp / dynamic
  if (status === "running" && progressMessage !== undefined) return progressMessage
  if (errorMessage !== undefined) return errorMessage
  if (resultSnippet !== undefined) return firstLine(presentText(resultSnippet))
  if (args !== undefined && args.length > 0) return `${args.length} arg${args.length === 1 ? "" : "s"}`
  return status === "running" ? "Running…" : ""
}

const structuredMeta = (props: DesktopToolCallCardProps): string | undefined => {
  const { durationMs, resultCount, toolKind } = props
  const parts: Array<string> = []
  if (durationMs !== undefined) parts.push(`${Math.max(0, Math.round(durationMs))}MS`)
  if (toolKind === "web" && resultCount !== undefined) parts.push(`${resultCount} RESULT${resultCount === 1 ? "" : "S"}`)
  return parts.length === 0 ? undefined : parts.join(" · ")
}

const structuredBody = (props: DesktopToolCallCardProps): ReactNode => {
  const { appContext, args, errorMessage, path, progressMessage, query, resultCount, resultSnippet, status, toolKind } = props
  const rows: Array<ReactNode> = []

  if (toolKind === "web") {
    rows.push(<div className="oa-react-tool-args" key="web-query">
      <div className="oa-react-tool-arg-row"><span>Query</span><code>{query === undefined ? "—" : presentText(query)}</code></div>
      <div className="oa-react-tool-arg-row"><span>Results</span><code>{resultCount ?? "—"}</code></div>
    </div>)
  } else if (toolKind === "image") {
    rows.push(<div className="oa-react-tool-args" key="image-path">
      <div className="oa-react-tool-arg-row"><span>Path</span><code>{path === undefined ? "—" : presentText(path)}</code></div>
    </div>)
  } else if (args !== undefined && args.length > 0) {
    rows.push(<div className="oa-react-tool-args" key="args">
      {args.map(arg => <div className="oa-react-tool-arg-row" key={arg.key}><span>{arg.key}</span><code>{presentText(arg.value)}</code></div>)}
    </div>)
  }

  if (appContext !== undefined) rows.push(<span className="oa-react-tool-badge" key="app-context">{appContext}</span>)
  if (status === "running" && progressMessage !== undefined) {
    rows.push(<p className="oa-react-tool-progress" key="progress">{progressMessage}</p>)
  }
  if (resultSnippet !== undefined) {
    rows.push(<pre className="oa-react-tool-result" key="result"><code>{presentText(resultSnippet)}</code></pre>)
  }
  if (errorMessage !== undefined) rows.push(<p className="oa-react-tool-error" key="error">{presentText(errorMessage)}</p>)

  if (rows.length === 0) {
    return <p className="oa-react-tool-empty">{status === "running" ? "Waiting for result…" : "No additional detail recorded."}</p>
  }
  return <>{rows}</>
}

const hasStructuredPayload = (props: DesktopToolCallCardProps): boolean =>
  props.tool !== undefined || props.server !== undefined || props.namespace !== undefined ||
  props.args !== undefined || props.resultSnippet !== undefined || props.errorMessage !== undefined ||
  props.durationMs !== undefined || props.query !== undefined || props.resultCount !== undefined ||
  props.path !== undefined || props.appContext !== undefined || props.progressMessage !== undefined

export const DesktopToolCallCard = (props: DesktopToolCallCardProps): ReactElement => {
  const { body, defaultOpen, itemKey, label, meta, status, summary, toolKind } = props
  const structured = hasStructuredPayload(props)
  return <DesktopProtocolCard
    body={body ?? (structured ? structuredBody(props) : "")}
    defaultOpen={defaultOpen}
    icon={toolIcons[toolKind]}
    itemKey={itemKey}
    meta={meta ?? (structured ? structuredMeta(props) : undefined)}
    status={status}
    summary={summary ?? (structured ? structuredSummary(props) : "")}
    title={label ?? (structured ? structuredTitle(props) : toolKind)}
    variant={toolKind === "mcp" ? "mcpToolCall" : toolKind === "web" ? "webSearch" : toolKind === "image" ? "imageView" : "dynamicToolCall"}
  />
}
