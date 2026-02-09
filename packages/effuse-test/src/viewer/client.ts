import { html, renderToString } from "../../../effuse/src/index.ts"
import {
  InfiniteCanvas,
  hydrateInfiniteCanvas,
  LeafNode,
  NodeDetailsPanel,
  RootNode,
  SkeletonNode,
  TreeLayout,
  type FlowNode,
  type FlowNodeMetadata,
  type FlowNodeStatus,
} from "../../../effuse-flow/src/index.ts"

type SpanRecord = {
  readonly spanId: string
  readonly parentSpanId?: string | null
  readonly name?: string | null
  readonly kind?: string | null
  readonly startTs?: number | null
  readonly endTs?: number | null
  readonly status?: "passed" | "failed" | null
  readonly durationMs?: number | null
}

const statusEl = document.getElementById("status")
const eventsEl = document.getElementById("events")
const graphRootEl = document.getElementById("graph-root")

if (!(statusEl instanceof HTMLElement)) throw new Error("Missing #status")
if (!(eventsEl instanceof HTMLElement)) throw new Error("Missing #events")
if (!(graphRootEl instanceof HTMLElement)) throw new Error("Missing #graph-root")

const spans = new Map<string, SpanRecord>()

let runStatus: { readonly status: "passed" | "failed"; readonly durationMs: number } | null = null
let selectedNodeId: string | null = null

const appendEventRow = (evt: unknown) => {
  const row = document.createElement("div")
  row.className = "row"

  const typeEl = document.createElement("div")
  typeEl.className = "type"
  typeEl.textContent = String((evt as any)?.type ?? "")

  const metaEl = document.createElement("div")
  metaEl.className = "meta"
  try {
    metaEl.textContent = JSON.stringify(evt).slice(0, 260)
  } catch {
    metaEl.textContent = String(evt)
  }

  row.append(typeEl, metaEl)
  eventsEl.prepend(row)
}

const toFlowStatus = (s: SpanRecord): FlowNodeStatus => {
  if (s.status === "failed") return "error"
  if (s.status === "passed") return "ok"
  if (typeof s.startTs === "number") return "running"
  return "pending"
}

const toSpanNodeMetadata = (s: SpanRecord): FlowNodeMetadata => {
  const kind = s.kind ?? undefined
  const status = toFlowStatus(s)
  const subtitle = kind ? String(kind) : ""
  const duration = typeof s.durationMs === "number" ? Math.max(0, Math.round(s.durationMs)) : null
  const badge = duration != null ? { label: `${duration}ms`, tone: "neutral" as const } : undefined

  return {
    type: "leaf",
    kind: kind ? String(kind) : undefined,
    subtitle,
    status,
    ...(badge ? { badge } : {}),
    detail:
      duration != null
        ? `Duration: ${duration}ms`
        : status === "running"
          ? "In progress"
          : undefined,
  }
}

const buildFlowTreeFromSpans = (): { readonly root: FlowNode; readonly byId: Map<string, FlowNode> } => {
  const byId = new Map<string, FlowNode>()

  const rootStatus: FlowNodeStatus =
    runStatus?.status === "failed" ? "error" : runStatus?.status === "passed" ? "ok" : "running"

  const root: FlowNode = {
    id: "run",
    label: "Effuse Test Runner",
    direction: "vertical",
    metadata: { type: "root", status: rootStatus },
    children: [],
  }
  byId.set(root.id, root)

  // Create leaf nodes.
  for (const s of spans.values()) {
    const node: FlowNode = {
      id: s.spanId,
      label: s.name ? String(s.name) : s.spanId,
      metadata: toSpanNodeMetadata(s),
      children: [],
    }
    byId.set(node.id, node)
  }

  // Link children.
  for (const s of spans.values()) {
    const node = byId.get(s.spanId)
    if (!node) continue
    const parentId = s.parentSpanId ? String(s.parentSpanId) : null
    const parent = parentId ? byId.get(parentId) : null
    const target = parent ?? root
    const children = Array.isArray(target.children) ? [...target.children] : []
    children.push(node)
    ;(target as any).children = children
  }

  const sortChildren = (node: FlowNode) => {
    const children = Array.isArray(node.children) ? [...node.children] : []
    children.sort((a, b) => {
      const sa = spans.get(a.id)?.startTs ?? 0
      const sb = spans.get(b.id)?.startTs ?? 0
      return Number(sa) - Number(sb)
    })
    ;(node as any).children = children
    for (const c of children) sortChildren(c)
  }

  sortChildren(root)

  return { root, byId }
}

