import { Effect, Scope } from "effect"

export type KhalaBackgroundQuality = "constrained" | "balanced" | "high"
export type KhalaDotShape = "box" | "circle" | "cross"

interface KhalaBackgroundBase {
  readonly seed?: number
  readonly color: string
}

export interface KhalaDotsBackground extends KhalaBackgroundBase {
  readonly kind: "dots"
  readonly shape: KhalaDotShape
  readonly spacing?: number
  readonly size?: number
  readonly origin?: readonly [number, number]
  readonly inverted?: boolean
}

export interface KhalaGridLinesBackground extends KhalaBackgroundBase {
  readonly kind: "grid-lines"
  readonly spacing?: number
  readonly lineWidth?: number
  readonly horizontalDash?: ReadonlyArray<number>
  readonly verticalDash?: ReadonlyArray<number>
}

export interface KhalaMovingLinesBackground extends KhalaBackgroundBase {
  readonly kind: "moving-lines"
  readonly count?: number
  readonly direction?: "up" | "down" | "left" | "right"
  readonly lineWidth?: number
  readonly length?: number
  readonly speed?: number
  readonly glow?: number
}

export interface KhalaPuffsBackground extends KhalaBackgroundBase {
  readonly kind: "puffs"
  readonly count?: number
  readonly minRadius?: number
  readonly maxRadius?: number
  readonly speed?: number
  readonly padding?: number
}

export type KhalaCanvasBackground =
  | KhalaDotsBackground
  | KhalaGridLinesBackground
  | KhalaMovingLinesBackground
  | KhalaPuffsBackground

export type KhalaBackgroundPrimitive =
  | { readonly kind: "dot"; readonly shape: KhalaDotShape; readonly x: number; readonly y: number; readonly size: number; readonly color: string; readonly opacity: number }
  | { readonly kind: "line"; readonly x1: number; readonly y1: number; readonly x2: number; readonly y2: number; readonly width: number; readonly color: string; readonly opacity: number; readonly dash: ReadonlyArray<number>; readonly glow: number }
  | { readonly kind: "puff"; readonly x: number; readonly y: number; readonly radius: number; readonly color: string; readonly opacity: number }

export interface KhalaBackgroundFrame {
  readonly width: number
  readonly height: number
  readonly progress: number
  readonly primitives: ReadonlyArray<KhalaBackgroundPrimitive>
}

const bound = (value: number, minimum: number, maximum: number): number =>
  Math.min(maximum, Math.max(minimum, Number.isFinite(value) ? value : minimum))

const seededRandom = (seed: number): (() => number) => {
  let value = (Number.isFinite(seed) ? Math.round(seed) : 1) | 0
  if (value === 0) value = 0x6d2b79f5
  return () => {
    value ^= value << 13
    value ^= value >>> 17
    value ^= value << 5
    return (value >>> 0) / 0x1_0000_0000
  }
}

const qualityLimit = (quality: KhalaBackgroundQuality): number =>
  quality === "constrained" ? 96 : quality === "balanced" ? 256 : 512

const dotsFrame = (
  descriptor: KhalaDotsBackground,
  width: number,
  height: number,
  progress: number,
  quality: KhalaBackgroundQuality
): ReadonlyArray<KhalaBackgroundPrimitive> => {
  const spacing = bound(descriptor.spacing ?? 32, 8, 256)
  const size = bound(descriptor.size ?? 2, 1, spacing / 2)
  const columns = Math.ceil(width / spacing) + 1
  const rows = Math.ceil(height / spacing) + 1
  const originX = bound(descriptor.origin?.[0] ?? 0.5, 0, 1) * width
  const originY = bound(descriptor.origin?.[1] ?? 0.5, 0, 1) * height
  const maximumDistance = Math.max(1, Math.hypot(width, height))
  const limit = qualityLimit(quality)
  const values: Array<KhalaBackgroundPrimitive> = []
  for (let row = 0; row < rows && values.length < limit; row += 1) {
    for (let column = 0; column < columns && values.length < limit; column += 1) {
      const x = column * spacing
      const y = row * spacing
      const distance = Math.hypot(x - originX, y - originY) / maximumDistance
      const revealed = bound((progress - distance * 0.75) / 0.25, 0, 1)
      values.push({
        kind: "dot",
        shape: descriptor.shape,
        x,
        y,
        size,
        color: descriptor.color,
        opacity: descriptor.inverted === true ? 1 - revealed : revealed
      })
    }
  }
  return values
}

