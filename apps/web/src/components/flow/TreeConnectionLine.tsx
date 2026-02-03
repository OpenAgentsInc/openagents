import { useMemo } from 'react';
import type { Point } from './layout-engine';
import {
  type LineTo,
  type PathCommand,
  type QuadraticCurve,
  curve,
  line,
  move,
  renderPath,
} from './path-commands';

const CORNER_RADIUS = 32;

/** Preset names for connection line animation. */
export type PresetName = 'dots' | 'dashes' | 'dots-slow' | 'dashes-fast' | 'pulse';

/** Resolved animation values used for SVG stroke/dash. */
type ResolvedAnimation = {
  dashLength: number;
  gapLength: number;
  speed: number;
  strokeWidth: number;
  color?: string;
};

/** Either a preset (with optional color override) or fully custom values. */
export type AnimationConfig =
  | { preset: PresetName; color?: string }
  | {
      custom: Partial<{
        dashLength: number;
        gapLength: number;
        speed: number;
        strokeWidth: number;
        color: string;
      }>;
    };

export const ANIMATION_PRESETS: Record<
  PresetName,
  { dashLength: number; gapLength: number; speed: number; strokeWidth: number; color: string }
> = {
  dots: { dashLength: 2, gapLength: 8, speed: 2, strokeWidth: 2.5, color: 'hsl(var(--muted-foreground) / 0.6)' },
  dashes: { dashLength: 8, gapLength: 6, speed: 2, strokeWidth: 2.5, color: 'hsl(var(--muted-foreground) / 0.6)' },
  'dots-slow': { dashLength: 2, gapLength: 8, speed: 3.5, strokeWidth: 2.5, color: 'hsl(var(--muted-foreground) / 0.6)' },
  'dashes-fast': { dashLength: 8, gapLength: 6, speed: 1, strokeWidth: 2.5, color: 'hsl(var(--muted-foreground) / 0.6)' },
  pulse: { dashLength: 4, gapLength: 4, speed: 1.5, strokeWidth: 2.5, color: 'hsl(var(--muted-foreground) / 0.6)' },
};

const DEFAULT_PRESET: PresetName = 'dots';

function resolveAnimation(config?: AnimationConfig): ResolvedAnimation {
  if (!config) {
    const p = ANIMATION_PRESETS[DEFAULT_PRESET];
    return { ...p };
  }
  if ('preset' in config) {
    const p = ANIMATION_PRESETS[config.preset];
    return { ...p, ...(config.color != null ? { color: config.color } : {}) };
  }
  const base = ANIMATION_PRESETS[DEFAULT_PRESET];
  const c = config.custom ?? {};
  return {
    dashLength: c.dashLength ?? base.dashLength,
    gapLength: c.gapLength ?? base.gapLength,
    speed: c.speed ?? base.speed,
    strokeWidth: c.strokeWidth ?? base.strokeWidth,
    color: c.color ?? base.color,
  };
}

type TreeConnectionLineProps = {
  path: Point[];
  animation?: AnimationConfig;
};

function buildPath(points: Point[]): string {
  if (points.length < 2) return '';
  if (points.length === 2) return renderPath([move(points[0]), line(points[1])]);

  const commands: PathCommand[] = [move(points[0])];

  for (let i = 0; i < points.length - 1; i++) {
    const segment = {
      current: points[i],
      next: points[i + 1],
      afterNext: points[i + 2],
    };

    if (segment.afterNext === undefined) {
      commands.push(line(segment.next));
      continue;
    }

    if (hasCorner(segment.current, segment.next, segment.afterNext)) {
      commands.push(...buildRoundedCorner(segment.current, segment.next, segment.afterNext));
    } else {
      commands.push(line(segment.next));
    }
  }

  return renderPath(commands);
}

function hasCorner(current: Point, corner: Point, next: Point): boolean {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y };
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y };

  return (
    (toCorner.dx !== 0 && toCorner.dy === 0 && fromCorner.dx === 0 && fromCorner.dy !== 0) ||
    (toCorner.dx === 0 && toCorner.dy !== 0 && fromCorner.dx !== 0 && fromCorner.dy === 0)
  );
}

function buildRoundedCorner(current: Point, corner: Point, next: Point): [LineTo, QuadraticCurve] {
  const toCorner = { dx: corner.x - current.x, dy: corner.y - current.y };
  const fromCorner = { dx: next.x - corner.x, dy: next.y - corner.y };

  const entryDistance = Math.sqrt(toCorner.dx ** 2 + toCorner.dy ** 2);
  const entryRadius = Math.min(CORNER_RADIUS, entryDistance / 2);
  const entryRatio = (entryDistance - entryRadius) / entryDistance;

  const entryPoint = {
    x: current.x + toCorner.dx * entryRatio,
    y: current.y + toCorner.dy * entryRatio,
  };

  const exitDistance = Math.sqrt(fromCorner.dx ** 2 + fromCorner.dy ** 2);
  const exitRadius = Math.min(CORNER_RADIUS, exitDistance / 2);
  const exitRatio = exitRadius / exitDistance;

  const exitPoint = {
    x: corner.x + fromCorner.dx * exitRatio,
    y: corner.y + fromCorner.dy * exitRatio,
  };

  return [line(entryPoint), curve(corner, exitPoint)];
}

export function TreeConnectionLine({ path, animation }: TreeConnectionLineProps) {
  const pathD = useMemo(() => buildPath(path), [path]);
  const resolved = useMemo(() => resolveAnimation(animation), [animation]);
  const { dashLength, gapLength, speed, strokeWidth, color } = resolved;
  const dashTotal = dashLength + gapLength;

  return (
    <path
      d={pathD}
      stroke={color}
      strokeWidth={strokeWidth}
      fill="none"
      strokeLinecap="round"
      strokeDasharray={`${dashLength} ${gapLength}`}
    >
      <animate
        attributeName="stroke-dashoffset"
        to={`-${dashTotal}`}
        from="0"
        dur={`${speed}s`}
        repeatCount="indefinite"
      />
    </path>
  );
}