const canvasId = "effuse-test-viewer"

// Render a stable canvas shell once.
graphRootEl.innerHTML = renderToString(
  InfiniteCanvas({
    id: canvasId,
    showGrid: true,
    children: html``,
    overlay: html``,
  }),
)

const canvasRoot = graphRootEl.querySelector(`[data-oa-flow-canvas-root="${canvasId}"]`)
if (!(canvasRoot instanceof HTMLElement)) throw new Error("Failed to mount flow canvas")

const contentEl = canvasRoot.querySelector('g[data-oa-flow-canvas-content="1"]')
if (!(contentEl instanceof SVGGElement)) throw new Error("Missing flow canvas content group")

const overlayEl = canvasRoot.querySelector('[data-oa-flow-canvas-overlay="1"]')
if (!(overlayEl instanceof HTMLElement)) throw new Error("Missing flow canvas overlay")

hydrateInfiniteCanvas(canvasRoot)

const renderGraph = () => {
  const { root, byId } = buildFlowTreeFromSpans()
  const selected = selectedNodeId ? byId.get(selectedNodeId) ?? null : null

  const graph = TreeLayout({
    data: root,
    nodeSpacing: { x: 60, y: 60 },
    layoutConfig: { direction: "vertical" },
    connectionAnimation: { preset: "dots" },
    renderNode: (node) => {
      const selected = node.id === selectedNodeId
      const type = node.metadata?.type ?? "leaf"
      if (type === "root") return RootNode({ node: node as any, selected })
      if (type === "skeleton") return SkeletonNode({ node: node as any, selected })
      return LeafNode({ node: node as any, selected })
    },
  })

  contentEl.innerHTML = renderToString(graph)
  overlayEl.innerHTML = renderToString(NodeDetailsPanel({ node: selected }))
}

// Node selection (event delegation).
canvasRoot.addEventListener("click", (e) => {
  const target = e.target as Element | null
  const wrap = target?.closest?.("[data-node-id]") as Element | null
  const nodeId = wrap?.getAttribute?.("data-node-id") ?? null
  if (!nodeId) return

  // Root click clears selection.
  selectedNodeId = nodeId === "run" ? null : nodeId
  renderGraph()
})

overlayEl.addEventListener("click", (e) => {
  const target = e.target as Element | null
  const action = target?.closest?.("[data-oa-flow-action]")?.getAttribute?.("data-oa-flow-action")
  if (action === "details.close") {
    selectedNodeId = null
    renderGraph()
  }
})

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (selectedNodeId != null) {
      selectedNodeId = null
      renderGraph()
    }
  }
})

const onEvent = (evt: any) => {
  appendEventRow(evt)

  if (evt?.type === "span.started" && typeof evt.spanId === "string") {
    spans.set(evt.spanId, {
      spanId: evt.spanId,
      parentSpanId: typeof evt.parentSpanId === "string" ? evt.parentSpanId : null,
      name: typeof evt.name === "string" ? evt.name : null,
      kind: typeof evt.kind === "string" ? evt.kind : null,
      startTs: typeof evt.ts === "number" ? evt.ts : null,
    })
    renderGraph()
    return
  }

  if (evt?.type === "span.finished" && typeof evt.spanId === "string") {
    const cur = spans.get(evt.spanId)
    if (cur) {
      spans.set(evt.spanId, {
        ...cur,
        endTs: typeof evt.ts === "number" ? evt.ts : cur.endTs ?? null,
        status: evt.status === "passed" || evt.status === "failed" ? evt.status : null,
        durationMs: typeof evt.durationMs === "number" ? evt.durationMs : cur.durationMs ?? null,
      })
      renderGraph()
    }
    return
  }

  if (evt?.type === "run.finished") {
    const status = evt.status === "passed" || evt.status === "failed" ? evt.status : null
    const durationMs = typeof evt.durationMs === "number" ? evt.durationMs : 0
    if (status) {
      runStatus = { status, durationMs }
      statusEl.textContent = `${status} (${durationMs}ms)`
      renderGraph()
    }
    return
  }
}

const proto = location.protocol === "https:" ? "wss:" : "ws:"
const wsUrl = `${proto}//${location.host}/ws`
const ws = new WebSocket(wsUrl)
ws.addEventListener("open", () => (statusEl.textContent = "connected"))
ws.addEventListener("close", () => (statusEl.textContent = "disconnected"))
ws.addEventListener("message", (e) => {
  try {
    onEvent(JSON.parse(String(e.data)))
  } catch (err) {
    console.error(err)
  }
})
