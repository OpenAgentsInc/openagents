import { html } from "@openagentsinc/effuse"
import type { TemplateResult } from "@openagentsinc/effuse"

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
      return html`<div
        class="flex flex-col min-h-0"
        style="${gapStyle} align-items:${align}; justify-content:${justify};"
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
          ? "text-[56px] leading-[1.06] tracking-tight font-semibold use-font-square721 [font-family:var(--font-square721)]"
          : style === "h2"
            ? "text-[40px] leading-[1.12] tracking-tight font-semibold use-font-square721 [font-family:var(--font-square721)]"
            : style === "h3"
              ? "text-[28px] leading-[1.18] font-semibold use-font-square721 [font-family:var(--font-square721)]"
              : style === "caption"
                ? "text-[14px] leading-5 text-text-dim"
                : style === "code"
                  ? "text-[18px] leading-6 font-mono"
                  : "text-[22px] leading-7"

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
      return tag === "ol"
        ? html`<ol class="${cls} pl-6" style="display:flex; flex-direction:column; gap:${px(gap)};">
            ${children.map((c) => renderNode(doc, theme, runtime, c))}
          </ol>`
        : html`<ul class="${cls} pl-6" style="display:flex; flex-direction:column; gap:${px(gap)};">
            ${children.map((c) => renderNode(doc, theme, runtime, c))}
          </ul>`
    }
    case "ListItem": {
      return html`<li>${renderChildren(doc, theme, runtime, children)}</li>`
    }
    case "CodeBlock": {
      const code = asString(props.code) ?? ""
      const language = asString(props.language) ?? ""
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
      return html`<img src="${url}" alt="${alt}" class="max-w-full max-h-full object-contain" />`
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

  const runtime = { slideIndex, slideCount, stepIndex, totalSteps }

  const template = html`
    <div class="relative w-full h-full overflow-hidden text-text-primary font-mono">
      <div class="absolute inset-0 flex items-center justify-center p-6">
        <div
          class="relative shadow-[0_0_0_1px_rgba(255,255,255,0.08),0_20px_80px_rgba(0,0,0,0.65)] rounded overflow-hidden border border-border-dark backdrop-blur-md"
          style="aspect-ratio: ${w} / ${h}; width: ${slideWidth}; background:${surfaceBg};"
        >
          <div class="absolute inset-0 p-12 flex flex-col min-h-0">
            ${filteredNodes.map((n) => renderNode(doc, theme, runtime, n))}
          </div>
        </div>
      </div>

      <div class="absolute top-4 right-4 flex items-center gap-2">
        ${hatcheryActionButton({
          label: presenting ? "Exit" : "Fullscreen",
          action: "toggle-fullscreen",
          subtle: presenting,
        })}
        ${presenting
          ? null
          : html`<div class="text-[12px] text-text-dim font-mono">
              ${slide.id}${totalSteps > 1 ? html` · step ${stepIndex}/${totalSteps}` : null}
            </div>`}
      </div>

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