const safeDash = (value: ReadonlyArray<number> | undefined): ReadonlyArray<number> =>
  (value ?? [8, 12]).slice(0, 8).map((part) => bound(part, 0, 256))

const gridFrame = (
  descriptor: KhalaGridLinesBackground,
  width: number,
  height: number,
  progress: number,
  quality: KhalaBackgroundQuality
): ReadonlyArray<KhalaBackgroundPrimitive> => {
  const spacing = bound(descriptor.spacing ?? 48, 12, 512)
  const lineWidth = bound(descriptor.lineWidth ?? 1, 0.25, 16)
  const opacity = bound(progress, 0, 1)
  const values: Array<KhalaBackgroundPrimitive> = []
  const limit = qualityLimit(quality)
  for (let y = 0; y <= height && values.length < limit; y += spacing) {
    values.push({ kind: "line", x1: 0, y1: y, x2: width, y2: y, width: lineWidth, color: descriptor.color, opacity, dash: safeDash(descriptor.horizontalDash), glow: 0 })
  }
  for (let x = 0; x <= width && values.length < limit; x += spacing) {
    values.push({ kind: "line", x1: x, y1: 0, x2: x, y2: height, width: lineWidth, color: descriptor.color, opacity, dash: safeDash(descriptor.verticalDash), glow: 0 })
  }
  return values
}

const movingLinesFrame = (
  descriptor: KhalaMovingLinesBackground,
  width: number,
  height: number,
  progress: number,
  quality: KhalaBackgroundQuality
): ReadonlyArray<KhalaBackgroundPrimitive> => {
  const random = seededRandom(descriptor.seed ?? 1)
  const cap = quality === "constrained" ? 12 : quality === "balanced" ? 32 : 64
  const count = Math.round(bound(descriptor.count ?? 20, 1, cap))
  const direction = descriptor.direction ?? "down"
  const vertical = direction === "up" || direction === "down"
  const extent = vertical ? height : width
  const cross = vertical ? width : height
  const length = bound(descriptor.length ?? extent * 0.16, 4, Math.max(4, extent))
  const speed = bound(descriptor.speed ?? 1, 0, 8)
  const sign = direction === "up" || direction === "left" ? -1 : 1
  return Array.from({ length: count }, () => {
    const crossPosition = random() * cross
    const base = random() * (extent + length) - length
    const travel = ((base + sign * progress * extent * speed + extent + length) % (extent + length)) - length
    const opacity = 0.2 + random() * 0.8
    const common = { kind: "line" as const, width: bound(descriptor.lineWidth ?? 1, 0.25, 12), color: descriptor.color, opacity, dash: [] as ReadonlyArray<number>, glow: bound(descriptor.glow ?? 8, 0, 64) }
    return vertical
      ? { ...common, x1: crossPosition, y1: travel, x2: crossPosition, y2: travel + sign * length }
      : { ...common, x1: travel, y1: crossPosition, x2: travel + sign * length, y2: crossPosition }
  })
}

const puffsFrame = (
  descriptor: KhalaPuffsBackground,
  width: number,
  height: number,
  progress: number,
  quality: KhalaBackgroundQuality
): ReadonlyArray<KhalaBackgroundPrimitive> => {
  const random = seededRandom(descriptor.seed ?? 1)
  const cap = quality === "constrained" ? 8 : quality === "balanced" ? 20 : 40
  const count = Math.round(bound(descriptor.count ?? 12, 1, cap))
  const minimum = bound(descriptor.minRadius ?? 24, 4, 512)
  const maximum = bound(descriptor.maxRadius ?? 120, minimum, 1_024)
  const padding = bound(descriptor.padding ?? maximum, 0, 1_024)
  const speed = bound(descriptor.speed ?? 1, 0, 8)
  return Array.from({ length: count }, () => {
    const phase = (random() + progress * speed) % 1
    return {
      kind: "puff" as const,
      x: -padding + random() * (width + padding * 2),
      y: -padding + random() * (height + padding * 2),
      radius: minimum + (maximum - minimum) * phase,
      color: descriptor.color,
      opacity: Math.sin(Math.PI * phase) * (0.2 + random() * 0.45)
    }
  })
}

