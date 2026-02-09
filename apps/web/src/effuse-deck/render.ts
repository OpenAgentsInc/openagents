import { html } from "@openagentsinc/effuse"
import type { TemplateResult } from "@openagentsinc/effuse"

import {
  LeafNode,
  NODE_SIZES,
  RootNode,
  SkeletonNode,
  TreeConnectionLine,
  TreeElementNode,
  type AnimationConfig,
  type FlowNode,
  type FlowNodeBadge,
  type FlowNodeBadgeTone,
  type FlowNodeStatus,
  type FlowNodeType,
  type Point,
} from "@openagentsinc/effuse-flow"

import { getStoryById } from "../storybook"
import { autopilotStories } from "../storybook/stories/autopilot"
import { homeStories } from "../storybook/stories/home"

import { streamdown } from "../lib/effuseStreamdown"

import { DECK_STORY_BY_ID } from "./deckStories"

import {
  isRefNode,
  parseDeckDocument,
  renderUnknownNode,
  resolveTokenValue,
} from "./dsl"

import type {
  DeckDocument,
  DeckLayout,
  DeckNodeChild,
  DeckParseResult,
  DeckSlide,
  DeckTheme,
} from "./dsl"

const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value)

const asNumber = (value: unknown): number | null => (typeof value === "number" && Number.isFinite(value) ? value : null)

const asString = (value: unknown): string | null => (typeof value === "string" ? value : null)

const px = (value: number): string => `${Math.round(value)}px`

const isFlowNodeType = (value: unknown): value is FlowNodeType =>
  value === "root" || value === "leaf" || value === "skeleton"

const looksLikeMarkdownTable = (markdown: string): boolean => {
  const lines = markdown
    .split(/\r?\n/g)
    .map((l) => l.trim())
    .filter((l) => l.length > 0)

  if (lines.length < 2) return false

  const header = lines[0]!
  const divider = lines[1]!

  if (!header.includes("|")) return false

  // Matches: | --- | --- |, |:---|---:|, etc. Requires at least 2 columns.
  const dividerRe = /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/
  return dividerRe.test(divider)
}

const isFlowNodeStatus = (value: unknown): value is FlowNodeStatus =>
  value === "ok" || value === "live" || value === "running" || value === "pending" || value === "error"

const isFlowBadgeTone = (value: unknown): value is FlowNodeBadgeTone =>
  value === "neutral" ||
  value === "info" ||
  value === "success" ||
  value === "warning" ||
  value === "destructive"

const isGraphPresetName = (value: unknown): value is "dots" | "dashes" | "dots-slow" | "dashes-fast" | "pulse" =>
  value === "dots" || value === "dashes" || value === "dots-slow" || value === "dashes-fast" || value === "pulse"

const computeDeckCanvasSize = (doc: DeckDocument): { readonly width: number; readonly height: number } => {
  const size = doc.deck.size
  if (size && Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0) {
    return { width: Math.round(size.width), height: Math.round(size.height) }
  }

  const { w, h } = computeAspectRatio(doc)
  // Default to a common deck canvas size so "pixel" placement is stable across viewports.
  const baseWidth = 1920
  const ratio = w / h
  const height = Math.round(baseWidth / ratio)
  return { width: baseWidth, height }
}

const parseGraphLength = (theme: DeckTheme | undefined, raw: unknown, axis: number): number | null => {
  const resolved = resolveTokenValue(theme, raw)
  const n = asNumber(resolved)
  if (n != null) return n

  const s = asString(resolved)?.trim()
  if (!s) return null

  if (s.endsWith("%")) {
    const pct = Number(s.slice(0, -1))
    return Number.isFinite(pct) ? (axis * pct) / 100 : null
  }

  if (s.endsWith("px")) {
    const pxVal = Number(s.slice(0, -2))
    return Number.isFinite(pxVal) ? pxVal : null
  }

  const num = Number(s)
  return Number.isFinite(num) ? num : null
}

type GraphAnchor = "center" | "top-left" | "top-right" | "bottom-left" | "bottom-right"

const parseGraphAnchor = (value: unknown): GraphAnchor => {
  switch (value) {
    case "center":
    case "top-left":
    case "top-right":
    case "bottom-left":
    case "bottom-right":
      return value
    default:
      return "top-left"
  }
}

const anchorToCenter = (
  anchor: GraphAnchor,
  anchorPos: Point,
  size: { readonly width: number; readonly height: number },
): Point => {
  switch (anchor) {
    case "center":
      return anchorPos
    case "top-left":
      return { x: anchorPos.x + size.width / 2, y: anchorPos.y + size.height / 2 }
    case "top-right":
      return { x: anchorPos.x - size.width / 2, y: anchorPos.y + size.height / 2 }
    case "bottom-left":
      return { x: anchorPos.x + size.width / 2, y: anchorPos.y - size.height / 2 }
    case "bottom-right":
      return { x: anchorPos.x - size.width / 2, y: anchorPos.y - size.height / 2 }
  }
}

