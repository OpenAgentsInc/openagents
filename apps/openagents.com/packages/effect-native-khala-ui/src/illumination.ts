import { Effect, Scope } from "effect"

export interface KhalaIlluminationDescriptor {
  readonly color: string
  readonly radius: number
  readonly intensity?: number
}

export interface KhalaIlluminationPoint {
  readonly x: number
  readonly y: number
  readonly active: boolean
  readonly source: "pointer" | "focus" | "static"
}

export interface KhalaIlluminationNativePlan {
  readonly kind: "static-outline"
  readonly color: string
  readonly opacity: number
  readonly radius: number
}

export interface KhalaAnimationFrameScheduler {
  readonly request: (callback: FrameRequestCallback) => number
  readonly cancel: (handle: number) => void
}

export interface KhalaDomIlluminatorOptions {
  readonly descriptor: KhalaIlluminationDescriptor
  readonly mode?: "html" | "svg"
  readonly stableId?: string
  readonly reducedMotion?: boolean
  readonly coarsePointer?: boolean
  readonly scheduler?: KhalaAnimationFrameScheduler
  readonly getBounds?: () => DOMRect
  readonly onPoint?: (point: KhalaIlluminationPoint) => void
}

export interface KhalaIlluminatorHandle {
  readonly refreshBounds: () => void
  readonly point: () => KhalaIlluminationPoint
}

const bounded = (value: number, minimum: number, maximum: number): number =>
  Math.min(maximum, Math.max(minimum, Number.isFinite(value) ? value : minimum))

const hashStableId = (value: string): string => {
  let hash = 2166136261
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(36)
}

export const khalaSvgIlluminationGradientId = (stableId: string): string =>
  `en-khala-illumination-${hashStableId(stableId)}`

export const makeKhalaIlluminationNativePlan = (
  descriptor: KhalaIlluminationDescriptor
): KhalaIlluminationNativePlan => ({
  kind: "static-outline",
  color: descriptor.color,
  opacity: bounded(descriptor.intensity ?? 0.42, 0, 1),
  radius: bounded(descriptor.radius, 8, 1_024)
})

const defaultScheduler = (root: Element): KhalaAnimationFrameScheduler => {
  const view = root.ownerDocument.defaultView
  return {
    request: (callback) => view?.requestAnimationFrame(callback) ?? setTimeout(() => callback(0), 16) as unknown as number,
    cancel: (handle) => view?.cancelAnimationFrame(handle) ?? clearTimeout(handle)
  }
}

interface InstalledIlluminator {
  readonly handle: KhalaIlluminatorHandle
  readonly dispose: () => void
}