export const makeKhalaBackgroundFrame = (
  descriptor: KhalaCanvasBackground,
  logicalWidth: number,
  logicalHeight: number,
  progress: number,
  quality: KhalaBackgroundQuality = "balanced"
): KhalaBackgroundFrame => {
  const width = bound(logicalWidth, 1, 8_192)
  const height = bound(logicalHeight, 1, 8_192)
  const normalized = ((Number.isFinite(progress) ? progress : 0) % 1 + 1) % 1
  const primitives = descriptor.kind === "dots"
    ? dotsFrame(descriptor, width, height, bound(progress, 0, 1), quality)
    : descriptor.kind === "grid-lines"
      ? gridFrame(descriptor, width, height, bound(progress, 0, 1), quality)
      : descriptor.kind === "moving-lines"
        ? movingLinesFrame(descriptor, width, height, normalized, quality)
        : puffsFrame(descriptor, width, height, normalized, quality)
  const frameProgress = descriptor.kind === "dots" || descriptor.kind === "grid-lines"
    ? bound(progress, 0, 1)
    : normalized
  return { width, height, progress: frameProgress, primitives }
}

export const khalaCanvasPixelSize = (
  logicalWidth: number,
  logicalHeight: number,
  devicePixelRatio: number,
  quality: KhalaBackgroundQuality
): { readonly width: number; readonly height: number; readonly dpr: number } => {
  const cap = quality === "constrained" ? 1 : quality === "balanced" ? 1.5 : 2
  const requestedDpr = bound(devicePixelRatio, 1, cap)
  const width = bound(logicalWidth, 1, 8_192)
  const height = bound(logicalHeight, 1, 8_192)
  const maximumPixels = quality === "constrained" ? 2_000_000 : quality === "balanced" ? 6_000_000 : 12_000_000
  const memoryScale = Math.min(1, Math.sqrt(maximumPixels / (width * height * requestedDpr * requestedDpr)))
  const dpr = requestedDpr * memoryScale
  return { width: Math.round(width * dpr), height: Math.round(height * dpr), dpr }
}

export interface KhalaCanvasBackgroundPolicy {
  readonly visible: boolean
  readonly offscreen: boolean
  readonly focused: boolean
  readonly power: "normal" | "low"
  readonly reducedMotion: boolean
  readonly quality: KhalaBackgroundQuality
}

export interface KhalaCanvasFrameScheduler {
  readonly request: (callback: FrameRequestCallback) => number
  readonly cancel: (handle: number) => void
}

export interface KhalaCanvasBackgroundOptions {
  readonly policy?: Partial<KhalaCanvasBackgroundPolicy>
  readonly scheduler?: KhalaCanvasFrameScheduler
  readonly now?: () => number
  readonly getSize?: () => { readonly width: number; readonly height: number; readonly dpr: number }
  readonly maxActiveSurfaces?: number
}

export interface KhalaCanvasBackgroundSurface {
  readonly setPolicy: (policy: Partial<KhalaCanvasBackgroundPolicy>) => void
  readonly render: () => void
  readonly framesRendered: () => number
  readonly policy: () => KhalaCanvasBackgroundPolicy
}

const defaultPolicy: KhalaCanvasBackgroundPolicy = {
  visible: true,
  offscreen: false,
  focused: true,
  power: "normal",
  reducedMotion: false,
  quality: "balanced"
}

interface AmbientSurfaceRegistration {
  readonly token: symbol
  readonly eligible: () => boolean
  readonly reconcile: () => void
}

const ambientSurfaceRegistry = new Map<symbol, AmbientSurfaceRegistration>()