const pickNodeTemplate = (node: FlowNode): TemplateResult => {
  const type = node.metadata?.type
  if (type === "root") return RootNode({ node: node as any })
  if (type === "skeleton") return SkeletonNode({ node: node as any })
  return LeafNode({ node: node as any })
}

const routeEdge = (
  a: { readonly center: Point; readonly size: { readonly width: number; readonly height: number } },
  b: { readonly center: Point; readonly size: { readonly width: number; readonly height: number } },
): ReadonlyArray<Point> => {
  const dx = b.center.x - a.center.x
  const dy = b.center.y - a.center.y

  // Prefer a clean 90-degree "Z" route that matches the old flow-graph aesthetic.
  if (Math.abs(dx) >= Math.abs(dy)) {
    const start: Point = {
      x: dx >= 0 ? a.center.x + a.size.width / 2 : a.center.x - a.size.width / 2,
      y: a.center.y,
    }
    const end: Point = {
      x: dx >= 0 ? b.center.x - b.size.width / 2 : b.center.x + b.size.width / 2,
      y: b.center.y,
    }
    const midX = start.x + (end.x - start.x) * 0.5
    return [start, { x: midX, y: start.y }, { x: midX, y: end.y }, end]
  }

  const start: Point = {
    x: a.center.x,
    y: dy >= 0 ? a.center.y + a.size.height / 2 : a.center.y - a.size.height / 2,
  }
  const end: Point = {
    x: b.center.x,
    y: dy >= 0 ? b.center.y - b.size.height / 2 : b.center.y + b.size.height / 2,
  }
  const midY = start.y + (end.y - start.y) * 0.5
  return [start, { x: start.x, y: midY }, { x: end.x, y: midY }, end]
}

const toGapStyle = (theme: DeckTheme | undefined, gap: unknown): string | null => {
  const resolved = resolveTokenValue(theme, gap)
  const n = asNumber(resolved)
  return n != null ? `gap: ${px(n)};` : null
}

const toPaddingStyle = (theme: DeckTheme | undefined, padding: unknown): string | null => {
  const resolved = resolveTokenValue(theme, padding)
  const n = asNumber(resolved)
  return n != null ? `padding: ${px(n)};` : null
}

const toSizeStyle = (theme: DeckTheme | undefined, key: "width" | "height", value: unknown): string | null => {
  const resolved = resolveTokenValue(theme, value)
  const n = asNumber(resolved)
  if (n != null) return `${key}: ${px(n)};`
  const s = asString(resolved)
  if (s) return `${key}: ${s};`
  return null
}

const toColorStyle = (theme: DeckTheme | undefined, cssKey: string, value: unknown): string | null => {
  const resolved = resolveTokenValue(theme, value)
  const s = asString(resolved)
  return s ? `${cssKey}: ${s};` : null
}

const getBuildRange = (props: Record<string, unknown> | undefined): { readonly in: number; readonly out: number | null } | null => {
  const build = props?.build
  if (!isRecord(build)) return null
  const inRaw = build["in"]
  const outRaw = build["out"]
  const inStep = typeof inRaw === "number" && Number.isFinite(inRaw) ? Math.max(1, Math.floor(inRaw)) : 1
  const outStep =
    outRaw == null
      ? null
      : typeof outRaw === "number" && Number.isFinite(outRaw)
        ? Math.max(1, Math.floor(outRaw))
        : null
  return { in: inStep, out: outStep }
}

const isVisibleAtStep = (range: { readonly in: number; readonly out: number | null }, step: number): boolean => {
  if (step < range.in) return false
  if (range.out != null && step >= range.out) return false
  return true
}

const computeMaxStep = (node: DeckNodeChild): number => {
  if (typeof node === "string" || isRefNode(node)) return 1
  const props = (node.props ?? {}) as Record<string, unknown>
  const build = getBuildRange(props)
  let max = 1
  if (build) {
    max = Math.max(max, build.in, build.out ?? 1)
  }
  const children = Array.isArray(node.children) ? node.children : []
  for (const child of children) {
    max = Math.max(max, computeMaxStep(child))
  }
  return max
}

export const computeSlideTotalSteps = (nodes: ReadonlyArray<DeckNodeChild>): number => {
  let max = 1
  for (const n of nodes) max = Math.max(max, computeMaxStep(n))
  return max
}