const installIlluminator = (
  root: HTMLElement | SVGElement,
  layer: HTMLElement | SVGElement,
  options: KhalaDomIlluminatorOptions
): InstalledIlluminator => {
  const descriptor = options.descriptor
  const radius = bounded(descriptor.radius, 8, 1_024)
  const intensity = bounded(descriptor.intensity ?? 0.42, 0, 1)
  const mode = options.mode ?? "html"
  const staticMode = options.reducedMotion === true || options.coarsePointer === true
  if (mode === "svg" && (options.stableId === undefined || options.stableId.length === 0)) {
    throw new Error("SVG illumination requires a caller-owned stableId")
  }
  const previousStyle = layer.getAttribute("style")
  const previousHidden = layer.getAttribute("aria-hidden")
  const previousPointer = layer.getAttribute("pointer-events")
  const previousState = root.getAttribute("data-en-khala-illumination")
  const previousGradientId = layer.getAttribute("id")
  const previousCx = layer.getAttribute("cx")
  const previousCy = layer.getAttribute("cy")
  const previousRadius = layer.getAttribute("r")
  const previousOpacity = layer.getAttribute("data-en-khala-opacity")
  let bounds = options.getBounds?.() ?? root.getBoundingClientRect()
  let current: KhalaIlluminationPoint = {
    x: bounds.width / 2,
    y: bounds.height / 2,
    active: staticMode,
    source: "static"
  }
  let pending: KhalaIlluminationPoint | undefined
  let frame: number | undefined
  const scheduler = options.scheduler ?? defaultScheduler(root)

  layer.setAttribute("aria-hidden", "true")
  layer.setAttribute("pointer-events", "none")
  root.setAttribute("data-en-khala-illumination", staticMode ? "static" : "idle")
  if (mode === "svg") {
    layer.setAttribute("id", khalaSvgIlluminationGradientId(options.stableId!))
  }

  const paint = (point: KhalaIlluminationPoint): void => {
    current = point
    root.setAttribute("data-en-khala-illumination", point.active ? point.source : "idle")
    if (mode === "svg") {
      layer.setAttribute("cx", String(point.x))
      layer.setAttribute("cy", String(point.y))
      layer.setAttribute("r", String(radius))
      layer.setAttribute("data-en-khala-opacity", String(intensity))
    } else {
      layer.style.background = `radial-gradient(circle ${radius}px at ${point.x}px ${point.y}px, color-mix(in srgb, ${descriptor.color} ${Math.round(intensity * 100)}%, transparent), transparent 100%)`
      layer.style.opacity = point.active ? "1" : "0"
    }
    options.onPoint?.(point)
  }

  const queuePaint = (point: KhalaIlluminationPoint): void => {
    pending = point
    if (frame !== undefined) return
    frame = scheduler.request(() => {
      frame = undefined
      const next = pending
      pending = undefined
      if (next !== undefined) paint(next)
    })
  }
  const refreshBounds = (): void => {
    bounds = options.getBounds?.() ?? root.getBoundingClientRect()
  }
  const localPoint = (event: PointerEvent): KhalaIlluminationPoint => ({
    x: bounded(event.clientX - bounds.left, 0, Math.max(0, bounds.width)),
    y: bounded(event.clientY - bounds.top, 0, Math.max(0, bounds.height)),
    active: true,
    source: "pointer"
  })
  const onPointerEnter = (event: Event): void => {
    refreshBounds()
    queuePaint(localPoint(event as PointerEvent))
  }
  const onPointerMove = (event: Event): void => queuePaint(localPoint(event as PointerEvent))
  const onPointerLeave = (): void => queuePaint({ ...current, active: false, source: "pointer" })
  const onFocusIn = (): void => {
    refreshBounds()
    queuePaint({ x: bounds.width / 2, y: bounds.height / 2, active: true, source: "focus" })
  }
  const onFocusOut = (): void => queuePaint({ ...current, active: false, source: "focus" })
  const onResize = (): void => refreshBounds()

  if (staticMode) {
    paint(current)
  } else {
    root.addEventListener("pointerenter", onPointerEnter)
    root.addEventListener("pointermove", onPointerMove)
    root.addEventListener("pointerleave", onPointerLeave)
    root.addEventListener("focusin", onFocusIn)
    root.addEventListener("focusout", onFocusOut)
    root.ownerDocument.defaultView?.addEventListener("resize", onResize)
    paint(current)
  }

  return {
    handle: { refreshBounds, point: () => current },
    dispose: () => {
      root.removeEventListener("pointerenter", onPointerEnter)
      root.removeEventListener("pointermove", onPointerMove)
      root.removeEventListener("pointerleave", onPointerLeave)
      root.removeEventListener("focusin", onFocusIn)
      root.removeEventListener("focusout", onFocusOut)
      root.ownerDocument.defaultView?.removeEventListener("resize", onResize)
      if (frame !== undefined) scheduler.cancel(frame)
      if (previousStyle === null) layer.removeAttribute("style")
      else layer.setAttribute("style", previousStyle)
      if (previousHidden === null) layer.removeAttribute("aria-hidden")
      else layer.setAttribute("aria-hidden", previousHidden)
      if (previousPointer === null) layer.removeAttribute("pointer-events")
      else layer.setAttribute("pointer-events", previousPointer)
      if (previousState === null) root.removeAttribute("data-en-khala-illumination")
      else root.setAttribute("data-en-khala-illumination", previousState)
      if (mode === "svg") {
        if (previousGradientId === null) layer.removeAttribute("id")
        else layer.setAttribute("id", previousGradientId)
        if (previousCx === null) layer.removeAttribute("cx")
        else layer.setAttribute("cx", previousCx)
        if (previousCy === null) layer.removeAttribute("cy")
        else layer.setAttribute("cy", previousCy)
        if (previousRadius === null) layer.removeAttribute("r")
        else layer.setAttribute("r", previousRadius)
        if (previousOpacity === null) layer.removeAttribute("data-en-khala-opacity")
        else layer.setAttribute("data-en-khala-opacity", previousOpacity)
      }
    }
  }
}

/** Acquire a container-local illuminator; the enclosing Effect Scope owns cleanup. */
export const makeKhalaDomIlluminator = (
  root: HTMLElement | SVGElement,
  layer: HTMLElement | SVGElement,
  options: KhalaDomIlluminatorOptions
): Effect.Effect<KhalaIlluminatorHandle, never, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.sync(() => installIlluminator(root, layer, options)),
    (installed) => Effect.sync(installed.dispose)
  ).pipe(Effect.map((installed) => installed.handle))