const drawFrame = (context: CanvasRenderingContext2D, frame: KhalaBackgroundFrame, dpr: number): void => {
  context.setTransform(dpr, 0, 0, dpr, 0, 0)
  context.clearRect(0, 0, frame.width, frame.height)
  for (const primitive of frame.primitives) {
    context.globalAlpha = primitive.opacity
    context.fillStyle = primitive.color
    context.strokeStyle = primitive.color
    if (primitive.kind === "dot") {
      if (primitive.shape === "circle") {
        context.beginPath(); context.arc(primitive.x, primitive.y, primitive.size, 0, Math.PI * 2); context.fill()
      } else if (primitive.shape === "box") {
        context.fillRect(primitive.x - primitive.size, primitive.y - primitive.size, primitive.size * 2, primitive.size * 2)
      } else {
        context.lineWidth = Math.max(1, primitive.size / 2); context.setLineDash([]); context.beginPath()
        context.moveTo(primitive.x - primitive.size, primitive.y); context.lineTo(primitive.x + primitive.size, primitive.y)
        context.moveTo(primitive.x, primitive.y - primitive.size); context.lineTo(primitive.x, primitive.y + primitive.size); context.stroke()
      }
    } else if (primitive.kind === "line") {
      context.lineWidth = primitive.width; context.setLineDash([...primitive.dash]); context.shadowBlur = primitive.glow; context.shadowColor = primitive.color
      context.beginPath(); context.moveTo(primitive.x1, primitive.y1); context.lineTo(primitive.x2, primitive.y2); context.stroke(); context.shadowBlur = 0
    } else {
      const gradient = context.createRadialGradient(primitive.x, primitive.y, 0, primitive.x, primitive.y, primitive.radius)
      gradient.addColorStop(0, primitive.color); gradient.addColorStop(1, "transparent")
      context.fillStyle = gradient; context.beginPath(); context.arc(primitive.x, primitive.y, primitive.radius, 0, Math.PI * 2); context.fill()
    }
  }
  context.globalAlpha = 1
}