const filterByBuild = (node: DeckNodeChild, step: number): DeckNodeChild | null => {
  if (typeof node === "string" || isRefNode(node)) return node
  const props = (node.props ?? {}) as Record<string, unknown>
  const build = getBuildRange(props)
  if (build && !isVisibleAtStep(build, step)) return null

  const children = Array.isArray(node.children) ? node.children : []
  const nextChildren = children.map((c) => filterByBuild(c, step)).filter((c): c is DeckNodeChild => c != null)
  return { ...(node as any), children: nextChildren }
}

const cloneFillSlots = (
  node: DeckNodeChild,
  regions: Readonly<Record<string, ReadonlyArray<DeckNodeChild>>> | undefined,
): DeckNodeChild => {
  if (typeof node === "string" || isRefNode(node)) return node

  if (node.type === "Slot") {
    const name = isRecord(node.props) ? asString((node.props as any).name) : null
    const filled = (name && regions?.[name]) ? regions[name]! : []
    return { type: "Fragment", children: filled }
  }

  const children = Array.isArray(node.children) ? node.children : []
  return {
    ...(node as any),
    children: children.map((c) => cloneFillSlots(c, regions)),
  }
}

export const expandSlideContent = (doc: DeckDocument, slide: DeckSlide): ReadonlyArray<DeckNodeChild> => {
  const layouts = doc.layouts ?? {}
  const layoutName = slide.layout ?? doc.deck.defaultSlideLayout ?? null

  if (layoutName && Object.prototype.hasOwnProperty.call(layouts, layoutName)) {
    const layout = (layouts as any)[layoutName] as DeckLayout
    const filled = cloneFillSlots(layout, slide.regions)
    return [filled]
  }

  if (Array.isArray(slide.content)) {
    return slide.content
  }

  // Fallback: if regions exist without a layout, render all regions in order.
  const regions = slide.regions ? Object.entries(slide.regions) : []
  if (regions.length > 0) {
    return [
      {
        type: "Column",
        props: { gap: 16 },
        children: regions.flatMap(([_name, nodes]) => nodes),
      },
    ]
  }

  return []
}

export type DeckRuntime = {
  readonly doc: DeckDocument
  readonly slideIndex: number
  readonly stepIndex: number
}

const resolveAssetImageUrl = (doc: DeckDocument, assetId: string): string | null => {
  const assets = doc.assets
  if (!assets || !isRecord(assets)) return null
  const images = assets["images"]
  if (!isRecord(images)) return null
  const entry = images[assetId]
  if (!isRecord(entry)) return null
  const url = entry["url"]
  return typeof url === "string" ? url : null
}

const renderChildren = (
  doc: DeckDocument,
  theme: DeckTheme | undefined,
  runtime: { readonly slideIndex: number; readonly slideCount: number; readonly stepIndex: number; readonly totalSteps: number },
  children: ReadonlyArray<DeckNodeChild> | undefined,
): TemplateResult => html`${(children ?? []).map((c) => renderNode(doc, theme, runtime, c))}`

function cx(...parts: Array<string | null | undefined | false>): string {
  return parts.filter(Boolean).join(" ")
}

const parseFlowBadge = (value: unknown): FlowNodeBadge | undefined => {
  if (!isRecord(value)) return undefined
  const label = asString(value["label"])
  if (!label) return undefined
  const toneRaw = value["tone"]
  const tone = isFlowBadgeTone(toneRaw) ? toneRaw : undefined
  return { label, ...(tone ? { tone } : {}) }
}

const hatcheryActionButton = (input: {
  readonly label: string
  readonly action: string
  readonly subtle?: boolean
}): TemplateResult => {
  const minHeightClass = "min-h-8"
  const contentSizeClass = "px-3 py-1 gap-1.5 text-xs"
  const opacityClass = input.subtle ? "opacity-60 hover:opacity-100" : "opacity-100"

  return html`
    <button
      type="button"
      data-deck-action="${input.action}"
      class="${cx(
        "group relative inline-flex max-w-full items-stretch justify-stretch",
        minHeightClass,
        "m-0 border-0 bg-transparent p-0",
        "cursor-pointer select-none",
        "text-white transition-[color,opacity] duration-200 ease-out",
        "uppercase tracking-[0.08em] font-semibold",
        "use-font-square721 [font-family:var(--font-square721)]",
        "focus-visible:outline-none",
        opacityClass,
      )}"
    >
      <svg
        class="${cx(
          "pointer-events-none absolute inset-0 h-full w-full",
          "opacity-75 transition-[opacity,transform] duration-200 ease-out",
          "group-hover:opacity-100 group-hover:scale-[1.02]",
          "group-focus-visible:opacity-100 group-focus-visible:scale-[1.02]",
        )}"
        viewBox="0 0 100 40"
        preserveAspectRatio="none"
        role="presentation"
        aria-hidden="true"
      >
        <polygon
          class="fill-[hsla(0,0%,100%,0.08)]"
          points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
        />
        <polygon
          class="${cx(
            "fill-none",
            "stroke-[hsla(0,0%,100%,0.9)] [stroke-width:2]",
            "transition-[stroke] duration-200 ease-out",
            "group-hover:stroke-[hsla(0,0%,100%,1)]",
            "group-focus-visible:stroke-[hsla(0,0%,100%,1)]",
          )}"
          points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
        />
      </svg>
      <span
        class="${cx(
          "relative flex w-full min-w-0 max-w-full flex-wrap items-center justify-center",
          contentSizeClass,
          "leading-[1.2] whitespace-normal text-center [overflow-wrap:anywhere]",
        )}"
      >
        ${input.label}
      </span>
    </button>
  `
}

