import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import { GridPattern } from "./GridPattern.js"
import type { Point } from "./layout-engine.js"

const DEFAULT_MIN_ZOOM = 0.5
const DEFAULT_MAX_ZOOM = 3
const DEFAULT_ZOOM = 1
const DEFAULT_ZOOM_SPEED = 0.001
const DEFAULT_GRID_SIZE = 24
const DEFAULT_DOT_RADIUS = 0.8
const PRIMARY_MOUSE_BUTTON = 0
const FRICTION = 0.92
const MIN_VELOCITY = 0.5

export type InfiniteCanvasProps = {
  /** Unique id used to namespace SVG ids (pattern ids). */
  readonly id: string
  readonly minZoom?: number
  readonly maxZoom?: number
  readonly defaultZoom?: number
  readonly zoomSpeed?: number
  readonly gridSize?: number
  readonly dotRadius?: number
  readonly dotClassName?: string
  readonly showGrid?: boolean
  readonly className?: string
  readonly children: TemplateResult
  readonly overlay?: TemplateResult
}

const cx = (...parts: Array<string | null | undefined | false>): string =>
  parts.filter(Boolean).join(" ")

export function InfiniteCanvas({
  id,
  minZoom = DEFAULT_MIN_ZOOM,
  maxZoom = DEFAULT_MAX_ZOOM,
  defaultZoom = DEFAULT_ZOOM,
  zoomSpeed = DEFAULT_ZOOM_SPEED,
  gridSize = DEFAULT_GRID_SIZE,
  dotRadius = DEFAULT_DOT_RADIUS,
  dotClassName = "oa-flow-grid-dot",
  showGrid = true,
  className,
  children,
  overlay,
}: InfiniteCanvasProps): TemplateResult {
  const patternId = `oa-flow-dot-grid-${id}`

  // Note: We intentionally render a stable SVG shell and mutate the transform group
  // imperatively in hydrateInfiniteCanvas for smooth pan/zoom (no full re-render loop).
  return html`
    <div class="${cx("oa-flow-canvas", className)}" data-oa-flow-canvas-root="${id}">
      <svg
        class="oa-flow-canvas__svg"
        data-oa-flow-canvas-svg="1"
        data-oa-flow-canvas-min-zoom="${String(minZoom)}"
        data-oa-flow-canvas-max-zoom="${String(maxZoom)}"
        data-oa-flow-canvas-default-zoom="${String(defaultZoom)}"
        data-oa-flow-canvas-zoom-speed="${String(zoomSpeed)}"
      >
        <g data-oa-flow-canvas-transform="1" transform="translate(0,0)scale(1)">
          ${showGrid
            ? GridPattern({
                gridSize,
                dotRadius,
                dotClassName,
                patternId,
              })
            : html``}
          <g data-oa-flow-canvas-content="1">${children}</g>
        </g>
      </svg>
      <div class="oa-flow-canvas__overlay" data-oa-flow-canvas-overlay="1">
        ${overlay ?? html``}
      </div>
    </div>
  `
}

export type InfiniteCanvasHydrateOptions = {
  readonly minZoom: number
  readonly maxZoom: number
  readonly defaultZoom: number
  readonly zoomSpeed: number
}

export type InfiniteCanvasHandle = {
  readonly destroy: () => void
}

const clamp = (n: number, min: number, max: number): number => Math.max(min, Math.min(max, n))