const installCanvasBackground = (
  canvas: HTMLCanvasElement,
  descriptor: KhalaCanvasBackground,
  options: KhalaCanvasBackgroundOptions
): { readonly surface: KhalaCanvasBackgroundSurface; readonly dispose: () => void } => {
  const context = canvas.getContext("2d")
  if (context === null) throw new Error("Khala Canvas background requires a 2D context")
  const view = canvas.ownerDocument.defaultView
  const scheduler = options.scheduler ?? {
    request: (callback: FrameRequestCallback) => view?.requestAnimationFrame(callback) ?? setTimeout(() => callback(0), 16) as unknown as number,
    cancel: (handle: number) => view?.cancelAnimationFrame(handle) ?? clearTimeout(handle)
  }
  const now = options.now ?? Date.now
  const previousHidden = canvas.getAttribute("aria-hidden")
  const previousPointerEvents = canvas.style.pointerEvents
  let policy = { ...defaultPolicy, ...options.policy }
  let frameHandle: number | undefined
  let started = now()
  let count = 0
  let disposed = false
  const token = Symbol("khala-canvas-background")
  const maximumActive = Math.round(bound(options.maxActiveSurfaces ?? 4, 1, 16))
  const getSize = options.getSize ?? (() => ({ width: canvas.clientWidth || 1, height: canvas.clientHeight || 1, dpr: view?.devicePixelRatio ?? 1 }))
  const eligible = (): boolean => policy.visible && !policy.offscreen && policy.focused && policy.power === "normal" && !policy.reducedMotion && policy.quality !== "constrained"
  const active = (): boolean =>
    eligible() && [...ambientSurfaceRegistry.values()].filter((entry) => entry.eligible()).slice(0, maximumActive).some((entry) => entry.token === token)
  const render = (): void => {
    const size = getSize()
    const pixels = khalaCanvasPixelSize(size.width, size.height, size.dpr, policy.quality)
    if (canvas.width !== pixels.width) canvas.width = pixels.width
    if (canvas.height !== pixels.height) canvas.height = pixels.height
    const progress = active() ? (now() - started) / 8_000 : 1
    drawFrame(context, makeKhalaBackgroundFrame(descriptor, size.width, size.height, progress, policy.quality), pixels.dpr)
    count += 1
  }
  const tick = (): void => {
    frameHandle = undefined
    if (disposed || !active()) return
    render()
    frameHandle = scheduler.request(tick)
  }
  const reconcile = (): void => {
    if (frameHandle !== undefined) { scheduler.cancel(frameHandle); frameHandle = undefined }
    render()
    if (active()) frameHandle = scheduler.request(tick)
  }
  const onVisibility = (): void => {
    policy = { ...policy, visible: canvas.ownerDocument.visibilityState !== "hidden" }
    reconcile()
  }
  const onResize = (): void => reconcile()
  const onFocus = (): void => { policy = { ...policy, focused: true }; reconcile() }
  const onBlur = (): void => { policy = { ...policy, focused: false }; reconcile() }
  let hostAttached = false
  let intersectionObserver: IntersectionObserver | undefined
  let resizeObserver: ResizeObserver | undefined
  const installHostPolicy = (): void => {
    if (hostAttached || policy.reducedMotion || policy.quality === "constrained") return
    hostAttached = true
    canvas.ownerDocument.addEventListener("visibilitychange", onVisibility)
    view?.addEventListener("resize", onResize)
    view?.addEventListener("focus", onFocus)
    view?.addEventListener("blur", onBlur)
    if (view?.IntersectionObserver !== undefined) {
      intersectionObserver = new view.IntersectionObserver((entries) => {
        const entry = entries.at(-1)
        if (entry !== undefined) { policy = { ...policy, offscreen: !entry.isIntersecting }; reconcile() }
      })
      intersectionObserver.observe(canvas)
    }
    if (view?.ResizeObserver !== undefined) {
      resizeObserver = new view.ResizeObserver(() => reconcile())
      resizeObserver.observe(canvas)
    }
  }
  const removeHostPolicy = (): void => {
    if (!hostAttached) return
    hostAttached = false
    canvas.ownerDocument.removeEventListener("visibilitychange", onVisibility)
    view?.removeEventListener("resize", onResize)
    view?.removeEventListener("focus", onFocus)
    view?.removeEventListener("blur", onBlur)
    intersectionObserver?.disconnect()
    resizeObserver?.disconnect()
    intersectionObserver = undefined
    resizeObserver = undefined
  }
  const configureHostPolicy = (): void => {
    if (policy.reducedMotion || policy.quality === "constrained") removeHostPolicy()
    else installHostPolicy()
  }
  canvas.setAttribute("aria-hidden", "true")
  canvas.style.pointerEvents = "none"
  ambientSurfaceRegistry.set(token, { token, eligible, reconcile })
  configureHostPolicy()
  reconcile()
  return {
    surface: {
      setPolicy: (next) => {
        policy = { ...policy, ...next }
        if (next.reducedMotion === false) started = now()
        configureHostPolicy()
        reconcile()
        for (const registration of ambientSurfaceRegistry.values()) {
          if (registration.token !== token) registration.reconcile()
        }
      },
      render,
      framesRendered: () => count,
      policy: () => policy
    },
    dispose: () => {
      disposed = true
      if (frameHandle !== undefined) scheduler.cancel(frameHandle)
      ambientSurfaceRegistry.delete(token)
      removeHostPolicy()
      context.clearRect(0, 0, canvas.width, canvas.height)
      if (previousHidden === null) canvas.removeAttribute("aria-hidden")
      else canvas.setAttribute("aria-hidden", previousHidden)
      canvas.style.pointerEvents = previousPointerEvents
      for (const registration of ambientSurfaceRegistry.values()) registration.reconcile()
    }
  }
}

/** Mount one bounded Khala ambient Canvas surface in the caller's Effect Scope. */
export const makeKhalaCanvasBackground = (
  canvas: HTMLCanvasElement,
  descriptor: KhalaCanvasBackground,
  options: KhalaCanvasBackgroundOptions = {}
): Effect.Effect<KhalaCanvasBackgroundSurface, never, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.sync(() => installCanvasBackground(canvas, descriptor, options)),
    (installed) => Effect.sync(installed.dispose)
  ).pipe(Effect.map((installed) => installed.surface))