const renderNode = (
  doc: DeckDocument,
  theme: DeckTheme | undefined,
  runtime: { readonly slideIndex: number; readonly slideCount: number; readonly stepIndex: number; readonly totalSteps: number },
  node: DeckNodeChild,
): TemplateResult => {
  if (typeof node === "string") {
    return html`${node}`
  }

  if (isRefNode(node)) {
    return html`<span class="text-text-dim">[ref:${node.$ref}]</span>`
  }

  const props = (node.props ?? {}) as Record<string, unknown>
  const children = Array.isArray(node.children) ? node.children : []

  switch (node.type) {
    case "Fragment": {
      return html`${children.map((c) => renderNode(doc, theme, runtime, c))}`
    }
    case "Row": {
      const gapStyle = toGapStyle(theme, props.gap) ?? ""
      const align = asString(props.align) ?? "stretch"
      const justify = asString(props.justify) ?? "start"
      return html`<div
        class="flex flex-row"
        style="${gapStyle} align-items:${align}; justify-content:${justify};"
      >
        ${renderChildren(doc, theme, runtime, children)}
      </div>`
    }
    case "Column": {
      const gapStyle = toGapStyle(theme, props.gap) ?? ""
      const align = asString(props.align) ?? "stretch"
      const justify = asString(props.justify) ?? "start"
      const fill = props.fill === true
      const position = asString(props.position) ?? ""
      const posStyle = position === "relative" ? "position:relative;" : ""
      const centered = align === "center" && justify === "center"
      const widthStyle = centered ? "width:80%; max-width:80%; min-width:0;" : ""
      return html`<div
        class="flex flex-col min-h-0${fill ? " flex-1" : ""}"
        style="${posStyle} ${widthStyle} ${gapStyle} align-items:${align}; justify-content:${justify};"
      >
        ${renderChildren(doc, theme, runtime, children)}
      </div>`
    }
    case "Box": {
      const styles = [
        toPaddingStyle(theme, props.padding),
        toSizeStyle(theme, "width", props.width),
        toSizeStyle(theme, "height", props.height),
        toColorStyle(theme, "background", props.background),
        toColorStyle(theme, "color", props.color),
      ]
        .filter(Boolean)
        .join(" ")
      const border = props.border ? "border border-border-dark" : ""
      return html`<div class="${border} rounded ${border ? "p-0" : ""}" style="${styles}">${renderChildren(
        doc,
        theme,
        runtime,
        children,
      )}</div>`
    }
    case "Layer": {
      const inset = props.inset !== false
      const insetClass = inset ? " inset-0" : ""
      const zIndex = asNumber(props.zIndex) ?? 0
      const bg = asString(props.background) ?? ""
      const bgStyle = bg ? `background:${bg};` : ""
      const pointerEvents = props.pointerEvents === false ? "pointer-events:none;" : ""
      return html`<div
        class="absolute${insetClass} min-h-0 min-w-0 overflow-hidden h-full flex flex-col"
        style="z-index:${zIndex}; min-height:100%; ${bgStyle} ${pointerEvents}"
      >
        ${renderChildren(doc, theme, runtime, children)}
      </div>`
    }
    case "Graph": {
      const deckCanvas = computeDeckCanvasSize(doc)
      const widthResolved = resolveTokenValue(theme, props.width)
      const heightResolved = resolveTokenValue(theme, props.height)
      const width = asNumber(widthResolved) ?? deckCanvas.width
      const height = asNumber(heightResolved) ?? deckCanvas.height

      const inset = props.inset !== false
      const zIndex = asNumber(resolveTokenValue(theme, props.zIndex)) ?? -1
      const opacityRaw = resolveTokenValue(theme, props.opacity)
      const opacity = typeof opacityRaw === "number" && Number.isFinite(opacityRaw) ? Math.max(0, Math.min(1, opacityRaw)) : 0.65

      const fit = asString(props.fit) ?? "stretch"
      const preserveAspectRatio = fit === "contain" ? "xMidYMid meet" : fit === "cover" ? "xMidYMid slice" : "none"

      const pointerEvents = props.pointerEvents === true ? "" : "pointer-events:none;"

      const presetRaw = resolveTokenValue(theme, props.preset)
      const graphPreset = isGraphPresetName(presetRaw) ? presetRaw : "dots-slow"

      type NodeResolved = {
        readonly id: string
        readonly node: FlowNode
        readonly center: Point
        readonly size: { readonly width: number; readonly height: number }
      }

      const nodes: NodeResolved[] = []
      const byId = new Map<string, NodeResolved>()
      const edges: Array<{ readonly key: string; readonly path: ReadonlyArray<Point>; readonly animation: AnimationConfig }> = []
      const errors: string[] = []

      // Pass 1: parse nodes + edge specs.
      const edgeSpecs: Array<{ readonly from: string; readonly to: string; readonly preset?: string; readonly color?: string }> = []

      for (const c of children) {
        if (typeof c === "string" || isRefNode(c)) continue
        if (c.type === "GraphNode") {
          const p = (c.props ?? {}) as Record<string, unknown>
          const id = asString(p.nodeId) ?? asString(p.id)
          if (!id) {
            errors.push("GraphNode missing props.nodeId")
            continue
          }

          const nodeTypeRaw = resolveTokenValue(theme, p.nodeType ?? p.type)
          const nodeType: FlowNodeType = isFlowNodeType(nodeTypeRaw) ? nodeTypeRaw : "leaf"
          const size = NODE_SIZES[nodeType] ?? NODE_SIZES.leaf

          const x = parseGraphLength(theme, p.x, width)
          const y = parseGraphLength(theme, p.y, height)
          if (x == null || y == null) {
            errors.push(`GraphNode(${id}) missing/invalid x/y`)
            continue
          }

          const anchor = parseGraphAnchor(resolveTokenValue(theme, p.anchor))
          const center = anchorToCenter(anchor, { x, y }, size)

          const label = asString(resolveTokenValue(theme, p.label)) ?? id
          const subtitle = asString(resolveTokenValue(theme, p.subtitle)) ?? undefined
          const statusRaw = resolveTokenValue(theme, p.status)
          const status = isFlowNodeStatus(statusRaw) ? statusRaw : undefined
          const badge = parseFlowBadge(resolveTokenValue(theme, p.badge))

          const flowNode: FlowNode = {
            id,
            label,
            metadata: {
              type: nodeType,
              ...(subtitle ? { subtitle } : {}),
              ...(status ? { status } : {}),
              ...(badge ? { badge } : {}),
            },
          }

          const resolved: NodeResolved = { id, node: flowNode, center, size }
          nodes.push(resolved)
          byId.set(id, resolved)
          continue
        }

        if (c.type === "GraphEdge") {
          const p = (c.props ?? {}) as Record<string, unknown>
          const from = asString(p.from)
          const to = asString(p.to)
          if (!from || !to) {
            errors.push("GraphEdge missing props.from/props.to")
            continue
          }
          const preset = asString(resolveTokenValue(theme, p.preset)) ?? undefined
          const color = asString(resolveTokenValue(theme, p.color)) ?? undefined
          edgeSpecs.push({ from, to, ...(preset ? { preset } : {}), ...(color ? { color } : {}) })
          continue
        }
      }

      // Pass 2: route + render edges.
      for (const spec of edgeSpecs) {
        const a = byId.get(spec.from)
        const b = byId.get(spec.to)
        if (!a || !b) {
          errors.push(`GraphEdge references missing node: ${spec.from} -> ${spec.to}`)
          continue
        }

        const preset = isGraphPresetName(spec.preset) ? spec.preset : graphPreset
        const animation: AnimationConfig = spec.color ? { preset, color: spec.color } : { preset }
        const path = routeEdge(a, b)
        edges.push({ key: `${spec.from}-${spec.to}`, path, animation })
      }

      const className = asString(props.className) ?? ""

      const graphVars = [
        "--oa-flow-bg: transparent;",
        "--oa-flow-panel: rgba(255,255,255,0.05);",
        "--oa-flow-panel2: rgba(255,255,255,0.07);",
        "--oa-flow-text: rgba(255,255,255,0.92);",
        "--oa-flow-muted: rgba(255,255,255,0.62);",
        "--oa-flow-stroke: rgba(255,255,255,0.10);",
        "--oa-flow-connection-stroke: rgba(255,255,255,0.12);",
      ].join(" ")

      return html`
        <div
          class="${cx("oa-deck-graph", inset ? "absolute inset-0" : "relative", className)}"
          style="z-index:${zIndex}; opacity:${opacity}; ${pointerEvents} ${graphVars}"
          data-deck-graph="1"
        >
          ${errors.length > 0
            ? html`<div class="absolute top-3 left-3 text-[12px] text-red-400 font-mono bg-black/50 rounded px-2 py-1">
                ${errors.join(" · ")}
              </div>`
            : null}
          <svg
            viewBox="${`0 0 ${Math.round(width)} ${Math.round(height)}`}"
            preserveAspectRatio="${preserveAspectRatio}"
            class="w-full h-full"
          >
            <g data-deck-graph-edges="1">
              ${edges.map((e) => html`<g data-deck-graph-edge="${e.key}">${TreeConnectionLine({ path: e.path, animation: e.animation })}</g>`)}
            </g>
            <g data-deck-graph-nodes="1">
              ${nodes.map((n) =>
                TreeElementNode({
                  id: n.id,
                  position: n.center,
                  children: pickNodeTemplate(n.node),
                }),
              )}
            </g>
          </svg>
        </div>
      `
    }
    case "GraphNode":
    case "GraphEdge": {
      return html`<div class="text-xs text-red-400">[${node.type} must be a child of Graph]</div>`
    }
    case "Spacer": {
      const sizeResolved = resolveTokenValue(theme, props.size)
      const size = asNumber(sizeResolved) ?? 16
      return html`<div style="height:${px(size)};"></div>`
    }
    case "Divider": {
      const thicknessResolved = resolveTokenValue(theme, props.thickness)
      const thickness = asNumber(thicknessResolved) ?? 1
      const colorStyle = toColorStyle(theme, "background", props.color) ?? "background: var(--color-border-dark);"
      return html`<div style="height:${px(thickness)}; ${colorStyle}"></div>`
    }
    case "Text": {
      const style = asString(props.style) ?? "body"
      const align = asString(props.align) ?? "left"
      const colorStyle = toColorStyle(theme, "color", props.color) ?? ""

      const cls =
        style === "h1"
          ? "text-[96px] leading-[1.06] tracking-tight font-semibold [font-family:var(--font-sans)]"
        : style === "h2"
            ? "text-[36px] leading-[1.2] font-normal tracking-[0.1em] use-font-square721 [font-family:var(--font-square721)]"
        : style === "h3"
              ? "text-[38px] leading-[1.14] font-semibold use-font-square721 [font-family:var(--font-square721)]"
        : style === "problemLabel"
                ? "text-[18px] leading-5 tracking-wider uppercase text-white/70 [font-family:var(--font-sans)]"
        : style === "problemHeadline"
                  ? "text-[66px] leading-[1.06] tracking-tight font-semibold [font-family:var(--font-sans)]"
        : style === "caption"
                    ? "text-[18px] leading-7 text-white/80"
        : style === "code"
                      ? "text-[20px] leading-7 font-mono"
                      : "text-[24px] leading-8"

      return html`<div class="${cls}" style="text-align:${align}; ${colorStyle}">
        ${renderChildren(doc, theme, runtime, children)}
      </div>`
    }
    case "Inline": {
      const text = asString(props.text) ?? ""
      return html`<span>${text}</span>`
    }
    case "List": {
      const ordered = Boolean(props.ordered)
      const gapResolved = resolveTokenValue(theme, props.gap)
      const gap = asNumber(gapResolved) ?? 12
      const cls = ordered ? "list-decimal" : "list-disc"
      const tag = ordered ? "ol" : "ul"
      const textCls = "text-[28px] leading-[1.32] text-white/90 [font-family:var(--font-sans)]"
      return tag === "ol"
        ? html`<ol class="${cls} ${textCls} pl-6" style="display:flex; flex-direction:column; gap:${px(gap)};">
            ${children.map((c) => renderNode(doc, theme, runtime, c))}
          </ol>`
        : html`<ul class="${cls} ${textCls} pl-6" style="display:flex; flex-direction:column; gap:${px(gap)};">
            ${children.map((c) => renderNode(doc, theme, runtime, c))}
          </ul>`
    }
    case "ListItem": {
      return html`<li>${renderChildren(doc, theme, runtime, children)}</li>`
    }
    case "CodeBlock": {
      const code = asString(props.code) ?? ""
      const language = asString(props.language) ?? ""
      const lang = language.trim().toLowerCase()

      if ((lang === "markdown" || lang === "md") && looksLikeMarkdownTable(code)) {
        // Render tables as real HTML (not preformatted code) so they're legible in decks.
        return html`<div class="w-full overflow-auto">
          ${streamdown(code, { mode: "static", parseIncompleteMarkdown: false })}
        </div>`
      }

      return html`<pre class="rounded border border-border-dark bg-surface-primary p-4 overflow-auto text-[14px] leading-5">
<code class="font-mono" data-language="${language}">${code}</code>
</pre>`
    }
    case "Image": {
      const url =
        (asString(props.url) ??
          (asString(props.assetId) ? resolveAssetImageUrl(doc, String(props.assetId)) : null)) ??
        ""
      const alt = asString(props.alt) ?? ""
      if (!url) return html`<div class="text-xs text-text-dim">[missing image url]</div>`
      const objectFit = asString(props.objectFit) ?? "contain"
      const fill = objectFit === "cover"
      const maxW = asNumber(props.maxWidth)
      const maxH = asNumber(props.maxHeight)
      const sizeStyle =
        maxW != null || maxH != null
          ? [maxW != null ? `max-width:${px(maxW)}` : "", maxH != null ? `max-height:${px(maxH)}` : ""]
              .filter(Boolean)
              .join("; ")
          : ""
      const imgClass = fill
        ? "absolute inset-0 w-full h-full object-cover"
        : "max-w-full max-h-full object-contain"
      const wrapperClass = fill ? "absolute inset-0" : ""
      if (fill) {
        return html`<div class="${wrapperClass}"><img src="${url}" alt="${alt}" class="${imgClass}" /></div>`
      }
      if (sizeStyle) {
        return html`<img src="${url}" alt="${alt}" class="${imgClass}" style="${sizeStyle}" />`
      }
      return html`<img src="${url}" alt="${alt}" class="${imgClass}" />`
    }
    case "Header":
    case "Footer": {
      const left = props.left
      const center = props.center
      const right = props.right
      const toNodes = (v: unknown): ReadonlyArray<DeckNodeChild> =>
        Array.isArray(v) ? (v as any) : v ? [v as any] : []
      return html`<div class="flex items-center justify-between w-full text-[14px] text-text-dim">
        <div class="flex items-center gap-2">${toNodes(left).map((c) => renderNode(doc, theme, runtime, c))}</div>
        <div class="flex items-center gap-2">${toNodes(center).map((c) => renderNode(doc, theme, runtime, c))}</div>
        <div class="flex items-center gap-2">${toNodes(right).map((c) => renderNode(doc, theme, runtime, c))}</div>
      </div>`
    }
    case "SlideNumber": {
      const format = asString(props.format) ?? "current/total"
      const current = runtime.slideIndex + 1
      const total = runtime.slideCount
      const text = format === "current" ? String(current) : format === "total" ? String(total) : `${current}/${total}`
      return html`<span class="text-[12px] text-text-dim font-mono">${text}</span>`
    }
    case "Embed": {
      const src = asString(props.src) ?? ""
      const title = asString(props.title) ?? "Embedded content"
      const minHeight = asNumber(props.minHeight) ?? 320
      if (!src) return html`<div class="text-xs text-text-dim rounded border border-border-dark p-3">[Embed: missing src]</div>`
      return html`<iframe
        src="${src}"
        title="${title}"
        class="w-full h-full min-w-0 min-h-0 rounded border-0 bg-bg-primary overflow-hidden"
        style="min-height:${px(minHeight)}; height:100%;"
        referrerpolicy="no-referrer"
      ></iframe>`
    }
    case "Story": {
      const storyId = asString(props.storyId) ?? ""
      if (!storyId) return html`<div class="text-xs text-text-dim rounded border border-border-dark p-3">[Story: missing storyId]</div>`
      const story =
        DECK_STORY_BY_ID[storyId] ??
        getStoryById(storyId) ??
        (() => {
          for (const s of autopilotStories) if (s.id === storyId) return s
          for (const s of homeStories) if (s.id === storyId) return s
          return null
        })()
      if (!story) return html`<div class="text-xs text-text-dim rounded border border-border-dark p-3">[Story not found: ${storyId}]</div>`
      return html`<div class="h-full w-full min-h-0 min-w-0 overflow-hidden">${story.render()}</div>`
    }
    default:
      return renderUnknownNode(node.type)
  }
}

