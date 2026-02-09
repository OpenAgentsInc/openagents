import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import type { Point } from "./layout-engine.js"
import {
  type LineTo,
  type PathCommand,
  type QuadraticCurve,
  curve,
  line,
  move,
  renderPath,
} from "./path-commands.js"

const CORNER_RADIUS = 32

/** Preset names for connection line animation. */
export type PresetName = "dots" | "dashes" | "dots-slow" | "dashes-fast" | "pulse"

/** Resolved animation values used for SVG stroke/dash. */
type ResolvedAnimation = {
  readonly dashLength: number
  readonly gapLength: number
  readonly speed: number
  readonly strokeWidth: number
  readonly color?: string
}

/** Either a preset (with optional color override) or fully custom values. */
export type AnimationConfig =
  | { readonly preset: PresetName; readonly color?: string }
  | {
      readonly custom: Partial<{
        readonly dashLength: number
        readonly gapLength: number
        readonly speed: number
        readonly strokeWidth: number
        readonly color: string
      }>
    }

export const ANIMATION_PRESETS: Record<
  PresetName,
  { readonly dashLength: number; readonly gapLength: number; readonly speed: number; readonly strokeWidth: number; readonly color: string }
> = {
  // `speed` is the SVG animate `dur` in seconds; lower = faster.
  dots: { dashLength: 2, gapLength: 8, speed: 1.2, strokeWidth: 2.5, color: "var(--oa-flow-connection-stroke)" },
  dashes: { dashLength: 8, gapLength: 6, speed: 1.1, strokeWidth: 2.5, color: "var(--oa-flow-connection-stroke)" },
  "dots-slow": { dashLength: 2, gapLength: 8, speed: 1.8, strokeWidth: 2.5, color: "var(--oa-flow-connection-stroke)" },
  "dashes-fast": { dashLength: 8, gapLength: 6, speed: 0.45, strokeWidth: 2.5, color: "var(--oa-flow-connection-stroke)" },
  pulse: { dashLength: 4, gapLength: 4, speed: 0.85, strokeWidth: 2.5, color: "var(--oa-flow-connection-stroke)" },
}

const DEFAULT_PRESET: PresetName = "dots"

function resolveAnimation(config?: AnimationConfig): ResolvedAnimation {
  if (!config) {
    const p = ANIMATION_PRESETS[DEFAULT_PRESET]
    return { ...p }
  }
  if ("preset" in config) {
    const p = ANIMATION_PRESETS[config.preset]
    return { ...p, ...(config.color != null ? { color: config.color } : {}) }
  }
  const base = ANIMATION_PRESETS[DEFAULT_PRESET]
  const c = config.custom ?? {}
  return {
    dashLength: c.dashLength ?? base.dashLength,
    gapLength: c.gapLength ?? base.gapLength,
    speed: c.speed ?? base.speed,
    strokeWidth: c.strokeWidth ?? base.strokeWidth,
    color: c.color ?? base.color,
  }
}

export type TreeConnectionLineProps = {
  readonly path: ReadonlyArray<Point>
  readonly animation?: AnimationConfig
}

export function buildPathD(points: ReadonlyArray<Point>): string {
  if (points.length < 2) return ""
  if (points.length === 2) return renderPath([move(points[0]!), line(points[1]!)])

  const commands: PathCommand[] = [move(points[0]!)]

  for (let i = 0; i < points.length - 1; i++) {
    const current = points[i]!
    const next = points[i + 1]!
    const afterNext = points[i + 2]

    if (afterNext === undefined) {
      commands.push(line(next))
      continue
    }

    if (hasCorner(current, next, afterNext)) {
      commands.push(...buildRoundedCorner(current, next, afterNext))
    } else {
      commands.push(line(next))
    }
  }

  return renderPath(commands)
}

function hasCorner(current: Point, corner: Point, next: Point): boolean {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y }
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y }

  return (
    (toCorner.dx !== 0 &&
      toCorner.dy === 0 &&
      fromCorner.dx === 0 &&
      fromCorner.dy !== 0) ||
    (toCorner.dx === 0 &&
      toCorner.dy !== 0 &&
      fromCorner.dx !== 0 &&
      fromCorner.dy === 0)
  )
}

function buildRoundedCorner(current: Point, corner: Point, next: Point): [LineTo, QuadraticCurve] {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y }
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y }

  const entryDistance = Math.sqrt(toCorner.dx ** 2 + toCorner.dy ** 2)
  const entryRadius = Math.min(CORNER_RADIUS, entryDistance / 2)
  const entryRatio = (entryDistance - entryRadius) / entryDistance

  const entryPoint = {
    x: current.x + toCorner.dx * entryRatio,
    y: current.y + toCorner.dy * entryRatio,
  }

  const exitDistance = Math.sqrt(fromCorner.dx ** 2 + fromCorner.dy ** 2)
  const exitRadius = Math.min(CORNER_RADIUS, exitDistance / 2)
  const exitRatio = exitRadius / exitDistance

  const exitPoint = {
    x: corner.x + fromCorner.dx * exitRatio,
    y: corner.y + fromCorner.dy * exitRatio,
  }

  return [line(entryPoint), curve(corner, exitPoint)]
}

export function TreeConnectionLine({ path, animation }: TreeConnectionLineProps): TemplateResult {
  const pathD = buildPathD(path)
  const resolved = resolveAnimation(animation)
  const { dashLength, gapLength, speed, strokeWidth, color } = resolved
  const dashTotal = dashLength + gapLength

  return html`
    <path
      d="${pathD}"
      stroke="${color}"
      stroke-width="${String(strokeWidth)}"
      fill="none"
      stroke-linecap="round"
      stroke-dasharray="${`${dashLength} ${gapLength}`}"
    >
      <animate
        attributeName="stroke-dashoffset"
        to="${`-${dashTotal}`}"
        from="0"
        dur="${`${speed}s`}"
        repeatCount="indefinite"
      />
    </path>
  `
}