export const hydrateInfiniteCanvas = (
  root: Element,
  options?: Partial<InfiniteCanvasHydrateOptions>,
): InfiniteCanvasHandle => {
  const svg =
    root instanceof SVGSVGElement
      ? root
      : (root.querySelector('svg[data-oa-flow-canvas-svg="1"]') as SVGSVGElement | null)

  const transformGroup = svg?.querySelector('g[data-oa-flow-canvas-transform="1"]') as SVGGElement | null
  if (!svg || !transformGroup) {
    return { destroy: () => {} }
  }

  const readNumberAttr = (name: string, fallback: number): number => {
    const raw = svg.getAttribute(name)
    const n = raw == null ? NaN : Number(raw)
    return Number.isFinite(n) ? n : fallback
  }

  const minZoom = options?.minZoom ?? readNumberAttr("data-oa-flow-canvas-min-zoom", DEFAULT_MIN_ZOOM)
  const maxZoom = options?.maxZoom ?? readNumberAttr("data-oa-flow-canvas-max-zoom", DEFAULT_MAX_ZOOM)
  const defaultZoom = options?.defaultZoom ?? readNumberAttr("data-oa-flow-canvas-default-zoom", DEFAULT_ZOOM)
  const zoomSpeed = options?.zoomSpeed ?? readNumberAttr("data-oa-flow-canvas-zoom-speed", DEFAULT_ZOOM_SPEED)

  let isPanning = false
  let scale = defaultZoom
  let offset: Point = { x: 0, y: 0 }
  let startPan: Point = { x: 0, y: 0 }
  let lastPos: Point = { x: 0, y: 0 }
  let velocity: Point = { x: 0, y: 0 }
  let animationFrame: number | null = null

  const setTransform = () => {
    transformGroup.setAttribute("transform", `translate(${offset.x},${offset.y})scale(${scale})`)
  }

  const stopInertia = () => {
    if (animationFrame != null) {
      cancelAnimationFrame(animationFrame)
      animationFrame = null
    }
    velocity = { x: 0, y: 0 }
  }

  const animate = () => {
    if (Math.abs(velocity.x) < MIN_VELOCITY && Math.abs(velocity.y) < MIN_VELOCITY) {
      velocity = { x: 0, y: 0 }
      animationFrame = null
      return
    }

    velocity = { x: velocity.x * FRICTION, y: velocity.y * FRICTION }
    offset = { x: offset.x + velocity.x, y: offset.y + velocity.y }
    setTransform()
    animationFrame = requestAnimationFrame(animate)
  }

  const onMouseDown = (e: MouseEvent) => {
    if (e.button !== PRIMARY_MOUSE_BUTTON) return
    stopInertia()
    isPanning = true
    ;(root as any).dataset.oaFlowPanning = "1"
    lastPos = { x: e.clientX, y: e.clientY }
    startPan = { x: e.clientX - offset.x, y: e.clientY - offset.y }
  }

  const onMouseMove = (e: MouseEvent) => {
    if (!isPanning) return
    velocity = { x: e.clientX - lastPos.x, y: e.clientY - lastPos.y }
    lastPos = { x: e.clientX, y: e.clientY }
    offset = { x: e.clientX - startPan.x, y: e.clientY - startPan.y }
    setTransform()
  }

  const onMouseUp = () => {
    if (!isPanning) return
    isPanning = false
    ;(root as any).dataset.oaFlowPanning = "0"
    if (Math.abs(velocity.x) > MIN_VELOCITY || Math.abs(velocity.y) > MIN_VELOCITY) {
      animationFrame = requestAnimationFrame(animate)
    }
  }

  const onWheel = (e: WheelEvent) => {
    e.preventDefault()
    e.stopPropagation()
    stopInertia()

    const rect = svg.getBoundingClientRect()
    const mouseX = e.clientX - rect.left
    const mouseY = e.clientY - rect.top

    const delta = e.deltaY * -zoomSpeed
    const newScale = clamp(scale + delta, minZoom, maxZoom)
    if (newScale === scale) return

    const scaleRatio = newScale / scale
    offset = {
      x: mouseX - (mouseX - offset.x) * scaleRatio,
      y: mouseY - (mouseY - offset.y) * scaleRatio,
    }
    scale = newScale
    setTransform()
  }

  // Initial centering (matches the old React implementation).
  const center = () => {
    const rect = svg.getBoundingClientRect()
    offset = { x: rect.width / 2, y: rect.height / 6 }
    scale = defaultZoom
    setTransform()
  }

  // Center after layout; defer to the next frame so the SVG has correct size.
  const raf = requestAnimationFrame(center)

  svg.addEventListener("mousedown", onMouseDown)
  svg.addEventListener("mousemove", onMouseMove)
  svg.addEventListener("mouseup", onMouseUp)
  svg.addEventListener("mouseleave", onMouseUp)
  svg.addEventListener("wheel", onWheel, { passive: false })

  return {
    destroy: () => {
      cancelAnimationFrame(raf)
      stopInertia()
      svg.removeEventListener("mousedown", onMouseDown)
      svg.removeEventListener("mousemove", onMouseMove)
      svg.removeEventListener("mouseup", onMouseUp)
      svg.removeEventListener("mouseleave", onMouseUp)
      svg.removeEventListener("wheel", onWheel as any)
    },
  }
}