const computeAspectRatio = (doc: DeckDocument): { readonly w: number; readonly h: number } => {
  const size = doc.deck.size
  if (size && Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0) {
    return { w: size.width, h: size.height }
  }

  const ar = doc.deck.aspectRatio
  if (typeof ar === "string") {
    const m = ar.match(/^(\d+)\s*:\s*(\d+)$/)
    if (m) {
      const w = Number(m[1])
      const h = Number(m[2])
      if (Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0) return { w, h }
    }
  }

  return { w: 16, h: 9 }
}

export type DeckRenderInput = {
  readonly doc: DeckDocument
  readonly slideIndex: number
  readonly stepIndex: number
  readonly presenting?: boolean
}

export type DeckRenderOutput = {
  readonly slideIndex: number
  readonly slideCount: number
  readonly stepIndex: number
  readonly totalSteps: number
  readonly slideId: string
  readonly deckTitle: string | null
  readonly template: TemplateResult
}

export const renderDeck = (input: DeckRenderInput): DeckRenderOutput => {
  const doc = input.doc
  const presenting = Boolean(input.presenting)
  const theme: DeckTheme | undefined = doc.theme ? { tokens: doc.theme.tokens, defaults: doc.theme.defaults } : undefined

  const slides = doc.deck.slides
  const slideCount = slides.length
  const slideIndex = Math.max(0, Math.min(slideCount - 1, input.slideIndex))
  const slide = slides[slideIndex]!

  const nodes = expandSlideContent(doc, slide)
  const totalSteps = computeSlideTotalSteps(nodes)
  const stepIndex = Math.max(1, Math.min(totalSteps, input.stepIndex))

  const filteredNodes = nodes
    .map((n) => filterByBuild(n, stepIndex))
    .filter((n): n is DeckNodeChild => n != null)

  const deckTitle =
    doc.meta && isRecord(doc.meta) && typeof (doc.meta as any).title === "string" ? String((doc.meta as any).title) : null

  const { w, h } = computeAspectRatio(doc)
  const ratio = w / h
  const slideWidth = `min(100%, calc(100vh * ${ratio.toFixed(6)}))`

  const slideBgResolved = resolveTokenValue(theme, slide.background)
  const surfaceBg = typeof slideBgResolved === "string" ? slideBgResolved : "rgba(0,0,0,0.35)"
  /* Slightly transparent in presenting so shell dots grid shows through */
  const surfaceBgPresenting = "rgba(0,0,0,0.25)"

  const runtime = { slideIndex, slideCount, stepIndex, totalSteps }
  const isFullbleed = slide.layout === "solution" || slide.layout === "fullbleed"

  const template = html`
    <div class="relative w-full h-full min-h-0 overflow-hidden text-text-primary font-mono">
      ${isFullbleed && presenting
        ? html`
            <div class="absolute inset-0 z-0 min-w-0 min-h-0 flex flex-col h-full" style="min-height:100%;">
              ${filteredNodes.map((n) => renderNode(doc, theme, runtime, n))}
            </div>
          `
        : presenting
          ? html`
            <div
              class="absolute inset-0 z-0 flex flex-col min-h-0"
              style="background:${surfaceBgPresenting};"
            >
              <div class="flex-1 min-h-0 flex flex-col">
                ${filteredNodes.map((n) => renderNode(doc, theme, runtime, n))}
              </div>
            </div>
          `
          : html`
      <div class="absolute inset-0 flex items-center justify-center p-6">
        <div
          class="relative shadow-[0_0_0_1px_rgba(255,255,255,0.08),0_20px_80px_rgba(0,0,0,0.65)] rounded overflow-hidden border border-border-dark backdrop-blur-md"
          style="aspect-ratio: ${w} / ${h}; width: ${slideWidth}; background:${surfaceBg};"
        >
          <div class="absolute inset-0 z-0 flex flex-col min-h-0">
            ${filteredNodes.map((n) => renderNode(doc, theme, runtime, n))}
          </div>
        </div>
      </div>

      <div class="absolute top-4 right-4 flex items-center gap-2">
        ${hatcheryActionButton({
          label: "Fullscreen",
          action: "toggle-fullscreen",
          subtle: false,
        })}
      </div>
          `}

      ${presenting
        ? null
        : html`
            <div class="absolute top-4 left-4 text-[12px] text-text-dim">
              ${deckTitle ? deckTitle : "Deck"}
            </div>
            <div class="absolute bottom-4 right-4 text-[12px] text-text-dim font-mono">
              ${slideIndex + 1}/${slideCount}
            </div>
            <div class="absolute bottom-4 left-4 text-[12px] text-text-dim">
              <span class="opacity-80">Keys:</span> <span class="font-mono">←/→</span> step,
              <span class="font-mono">PgUp/PgDn</span> slide, <span class="font-mono">R</span> reload,
              <span class="font-mono">F</span> fullscreen
            </div>
          `}
    </div>
  `

  return { slideIndex, slideCount, stepIndex, totalSteps, slideId: slide.id, deckTitle, template }
}

export const parseDeckJsonString = (text: string): DeckParseResult =>
  (() => {
    try {
      const json = JSON.parse(text)
      return parseDeckDocument(json)
    } catch (err) {
      return { _tag: "Error", message: err instanceof Error ? err.message : "Invalid JSON." }
    }
  })()
